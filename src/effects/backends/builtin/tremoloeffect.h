#pragma once

#include "effects/backends/builtin/djfxbase.h"

// Trans / gate: beat-locked volume chopping. Square edges are rounded
// over a couple of milliseconds so it is punchy without clicks, and a
// sidechain style "pump" shape ducks after every beat.
class TremoloGroupState : public DjFxState {
  public:
    explicit TremoloGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::Smoother amount;
    fxdsp::Smoother gain;
};

class TremoloEffect : public DjFxEffect<TremoloGroupState, TremoloEffect> {
    DJFX_EFFECT_INTERFACE(TremoloGroupState)

  private:
    enum Param {
        kRate,
        kAmount,
        kShape,
        kDuty,
        kOffset,
    };
};
