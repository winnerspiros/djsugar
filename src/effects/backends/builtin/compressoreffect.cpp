#include "effects/backends/builtin/compressoreffect.h"

namespace {

constexpr float kKneeDb = 6.0f;

// Static curve with a soft knee (Giannoulis, Massberg, Reiss, "Digital
// Dynamic Range Compressor Design", JAES 2012). Returns gain reduction
// in dB (<= 0).
float gainReductionDb(float levelDb, float thresholdDb, float ratio) {
    const float over = levelDb - thresholdDb;
    if (2.0f * over < -kKneeDb) {
        return 0.0f;
    }
    const float slope = 1.0f / ratio - 1.0f;
    if (2.0f * std::fabs(over) <= kKneeDb) {
        const float x = over + kKneeDb * 0.5f;
        return slope * x * x / (2.0f * kKneeDb);
    }
    return slope * over;
}

} // namespace

QString CompressorEffect::getId() {
    return QStringLiteral("org.mixxx.effects.compressor");
}

EffectManifestPointer CompressorEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Compressor"),
            QObject::tr("Comp"),
            QObject::tr("Stereo glue compressor with soft knee and automatic "
                        "makeup. One knob from gentle glue to squashed and "
                        "pumping."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Lowers the threshold and raises the ratio together."),
            0.0,
            0.3,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("attack"),
            QObject::tr("Attack"),
            QObject::tr("Attack"),
            QObject::tr("Slower attack lets the kick punch through."),
            0.1,
            10.0,
            100.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Millisecond);
    djfx::knob(pManifest,
            QStringLiteral("release"),
            QObject::tr("Release"),
            QObject::tr("Release"),
            QObject::tr("Short release pumps, long release glues."),
            20.0,
            150.0,
            1500.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Millisecond);
    djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Parallel (New York) compression: blend with the "
                        "uncompressed track."),
            0.0,
            1.0,
            1.0);
    return pManifest;
}

CompressorGroupState::CompressorGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    amount.setTime(0.05f, m_sampleRate);
    mix.setTime(0.03f, m_sampleRate);
}

void CompressorEffect::processChannel(CompressorGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        pState->envelopeDb = 0.0f;
        pState->slowRelease = 0.0f;
        pState->amount.reset(param(kAmount));
        pState->mix.reset(param(kMix));
    }
    pState->amount.setTarget(param(kAmount));
    pState->mix.setTarget(param(kMix));
    const float rate = pState->sampleRate();
    const float attack = fxdsp::onePoleCoeff(param(kAttack) * 0.001f, rate);
    const float release = fxdsp::onePoleCoeff(param(kRelease) * 0.001f, rate);
    const float slowCoeff = fxdsp::onePoleCoeff(param(kRelease) * 0.004f, rate);

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float amount = pState->amount.next();
        // Amount: threshold 0 dB -> -30 dB, ratio 1.5:1 -> 10:1.
        const float thresholdDb = -30.0f * amount;
        const float ratio = 1.5f + 8.5f * amount * amount;
        const float peak = std::max(std::fabs(inL), std::fabs(inR));
        const float levelDb = fxdsp::gainToDb(peak);
        // Gain computer in the log domain, then a branching smoother
        // (attack on more reduction, release on less).
        const float target = gainReductionDb(levelDb, thresholdDb, ratio);
        const float coeff = target < pState->envelopeDb ? attack : release;
        pState->envelopeDb = target + coeff * (pState->envelopeDb - target);
        // Program dependent release: a slower stage holds sustained
        // reduction, so long loud passages do not pump.
        pState->slowRelease = std::min(pState->envelopeDb,
                pState->envelopeDb + slowCoeff * (pState->slowRelease - pState->envelopeDb));
        const float reductionDb = 0.5f * (pState->envelopeDb + pState->slowRelease);
        // Auto makeup: 40% of the reduction a full-scale peak gets, which
        // keeps loudness roughly level without pushing peaks over 0 dBFS.
        const float makeupDb = -0.4f * gainReductionDb(0.0f, thresholdDb, ratio);
        const float gain = fxdsp::dbToGain(reductionDb + makeupDb);
        const float mix = pState->mix.next();
        pOutput[2 * i] = fxdsp::lerp(inL, inL * gain, mix);
        pOutput[2 * i + 1] = fxdsp::lerp(inR, inR * gain, mix);
    }
}
