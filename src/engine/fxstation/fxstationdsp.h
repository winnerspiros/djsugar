#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <vector>

// Building blocks of the FX Station (fxstation.h): small, allocation free
// once prepared, for the engine thread. Stereo signals are interleaved.

namespace mixxx {
namespace fxstation {

constexpr float kPi = 3.14159265358979f;

/// A fast tanh for saturation, accurate enough for audio up to |x| < 3
inline float softClip(float x) {
    x = std::clamp(x, -3.f, 3.f);
    const float x2 = x * x;
    return x * (27.f + x2) / (27.f + 9.f * x2);
}

/// A one pole smoother, e.g. for parameters (per sample or per buffer)
struct Smoother {
    float value = 0.f;
    void reset(float v) {
        value = v;
    }
    float step(float target, float coefficient) {
        value += (target - value) * coefficient;
        return value;
    }
};

/// A state variable filter (topology preserving transform, Zavalishin):
/// stable when its frequency moves on every buffer. Low, band and high
/// pass outputs at once.
struct Svf {
    float g = 0.f;
    float k = 1.f;
    float a1 = 0.f;
    float a2 = 0.f;
    float a3 = 0.f;
    std::array<float, 2> ic1{};
    std::array<float, 2> ic2{};

    void set(float frequency, float q, float sampleRate) {
        frequency = std::clamp(frequency, 10.f, 0.45f * sampleRate);
        g = std::tan(kPi * frequency / sampleRate);
        k = 1.f / std::max(0.1f, q);
        a1 = 1.f / (1.f + g * (g + k));
        a2 = g * a1;
        a3 = g * a2;
    }
    void reset() {
        ic1 = {};
        ic2 = {};
    }
    /// Processes a sample of a channel; returns low, band and high pass
    void process(int channel, float x, float* pLow, float* pBand, float* pHigh) {
        const float v3 = x - ic2[channel];
        const float v1 = a1 * ic1[channel] + a2 * v3;
        const float v2 = ic2[channel] + a2 * ic1[channel] + a3 * v3;
        ic1[channel] = 2.f * v1 - ic1[channel];
        ic2[channel] = 2.f * v2 - ic2[channel];
        *pLow = v2;
        *pBand = v1;
        *pHigh = x - k * v1 - v2;
    }
};

/// A biquad (transposed direct form II) with Butterworth low and high
/// pass, for the fixed crossovers of Isolate
struct Biquad {
    float b0 = 1.f, b1 = 0.f, b2 = 0.f, a1 = 0.f, a2 = 0.f;
    std::array<float, 2> z1{};
    std::array<float, 2> z2{};

    void setLowPass(float frequency, float sampleRate) {
        const float w = 2.f * kPi * frequency / sampleRate;
        const float alpha = std::sin(w) / (2.f * 0.70710678f);
        const float c = std::cos(w);
        const float a0 = 1.f + alpha;
        b0 = (1.f - c) / 2.f / a0;
        b1 = (1.f - c) / a0;
        b2 = b0;
        a1 = -2.f * c / a0;
        a2 = (1.f - alpha) / a0;
    }
    void setHighPass(float frequency, float sampleRate) {
        const float w = 2.f * kPi * frequency / sampleRate;
        const float alpha = std::sin(w) / (2.f * 0.70710678f);
        const float c = std::cos(w);
        const float a0 = 1.f + alpha;
        b0 = (1.f + c) / 2.f / a0;
        b1 = -(1.f + c) / a0;
        b2 = b0;
        a1 = -2.f * c / a0;
        a2 = (1.f - alpha) / a0;
    }
    void reset() {
        z1 = {};
        z2 = {};
    }
    float process(int channel, float x) {
        const float y = b0 * x + z1[channel];
        z1[channel] = b1 * x - a1 * y + z2[channel];
        z2[channel] = b2 * x - a2 * y;
        return y;
    }
};

/// A Linkwitz-Riley crossover (two Butterworth biquads per side): the low
/// and high parts sum to an all-pass of the input
struct Crossover {
    std::array<Biquad, 2> low;
    std::array<Biquad, 2> high;

