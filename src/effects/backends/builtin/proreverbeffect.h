#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/fdnreverb.h"

// Shimmer reverb: the reverb tail is pitch-shifted and fed back into the
// reverb, so it blooms into an ethereal, rising pad. Built for breakdowns
// and outros.
class ProReverbGroupState : public DjFxState {
  public:
    explicit ProReverbGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::FdnReverb reverb;
    fxdsp::PitchShifter shift[2];
    fxdsp::OnePole shimmerLowCut[2];
    fxdsp::Smoother amount;
    fxdsp::Smoother shimmer;
    float feedbackL = 0.0f;
    float feedbackR = 0.0f;
    float lastDecay = -1.0f;
    float lastTone = -1.0f;
};

class ProReverbEffect : public DjFxEffect<ProReverbGroupState, ProReverbEffect> {
    DJFX_EFFECT_INTERFACE(ProReverbGroupState)

  private:
    enum Param {
        kAmount,
        kDecay,
        kShimmer,
        kInterval,
        kTone,
        kFreeze,
    };
};
