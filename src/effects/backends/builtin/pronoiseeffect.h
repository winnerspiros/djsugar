#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/fdnreverb.h"
#include "effects/backends/builtin/dsp/filters.h"

// Space: one-knob color reverb in the style of club mixer "Space"
// controls. Right of center sends the track into a bright, high-passed
// hall that thins the low end out (a build-up wash); left of center a
// dark, low-passed room that pushes the track into the distance. The
// tail rings out after the knob returns to center.
class ProNoiseGroupState : public DjFxState {
  public:
    explicit ProNoiseGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::FdnReverb reverb;
    fxdsp::Svf sendFilter[2];
    fxdsp::Svf dryFilter[2];
    fxdsp::Smoother position;
    float lastDecay = -1.0f;
};

class ProNoiseEffect : public DjFxEffect<ProNoiseGroupState, ProNoiseEffect> {
    DJFX_EFFECT_INTERFACE(ProNoiseGroupState)

  private:
    enum Param {
        kSpace,
        kDecay,
    };
};
