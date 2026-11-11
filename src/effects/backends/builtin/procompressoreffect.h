#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"

// Pump: the sidechain-compression "breathing" of dance music, locked to
// the beat grid. Ducks on every beat and swells back, with a curve shaped
// like a real compressor release. Can leave the bass alone, or duck only
// the bass to make room for a new kick in a blend.
class ProCompressorGroupState : public DjFxState {
  public:
    explicit ProCompressorGroupState(const mixxx::EngineParameters& engineParameters);

    fxdsp::OnePole crossover[2];
    fxdsp::Smoother depth;
    fxdsp::Smoother gain;
};

class ProCompressorEffect : public DjFxEffect<ProCompressorGroupState, ProCompressorEffect> {
    DJFX_EFFECT_INTERFACE(ProCompressorGroupState)

  private:
    enum Param {
        kRate,
        kDepth,
        kRelease,
        kBand,
    };
};