    void set(float frequency, float sampleRate) {
        for (auto& stage : low) {
            stage.setLowPass(frequency, sampleRate);
        }
        for (auto& stage : high) {
            stage.setHighPass(frequency, sampleRate);
        }
    }
    void reset() {
        for (auto& stage : low) {
            stage.reset();
        }
        for (auto& stage : high) {
            stage.reset();
        }
    }
    void split(int channel, float x, float* pLow, float* pHigh) {
        *pLow = low[1].process(channel, low[0].process(channel, x));
        *pHigh = high[1].process(channel, high[0].process(channel, x));
    }
};

/// A stereo delay line read with a fractional delay
class DelayLine {
  public:
    void allocate(int maxFrames) {
        m_buffer.assign(2 * static_cast<size_t>(maxFrames), 0.f);
        m_frames = maxFrames;
        m_write = 0;
    }
    void clear() {
        std::fill(m_buffer.begin(), m_buffer.end(), 0.f);
    }
    int maxFrames() const {
        return m_frames;
    }
    /// The sample of a channel delayFrames before the next write
    float read(int channel, float delayFrames) const {
        delayFrames = std::clamp(delayFrames, 1.f, static_cast<float>(m_frames - 2));
        float position = static_cast<float>(m_write) - delayFrames;
        if (position < 0.f) {
            position += static_cast<float>(m_frames);
        }
        const int index = static_cast<int>(position);
        const float fraction = position - static_cast<float>(index);
        const int next = index + 1 == m_frames ? 0 : index + 1;
        const float a = m_buffer[2 * index + channel];
        const float b = m_buffer[2 * next + channel];
        return a + (b - a) * fraction;
    }
    void write(float left, float right) {
        m_buffer[2 * m_write] = left;
        m_buffer[2 * m_write + 1] = right;
        if (++m_write == m_frames) {
            m_write = 0;
        }
    }
    /// For reading at any position (e.g. Release): the frame written last
    int writeIndex() const {
        return m_write;
    }
    float at(int channel, double position) const {
        double wrapped = std::fmod(position, static_cast<double>(m_frames));
        if (wrapped < 0.0) {
            wrapped += static_cast<double>(m_frames);
        }
        const int index = static_cast<int>(wrapped);
        const float fraction = static_cast<float>(wrapped - index);
        const int next = index + 1 == m_frames ? 0 : index + 1;
        const float a = m_buffer[2 * index + channel];
        const float b = m_buffer[2 * next + channel];
        return a + (b - a) * fraction;
    }

  private:
    std::vector<float> m_buffer;
    int m_frames = 0;
    int m_write = 0;
};

/// A pitch shifter with two crossfaded taps of a short delay (Doppler
/// shifting): cheap, for the repeats of the Spiral echoes
class PitchShifter {
  public:
    void allocate(int windowFrames) {
        m_window = windowFrames;
        m_line.allocate(windowFrames * 2 + 4);
        m_phase = 0.f;
    }
    void clear() {
        m_line.clear();
    }
    void process(float* pLeft, float* pRight, float ratio) {
        m_line.write(*pLeft, *pRight);
        m_phase += (1.f - ratio) / static_cast<float>(m_window);
        m_phase -= std::floor(m_phase);
        const float phase2 = m_phase + 0.5f >= 1.f ? m_phase - 0.5f : m_phase + 0.5f;
        // Triangular windows, summing to one
        const float gain1 = 1.f - std::fabs(2.f * m_phase - 1.f);
        const float gain2 = 1.f - gain1;
        const float delay1 = 1.f + m_phase * static_cast<float>(m_window);
        const float delay2 = 1.f + phase2 * static_cast<float>(m_window);
        *pLeft = gain1 * m_line.read(0, delay1) + gain2 * m_line.read(0, delay2);
        *pRight = gain1 * m_line.read(1, delay1) + gain2 * m_line.read(1, delay2);
    }

