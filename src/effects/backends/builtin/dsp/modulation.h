#pragma once

// Tempo clock, LFO shapes, envelope follower and a dual-head granular
// pitch shifter.

#include <optional>

#include "effects/backends/builtin/dsp/delayline.h"

namespace fxdsp {

// Beat-locked phase. Uses the deck's beat grid when available and falls
// back to a free-running clock at the last known (or 120) BPM, so effects
// keep moving on tracks without a grid.
class BeatClock {
  public:
    // Call once per buffer. beatSeconds and beatFractionAtEnd come from
    // GroupFeatureState; pass std::nullopt when unknown.
    void update(std::optional<double> beatSeconds,
            std::optional<double> beatFractionAtEnd,
            int frames,
            float sampleRate) {
        if (beatSeconds && *beatSeconds > 0.05 && *beatSeconds < 4.0) {
            m_beatSeconds = *beatSeconds;
        }
        m_samplesPerBeat = m_beatSeconds * sampleRate;
        m_inc = 1.0 / m_samplesPerBeat;
        if (beatFractionAtEnd) {
            // Phase at the start of the buffer, in beats. The integer beat
            // count is kept locally so multi-beat periods stay continuous.
            double start = *beatFractionAtEnd - frames * m_inc;
            const double frac = start - std::floor(start);
            double whole = std::floor(m_beats);
            if (frac < 0.25 && m_beats - whole > 0.75) {
                whole += 1.0;
            } else if (frac > 0.75 && m_beats - whole < 0.25) {
                whole -= 1.0;
            }
            m_beats = whole + frac;
            m_synced = true;
        } else {
            m_synced = false;
        }
        // Keep the counter small: 64 beats covers every period offered.
        if (m_beats > 4096.0) {
            m_beats -= 4096.0;
        }
    }

    // Advances one frame and returns the phase in [0, 1) of a cycle that
    // lasts periodBeats beats.
    float tick(double periodBeats) {
        m_beats += m_inc;
        const double p = m_beats / std::max(periodBeats, 1e-3);
        return static_cast<float>(p - std::floor(p));
    }

    double beats() const {
        return m_beats;
    }
    double samplesPerBeat() const {
        return m_samplesPerBeat;
    }
    double beatSeconds() const {
        return m_beatSeconds;
    }
    bool synced() const {
        return m_synced;
    }

  private:
    double m_beatSeconds = 0.5;
    double m_samplesPerBeat = 24000.0;
    double m_inc = 1.0 / 24000.0;
    double m_beats = 0.0;
    bool m_synced = false;
};

enum class LfoShape {
    Sine,
    Triangle,
    // Square with raised-cosine edges: the gate/trans shape. Width of the
    // edges set by the smooth parameter, so it never clicks.
    SmoothSquare,
    RampUp,
    RampDown,
};

// Unipolar [0, 1] LFO value for phase in [0, 1).
inline float lfo(LfoShape shape, float phase, float smooth = 0.08f) {
    switch (shape) {
    case LfoShape::Sine:
        return 0.5f - 0.5f * std::cos(kTwoPi * phase);
    case LfoShape::Triangle:
        return phase < 0.5f ? 2.0f * phase : 2.0f - 2.0f * phase;
    case LfoShape::SmoothSquare: {
        const float e = clampf(smooth, 0.002f, 0.25f);
        if (phase < e) {
            return 0.5f - 0.5f * std::cos(kPi * phase / e);
        }
        if (phase < 0.5f) {
            return 1.0f;
        }
        if (phase < 0.5f + e) {
            return 0.5f + 0.5f * std::cos(kPi * (phase - 0.5f) / e);
        }
        return 0.0f;
    }
    case LfoShape::RampUp:
        return phase;
    case LfoShape::RampDown:
        return 1.0f - phase;
    }
    return 0.0f;
}

// Peak envelope follower with separate attack and release.
class EnvelopeFollower {
  public:
    void setup(float attackSeconds, float releaseSeconds, float sampleRate) {
        m_attack = onePoleCoeff(attackSeconds, sampleRate);
        m_release = onePoleCoeff(releaseSeconds, sampleRate);
    }
    void reset() {
        m_env = 0.0f;
    }
    float process(float x) {
        const float a = std::fabs(x);
        const float c = a > m_env ? m_attack : m_release;
        m_env = flush(a + c * (m_env - a));
        return m_env;
    }
    float value() const {
        return m_env;
    }

  private:
    float m_attack = 0.0f;
    float m_release = 0.0f;
    float m_env = 0.0f;
};

// Delay-line pitch shifter with two read heads half a window apart and a
// raised-cosine crossfade. Latency is one window; at 40-80 ms it sounds
// smooth on full mixes and costs almost nothing.
class PitchShifter {
  public:
    // Allocates for windows up to maxWindowSeconds; the window starts at
    // that size.
    void setup(float sampleRate, float maxWindowSeconds = 0.06f) {
        m_window = maxWindowSeconds * sampleRate;
        m_line.allocate(static_cast<int>(m_window) + 8);
        m_phase = 0.0f;
    }
    // Changes the window without allocating; clamped to the setup size.
    void setWindow(float seconds, float sampleRate) {
        m_window = clampf(seconds * sampleRate,
                64.0f,
                static_cast<float>(m_line.capacity() - 4));
    }
    void clear() {
        m_line.clear();
        m_phase = 0.0f;
    }
    // ratio = 2^(semitones / 12).
    float process(float x, float ratio) {
        m_line.write(x);
        // Read heads move at `ratio` while the write head moves at 1, so
        // the delay changes by (1 - ratio) per sample.
        m_phase += (1.0f - ratio) / m_window;
        m_phase -= std::floor(m_phase);
        const float p2 = m_phase + 0.5f >= 1.0f ? m_phase - 0.5f : m_phase + 0.5f;
        const float d1 = 2.0f + m_phase * m_window;
        const float d2 = 2.0f + p2 * m_window;
        const float w1 = 0.5f - 0.5f * std::cos(kTwoPi * m_phase);
        const float w2 = 1.0f - w1;
        return w1 * m_line.readHermite(d1) + w2 * m_line.readHermite(d2);
    }

  private:
    DelayLine m_line;
    float m_window = 2880.0f;
    float m_phase = 0.0f;
};

} // namespace fxdsp
