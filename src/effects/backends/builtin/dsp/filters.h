#pragma once

// Zero-delay-feedback (topology preserving transform) filters.
// The state variable filter follows Andy Simper's "Linear Trapezoidal
// Integrated SVF" (Cytomic, 2013); the ladder and one-pole follow
// Zavalishin, "The Art of VA Filter Design" (rev. 2.1.2). TPT filters stay
// stable and click-free under fast audio-rate cutoff modulation, unlike
// direct-form biquads, which is what DJ filter sweeps need.

#include "effects/backends/builtin/dsp/fxmath.h"
#include "effects/backends/builtin/dsp/saturation.h"

namespace fxdsp {

struct SvfOut {
    float lp;
    float bp;
    float hp;
};

class Svf {
  public:
    void reset() {
        m_ic1 = 0.0f;
        m_ic2 = 0.0f;
    }

    // q > 0. Resonance near self-oscillation is around q = 20.
    void setCoeffs(float hz, float q, float sampleRate) {
        m_g = prewarp(hz, sampleRate);
        m_k = 1.0f / std::max(q, 0.05f);
        m_a1 = 1.0f / (1.0f + m_g * (m_g + m_k));
        m_a2 = m_g * m_a1;
        m_a3 = m_g * m_a2;
    }

    float k() const {
        return m_k;
    }

    SvfOut process(float v0) {
        const float v3 = v0 - m_ic2;
        const float v1 = m_a1 * m_ic1 + m_a2 * v3;
        const float v2 = m_ic2 + m_a2 * m_ic1 + m_a3 * v3;
        m_ic1 = flush(2.0f * v1 - m_ic1);
        m_ic2 = flush(2.0f * v2 - m_ic2);
        return {v2, v1, v0 - m_k * v1 - v2};
    }

    // Same as process(), with a soft limit on the band-pass integrator.
    // High resonance then saturates into a round, vocal peak instead of
    // the piercing whistle of a linear filter, and can never blow up.
    SvfOut processDriven(float v0, float limit) {
        const float v3 = v0 - m_ic2;
        const float v1 = m_a1 * m_ic1 + m_a2 * v3;
        const float v2 = m_ic2 + m_a2 * m_ic1 + m_a3 * v3;
        m_ic1 = flush(limit * softClip((2.0f * v1 - m_ic1) / limit));
        m_ic2 = flush(2.0f * v2 - m_ic2);
        return {v2, v1, v0 - m_k * v1 - v2};
    }

  private:
    float m_g = 0.0f;
    float m_k = 1.0f;
    float m_a1 = 0.0f;
    float m_a2 = 0.0f;
    float m_a3 = 0.0f;
    float m_ic1 = 0.0f;
    float m_ic2 = 0.0f;
};

// TPT one-pole. lowpass() and highpass() share the same state, so use one
// instance per output.
class OnePole {
  public:
    void reset(float value = 0.0f) {
        m_s = value;
    }
    void setCutoff(float hz, float sampleRate) {
        const float g = prewarp(hz, sampleRate);
        m_G = g / (1.0f + g);
    }
    float lowpass(float x) {
        const float v = (x - m_s) * m_G;
        const float y = v + m_s;
        m_s = flush(y + v);
        return y;
    }
    float highpass(float x) {
        return x - lowpass(x);
    }
    // First-order allpass, the phaser stage.
    float allpass(float x) {
        return 2.0f * lowpass(x) - x;
    }

  private:
    float m_G = 0.0f;
    float m_s = 0.0f;
};

// Removes DC offset left behind by asymmetric saturation and feedback.
class DcBlocker {
  public:
    void setup(float sampleRate) {
        m_r = 1.0f - (kTwoPi * 10.0f / sampleRate);
    }
    void reset() {
        m_x1 = 0.0f;
        m_y1 = 0.0f;
    }
    float process(float x) {
        const float y = x - m_x1 + m_r * m_y1;
        m_x1 = x;
        m_y1 = flush(y);
        return y;
    }

  private:
    float m_r = 0.999f;
    float m_x1 = 0.0f;
    float m_y1 = 0.0f;
};

// Four-pole transistor ladder (Zavalishin ch. 5) with the feedback solved
// linearly and a tanh in the input stage. resonance in [0, 1] maps to
// k in [0, 4]; the passband is compensated so bass does not vanish as
// resonance rises.
class Ladder {
  public:
    void reset() {
        m_s[0] = m_s[1] = m_s[2] = m_s[3] = 0.0f;
    }
    void setCoeffs(float hz, float resonance, float sampleRate) {
        const float g = prewarp(hz, sampleRate);
        m_G = g / (1.0f + g);
        m_k = 4.0f * clampf(resonance, 0.0f, 1.0f) * 0.985f;
    }
    // Returns the 24 dB/oct low-pass output.
    float process(float x, float drive) {
        const float G = m_G;
        const float oneMinusG = 1.0f - G;
        const float b1 = oneMinusG * m_s[0];
        const float b2 = oneMinusG * m_s[1];
        const float b3 = oneMinusG * m_s[2];
        const float b4 = oneMinusG * m_s[3];
        const float G2 = G * G;
        const float sigma = G2 * G * b1 + G2 * b2 + G * b3 + b4;
        const float G4 = G2 * G2;
        const float xin = x * (1.0f + 0.5f * m_k);
        const float y4est = (G4 * xin + sigma) / (1.0f + m_k * G4);
        float u = fastTanh(drive * (xin - m_k * y4est)) / drive;
        for (float& s : m_s) {
            const float v = (u - s) * G;
            const float y = v + s;
            s = flush(y + v);
            u = y;
        }
        return u;
    }

  private:
    float m_G = 0.0f;
    float m_k = 0.0f;
    float m_s[4] = {0.0f, 0.0f, 0.0f, 0.0f};
};

} // namespace fxdsp
