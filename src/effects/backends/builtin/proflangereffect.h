#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/delayline.h"
#include "effects/backends/builtin/dsp/filters.h"

// Ensemble chorus: three modulated voices per side with decorrelated,
// slightly irregular LFOs, the lush width of a string ensemble chorus.
// Low frequencies stay mono and untouched, so the kick and bass stay
// tight on a club system.
class ProFlangerGroupState : public DjFxState {
  public:
    static constexpr int kVoices = 3;

    explicit ProFlangerGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::DelayLine line[2];
    fxdsp::OnePole crossover[2];
    fxdsp::Smoother amount;
    fxdsp::Smoother depth;
    float phase[kVoices] = {0.0f, 0.33f, 0.67f};
    float jitterPhase = 0.0f;
};

class ProFlangerEffect : public DjFxEffect<ProFlangerGroupState, ProFlangerEffect> {
    DJFX_EFFECT_INTERFACE(ProFlangerGroupState)

  private:
    enum Param {
        kAmount,
        kRate,
        kDepth,
        kWidth,
    };
};
