#include "effects/backends/builtin/procompressoreffect.h"

namespace {

enum Band {
    kFull,
    kAboveBass,
    kBassOnly,
};

} // namespace

QString ProCompressorEffect::getId() {
    return QStringLiteral("org.mixxx.effects.procompressor");
}

EffectManifestPointer ProCompressorEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Pump"),
            QObject::tr("Pump"),
            QObject::tr("Beat-locked sidechain pumping. Ducks on every beat and "
                        "swells back like a compressor keyed by a kick."));

    djfx::division(pManifest,
            QStringLiteral("period"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Pump every this many beats."),
            1.0);
    auto pDepth = djfx::knob(pManifest,
            QStringLiteral("depth"),
            QObject::tr("Depth"),
            QObject::tr("Depth"),
            QObject::tr("How far the track ducks."),
            0.0,
            0.6,
            1.0);
    djfx::linkToMetaknob(pDepth);
    djfx::knob(pManifest,
            QStringLiteral("release"),
            QObject::tr("Release"),
            QObject::tr("Release"),
            QObject::tr("How much of the beat the swell back takes."),
            0.1,
            0.6,
            1.0);
    djfx::choice(pManifest,
            QStringLiteral("band"),
            QObject::tr("Band"),
            QObject::tr("Band"),
            QObject::tr("Full track, everything above the bass, or only the bass "
                        "(clears room for the other deck's kick)."),
            {QObject::tr("Full"), QObject::tr("Above Bass"), QObject::tr("Bass Only")});
    return pManifest;
}

ProCompressorGroupState::ProCompressorGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    for (auto& f : crossover) {
        f.setCutoff(160.0f, m_sampleRate);
    }
    depth.setTime(0.02f, m_sampleRate);
    // Attack of the duck: fast but not a click.
    gain.setTime(0.003f, m_sampleRate);
}

void ProCompressorEffect::processChannel(ProCompressorGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->crossover[0].reset();
        pState->crossover[1].reset();
        pState->depth.reset(param(kDepth));
        pState->gain.reset(1.0f);
    }
    pState->depth.setTarget(param(kDepth));
    const double period = paramBeats(kRate);
    const float release = param(kRelease);
    const int band = std::clamp(paramInt(kBand), 0, 2);

    for (int i = 0; i < frames; ++i) {
        const float phase = pState->clock.tick(period);
        // Duck curve: held down briefly after the beat, then an
        // exponential-feeling recovery (smoothstep of a power curve) that
        // completes at `release` of the period.
        const float t = fxdsp::clampf((phase - 0.02f) / release, 0.0f, 1.0f);
        const float shaped = 1.0f - (1.0f - t) * (1.0f - t) * (1.0f - t);
        const float depth = pState->depth.next();
        // Depth to -30 dB at full, in dB so it feels even.
        pState->gain.setTarget(fxdsp::dbToGain(-30.0f * depth * (1.0f - shaped)));
        const float g = pState->gain.next();
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            const float low = pState->crossover[c].lowpass(x);
            const float high = x - low;
            float y;
            switch (band) {
            case kAboveBass:
                y = low + high * g;
                break;
            case kBassOnly:
                y = low * g + high;
                break;
            case kFull:
            default:
                y = x * g;
                break;
            }
            pOutput[2 * i + c] = y;
        }
    }
}
