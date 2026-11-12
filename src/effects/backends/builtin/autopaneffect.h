#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Beat-synced auto pan with a balance law that never clips. Bass below the crossover
// stays centered, so the low end never swings across a club system.
class AutoPanGroupState : public DjFxState {
  public:
    explicit AutoPanGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::OnePole crossover[2];
    fxdsp::Smoother amount;
};

class AutoPanEffect : public DjFxEffect<AutoPanGroupState, AutoPanEffect> {
    DJFX_EFFECT_INTERFACE(AutoPanGroupState)

  private:
    enum Param {
        kRate,
        kAmount,
        kShape,
    };
};
