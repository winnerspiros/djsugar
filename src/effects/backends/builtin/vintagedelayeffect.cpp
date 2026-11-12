#include "effects/backends/builtin/vintagedelayeffect.h"

QString VintageDelayEffect::getId() {
    return QStringLiteral("org.mixxx.effects.vintagedelay");
}

EffectManifestPointer VintageDelayEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Tape Echo"),
            QObject::tr("TapeEcho"),
            QObject::tr("Tape machine echo: gliding head, wow and flutter, warm "
                        "saturation and repeats that lose their top end."));

    djfx::division(pManifest,
            QStringLiteral("delay_time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("Echo time in beats. Changing it glides the tape head."),
            0.75);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the echoes."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("feedback"),
            QObject::tr("Intensity"),
            QObject::tr("Intens"),
            QObject::tr("Number of repeats. Above three quarters the tape starts "
                        "to run away."),
            0.0,
            0.55,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("age"),
            QObject::tr("Age"),
            QObject::tr("Age"),
            QObject::tr("Wow, flutter and high-frequency loss of a worn tape."),
            0.0,
            0.35,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("saturation"),
            QObject::tr("Saturation"),
            QObject::tr("Sat"),
            QObject::tr("Tape saturation in the loop."),
            0.0,
            0.5,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Closes the input and loops the current echoes forever."));
    return pManifest;
}

VintageDelayGroupState::VintageDelayGroupState(
        const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters),
          random(0xA11CEu) {
    delay.setup(m_sampleRate, 4.0f);
    flutterNoise.setCutoff(12.0f, m_sampleRate);
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.08f, m_sampleRate);
    age.setTime(0.1f, m_sampleRate);
}

void VintageDelayEffect::processChannel(VintageDelayGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->delay.clear();
        pState->flutterNoise.reset();
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
        pState->age.reset(param(kAge));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));
    pState->age.setTarget(param(kAge));

    fxdsp::TempoDelay::Settings settings;
    double delaySamples = paramBeats(kTime) * pState->clock.samplesPerBeat();
    while (delaySamples > pState->delay.maxDelaySamples() - 64.0) {
        delaySamples *= 0.5;
    }
    settings.delaySamples = static_cast<float>(delaySamples);
    settings.glide = true;
    settings.freeze = paramBool(kFreeze);
    settings.drive = 0.15f + 0.85f * param(kSaturation);
    settings.lowCutHz = 90.0f;

    const float rate = pState->sampleRate();
    const float msToSamples = rate * 0.001f;
    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float amount = pState->amount.next();
        settings.feedback = 1.08f * pState->feedback.next();
        const float age = pState->age.next();

        // Wow: slow capstan wobble. Flutter: faster, partly random.
        pState->wowPhase += 0.55f / rate;
        pState->wowPhase -= std::floor(pState->wowPhase);
        pState->flutterPhase += 6.8f / rate;
        pState->flutterPhase -= std::floor(pState->flutterPhase);
        const float wow = std::sin(fxdsp::kTwoPi * pState->wowPhase);
        const float flutter = std::sin(fxdsp::kTwoPi * pState->flutterPhase) +
                2.0f * pState->flutterNoise.lowpass(pState->random.bipolar());
        settings.modulationSamples = age * msToSamples * (2.2f * (1.0f + wow) + 0.25f * flutter);
        settings.highCutHz = fxdsp::knobToHz(1.0f - age, 2500.0f, 14000.0f);

        float wetL;
        float wetR;
        pState->delay.process(settings, inL, inR, &wetL, &wetR);
        const float gain = 0.75f * amount;
        pOutput[2 * i] = inL + wetL * gain;
        pOutput[2 * i + 1] = inR + wetR * gain;
    }
}
