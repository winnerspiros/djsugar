#pragma once

// Stereo feedback delay network reverb.
//
// Structure (after Geraint Luff, "Let's write a reverb", ADC 2021, and
// Jot's FDN work): a 4-step Hadamard diffuser builds echo density fast,
// then an 8-line feedback network with a Householder matrix sustains the
// tail. Every line has a decay gain derived from RT60, a one-pole high
// damping and slow, decorrelated delay modulation that removes the
// metallic ringing of static FDNs. Freeze holds the tank at unity gain.

#include <array>

#include "effects/backends/builtin/dsp/delayline.h"
#include "effects/backends/builtin/dsp/filters.h"

namespace fxdsp {

class FdnReverb {
  public:
    static constexpr int kLines = 8;
    static constexpr int kDiffusionSteps = 4;

    void setup(float sampleRate) {
        m_sampleRate = sampleRate;
        const float msToSamples = sampleRate * 0.001f;
        Random rnd(0x5EED1234u);
        // Diffuser step k spans about 10, 20, 40, 80 ms: early echoes
        // arrive densely without a flam.
        for (int step = 0; step < kDiffusionSteps; ++step) {
            const float rangeMs = 10.0f * static_cast<float>(1 << step);
            for (int c = 0; c < kLines; ++c) {
                const float lo = rangeMs * static_cast<float>(c) / kLines;
                const float hi = rangeMs * static_cast<float>(c + 1) / kLines;
                const float ms = lo + (hi - lo) * rnd.unipolar();
                const int len = std::max(1, static_cast<int>(ms * msToSamples));
                m_diffuseLen[step][c] = len;
                m_diffuse[step][c].allocate(len + 1);
                m_flip[step][c] = (rnd.next() & 1u) != 0;
            }
        }
        for (int c = 0; c < kLines; ++c) {
            // Up to 300 ms base * 1.6 size scale, plus modulation headroom.
            m_lines[c].allocate(static_cast<int>(sampleRate * 0.55f));
            m_lineDamp[c].reset();
            m_lineLowCut[c].reset();
            m_modPhase[c] = static_cast<float>(c) / kLines;
            m_length[c].setTime(0.25f, sampleRate);
        }
        m_preDelay[0].allocate(static_cast<int>(sampleRate * 0.25f));
        m_preDelay[1].allocate(static_cast<int>(sampleRate * 0.25f));
        m_inputLowCut[0].reset();
        m_inputLowCut[1].reset();
        setSize(0.5f, true);
        setDecay(2.0f);
        setDamping(8000.0f);
        setLowCut(80.0f);
        m_modDepthSamples = 0.0f;
    }

    void clear() {
        for (auto& steps : m_diffuse) {
            for (auto& line : steps) {
                line.clear();
            }
        }
        for (int c = 0; c < kLines; ++c) {
            m_lines[c].clear();
            m_lineDamp[c].reset();
            m_lineLowCut[c].reset();
        }
        m_preDelay[0].clear();
        m_preDelay[1].clear();
        m_inputLowCut[0].reset();
        m_inputLowCut[1].reset();
        m_energy = 0.0f;
    }

    // size in [0, 1]: delay lengths from small room to large hall.
    void setSize(float size, bool immediate = false) {
        const float scale = 0.35f + 1.25f * clampf(size, 0.0f, 1.0f);
        // Base lengths between 100 and 200 ms, exponentially spread so no
        // two lines share a common factor.
        for (int c = 0; c < kLines; ++c) {
            const float r = static_cast<float>(c) / (kLines - 1);
            const float ms = 100.0f * std::pow(2.0f, r) * scale;
            const float len = ms * 0.001f * m_sampleRate;
            if (immediate) {
                m_length[c].reset(len);
            } else {
                m_length[c].setTarget(len);
            }
        }
        updateGains();
    }

    // RT60 in seconds. A value <= 0 or freeze means infinite.
    void setDecay(float rt60Seconds) {
        m_rt60 = std::max(rt60Seconds, 0.05f);
        updateGains();
    }

    void setDamping(float hz) {
        m_dampHz = hz;
        for (auto& f : m_lineDamp) {
            f.setCutoff(hz, m_sampleRate);
        }
    }

    void setLowCut(float hz) {
        for (auto& f : m_lineLowCut) {
            f.setCutoff(hz, m_sampleRate);
        }
        m_inputLowCut[0].setCutoff(hz, m_sampleRate);
        m_inputLowCut[1].setCutoff(hz, m_sampleRate);
    }

    void setPreDelay(float seconds) {
        m_preDelaySamples = clampf(seconds * m_sampleRate,
                1.0f,
                static_cast<float>(m_preDelay[0].capacity() - 2));
    }

    // Depth in milliseconds of the slow chorus on the feedback lines.
    void setModulation(float ms) {
        m_modDepthSamples = ms * 0.001f * m_sampleRate;
    }

