#include "effects/backends/builtin/echoeffect.h"

namespace {

constexpr float kMaxSeconds = 4.0f;

// Maps the bipolar tone knob to the loop filters: left darkens each
// repeat (low-pass), right thins it (high-pass), center is open.
void toneToCutoffs(float tone, float* pLowCut, float* pHighCut) {
    const float t = tone - 0.5f;
    *pLowCut = t > 0.0f ? fxdsp::knobToHz(2.0f * t, 30.0f, 3000.0f) : 30.0f;
    *pHighCut = t < 0.0f ? fxdsp::knobToHz(1.0f + 2.0f * t, 600.0f, 18000.0f) : 18000.0f;
}

} // namespace

QString EchoEffect::getId() {
    return QStringLiteral("org.mixxx.effects.echo");
}

EffectManifestPointer EchoEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Echo"),
            QObject::tr("Echo"),
            QObject::tr("Beat-synced stereo echo with tone shaping, ping-pong, "
                        "ducking and freeze. Repeats ring out when the deck is "
                        "taken off the effect unit."));

    djfx::division(pManifest,
            QStringLiteral("delay_time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("Echo time in beats."),
            0.75);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("send_amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the echoes."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("feedback_amount"),
            QObject::tr("Feedback"),
            QObject::tr("Feedback"),
            QObject::tr("How long the echoes repeat. Full is near endless."),
            0.0,
            0.55,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("tone"),
            QObject::tr("Tone"),
            QObject::tr("Tone"),
            QObject::tr("Left: every repeat gets darker. Right: every repeat gets "
                        "thinner. Center: unchanged."),
            0.0,
            0.5,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("pingpong_amount"),
            QObject::tr("Ping Pong"),
            QObject::tr("PingPong"),
            QObject::tr("Bounces the repeats between left and right."),
            0.0,
            0.0,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("ducking"),
            QObject::tr("Ducking"),
            QObject::tr("Duck"),
            QObject::tr("Pushes the echoes down while the track is loud so they "
                        "bloom in the gaps."),
            0.0,
            0.0,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("tape"),
            QObject::tr("Tape"),
            QObject::tr("Tape"),
            QObject::tr("Time changes glide and bend the pitch like a tape "
                        "machine instead of crossfading."));
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Closes the input and loops the current echoes forever."));
    return pManifest;
}

EchoGroupState::EchoGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    delay.setup(m_sampleRate, kMaxSeconds);
    envelope.setup(0.005f, 0.25f, m_sampleRate);
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.05f, m_sampleRate);
    tone.setTime(0.05f, m_sampleRate);
    spread.setTime(0.05f, m_sampleRate);
}

void EchoEffect::processChannel(EchoGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->delay.clear();
        pState->envelope.reset();
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
        pState->tone.reset(param(kTone));
        pState->spread.reset(param(kSpread));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));
    pState->tone.setTarget(param(kTone));
    pState->spread.setTarget(param(kSpread));
    const float ducking = param(kDucking);

    fxdsp::TempoDelay::Settings settings;
    // Long divisions halve until they fit the buffer at slow tempos.
    double delaySamples = paramBeats(kTime) * pState->clock.samplesPerBeat();
    while (delaySamples > pState->delay.maxDelaySamples() - 16.0) {
        delaySamples *= 0.5;
    }
    settings.delaySamples = static_cast<float>(delaySamples);
    settings.glide = paramBool(kTape);
    settings.freeze = paramBool(kFreeze);
    settings.drive = 0.25f;

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float amount = pState->amount.next();
        // Feedback curve: most of the travel is in the useful 0.3..0.9 range.
        const float fb = pState->feedback.next();
        settings.feedback = 1.02f * (1.0f - (1.0f - fb) * (1.0f - fb));
        settings.spread = pState->spread.next();
        toneToCutoffs(pState->tone.next(), &settings.lowCutHz, &settings.highCutHz);

        float wetL;
        float wetR;
        pState->delay.process(settings, inL, inR, &wetL, &wetR);

        const float env = pState->envelope.process(0.5f * (inL + inR));
        const float duck = 1.0f - ducking * fxdsp::clampf(env * 2.5f, 0.0f, 0.85f);
        // 0.7 keeps repeats on top of a full-scale track out of the
        // master limiter.
        const float gain = 0.7f * amount * duck;
        pOutput[2 * i] = inL + wetL * gain;
        pOutput[2 * i + 1] = inR + wetR * gain;
    }
}
