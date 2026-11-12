#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <vector>

// The added Scene effects of the FX Station: modulation, filter, roll,
// gate, pitch and drive, synced to the beat of the FX Station clock.
// Allocation free once allocated, for the engine thread.

namespace mixxx {
namespace fxstation {
namespace scenes {

constexpr float kTwoPi = 6.28318530717959f;

/// A delay line read with 4-point Hermite interpolation: flat to about
/// 0.4 x the sample rate, no zipper or dulling when the delay moves
class HermiteLine {
  public:
    void allocate(int frames) {
        m_buffer.assign(static_cast<size_t>(frames), 0.f);
        m_write = 0;
    }
    void clear() {
        std::fill(m_buffer.begin(), m_buffer.end(), 0.f);
    }
    /// The sample `delay` whole frames before the last write
    float at(int delay) const {
        const int size = static_cast<int>(m_buffer.size());
        int i = m_write - 1 - delay;
        while (i < 0) {
            i += size;
        }
        return m_buffer[i];
    }
    void write(float x) {
        m_buffer[m_write] = x;
        if (++m_write == static_cast<int>(m_buffer.size())) {
            m_write = 0;
        }
    }
    /// The sample delay frames before the last write (delay >= 1)
    float read(float delay) const {
        const int size = static_cast<int>(m_buffer.size());
        delay = std::clamp(delay, 1.f, static_cast<float>(size - 4));
        float position = static_cast<float>(m_write - 1) - delay;
        while (position < 0.f) {
            position += static_cast<float>(size);
        }
        const int i = static_cast<int>(position);
        const float t = position - static_cast<float>(i);
        const float xm1 = m_buffer[(i - 1 + size) % size];
        const float x0 = m_buffer[i % size];
        const float x1 = m_buffer[(i + 1) % size];
        const float x2 = m_buffer[(i + 2) % size];
        const float c1 = 0.5f * (x1 - xm1);
        const float c2 = xm1 - 2.5f * x0 + 2.f * x1 - 0.5f * x2;
        const float c3 = 0.5f * (x2 - xm1) + 1.5f * (x0 - x1);
        return ((c3 * t + c2) * t + c1) * t + x0;
    }

  private:
    std::vector<float> m_buffer;
    int m_write = 0;
};

/// A DC blocker (one pole high pass at about 10 Hz)
struct DcBlocker {
    float x1 = 0.f;
    float y1 = 0.f;
    float process(float x, float r) {
        const float y = x - x1 + r * y1;
        x1 = x;
        y1 = y;
        return y;
    }
    void reset() {
        x1 = y1 = 0.f;
    }
};

/// A triangle of a phase in [0, 1), from -1 to 1, smooth at its turns
inline float softTriangle(float phase) {
    const float tri = 1.f - 4.f * std::fabs(phase - std::floor(phase + 0.5f));
    // A cubic easing of the corners: no clicks of the sweep direction
    return tri * (1.5f - 0.5f * tri * tri);
}

/// Flanger: a short delay swept by the beat, with feedback and the
/// dry signal on equal level (the deep comb notches of tape flanging).
/// Stereo with the right sweep a quarter of a cycle later.
class Flanger {
  public:
    void allocate(float maxSampleRate) {
        for (auto& line : m_lines) {
            line.allocate(static_cast<int>(0.02f * maxSampleRate) + 8);
        }
    }
    void clear() {
        for (auto& line : m_lines) {
            line.clear();
        }
        m_feedback = {};
        for (auto& dc : m_dc) {
            dc.reset();
        }
    }
    /// phase: of the sweep (beats / sweep length); feedback in [0, 1)
    void process(float* pLeft, float* pRight, float phase, float feedback, float sampleRate) {
        const std::array<float, 2> in = {*pLeft, *pRight};
        std::array<float, 2> out{};
        for (int c = 0; c < 2; ++c) {
            const float lfo = 0.5f + 0.5f * softTriangle(phase + 0.25f * static_cast<float>(c));
            // 0.25 to 6 ms, swept exponentially: even notch movement
            const float ms = 0.25f * std::pow(24.f, lfo);
            const float delayed = m_lines[c].read(ms * 0.001f * sampleRate);
            const float x = in[c] + feedback * m_feedback[c];
            m_lines[c].write(std::tanh(x));
            m_feedback[c] = m_dc[c].process(delayed, 0.9995f);
            out[c] = 0.5f * (in[c] + delayed);
        }
        *pLeft = out[0];
        *pRight = out[1];
    }

