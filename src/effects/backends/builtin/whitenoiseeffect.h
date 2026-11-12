#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Noise: stereo noise for risers and transitions. A resonant filter
// shapes it (left: low-pass rumble, right: high-pass hiss), and the
// noise can follow the loudness of the track so it breathes with the
// groove instead of sitting on top as a flat hiss.
class WhiteNoiseGroupState : public DjFxState {
  public:
    explicit WhiteNoiseGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::Random random[2];
    fxdsp::Svf filter[2];
    fxdsp::EnvelopeFollower envelope;
    fxdsp::Smoother amount;
    fxdsp::Smoother color;
    float pink[2][3] = {{0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f}};
};

class WhiteNoiseEffect : public DjFxEffect<WhiteNoiseGroupState, WhiteNoiseEffect> {
    DJFX_EFFECT_INTERFACE(WhiteNoiseGroupState)

  private:
    enum Param {
        kAmount,
        kColor,
        kResonance,
        kFollow,
    };
};
