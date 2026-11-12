#include "effects/backends/builtin/djechoeffect.h"

QString DJEchoEffect::getId() {
    return QStringLiteral("org.mixxx.effects.djecho");
}

EffectManifestPointer DJEchoEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Dub Echo"),
            QObject::tr("DubEcho"),
            QObject::tr("Sound system dub delay: repeats get darker and grittier, "
                        "feedback past unity builds into a controlled howl."));

    djfx::division(pManifest,
            QStringLiteral("delay_time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("Echo time in beats."),
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
            QObject::tr("Feedback"),
            QObject::tr("Feedback"),
            QObject::tr("Above three quarters the loop sustains and builds."),
            0.0,
            0.65,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("band"),
            QObject::tr("Band"),
            QObject::tr("Band"),
            QObject::tr("Moves the band-pass in the loop from boomy to telephone "
                        "thin. Sweep it while the echoes build."),
            0.0,
            0.4,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("drive"),
            QObject::tr("Drive"),
            QObject::tr("Drive"),
            QObject::tr("Saturation in the loop."),
            0.0,
            0.5,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("spread"),
            QObject::tr("Spread"),
            QObject::tr("Spread"),
            QObject::tr("Ping-pong between left and right."),
            0.0,
            0.6,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Closes the input and loops the current echoes forever."));
    return pManifest;
}

DJEchoGroupState::DJEchoGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    delay.setup(m_sampleRate, 4.0f);
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.08f, m_sampleRate);
    band.setTime(0.06f, m_sampleRate);
    drive.setTime(0.05f, m_sampleRate);
}

void DJEchoEffect::processChannel(DJEchoGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->delay.clear();
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
        pState->band.reset(param(kBand));
        pState->drive.reset(param(kDrive));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));
    pState->band.setTarget(param(kBand));
    pState->drive.setTarget(param(kDrive));

    fxdsp::TempoDelay::Settings settings;
    double delaySamples = paramBeats(kTime) * pState->clock.samplesPerBeat();
    while (delaySamples > pState->delay.maxDelaySamples() - 16.0) {
        delaySamples *= 0.5;
    }
    settings.delaySamples = static_cast<float>(delaySamples);
    settings.spread = param(kSpread);
    settings.freeze = paramBool(kFreeze);
    // Tape-like glide makes knob moves of the time selector sound dubby.
    settings.glide = true;

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float amount = pState->amount.next();
        // 0..1 maps to loop gain 0..1.35. The band-pass and interpolation
        // lose about 2 dB per pass, so the top quarter of the knob builds
        // up and the saturator sets the ceiling.
        settings.feedback = 1.35f * pState->feedback.next();
        const float band = pState->band.next();
        // A two octave wide band whose center moves 150 Hz .. 4 kHz.
        const float center = fxdsp::knobToHz(band, 150.0f, 4000.0f);
        settings.lowCutHz = center * 0.5f;
        settings.highCutHz = std::min(center * 2.5f, 18000.0f);
        settings.drive = 0.2f + 0.8f * pState->drive.next();

        float wetL;
        float wetR;
        pState->delay.process(settings, inL, inR, &wetL, &wetR);
        const float gain = 0.75f * amount;
        pOutput[2 * i] = inL + wetL * gain;
        pOutput[2 * i + 1] = inR + wetR * gain;
    }
}