  private:
    std::array<HermiteLine, 2> m_lines;
    std::array<float, 2> m_feedback{};
    std::array<DcBlocker, 2> m_dc;
};

/// Phaser: 8 first order all passes (zero delay feedback form, stable at
/// any sweep speed), swept on a log scale from 120 Hz to 5 kHz by the beat,
/// with resonance. The channels sweep in opposite directions.
class Phaser {
  public:
    static constexpr int kStages = 8;
    void clear() {
        m_state = {};
        m_feedback = {};
    }
    void process(float* pLeft, float* pRight, float phase, float resonance, float sampleRate) {
        const std::array<float, 2> in = {*pLeft, *pRight};
        std::array<float, 2> out{};
        for (int c = 0; c < 2; ++c) {
            const float lfo = 0.5f + 0.5f * softTriangle(phase + 0.5f * static_cast<float>(c));
            const float frequency = 120.f * std::pow(5000.f / 120.f, lfo);
            const float g = std::tan(3.14159265f * std::min(frequency, 0.45f * sampleRate) /
                    sampleRate);
            const float a = g / (1.f + g);
            float x = std::tanh(in[c] + resonance * m_feedback[c]);
            for (int s = 0; s < kStages; ++s) {
                // All pass from the one pole low pass: 2 * lp - x
                float& z = m_state[c][s];
                const float v = (x - z) * a;
                const float lp = v + z;
                z = lp + v;
                x = 2.f * lp - x;
            }
            m_feedback[c] = x;
            out[c] = 0.5f * (in[c] + x);
        }
        *pLeft = out[0];
        *pRight = out[1];
    }

  private:
    std::array<std::array<float, kStages>, 2> m_state{};
    std::array<float, 2> m_feedback{};
};

/// DJ filter: one knob, low pass to the left of the center, high pass to
/// the right; 24 dB/octave (two zero delay feedback state variable
/// filters), resonance on the second one, its output softly limited so
/// that it sings without blowing up. Bypassed in the center.
class SweepFilter {
  public:
    void clear() {
        m_s = {};
    }
    /// position: -1 (low pass closed) .. 0 (open) .. 1 (high pass closed)
    void process(float* pLeft, float* pRight, float position, float resonance, float sampleRate) {
        const bool lowPass = position < 0.f;
        const float amount = std::min(1.f, std::fabs(position));
        // The center is the dry signal, eased into over the first 5 %
        const float wet = std::min(1.f, amount / 0.05f);
        if (wet <= 0.f) {
            m_s = {};
            return;
        }
        // Low pass 20 kHz -> 60 Hz, high pass 20 Hz -> 8 kHz
        const float frequency = lowPass ? 20000.f * std::pow(60.f / 20000.f, amount)
                                        : 20.f * std::pow(8000.f / 20.f, amount);
        const float g = std::tan(3.14159265f * std::min(frequency, 0.45f * sampleRate) /
                sampleRate);
        // Butterworth pair (Q 0.54 and 1.31), the second one up to Q 8
        const float k1 = 1.f / 0.5412f;
        const float k2 = 1.f / (1.3066f + 6.7f * resonance * amount);
        float* const p[2] = {pLeft, pRight};
        for (int c = 0; c < 2; ++c) {
            const float dry = *p[c];
            float x = stage(c, 0, dry, g, k1, lowPass);
            x = stage(c, 1, x, g, k2, lowPass);
            if (resonance > 0.f) {
                // The resonant peak, limited softly above full scale
                x = 1.5f * std::tanh(x / 1.5f);
            }
            *p[c] = dry + (x - dry) * wet;
        }
    }

