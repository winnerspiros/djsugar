#pragma once

// Shared plumbing for the DJ Sugar built-in effects: manifest helpers, a
// beat-division table shared by every tempo-synced effect and a state base
// that owns the beat clock.
//
// Conventions every effect follows:
//  * Output is the complete signal (dry + wet); manifests leave
//    addDryToWet off and let the engine crossfade on enable and disable.
//  * State is cleared on Enabling, never on Disabling, so time based
//    effects can ring out when a deck is unassigned (see
//    EngineEffectChain tails).
//  * Every knob is smoothed per sample; nothing allocates in process.
//  * The linked (super knob) parameter is the one that makes the effect
//    stronger when turned clockwise.

#include <QList>
#include <QPair>
#include <QString>
#include <QStringList>
#include <optional>
#include <vector>

#include "effects/backends/builtin/beatdivisions.h"
#include "effects/backends/builtin/dsp/fxmath.h"
#include "effects/backends/builtin/dsp/modulation.h"
#include "effects/backends/effectmanifest.h"
#include "effects/backends/effectprocessor.h"
#include "engine/effects/engineeffectparameter.h"
#include "engine/effects/groupfeaturestate.h"
#include "engine/engine.h"

namespace djfx {

using Scaler = EffectManifestParameter::ValueScaler;
using Units = EffectManifestParameter::UnitsHint;
using Link = EffectManifestParameter::LinkType;

inline double divisionBeats(const EngineEffectParameterPointer& pParam) {
    const auto& divisions = beatDivisions();
    const int i = std::clamp(pParam->toInt(), 0, static_cast<int>(divisions.size()) - 1);
    return divisions[i].second;
}

inline EffectManifestPointer manifest(const QString& id,
        const QString& name,
        const QString& shortName,
        const QString& description,
        double metaknobDefault = 0.0) {
    auto pManifest = EffectManifestPointer::create();
    pManifest->setId(id);
    pManifest->setName(name);
    pManifest->setShortName(shortName);
    pManifest->setAuthor(QStringLiteral("DJ Sugar"));
    pManifest->setVersion(QStringLiteral("3.0"));
    pManifest->setDescription(description);
    pManifest->setEffectRampsFromDry(false);
    pManifest->setAddDryToWet(false);
    pManifest->setMetaknobDefault(metaknobDefault);
    return pManifest;
}

inline EffectManifestParameterPointer knob(const EffectManifestPointer& pManifest,
        const QString& id,
        const QString& name,
        const QString& shortName,
        const QString& description,
        double minimum,
        double defaultValue,
        double maximum,
        Scaler scaler = Scaler::Linear,
        Units units = Units::Unknown) {
    auto pParam = pManifest->addParameter();
    pParam->setId(id);
    pParam->setName(name);
    pParam->setShortName(shortName);
    pParam->setDescription(description);
    pParam->setValueScaler(scaler);
    pParam->setUnitsHint(units);
    pParam->setRange(minimum, defaultValue, maximum);
    return pParam;
}

// Makes a parameter follow the effect's super knob. neutral is the knob
// position (0..1) where the parameter has no audible effect.
inline void linkToMetaknob(const EffectManifestParameterPointer& pParam,
        double neutral = 0.0,
        Link link = Link::Linked) {
    pParam->setDefaultLinkType(link);
    pParam->setNeutralPointOnScale(neutral);
}

inline EffectManifestParameterPointer toggle(const EffectManifestPointer& pManifest,
        const QString& id,
        const QString& name,
        const QString& shortName,
        const QString& description,
        bool defaultOn = false) {
    auto pParam = pManifest->addParameter();
    pParam->setId(id);
    pParam->setName(name);
    pParam->setShortName(shortName);
    pParam->setDescription(description);
    pParam->setValueScaler(Scaler::Toggle);
    pParam->setUnitsHint(Units::Unknown);
    pParam->setRange(0, defaultOn ? 1 : 0, 1);
    return pParam;
}

// A stepped selector. Steps are numbered 0..n-1.
inline EffectManifestParameterPointer choice(const EffectManifestPointer& pManifest,
        const QString& id,
        const QString& name,
        const QString& shortName,
        const QString& description,
        const QStringList& steps,
        int defaultStep = 0) {
    auto pParam = pManifest->addParameter();
    pParam->setId(id);
    pParam->setName(name);
    pParam->setShortName(shortName);
    pParam->setDescription(description);
    pParam->setValueScaler(Scaler::Toggle);
    pParam->setUnitsHint(Units::Unknown);
    pParam->setRange(0, defaultStep, steps.size() - 1);
    for (int i = 0; i < steps.size(); ++i) {
        pParam->appendStep(qMakePair(steps[i], static_cast<double>(i)));
    }
    return pParam;
}

// A beat-division selector, see beatDivisions().
inline EffectManifestParameterPointer division(const EffectManifestPointer& pManifest,
        const QString& id,
        const QString& name,
        const QString& shortName,
        const QString& description,
        double defaultBeats) {
    auto pParam = pManifest->addParameter();
    pParam->setId(id);
    pParam->setName(name);
    pParam->setShortName(shortName);
    pParam->setDescription(description);
    pParam->setValueScaler(Scaler::Toggle);
    pParam->setUnitsHint(Units::Beats);
    const auto& divisions = beatDivisions();
    pParam->setRange(0, divisionIndex(defaultBeats), divisions.size() - 1);
    for (int i = 0; i < divisions.size(); ++i) {
        pParam->appendStep(qMakePair(divisions[i].first, static_cast<double>(i)));
    }
    return pParam;
}

inline float value(const EngineEffectParameterPointer& pParam) {
    return static_cast<float>(pParam->value());
}

} // namespace djfx

