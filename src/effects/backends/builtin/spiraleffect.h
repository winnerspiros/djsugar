#pragma once

#include <QString>
#include <QVector>

#include "effects/backends/effectmanifest.h"
#include "effects/backends/effectprocessor.h"
#include "engine/effects/engineeffectparameter.h"
#include "util/class.h"
#include "util/types.h"

// Spiral: a beat-synced feedback echo whose repeats darken and detune with each
// pass, so the tail winds down in pitch like a spiral.
class SpiralState : public EffectState {
  public:
    explicit SpiralState(const mixxx::EngineParameters& engineParameters);
    ~SpiralState() override = default;

    // Re-allocates the delay line when the sample rate or channel count changes.
    void ensureLayout(const mixxx::EngineParameters& engineParameters);
    void clear();

    int channelCount = 0;
    unsigned int maxFrames = 0;
    unsigned int writeFrame = 0;
    double lfoPhase = 0.0;
    QVector<CSAMPLE> buffer;   // interleaved delay line, maxFrames * channelCount
    QVector<double> lowpass;   // one-pole lowpass state per channel
};

class SpiralEffect : public EffectProcessorImpl<SpiralState> {
  public:
    SpiralEffect() = default;
    ~SpiralEffect() override = default;

    static QString getId();
    static EffectManifestPointer getManifest();

    void loadEngineEffectParameters(
            const QMap<QString, EngineEffectParameterPointer>& parameters) override;

    void processChannel(
            SpiralState* pState,
            const CSAMPLE* pInput,
            CSAMPLE* pOutput,
            const mixxx::EngineParameters& engineParameters,
            const EffectEnableState enableState,
            const GroupFeatureState& groupFeatures) override;

  private:
    QString debugString() const {
        return getId();
    }

    EngineEffectParameterPointer m_pBeatsParameter;
    EngineEffectParameterPointer m_pFeedbackParameter;
    EngineEffectParameterPointer m_pSpiralParameter;
    EngineEffectParameterPointer m_pDepthParameter;

    DISALLOW_COPY_AND_ASSIGN(SpiralEffect);
};