  private:
    float stage(int c, int s, float x, float g, float k, bool lowPass) {
        auto& [ic1, ic2] = m_s[c][s];
        const float a1 = 1.f / (1.f + g * (g + k));
        const float v3 = x - ic2;
        const float v1 = a1 * ic1 + g * a1 * v3;
        const float v2 = ic2 + g * v1;
        ic1 = 2.f * v1 - ic1;
        ic2 = 2.f * v2 - ic2;
        return lowPass ? v2 : x - k * v1 - v2;
    }
    std::array<std::array<std::array<float, 2>, 2>, 2> m_s{};
};

/// Beat roll: the first slice after the press is recorded while it plays,
/// then repeated, every repeat quieter with the decay. Each repeat fades
/// in and out over 2 ms, so the loop points never click.
class Roll {
  public:
    void allocate(float maxSampleRate, float maxSeconds) {
        m_buffer.assign(2 * static_cast<size_t>(maxSampleRate * maxSeconds), 0.f);
    }
    void start() {
        m_recorded = 0;
        m_position = 0;
        m_gain = 1.f;
        m_active = true;
    }
    void stop() {
        m_active = false;
    }
    bool active() const {
        return m_active;
    }
    void process(float* pLeft, float* pRight, int sliceFrames, float decay, float sampleRate) {
        if (!m_active) {
            return;
        }
        const int capacity = static_cast<int>(m_buffer.size() / 2);
        sliceFrames = std::clamp(sliceFrames, 64, capacity);
        if (m_recorded < sliceFrames) {
            m_buffer[2 * m_recorded] = *pLeft;
            m_buffer[2 * m_recorded + 1] = *pRight;
            ++m_recorded;
            m_length = m_recorded;
            return;
        }
        const int length = std::min(sliceFrames, m_length);
        if (m_position >= length) {
            m_position = 0;
            m_gain *= 1.f - 0.5f * decay;
        }
        const int fadeFrames = std::max(1, static_cast<int>(0.002f * sampleRate));
        const int edge = std::min(m_position, length - 1 - m_position);
        const float fade = edge >= fadeFrames
                ? 1.f
                : 0.5f - 0.5f * std::cos(3.14159265f * static_cast<float>(edge) / fadeFrames);
        *pLeft = m_buffer[2 * m_position] * fade * m_gain;
        *pRight = m_buffer[2 * m_position + 1] * fade * m_gain;
        ++m_position;
    }

  private:
    std::vector<float> m_buffer;
    int m_recorded = 0;
    int m_length = 0;
    int m_position = 0;
    float m_gain = 1.f;
    bool m_active = false;
};

/// Gate (trans): chops the sound on the beat grid, with raised cosine
/// edges whose length follows the shape knob (hard chop .. soft pulse)
inline float gateGain(double beat, double division, float width, float softness) {
    const double phase = beat / division - std::floor(beat / division);
    const float p = static_cast<float>(phase);
    // Open for `width` of the step, with edges of up to a quarter step
    const float edge = 0.004f + 0.25f * softness;
    if (p < edge) {
        return 0.5f - 0.5f * std::cos(3.14159265f * p / edge);
    }
    if (p < width - edge) {
        return 1.f;
    }
    if (p < width) {
        return 0.5f - 0.5f * std::cos(3.14159265f * (width - p) / edge);
    }
    return 0.f;
}

/// Pitch shifter (WSOLA): a read tap runs through the recent input at the
/// pitch ratio. When it drifts out of its window it jumps a period back or
/// forward, to the position (searched over +-5 ms) whose waveform matches
/// the one playing best, and crossfades until the next jump (up to 30 ms). Joins in phase do not
/// warble or comb like fixed grains; about 20 ms of latency.
class WsolaPitch {
  public:
    static constexpr float kPeriod = 0.03f;
    static constexpr float kSearch = 0.006f;
    static constexpr float kFade = 0.03f;
    static constexpr float kMatch = 0.012f;
    void allocate(float maxSampleRate) {
        const int frames = static_cast<int>(0.25f * maxSampleRate) + 16;
        for (auto& line : m_lines) {
            line.allocate(frames);
        }
        m_mono.allocate(frames);
        setSampleRate(maxSampleRate);
    }
    void setSampleRate(float sampleRate) {
        m_sampleRate = sampleRate;
        m_period = static_cast<int>(kPeriod * sampleRate);
        m_search = static_cast<int>(kSearch * sampleRate);
        m_maxFade = static_cast<int>(kFade * sampleRate);
        m_fade = m_maxFade;
        m_match = static_cast<int>(kMatch * sampleRate);
        clear();
    }
    void clear() {
        for (auto& line : m_lines) {
            line.clear();
        }
        m_mono.clear();
        m_delay = -1.f;
        m_fadeLeft = 0;
        m_low = {};
    }
    void process(float* pLeft, float* pRight, float ratio) {
        ratio = std::clamp(ratio, 0.25f, 4.f);
        // The longest crossfade that ends before the next jump
        m_fade = std::max(16,
                std::min(m_maxFade,
                        static_cast<int>(0.9f * static_cast<float>(m_period) /
                                std::max(1e-3f, std::fabs(1.f - ratio)))));
        m_lines[0].write(*pLeft);
        m_lines[1].write(*pRight);
        m_mono.write(*pLeft + *pRight);
        // The window of the tap: the old tap still plays the fade, at the
        // ratio, and must stay behind the write position
        const float base = 4.f + static_cast<float>(m_search) +
                static_cast<float>(m_fade) * std::max(0.f, ratio - 1.f);
        if (m_delay < 0.f) {
            m_delay = base + 0.5f * static_cast<float>(m_period);
        }
        if (m_fadeLeft == 0) {
            if (m_delay < base) {
                jump(m_delay + static_cast<float>(m_period));
            } else if (m_delay > base + static_cast<float>(m_period)) {
                jump(m_delay - static_cast<float>(m_period));
            }
        }
        float left = m_lines[0].read(m_delay);
        float right = m_lines[1].read(m_delay);
        if (m_fadeLeft > 0) {
            // Raised cosine: the joined waveforms are in phase, their
            // amplitudes add up to one
            const float t = static_cast<float>(m_fadeLeft) / static_cast<float>(m_fade);
            const float out = 0.5f - 0.5f * std::cos(3.14159265f * t);
            left = left * (1.f - out) + m_lines[0].read(m_oldDelay) * out;
            right = right * (1.f - out) + m_lines[1].read(m_oldDelay) * out;
            m_oldDelay += 1.f - ratio;
            --m_fadeLeft;
        }
        m_delay += 1.f - ratio;
        // Shifted up, the images above the old Nyquist are softened
        if (ratio > 1.f) {
            const float cutoff = std::min(0.45f, 0.5f / ratio) * m_sampleRate;
            const float a = 1.f - std::exp(-kTwoPi * cutoff / m_sampleRate);
            m_low[0] += (left - m_low[0]) * a;
            m_low[1] += (right - m_low[1]) * a;
            const float blend = std::min(1.f, ratio - 1.f);
            left += (m_low[0] - left) * blend;
            right += (m_low[1] - right) * blend;
        } else {
            m_low = {left, right};
        }
        *pLeft = left;
        *pRight = right;
    }

