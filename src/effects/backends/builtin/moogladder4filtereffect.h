#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Auto Filter: a 24 dB/oct transistor ladder low-pass that sweeps by
// itself, locked to the beat grid, or opens with the dynamics of the
// track like an envelope filter (auto-wah). Fat analog resonance that
// bites without whistling.
class MoogLadder4FilterGroupState : public DjFxState {
  public:
    explicit MoogLadder4FilterGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::Ladder ladder[2];
    fxdsp::EnvelopeFollower envelope;
    fxdsp::Smoother amount;
    fxdsp::Smoother cutoff;
};

class MoogLadder4FilterEffect
        : public DjFxEffect<MoogLadder4FilterGroupState, MoogLadder4FilterEffect> {
    DJFX_EFFECT_INTERFACE(MoogLadder4FilterGroupState)

  private:
    enum Param {
        kRate,
        kAmount,
        kCutoff,
        kResonance,
        kMode,
        kDrive,
    };
};