// Base state: sample rate, the deck's beat clock and a buffer frame count.
// Effects with stereo state process interleaved stereo frames; Mixxx
// effect buffers are always stereo (mixxx::kEngineChannelOutputCount).
class DjFxState : public EffectState {
  public:
    explicit DjFxState(const mixxx::EngineParameters& engineParameters)
            : EffectState(engineParameters),
              m_sampleRate(static_cast<float>(engineParameters.sampleRate())) {
    }

    // Updates the beat clock; call at the start of every buffer.
    void updateClock(const GroupFeatureState& features, int frames) {
        std::optional<double> beatSeconds;
        if (features.beat_length) {
            beatSeconds = features.beat_length->seconds;
        }
        clock.update(beatSeconds, features.beat_fraction_buffer_end, frames, m_sampleRate);
    }

    float sampleRate() const {
        return m_sampleRate;
    }

    fxdsp::BeatClock clock;

  protected:
    float m_sampleRate;
};

// Effect base: binds the engine parameters in manifest order so effects
// address them by an enum instead of string lookups in the audio thread.
template<class State, class Derived>
class DjFxEffect : public EffectProcessorImpl<State> {
  public:
    void loadEngineEffectParameters(
            const QMap<QString, EngineEffectParameterPointer>& parameters) override {
        m_params.clear();
        const auto pManifest = Derived::getManifest();
        for (const auto& pParam : pManifest->parameters()) {
            m_params.push_back(parameters.value(pParam->id()));
        }
    }

  protected:
    float param(int index) const {
        return static_cast<float>(m_params[static_cast<std::size_t>(index)]->value());
    }
    int paramInt(int index) const {
        return m_params[static_cast<std::size_t>(index)]->toInt();
    }
    bool paramBool(int index) const {
        return m_params[static_cast<std::size_t>(index)]->toBool();
    }
    double paramBeats(int index) const {
        return djfx::divisionBeats(m_params[static_cast<std::size_t>(index)]);
    }

  private:
    std::vector<EngineEffectParameterPointer> m_params;
};

// Declares the members every DjFxEffect subclass implements.
#define DJFX_EFFECT_INTERFACE(StateClass)                    \
  public:                                                    \
    static QString getId();                                  \
    static EffectManifestPointer getManifest();              \
    void processChannel(StateClass* pState,                  \
            const CSAMPLE* pInput,                           \
            CSAMPLE* pOutput,                                \
            const mixxx::EngineParameters& engineParameters, \
            const EffectEnableState enableState,             \
            const GroupFeatureState& groupFeatures) override;
