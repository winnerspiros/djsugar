#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/modulation.h"

// Pitch: shifts the deck up or down by up to an octave without changing
// tempo, with smooth glides when the knob moves. A two-head granular
// shifter: its delay is half a grain (12 to 50 ms), cheap on phones.
class PitchShiftGroupState : public DjFxState {
  public:
    explicit PitchShiftGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::PitchShifter shifter[2];
    fxdsp::Smoother semitones;
    fxdsp::Smoother mix;
    float lastWindow = -1.0f;
};

class PitchShiftEffect : public DjFxEffect<PitchShiftGroupState, PitchShiftEffect> {
    DJFX_EFFECT_INTERFACE(PitchShiftGroupState)

  private:
    enum Param {
        kPitch,
        kRange,
        kMix,
        kGrain,
    };
};
