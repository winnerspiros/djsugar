#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/tempodelay.h"
#include "util/class.h"

// Beat-synced stereo echo. Time changes crossfade (or glide like tape),
// the feedback path carries a bipolar tone filter and soft saturation,
// the wet signal can duck under the dry signal, and freeze holds the loop.
class EchoGroupState : public DjFxState {
  public:
    explicit EchoGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::TempoDelay delay;
    fxdsp::EnvelopeFollower envelope;
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
    fxdsp::Smoother tone;
    fxdsp::Smoother spread;
};

class EchoEffect : public DjFxEffect<EchoGroupState, EchoEffect> {
    DJFX_EFFECT_INTERFACE(EchoGroupState)

  private:
    enum Param {
        kTime,
        kAmount,
        kFeedback,
        kTone,
        kSpread,
        kDucking,
        kTape,
        kFreeze,
    };
};
