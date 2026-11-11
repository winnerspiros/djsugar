#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Crush: sample-rate and bit-depth reduction in one musical knob, with a
// tone filter after it and a little dither so quiet passages buzz instead
// of chopping to silence.
class BitCrusherGroupState : public DjFxState {
  public:
    explicit BitCrusherGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::Svf tone[2];
    fxdsp::Random random;
    fxdsp::Smoother amount;
    float hold[2] = {0.0f, 0.0f};
    float counter = 0.0f;
};

class BitCrusherEffect : public DjFxEffect<BitCrusherGroupState, BitCrusherEffect> {
    DJFX_EFFECT_INTERFACE(BitCrusherGroupState)

  private:
    enum Param {
        kAmount,
        kBits,
        kTone,
        kMix,
    };
};