  private:
    DelayLine m_line;
    int m_window = 1;
    float m_phase = 0.f;
};

/// A compact stereo reverb (Freeverb: four combs and two all-passes per
/// channel), for Reverb Up and Reverb Down
class Reverb {
  public:
    void allocate(float maxSampleRate) {
        const float scale = maxSampleRate / 44100.f;
        for (int channel = 0; channel < 2; ++channel) {
            for (int i = 0; i < kCombs; ++i) {
                m_combs[channel][i].buffer.assign(
                        static_cast<size_t>((kCombLengths[i] + kSpread) * scale) + 2, 0.f);
            }
            for (int i = 0; i < kAllPasses; ++i) {
                m_allPasses[channel][i].buffer.assign(
                        static_cast<size_t>((kAllPassLengths[i] + kSpread) * scale) + 2, 0.f);
            }
        }
    }
    void setSampleRate(float sampleRate) {
        const float scale = sampleRate / 44100.f;
        for (int channel = 0; channel < 2; ++channel) {
            const int spread = channel ? kSpread : 0;
            for (int i = 0; i < kCombs; ++i) {
                auto& comb = m_combs[channel][i];
                comb.length = std::min(static_cast<int>((kCombLengths[i] + spread) * scale),
                        static_cast<int>(comb.buffer.size()) - 1);
                comb.index = 0;
            }
            for (int i = 0; i < kAllPasses; ++i) {
                auto& allPass = m_allPasses[channel][i];
                allPass.length = std::min(static_cast<int>((kAllPassLengths[i] + spread) * scale),
                        static_cast<int>(allPass.buffer.size()) - 1);
                allPass.index = 0;
            }
        }
        clear();
    }
    void clear() {
        for (auto& channel : m_combs) {
            for (auto& comb : channel) {
                std::fill(comb.buffer.begin(), comb.buffer.end(), 0.f);
                comb.store = 0.f;
            }
        }
        for (auto& channel : m_allPasses) {
            for (auto& allPass : channel) {
                std::fill(allPass.buffer.begin(), allPass.buffer.end(), 0.f);
            }
        }
    }
    /// size and damping 0..1
    void set(float size, float damping) {
        m_feedback = 0.7f + 0.28f * std::clamp(size, 0.f, 1.f);
        m_damping = 0.05f + 0.6f * std::clamp(damping, 0.f, 1.f);
    }
    void process(float inLeft, float inRight, float* pLeft, float* pRight) {
        const float input = (inLeft + inRight) * 0.015f;
        *pLeft = processChannel(0, input);
        *pRight = processChannel(1, input);
    }

  private:
    static constexpr int kCombs = 4;
    static constexpr int kAllPasses = 2;
    static constexpr int kSpread = 23;
    static constexpr int kCombLengths[kCombs] = {1116, 1188, 1277, 1356};
    static constexpr int kAllPassLengths[kAllPasses] = {556, 441};

    struct Comb {
        std::vector<float> buffer;
        int length = 1;
        int index = 0;
        float store = 0.f;
    };
    struct AllPass {
        std::vector<float> buffer;
        int length = 1;
        int index = 0;
    };

    float processChannel(int channel, float input) {
        float output = 0.f;
        for (auto& comb : m_combs[channel]) {
            const float y = comb.buffer[comb.index];
            comb.store = y * (1.f - m_damping) + comb.store * m_damping;
            comb.buffer[comb.index] = input + comb.store * m_feedback;
            if (++comb.index >= comb.length) {
                comb.index = 0;
            }
            output += y;
        }
        for (auto& allPass : m_allPasses[channel]) {
            const float buffered = allPass.buffer[allPass.index];
            allPass.buffer[allPass.index] = output + buffered * 0.5f;
            if (++allPass.index >= allPass.length) {
                allPass.index = 0;
            }
            output = buffered - output;
        }
        return output;
    }

    std::array<std::array<Comb, kCombs>, 2> m_combs;
    std::array<std::array<AllPass, kAllPasses>, 2> m_allPasses;
    float m_feedback = 0.84f;
    float m_damping = 0.2f;
};

} // namespace fxstation
} // namespace mixxx
