#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/tempodelay.h"

// Dub echo in the tradition of sound system delays and the DJM Dub Echo:
// a band-limited, saturating loop where every repeat gets darker and
// grittier. Feedback goes past unity into a controlled, singing build-up
// that the saturator keeps from exploding.
class DJEchoGroupState : public DjFxState {
  public:
    explicit DJEchoGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::TempoDelay delay;
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
    fxdsp::Smoother band;
    fxdsp::Smoother drive;
};

class DJEchoEffect : public DjFxEffect<DJEchoGroupState, DJEchoEffect> {
    DJFX_EFFECT_INTERFACE(DJEchoGroupState)

  private:
    enum Param {
        kTime,
        kAmount,
        kFeedback,
        kBand,
        kDrive,
        kSpread,
        kFreeze,
    };
};
