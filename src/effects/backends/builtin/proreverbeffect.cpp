#include "effects/backends/builtin/proreverbeffect.h"

namespace {

constexpr float kIntervals[] = {12.0f, 7.0f, 19.0f, -12.0f};

} // namespace

QString ProReverbEffect::getId() {
    return QStringLiteral("org.mixxx.effects.proreverb");
}

EffectManifestPointer ProReverbEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Shimmer"),
            QObject::tr("Shimmer"),
            QObject::tr("Reverb whose tail is pitch-shifted back into itself, "
                        "blooming into a rising, ethereal pad."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the shimmer."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("decay"),
            QObject::tr("Decay"),
            QObject::tr("Decay"),
            QObject::tr("Reverb time (RT60)."),
            1.0,
            6.0,
            30.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Seconds);
    djfx::knob(pManifest,
            QStringLiteral("shimmer"),
            QObject::tr("Shimmer"),
            QObject::tr("Shimmer"),
            QObject::tr("How much of the pitched tail is fed back. More makes it "
                        "climb further."),
            0.0,
            0.5,
            1.0);
    djfx::choice(pManifest,
            QStringLiteral("interval"),
            QObject::tr("Interval"),
            QObject::tr("Intrvl"),
            QObject::tr("Pitch interval of the shimmer."),
            {QObject::tr("Octave"),
                    QObject::tr("Fifth"),
                    QObject::tr("Octave + Fifth"),
                    QObject::tr("Octave Down")});
    djfx::knob(pManifest,
            QStringLiteral("tone"),
            QObject::tr("Tone"),
            QObject::tr("Tone"),
            QObject::tr("Brightness of the tail."),
            0.0,
            0.6,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Holds the current reverb forever and closes the input."));
    return pManifest;
}

ProReverbGroupState::ProReverbGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    reverb.setup(m_sampleRate);
    reverb.setSize(0.85f, true);
    reverb.setLowCut(180.0f);
    reverb.setPreDelay(0.03f);
    reverb.setModulation(1.4f);
    for (int c = 0; c < 2; ++c) {
        shift[c].setup(m_sampleRate, 0.08f);
        shimmerLowCut[c].setCutoff(400.0f, m_sampleRate);
    }
    amount.setTime(0.02f, m_sampleRate);
    shimmer.setTime(0.1f, m_sampleRate);
}

void ProReverbEffect::processChannel(ProReverbGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    auto& reverb = pState->reverb;
    if (enableState == EffectEnableState::Enabling) {
        reverb.clear();
        for (int c = 0; c < 2; ++c) {
            pState->shift[c].clear();
            pState->shimmerLowCut[c].reset();
        }
        pState->feedbackL = 0.0f;
        pState->feedbackR = 0.0f;
        pState->amount.reset(param(kAmount));
        pState->shimmer.reset(param(kShimmer));
    }
    const float decay = param(kDecay);
    if (decay != pState->lastDecay) {
        reverb.setDecay(decay);
        pState->lastDecay = decay;
    }
    const float tone = param(kTone);
    if (tone != pState->lastTone) {
        reverb.setDamping(fxdsp::knobToHz(tone, 2000.0f, 16000.0f));
        pState->lastTone = tone;
    }
    const bool freeze = paramBool(kFreeze);
    reverb.setFreeze(freeze);
    pState->amount.setTarget(param(kAmount));
    pState->shimmer.setTarget(param(kShimmer));
    const int interval = std::clamp(paramInt(kInterval), 0, 3);
    const float ratio = std::pow(2.0f, kIntervals[interval] / 12.0f);

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        // Below 0.55 the pitched feedback stays clearly under unity.
        const float shimmer = 0.55f * pState->shimmer.next();
        float wetL;
        float wetR;
        reverb.process(inL + pState->feedbackL * shimmer,
                inR + pState->feedbackR * shimmer,
                &wetL,
                &wetR);
        // Pitch the tail and keep the mud out of the feedback.
        pState->feedbackL = fxdsp::sanitize(
                pState->shimmerLowCut[0].highpass(pState->shift[0].process(wetL, ratio)));
        pState->feedbackR = fxdsp::sanitize(
                pState->shimmerLowCut[1].highpass(pState->shift[1].process(wetR, ratio)));
        const float gain = pState->amount.next();
        pOutput[2 * i] = inL + wetL * gain;
        pOutput[2 * i + 1] = inR + wetR * gain;
    }
}
