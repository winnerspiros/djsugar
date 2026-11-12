#include "effects/backends/builtin/spiraleffect.h"

QString SpiralEffect::getId() {
    return QStringLiteral("org.mixxx.effects.spiral");
}

EffectManifestPointer SpiralEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Spiral"),
            QObject::tr("Spiral"),
            QObject::tr("Echo whose repeats climb or fall in pitch on every pass "
                        "while they fade, smeared into a swirling tail."));

    djfx::division(pManifest,
            QStringLiteral("delay_time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("Echo time in beats."),
            0.5);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the spiral."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("feedback"),
            QObject::tr("Feedback"),
            QObject::tr("Feedback"),
            QObject::tr("How many passes the spiral makes."),
            0.0,
            0.7,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("pitch"),
            QObject::tr("Pitch Step"),
            QObject::tr("Pitch"),
            QObject::tr("Semitones added on every repeat. Positive spirals up, "
                        "negative spirals down."),
            -12.0,
            2.0,
            12.0);
    djfx::knob(pManifest,
            QStringLiteral("smear"),
            QObject::tr("Smear"),
            QObject::tr("Smear"),
            QObject::tr("Blurs the repeats into a reverb wash."),
            0.0,
            0.35,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Closes the input; the spiral keeps turning."));
    return pManifest;
}

SpiralGroupState::SpiralGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    delay.setup(m_sampleRate, 4.0f);
    smear.setup(m_sampleRate);
    smear.setSize(0.25f, true);
    smear.setDecay(1.2f);
    smear.setDamping(7000.0f);
    smear.setLowCut(150.0f);
    smear.setModulation(0.6f);
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.08f, m_sampleRate);
}

void SpiralEffect::processChannel(SpiralGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->delay.clear();
        pState->smear.clear();
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));

    fxdsp::TempoDelay::Settings settings;
    double delaySamples = paramBeats(kTime) * pState->clock.samplesPerBeat();
    while (delaySamples > pState->delay.maxDelaySamples() - 16.0) {
        delaySamples *= 0.5;
    }
    settings.delaySamples = static_cast<float>(delaySamples);
    settings.pitchSemitones = param(kPitch);
    // A pitch step of exactly 0 bypasses the shifter; keep it engaged so
    // sweeping through zero does not change the sound abruptly.
    if (settings.pitchSemitones == 0.0f) {
        settings.pitchSemitones = 1e-4f;
    }
    settings.freeze = paramBool(kFreeze);
    settings.spread = 0.35f;
    settings.lowCutHz = 120.0f;
    settings.highCutHz = 9000.0f;
    settings.drive = 0.2f;
    const float smear = param(kSmear);

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float amount = pState->amount.next();
        settings.feedback = pState->feedback.next();

        float wetL;
        float wetR;
        pState->delay.process(settings, inL, inR, &wetL, &wetR);
        float washL;
        float washR;
        pState->smear.process(wetL, wetR, &washL, &washR);
        const float l = fxdsp::lerp(wetL, washL * 1.6f, smear);
        const float r = fxdsp::lerp(wetR, washR * 1.6f, smear);
        const float gain = 0.75f * amount;
        pOutput[2 * i] = inL + l * gain;
        pOutput[2 * i + 1] = inR + r * gain;
    }
}