  private:
    void jump(float center) {
        // The waveform before the tap, against the one before each candidate
        const int from = static_cast<int>(m_delay);
        float best = -2.f;
        int bestDelay = static_cast<int>(center);
        for (int candidate = static_cast<int>(center) - m_search;
                candidate <= static_cast<int>(center) + m_search;
                candidate += 2) {
            if (candidate < 2) {
                continue;
            }
            // Around the tap: what it played and, as far as recorded, what
            // the crossfade will play
            float cross = 0.f;
            float energy = 1e-9f;
            const int ahead = std::min({m_match / 2, from, candidate});
            for (int k = -ahead; k < m_match - ahead; k += 2) {
                const float y = m_mono.at(candidate + k);
                cross += m_mono.at(from + k) * y;
                energy += y * y;
            }
            const float score = cross / std::sqrt(energy);
            if (score > best) {
                best = score;
                bestDelay = candidate;
            }
        }
        m_oldDelay = m_delay;
        m_delay = static_cast<float>(bestDelay) + (m_delay - static_cast<float>(from));
        m_fadeLeft = m_fade;
    }

    std::array<HermiteLine, 2> m_lines;
    HermiteLine m_mono;
    float m_sampleRate = 48000.f;
    int m_period = 1;
    int m_search = 1;
    int m_fade = 1;
    int m_maxFade = 1;
    int m_match = 1;
    float m_delay = -1.f;
    float m_oldDelay = 0.f;
    int m_fadeLeft = 0;
    std::array<float, 2> m_low{};
};

/// A 2x oversampler: a 31 tap half band low pass (Kaiser windowed sinc,
/// about 90 dB of stop band) for the way up and the way down
class Oversampler2x {
  public:
    static constexpr int kTaps = 31;
    Oversampler2x() {
        constexpr int kCenter = kTaps / 2;
        constexpr double kBeta = 8.0;
        double sum = 0.0;
        for (int i = 0; i < kTaps; ++i) {
            const int n = i - kCenter;
            const double sinc = n == 0 ? 0.5
                                       : std::sin(3.14159265358979 * n / 2.0) /
                            (3.14159265358979 * n);
            const double r = static_cast<double>(n) / kCenter;
            const double window =
                    bessel0(kBeta * std::sqrt(std::max(0.0, 1.0 - r * r))) /
                    bessel0(kBeta);
            m_coefficients[i] = static_cast<float>(sinc * window);
            sum += sinc * window;
        }
        for (auto& c : m_coefficients) {
            c = static_cast<float>(c / sum);
        }
    }
    void clear() {
        m_up = {};
        m_down = {};
    }
    /// One input sample to two output samples
    void up(float x, float* pOut0, float* pOut1) {
        push(&m_up, 2.f * x);
        *pOut0 = filter(m_up);
        push(&m_up, 0.f);
        *pOut1 = filter(m_up);
    }
    /// Two samples to one
    float down(float x0, float x1) {
        push(&m_down, x0);
        push(&m_down, x1);
        return filter(m_down);
    }

