#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/tempodelay.h"

// Tape echo: the delay time glides like a moving tape head, wow and
// flutter wobble the pitch of the repeats, the loop saturates and loses
// top end like worn tape.
class VintageDelayGroupState : public DjFxState {
  public:
    explicit VintageDelayGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::TempoDelay delay;
    fxdsp::Random random;
    fxdsp::OnePole flutterNoise;
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
    fxdsp::Smoother age;
    float wowPhase = 0.0f;
    float flutterPhase = 0.0f;
};

class VintageDelayEffect : public DjFxEffect<VintageDelayGroupState, VintageDelayEffect> {
    DJFX_EFFECT_INTERFACE(VintageDelayGroupState)

  private:
    enum Param {
        kTime,
        kAmount,
        kFeedback,
        kAge,
        kSaturation,
        kFreeze,
    };
};
