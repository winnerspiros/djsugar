#include "effects/backends/builtin/spiraleffect.h"

#include <algorithm>
#include <cmath>

namespace {
// Longest repeat the delay line can hold (beats may stretch it at slow tempos).
constexpr double kMaxDelaySeconds = 4.0;
// Used when the track has no beat grid: a quarter note at 120 BPM.
constexpr double kFallbackDelaySeconds = 0.5;
// Pitch wobble of the repeats: depth in seconds and rate in Hz.
constexpr double kWobbleSeconds = 0.0004;
constexpr double kWobbleHz = 5.0;
} // namespace

SpiralState::SpiralState(const mixxx::EngineParameters& engineParameters)
        : EffectState(engineParameters) {
    ensureLayout(engineParameters);
}

void SpiralState::ensureLayout(const mixxx::EngineParameters& engineParameters) {
    const int channels = engineParameters.channelCount();
    const auto frames = static_cast<unsigned int>(
            engineParameters.sampleRate() * kMaxDelaySeconds);
    if (channels == channelCount && frames == maxFrames) {
        return;
    }
    channelCount = channels;
    maxFrames = frames;
    buffer.fill(0.0f, static_cast<qsizetype>(maxFrames) * channelCount);
    lowpass.fill(0.0, channelCount);
    writeFrame = 0;
}

void SpiralState::clear() {
    buffer.fill(0.0f);
    lowpass.fill(0.0);
    writeFrame = 0;
}

// static
QString SpiralEffect::getId() {
    return "org.mixxx.effects.spiral";
}

// static
EffectManifestPointer SpiralEffect::getManifest() {
    EffectManifestPointer pManifest(new EffectManifest());
    pManifest->setId(getId());
    pManifest->setName(QObject::tr("Spiral"));
    pManifest->setShortName(QObject::tr("Spiral"));
    pManifest->setAuthor("The Mixxx Team");
    pManifest->setVersion("1.0");
    pManifest->setDescription(QObject::tr(
            "Beat-synced echo whose repeats darken and detune with every pass, "
            "so the tail spirals down in pitch."));

    EffectManifestParameterPointer beats = pManifest->addParameter();
    beats->setId("beats");
    beats->setName(QObject::tr("Beats"));
    beats->setShortName(QObject::tr("Beats"));
    beats->setDescription(QObject::tr(
            "Time between repeats in beats. Without a beat grid, 0.5 s is used."));
    beats->setValueScaler(EffectManifestParameter::ValueScaler::Logarithmic);
    beats->setUnitsHint(EffectManifestParameter::UnitsHint::Beats);
    beats->setRange(1.0 / 16, 1.0 / 2, 4);

    EffectManifestParameterPointer feedback = pManifest->addParameter();
    feedback->setId("feedback");
    feedback->setName(QObject::tr("Feedback"));
    feedback->setShortName(QObject::tr("Feedback"));
    feedback->setDescription(QObject::tr("How many repeats are heard before the tail dies."));
    feedback->setValueScaler(EffectManifestParameter::ValueScaler::Linear);
    feedback->setUnitsHint(EffectManifestParameter::UnitsHint::Unknown);
    feedback->setRange(0.0, 0.55, 0.9);

    EffectManifestParameterPointer spiral = pManifest->addParameter();
    spiral->setId("spiral");
    spiral->setName(QObject::tr("Spiral"));
    spiral->setShortName(QObject::tr("Spiral"));
    spiral->setDescription(QObject::tr(
            "How fast each repeat darkens and detunes. 0 keeps the echo clean."));
    spiral->setValueScaler(EffectManifestParameter::ValueScaler::Linear);
    spiral->setUnitsHint(EffectManifestParameter::UnitsHint::Unknown);
    spiral->setRange(0.0, 0.5, 1.0);

    EffectManifestParameterPointer depth = pManifest->addParameter();
    depth->setId("depth");
    depth->setName(QObject::tr("Level"));
    depth->setShortName(QObject::tr("Level"));
    depth->setDescription(QObject::tr("Loudness of the repeats against the dry signal."));
    depth->setValueScaler(EffectManifestParameter::ValueScaler::Linear);
    depth->setUnitsHint(EffectManifestParameter::UnitsHint::Unknown);
    depth->setDefaultLinkType(EffectManifestParameter::LinkType::Linked);
    depth->setRange(0.0, 0.7, 1.0);

    return pManifest;
}

void SpiralEffect::loadEngineEffectParameters(
        const QMap<QString, EngineEffectParameterPointer>& parameters) {
    m_pBeatsParameter = parameters.value("beats");
    m_pFeedbackParameter = parameters.value("feedback");
    m_pSpiralParameter = parameters.value("spiral");
    m_pDepthParameter = parameters.value("depth");
}

void SpiralEffect::processChannel(
        SpiralState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    pState->ensureLayout(engineParameters);
    if (enableState == EffectEnableState::Enabling) {
        pState->clear();
    }

    const int channels = engineParameters.channelCount();
    const double sampleRate = engineParameters.sampleRate();
    const auto maxFrames = pState->maxFrames;

    const double beats = m_pBeatsParameter ? m_pBeatsParameter->value() : 0.5;
    const double feedback = m_pFeedbackParameter ? m_pFeedbackParameter->value() : 0.55;
    const double spiral = m_pSpiralParameter ? m_pSpiralParameter->value() : 0.5;
    const double depth = m_pDepthParameter ? m_pDepthParameter->value() : 0.7;

    // Delay in frames: beat-synced when the deck has a beat grid.
    double delayFrames = kFallbackDelaySeconds * sampleRate;
    if (groupFeatures.beat_length.has_value()) {
        delayFrames = groupFeatures.beat_length->seconds * sampleRate * beats;
    }
    delayFrames = std::clamp(delayFrames, 1.0, static_cast<double>(maxFrames) - 2.0);

    // Repeats darken more with a higher spiral amount: 1.0 at 0, 0.4 at full.
    const double lowpassCoef = 1.0 - 0.6 * spiral;
    const double wobbleFrames = kWobbleSeconds * sampleRate * spiral;
    const double lfoStep = 2.0 * M_PI * kWobbleHz / sampleRate;

    for (SINT i = 0; i < engineParameters.samplesPerBuffer(); i += channels) {
        // Read head: one delay behind the write head, wobbling slightly.
        const double wobble = wobbleFrames * std::sin(pState->lfoPhase);
        pState->lfoPhase += lfoStep;
        if (pState->lfoPhase > 2.0 * M_PI) {
            pState->lfoPhase -= 2.0 * M_PI;
        }

        double readPos = static_cast<double>(pState->writeFrame) - delayFrames - wobble;
        while (readPos < 0.0) {
            readPos += maxFrames;
        }
        const auto i0 = static_cast<unsigned int>(readPos) % maxFrames;
        const auto i1 = (i0 + 1) % maxFrames;
        const double frac = readPos - std::floor(readPos);

        for (int c = 0; c < channels; c++) {
            const double a = pState->buffer[i0 * channels + c];
            const double b = pState->buffer[i1 * channels + c];
            const double delayed = a + (b - a) * frac;

            // Feedback path: darken each pass, then saturate to keep it bounded.
            double& lp = pState->lowpass[c];
            lp += lowpassCoef * (delayed - lp);
            const double written = std::tanh(pInput[i + c] + lp * feedback);
            pState->buffer[pState->writeFrame * channels + c] = static_cast<CSAMPLE>(written);

            pOutput[i + c] = pInput[i + c] + static_cast<CSAMPLE>(depth * delayed);
        }
        pState->writeFrame = (pState->writeFrame + 1) % maxFrames;
    }
}
