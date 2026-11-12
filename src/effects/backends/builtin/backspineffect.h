#pragma once

#include <vector>

#include "effects/backends/builtin/djfxbase.h"

// Back Spin: the DJ grabs the record and pulls it back, fast at first and
// slowing down, the track fading out under the hand. Starts on the beat.
class BackSpinGroupState : public DjFxState {
  public:
    explicit BackSpinGroupState(const mixxx::EngineParameters& engineParameters);
    void restart();

    std::vector<float> history; // interleaved stereo, linear from enable
    int capacityFrames = 0;
    int written = 0;
    double readPos = 0.0;
    double elapsed = 0.0;
    // Speed at the start of the spin, in times the normal speed
    double peakRate = 0.0;
    bool started = false;
    float lastPhase = 0.0f;
};

class BackSpinEffect : public DjFxEffect<BackSpinGroupState, BackSpinEffect> {
    DJFX_EFFECT_INTERFACE(BackSpinGroupState)

  private:
    enum Param {
        kTime,
        kMix,
        kQuantize,
    };
};
