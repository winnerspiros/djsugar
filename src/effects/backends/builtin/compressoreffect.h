#pragma once

#include "effects/backends/builtin/djfxbase.h"

// Glue compressor: stereo-linked, soft knee, program dependent release
// and automatic makeup gain. One knob from gentle glue to squashed and
// pumping.
class CompressorGroupState : public DjFxState {
  public:
    explicit CompressorGroupState(const mixxx::EngineParameters& engineParameters);

    float envelopeDb = -120.0f; // smoothed detector level
    float slowRelease = 0.0f;   // second, slower release stage
    fxdsp::Smoother amount;
    fxdsp::Smoother mix;
};

class CompressorEffect : public DjFxEffect<CompressorGroupState, CompressorEffect> {
    DJFX_EFFECT_INTERFACE(CompressorGroupState)

  private:
    enum Param {
        kAmount,
        kAttack,
        kRelease,
        kMix,
    };
};
