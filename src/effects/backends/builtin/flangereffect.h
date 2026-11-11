#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/delayline.h"
#include "effects/backends/builtin/dsp/filters.h"

// Beat-synced through-zero flanger. The sweep passes through zero delay
// (the dry path is delayed by the center of the sweep), which gives the
// deep jet-plane cancellation of tape flanging that ordinary digital
// flangers miss. Feedback can be positive (metallic) or negative (hollow).
class FlangerGroupState : public DjFxState {
  public:
    explicit FlangerGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::DelayLine wet[2];
    fxdsp::DelayLine dry[2];
    fxdsp::DcBlocker dc[2];
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
    fxdsp::Smoother depth;
    fxdsp::Smoother manual;
    float last[2] = {0.0f, 0.0f};
};

class FlangerEffect : public DjFxEffect<FlangerGroupState, FlangerEffect> {
    DJFX_EFFECT_INTERFACE(FlangerGroupState)

  private:
    enum Param {
        kRate,
        kAmount,
        kFeedback,
        kDepth,
        kManual,
        kStereo,
    };
};
