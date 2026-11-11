#pragma once

// Stereo tempo delay used by Echo, Dub Echo, Tape Echo and Spiral.
//
// Delay time changes either glide like a tape machine (pitch bends while
// the head moves) or crossfade between two read heads (no pitch artifact,
// what DJM style echoes do). The feedback path holds a tone filter, a soft
// saturator that keeps runaway feedback musical, a DC blocker and an
// optional pitch shifter; freeze turns the loop into an infinite hold.

#include "effects/backends/builtin/dsp/delayline.h"
#include "effects/backends/builtin/dsp/filters.h"
#include "effects/backends/builtin/dsp/modulation.h"
#include "effects/backends/builtin/dsp/saturation.h"

namespace fxdsp {

class TempoDelay {
  public:
    struct Settings {
        float delaySamples = 24000.0f;
        float feedback = 0.5f;
        // 0 = straight stereo, 1 = full ping-pong.
        float spread = 0.0f;
        float lowCutHz = 20.0f;
        float highCutHz = 20000.0f;
        // Gain into the saturator in the loop; 0 disables it.
        float drive = 0.0f;
        // Semitones applied to every repeat (0 disables the shifter).
        float pitchSemitones = 0.0f;
        bool glide = false;
        bool freeze = false;
        // Extra modulation of the read position in samples (wow/flutter).
        float modulationSamples = 0.0f;
    };

    void setup(float sampleRate, float maxSeconds) {
        m_sampleRate = sampleRate;
        const int length = static_cast<int>(sampleRate * maxSeconds);
        for (int c = 0; c < 2; ++c) {
            m_line[c].allocate(length);
            m_dc[c].setup(sampleRate);
            m_shift[c].setup(sampleRate, 0.07f);
        }
        m_glide.setTime(0.18f, sampleRate);
        m_freezeGain.setTime(0.02f, sampleRate);
        m_fadeIncrement = 1.0f / (0.04f * sampleRate);
        clear();
    }

    float maxDelaySamples() const {
        return static_cast<float>(m_line[0].capacity() - 4);
    }

    void clear() {
        for (int c = 0; c < 2; ++c) {
            m_line[c].clear();
            m_low[c].reset();
            m_high[c].reset();
            m_dc[c].reset();
            m_shift[c].clear();
            m_sat[c].reset();
        }
        m_initialized = false;
    }

    // Processes one stereo frame. sendL/sendR are written into the loop,
    // the return values are the wet echoes.
    void process(const Settings& s, float sendL, float sendR, float* pWetL, float* pWetR) {
        const float target = clampf(s.delaySamples, 2.0f, maxDelaySamples() - 8.0f);
        if (!m_initialized) {
            m_current = target;
            m_next = target;
            m_glide.reset(target);
            m_freezeGain.reset(s.freeze ? 1.0f : 0.0f);
            m_initialized = true;
        }

        float wet[2];
        if (s.glide) {
            m_glide.setTarget(target);
            const float d = m_glide.next() + s.modulationSamples;
            m_current = target;
            m_next = target;
            m_fade = 0.0f;
            for (int c = 0; c < 2; ++c) {
                wet[c] = m_line[c].readHermite(d);
            }
        } else {
            m_glide.reset(target);
            if (m_fade <= 0.0f && std::fabs(target - m_current) > 0.5f) {
                m_next = target;
                m_fade = m_fadeIncrement;
            }
            const float d0 = m_current + s.modulationSamples;
            if (m_fade > 0.0f) {
                float a;
                float b;
                equalPower(m_fade, &a, &b);
                const float d1 = m_next + s.modulationSamples;
                for (int c = 0; c < 2; ++c) {
                    wet[c] = a * m_line[c].readHermite(d0) + b * m_line[c].readHermite(d1);
                }
                m_fade += m_fadeIncrement;
                if (m_fade >= 1.0f) {
                    m_current = m_next;
                    m_fade = 0.0f;
                }
            } else {
                for (int c = 0; c < 2; ++c) {
                    wet[c] = m_line[c].readHermite(d0);
                }
            }
        }

        // Freeze ramps quickly so engaging it never clicks.
        m_freezeGain.setTarget(s.freeze ? 1.0f : 0.0f);
        const float freeze = m_freezeGain.next();

        const float lowCut = s.lowCutHz;
        const float highCut = s.highCutHz;
        if (lowCut != m_lastLowCut || highCut != m_lastHighCut) {
            for (int c = 0; c < 2; ++c) {
                m_low[c].setCoeffs(lowCut, 0.6f, m_sampleRate);
                m_high[c].setCoeffs(highCut, 0.6f, m_sampleRate);
            }
            m_lastLowCut = lowCut;
            m_lastHighCut = highCut;
        }
        const float ratio = std::pow(2.0f, s.pitchSemitones / 12.0f);

        float loop[2];
        for (int c = 0; c < 2; ++c) {
            float y = wet[c];
            float shaped = m_high[c].process(m_low[c].process(y).hp).lp;
            if (s.pitchSemitones != 0.0f) {
                shaped = m_shift[c].process(shaped, ratio);
            }
            if (s.drive > 0.0f) {
                const float g = 1.0f + 3.0f * s.drive;
                // Unity small-signal gain: the saturator only ever removes
                // energy, so it cannot turn feedback below 1 into a drone.
                shaped = m_sat[c].process(shaped * g) / g;
            }
            shaped = m_dc[c].process(shaped);
            // Frozen: the unfiltered loop recirculates at unity.
            loop[c] = lerp(shaped * s.feedback, y, freeze);
        }
        const float spread = clampf(s.spread, 0.0f, 1.0f);
        const float inGain = 1.0f - freeze;
        // Ping-pong: input enters on the left, repeats alternate sides.
        const float monoIn = 0.5f * (sendL + sendR);
        const float inL = lerp(sendL, monoIn, spread) * inGain;
        const float inR = sendR * (1.0f - spread) * inGain;
        const float fbL = lerp(loop[0], loop[1], spread);
        const float fbR = lerp(loop[1], loop[0], spread);
        // A gentle limiter keeps feedback above unity singing, not exploding.
        m_line[0].write(flush(sanitize(4.0f * softClip((inL + fbL) * 0.25f))));
        m_line[1].write(flush(sanitize(4.0f * softClip((inR + fbR) * 0.25f))));
        *pWetL = wet[0];
        *pWetR = wet[1];
    }

  private:
    float m_sampleRate = 48000.0f;
    DelayLine m_line[2];
    Svf m_low[2];
    Svf m_high[2];
    DcBlocker m_dc[2];
    PitchShifter m_shift[2];
    AdaaTanh m_sat[2];
    Smoother m_glide;
    Smoother m_freezeGain;
    float m_current = 0.0f;
    float m_next = 0.0f;
    float m_fade = 0.0f;
    float m_fadeIncrement = 0.001f;
    float m_lastLowCut = -1.0f;
    float m_lastHighCut = -1.0f;
    bool m_initialized = false;
};

} // namespace fxdsp
