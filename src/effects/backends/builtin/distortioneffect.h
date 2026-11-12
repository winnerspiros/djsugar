#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"
#include "effects/backends/builtin/dsp/saturation.h"

// Saturator with four anti-aliased shapes: warm tube, tape, hard clip and
// a wavefolder. Antiderivative anti-aliasing keeps the top end clean even
// at full drive, and the output level follows the input so turning up
// the drive adds grit, not volume.
class DistortionGroupState : public DjFxState {
  public:
    explicit DistortionGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::AdaaTanh tanh[2];
    fxdsp::AdaaHardClip clip[2];
    fxdsp::AdaaSineFold fold[2];
    fxdsp::DcBlocker dc[2];
    fxdsp::Svf tone[2];
    fxdsp::OnePole preEmphasis[2];
    fxdsp::Smoother drive;
    fxdsp::Smoother mix;
    // Slow power estimates of input and saturated output for the level
    // match.
    float inputPower = 0.0f;
    float outputPower = 0.0f;
    float powerCoeff = 0.0f;
    fxdsp::Smoother makeup;
};

class DistortionEffect : public DjFxEffect<DistortionGroupState, DistortionEffect> {
    DJFX_EFFECT_INTERFACE(DistortionGroupState)

  private:
    enum Param {
        kMode,
        kDrive,
        kTone,
        kMix,
    };
};
