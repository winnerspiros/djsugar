#pragma once

#include <vector>

#include "effects/backends/builtin/djfxbase.h"

// Brake: the deck winds down to a stop like a turntable with the motor
// switched off or a tape machine losing power, synced to the beat grid.
// Holding the effect keeps the silence; switching it off returns the live
// track through the engine's crossfade.
class TapeStopGroupState : public DjFxState {
  public:
    explicit TapeStopGroupState(const mixxx::EngineParameters& engineParameters);
    void restart();

    std::vector<float> history; // interleaved stereo, linear from enable
    int capacityFrames = 0;
    int written = 0;
    double readPos = 0.0;
    double elapsed = 0.0;
    bool started = false;
    float lastPhase = 0.0f;
};

class TapeStopEffect : public DjFxEffect<TapeStopGroupState, TapeStopEffect> {
    DJFX_EFFECT_INTERFACE(TapeStopGroupState)

  private:
    enum Param {
        kTime,
        kMix,
        kCurve,
        kQuantize,
    };
};