    void setFreeze(bool freeze) {
        m_freeze = freeze;
        updateGains();
    }

    // Approximate energy of the tank, used to tell when a tail has died.
    float energy() const {
        return m_energy;
    }

    // Processes one stereo frame. inL/inR feed the tank; outputs are wet.
    void process(float inL, float inR, float* pOutL, float* pOutR) {
        if (m_freeze) {
            inL = 0.0f;
            inR = 0.0f;
        }
        m_preDelay[0].write(m_inputLowCut[0].highpass(inL));
        m_preDelay[1].write(m_inputLowCut[1].highpass(inR));
        const float l = m_preDelay[0].readLinear(m_preDelaySamples);
        const float r = m_preDelay[1].readLinear(m_preDelaySamples);

        std::array<float, kLines> v;
        for (int c = 0; c < kLines; ++c) {
            v[c] = (c & 1) ? r : l;
        }
        for (int step = 0; step < kDiffusionSteps; ++step) {
            for (int c = 0; c < kLines; ++c) {
                auto& line = m_diffuse[step][c];
                line.write(v[c]);
                const float d = line.tap(m_diffuseLen[step][c]);
                v[c] = m_flip[step][c] ? -d : d;
            }
            hadamard(v);
        }

        // Feedback network.
        std::array<float, kLines> fb;
        float sum = 0.0f;
        for (int c = 0; c < kLines; ++c) {
            m_modPhase[c] += m_modRate[c] / m_sampleRate;
            if (m_modPhase[c] >= 1.0f) {
                m_modPhase[c] -= 1.0f;
            }
            const float mod = m_modDepthSamples *
                    (1.0f + std::sin(kTwoPi * m_modPhase[c]));
            const float len = m_length[c].next() + mod;
            float y = m_lines[c].readHermite(len);
            if (!m_freeze) {
                y = m_lineDamp[c].lowpass(y);
                y = m_lineLowCut[c].highpass(y);
            }
            fb[c] = y * m_gain[c];
            sum += fb[c];
        }
        // Householder reflection: energy preserving, every line feeds every
        // other one.
        const float h = sum * (2.0f / kLines);
        float energy = 0.0f;
        for (int c = 0; c < kLines; ++c) {
            const float x = sanitize(fb[c] - h + v[c]);
            m_lines[c].write(flush(x));
            energy += x * x;
        }
        m_energy = energy;

        // Decorrelated stereo outputs from the delay outputs.
        *pOutL = 0.5f * (fb[0] - fb[2] + fb[4] - fb[6] + fb[1] + fb[5]) * 0.6f;
        *pOutR = 0.5f * (fb[1] - fb[3] + fb[5] - fb[7] + fb[2] + fb[6]) * 0.6f;
    }

  private:
    static void hadamard(std::array<float, kLines>& v) {
        for (int h = 1; h < kLines; h <<= 1) {
            for (int i = 0; i < kLines; i += h << 1) {
                for (int j = i; j < i + h; ++j) {
                    const float a = v[j];
                    const float b = v[j + h];
                    v[j] = a + b;
                    v[j + h] = a - b;
                }
            }
        }
        const float scale = 0.35355339f; // 1 / sqrt(8)
        for (float& x : v) {
            x *= scale;
        }
    }

    void updateGains() {
        for (int c = 0; c < kLines; ++c) {
            if (m_freeze) {
                m_gain[c] = 1.0f;
                continue;
            }
            const float lenSeconds = m_length[c].target() / m_sampleRate;
            m_gain[c] = std::pow(10.0f, -3.0f * lenSeconds / m_rt60);
        }
    }

    float m_sampleRate = 48000.0f;
    std::array<std::array<DelayLine, kLines>, kDiffusionSteps> m_diffuse;
    std::array<std::array<int, kLines>, kDiffusionSteps> m_diffuseLen{};
    std::array<std::array<bool, kLines>, kDiffusionSteps> m_flip{};
    std::array<DelayLine, kLines> m_lines;
    std::array<OnePole, kLines> m_lineDamp;
    std::array<OnePole, kLines> m_lineLowCut;
    std::array<Smoother, kLines> m_length;
    std::array<float, kLines> m_gain{};
    std::array<float, kLines> m_modPhase{};
    std::array<float, kLines> m_modRate{
            0.13f, 0.17f, 0.21f, 0.27f, 0.31f, 0.37f, 0.41f, 0.47f};
    std::array<DelayLine, 2> m_preDelay;
    std::array<OnePole, 2> m_inputLowCut;
    float m_preDelaySamples = 1.0f;
    float m_modDepthSamples = 0.0f;
    float m_rt60 = 2.0f;
    float m_dampHz = 8000.0f;
    float m_energy = 0.0f;
    bool m_freeze = false;
};

} // namespace fxdsp