  private:
    static double bessel0(double x) {
        double sum = 1.0;
        double term = 1.0;
        for (int k = 1; k < 25; ++k) {
            term *= (x / (2.0 * k)) * (x / (2.0 * k));
            sum += term;
        }
        return sum;
    }
    static void push(std::array<float, kTaps>* pLine, float x) {
        std::move_backward(pLine->begin(), pLine->end() - 1, pLine->end());
        (*pLine)[0] = x;
    }
    float filter(const std::array<float, kTaps>& line) const {
        float y = 0.f;
        for (int i = 0; i < kTaps; ++i) {
            y += m_coefficients[i] * line[i];
        }
        return y;
    }
    std::array<float, kTaps> m_coefficients{};
    std::array<float, kTaps> m_up{};
    std::array<float, kTaps> m_down{};
};

/// Drive: tanh saturation at twice the rate with antiderivative
/// anti-aliasing (no fizz of folded harmonics at high drive), a bias for
/// even harmonics, DC removed, a tone low pass, and the level matched.
class Drive {
  public:
    void clear() {
        m_x1 = {};
        m_tone = {};
        for (auto& dc : m_dc) {
            dc.reset();
        }
        for (auto& os : m_oversamplers) {
            os.clear();
        }
    }
    void process(float* pLeft,
            float* pRight,
            float amount,
            float tone,
            float bias,
            float sampleRate) {
        const float gain = std::pow(40.f, amount);
        // Level matched: louder drive is not mistaken for better drive
        const float makeup = std::pow(gain, -0.3f);
        const float offset = 0.4f * bias;
        const float dcOffset = std::tanh(offset);
        const float cutoff = 1500.f * std::pow(12.f, tone);
        const float lp = 1.f -
                std::exp(-kTwoPi * std::min(cutoff, 0.45f * sampleRate) /
                        sampleRate);
        float* const p[2] = {pLeft, pRight};
        for (int c = 0; c < 2; ++c) {
            float a, b;
            m_oversamplers[c].up(*p[c] * gain, &a, &b);
            a = shape(c, a + offset) - dcOffset;
            b = shape(c, b + offset) - dcOffset;
            float y = m_oversamplers[c].down(a, b);
            y = m_dc[c].process(y, 0.9995f);
            m_tone[c] += (y - m_tone[c]) * lp;
            *p[c] = m_tone[c] * makeup;
        }
    }

  private:
    float shape(int c, float x) {
        const float dx = x - m_x1[c];
        float y;
        if (std::fabs(dx) < 1e-4f) {
            y = std::tanh(0.5f * (x + m_x1[c]));
        } else {
            y = (logCosh(x) - logCosh(m_x1[c])) / dx;
        }
        m_x1[c] = x;
        return y;
    }
    static float logCosh(float x) {
        // log(cosh(x)) without overflow
        const float a = std::fabs(x);
        return a + std::log1p(std::exp(-2.f * a)) - 0.69314718f;
    }
    std::array<Oversampler2x, 2> m_oversamplers;
    std::array<float, 2> m_x1{};
    std::array<float, 2> m_tone{};
    std::array<DcBlocker, 2> m_dc;
};

} // namespace scenes
} // namespace fxstation
} // namespace mixxx
