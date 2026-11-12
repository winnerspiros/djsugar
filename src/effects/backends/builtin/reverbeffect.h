#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/fdnreverb.h"

// Dense, modulated feedback delay network reverb with pre-delay, tone
// controls and freeze. The tail keeps ringing when the deck is taken off
// the effect unit.
class ReverbGroupState : public DjFxState {
  public:
    explicit ReverbGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::FdnReverb reverb;
    fxdsp::Smoother amount;
    float lastSize = -1.0f;
    float lastDecay = -1.0f;
    float lastDamping = -1.0f;
    float lastLowCut = -1.0f;
    float lastPreDelay = -1.0f;
};

class ReverbEffect : public DjFxEffect<ReverbGroupState, ReverbEffect> {
    DJFX_EFFECT_INTERFACE(ReverbGroupState)

  private:
    enum Param {
        kAmount,
        kSize,
        kDecay,
        kDamping,
        kLowCut,
        kPreDelay,
        kFreeze,
    };
};
