#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/fdnreverb.h"
#include "effects/backends/builtin/dsp/tempodelay.h"

// Spiral: an echo whose repeats are pitch-shifted on every pass, so they
// climb (or fall) while they fade, smeared by a short diffusing reverb.
// The rising tail of the DJM Spiral, with the pitch step under control.
class SpiralGroupState : public DjFxState {
  public:
    explicit SpiralGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::TempoDelay delay;
    fxdsp::FdnReverb smear;
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
};

class SpiralEffect : public DjFxEffect<SpiralGroupState, SpiralEffect> {
    DJFX_EFFECT_INTERFACE(SpiralGroupState)

  private:
    enum Param {
        kTime,
        kAmount,
        kFeedback,
        kPitch,
        kSmear,
        kFreeze,
    };
};
