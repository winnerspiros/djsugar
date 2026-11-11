#pragma once

#include <vector>

#include "effects/backends/builtin/djfxbase.h"

// Stutter: on every grid step it rolls the dice and either lets the track
// through or replays the previous step forward, reversed or at half
// speed. Beat-locked glitch edits like a remixer would cut by hand, with
// short crossfades at every switch so it never clicks.
class GlitchGroupState : public DjFxState {
  public:
    explicit GlitchGroupState(const mixxx::EngineParameters& engineParameters);
    void restart();

    enum class Action {
        Pass,
        Repeat,
        Reverse,
        HalfSpeed,
    };

    struct Voice {
        Action action = Action::Pass;
        int64_t start = 0;     // absolute frame where the replayed step began
        double position = 0.0; // frames played into the step
    };

    std::vector<float> history; // interleaved stereo ring
    int capacityFrames = 0;
    int64_t written = 0;
    int64_t stepStart = 0;
    int64_t previousStepStart = 0;
    Voice current;
    Voice fading;
    int fadeLeft = 0;
    float lastPhase = 0.0f;
    fxdsp::Random random;
    fxdsp::Smoother mix;
};

class GlitchEffect : public DjFxEffect<GlitchGroupState, GlitchEffect> {
    DJFX_EFFECT_INTERFACE(GlitchGroupState)

  private:
    enum Param {
        kStep,
        kChance,
        kReverse,
        kHalfSpeed,
        kMix,
    };
};
