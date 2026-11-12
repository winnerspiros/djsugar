#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Beat-synced phaser with 4 to 12 zero-delay-feedback allpass stages,
// resonance and a stereo offset. Stages are spread across the spectrum
// like an analog phaser's mismatched capacitors, which gives the sweep
// its vocal, swirling character.
class PhaserGroupState : public DjFxState {
  public:
    static constexpr int kMaxStages = 12;

    explicit PhaserGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::OnePole stages[2][kMaxStages];
    fxdsp::Smoother amount;
    fxdsp::Smoother feedback;
    float last[2] = {0.0f, 0.0f};
};

class PhaserEffect : public DjFxEffect<PhaserGroupState, PhaserEffect> {
    DJFX_EFFECT_INTERFACE(PhaserGroupState)

  private:
    enum Param {
        kRate,
        kAmount,
        kFeedback,
        kStages,
        kRange,
        kStereo,
    };
};
