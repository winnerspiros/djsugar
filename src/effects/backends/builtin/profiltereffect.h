#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Sweep: one bipolar knob with two moves. Right of center narrows the
// track to a resonant band-pass that sweeps up through the mix. Left of
// center is a transient gate: sustain and tails are cut away and only
// the hits remain, tightening a busy track into a percussive skeleton.
class ProFilterGroupState : public DjFxState {
  public:
    explicit ProFilterGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::Svf band[2];
    fxdsp::EnvelopeFollower fast;
    fxdsp::EnvelopeFollower slow;
    fxdsp::Smoother position;
    fxdsp::Smoother gateGain;
};

class ProFilterEffect : public DjFxEffect<ProFilterGroupState, ProFilterEffect> {
    DJFX_EFFECT_INTERFACE(ProFilterGroupState)

  private:
    enum Param {
        kSweep,
        kResonance,
    };
};
