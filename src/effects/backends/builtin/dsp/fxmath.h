#pragma once

// Shared realtime DSP building blocks for the built-in effects.
// Header-only, no Mixxx or Qt dependency, no allocation in process calls:
// every buffer is sized by the owning effect state outside the audio callback.

#include <algorithm>
#include <cmath>
#include <cstdint>

namespace fxdsp {

constexpr float kPi = 3.14159265358979f;
constexpr float kTwoPi = 6.28318530717959f;

inline float clampf(float x, float lo, float hi) {
    return std::min(std::max(x, lo), hi);
}

inline float lerp(float a, float b, float t) {
    return a + (b - a) * t;
}

inline float dbToGain(float db) {
    return std::pow(10.0f, db * 0.05f);
}

inline float gainToDb(float gain) {
    return 20.0f * std::log10(std::max(gain, 1e-9f));
}

// Flushes values that would become denormal in recursive structures.
// ARM64 runs with flush-to-zero in most audio threads, x86 does not
// necessarily, so feedback paths call this explicitly.
inline float flush(float x) {
    return std::fabs(x) < 1e-15f ? 0.0f : x;
}

// Replaces NaN and infinity with silence. Used at the outputs of feedback
// structures so a single bad sample can never latch an effect into noise.
inline float sanitize(float x) {
    return std::isfinite(x) ? x : 0.0f;
}

// Coefficient of a one-pole smoother reaching 63% of a step in timeSeconds.
inline float onePoleCoeff(float timeSeconds, float sampleRate) {
    if (timeSeconds <= 0.0f) {
        return 0.0f;
    }
    return std::exp(-1.0f / (timeSeconds * sampleRate));
}

// Equal-power crossfade gains for x in [0, 1].
inline void equalPower(float x, float* pA, float* pB) {
    const float t = clampf(x, 0.0f, 1.0f) * (kPi * 0.5f);
    *pA = std::cos(t);
    *pB = std::sin(t);
}

// Maps a normalized knob position [0, 1] onto a logarithmic frequency range.
inline float knobToHz(float knob, float minHz, float maxHz) {
    return minHz * std::pow(maxHz / minHz, clampf(knob, 0.0f, 1.0f));
}

// Fast tan for the bilinear prewarp, accurate to about 1e-4 relative
// below 0.45 * Nyquist and still monotonic above. The argument is
// pi * f / fs and is clamped short of pi / 2.
inline float prewarp(float hz, float sampleRate) {
    const float w = clampf(kPi * hz / sampleRate, 1e-6f, 1.5f);
    return std::tan(w);
}

// Small xorshift generator; deterministic, lock-free, cheap.
class Random {
  public:
    explicit Random(uint32_t seed = 0x9E3779B9u)
            : m_state(seed ? seed : 1u) {
    }
    uint32_t next() {
        m_state ^= m_state << 13;
        m_state ^= m_state >> 17;
        m_state ^= m_state << 5;
        return m_state;
    }
    // Uniform in [-1, 1).
    float bipolar() {
        return static_cast<float>(static_cast<int32_t>(next())) * 4.656612873e-10f;
    }
    // Uniform in [0, 1).
    float unipolar() {
        return static_cast<float>(next() >> 8) * 5.960464478e-8f;
    }

  private:
    uint32_t m_state;
};

// Per-sample one-pole parameter smoother. Rate-independent: the time
// constant is set in seconds.
class Smoother {
  public:
    void setTime(float seconds, float sampleRate) {
        m_coeff = onePoleCoeff(seconds, sampleRate);
    }
    void reset(float value) {
        m_value = value;
        m_target = value;
    }
    void setTarget(float target) {
        m_target = target;
    }
    float target() const {
        return m_target;
    }
    float value() const {
        return m_value;
    }
    float next() {
        m_value = m_target + m_coeff * (m_value - m_target);
        return m_value;
    }

  private:
    float m_coeff = 0.0f;
    float m_value = 0.0f;
    float m_target = 0.0f;
};

} // namespace fxdsp
