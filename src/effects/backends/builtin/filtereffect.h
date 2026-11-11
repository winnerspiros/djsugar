#pragma once

#include "effects/backends/builtin/djfxbase.h"
#include "effects/backends/builtin/dsp/filters.h"
#include "util/class.h"

// One-knob DJ filter. Left of center sweeps a low-pass down, right of
// center sweeps a high-pass up, with a dead zone at center so it is truly
// bypassed there. Zero-delay-feedback state variable filters with a
// saturating resonance stage give the round, singing peak of analog DJ
// mixers instead of a thin digital whistle, and stay clean under fast
// sweeps.
class FilterGroupState : public DjFxState {
  public:
    explicit FilterGroupState(const mixxx::EngineParameters& engineParameters);
    void clear();

    fxdsp::Svf lp[2];
    fxdsp::Svf hp[2];
    fxdsp::Smoother position;
    fxdsp::Smoother resonance;
    fxdsp::Smoother drive;
};

class FilterEffect : public EffectProcessorImpl<FilterGroupState> {
  public:
    FilterEffect() = default;

    static QString getId();
    static EffectManifestPointer getManifest();

    void loadEngineEffectParameters(
            const QMap<QString, EngineEffectParameterPointer>& parameters) override;

    void processChannel(FilterGroupState* pState,
            const CSAMPLE* pInput,
            CSAMPLE* pOutput,
            const mixxx::EngineParameters& engineParameters,
            const EffectEnableState enableState,
            const GroupFeatureState& groupFeatures) override;

  private:
    EngineEffectParameterPointer m_pFilter;
    EngineEffectParameterPointer m_pResonance;
    EngineEffectParameterPointer m_pDrive;

    DISALLOW_COPY_AND_ASSIGN(FilterEffect);
};
