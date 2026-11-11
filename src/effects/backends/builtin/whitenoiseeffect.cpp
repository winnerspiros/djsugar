#include "effects/backends/builtin/whitenoiseeffect.h"

QString WhiteNoiseEffect::getId() {
    return QStringLiteral("org.mixxx.effects.whitenoise");
}

EffectManifestPointer WhiteNoiseEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Noise"),
            QObject::tr("Noise"),
            QObject::tr("Filtered stereo noise for risers and transitions. Can "
                        "breathe with the groove of the track."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Noise level."),
            0.0,
            0.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("color"),
            QObject::tr("Color"),
            QObject::tr("Color"),
            QObject::tr("Left: low-pass from rumble up. Right: high-pass to a thin "
                        "hiss. Sweep it for risers."),
            0.0,
            0.5,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("resonance"),
            QObject::tr("Resonance"),
            QObject::tr("Res"),
            QObject::tr("Whistle at the filter frequency, a wind-like sweep."),
            0.0,
            0.3,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("follow"),
            QObject::tr("Follow"),
            QObject::tr("Follow"),
            QObject::tr("How much the noise follows the loudness of the track."),
            0.0,
            0.5,
            1.0);
    return pManifest;
}

WhiteNoiseGroupState::WhiteNoiseGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters),
          random{fxdsp::Random(0x1234567u), fxdsp::Random(0x7654321u)} {
    envelope.setup(0.002f, 0.12f, m_sampleRate);
    amount.setTime(0.02f, m_sampleRate);
    color.setTime(0.03f, m_sampleRate);
}

void WhiteNoiseEffect::processChannel(WhiteNoiseGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        for (int c = 0; c < 2; ++c) {
            pState->filter[c].reset();
            for (float& p : pState->pink[c]) {
                p = 0.0f;
            }
        }
        pState->envelope.reset();
        pState->amount.reset(param(kAmount));
        pState->color.reset(param(kColor));
    }
    pState->amount.setTarget(param(kAmount));
    pState->color.setTarget(param(kColor));
    const float q = 0.707f + 8.0f * param(kResonance) * param(kResonance);
    const float follow = param(kFollow);
    const float rate = pState->sampleRate();

    for (int i = 0; i < frames; ++i) {
        const float amount = pState->amount.next();
        const float color = pState->color.next();
        const float env = pState->envelope.process(0.5f * (pInput[2 * i] + pInput[2 * i + 1]));
        // Follow: noise level tracks the track envelope (scaled so a loud
        // track gives about full level).
        const float level = amount * amount * 0.5f *
                fxdsp::lerp(1.0f, fxdsp::clampf(env * 2.0f, 0.0f, 1.0f), follow);
        const bool lowPass = color < 0.5f;
        const float hz = lowPass ? fxdsp::knobToHz(color * 2.0f, 80.0f, 20000.0f)
                                 : fxdsp::knobToHz((color - 0.5f) * 2.0f, 20.0f, 12000.0f);
        for (int c = 0; c < 2; ++c) {
            // Pink-ish noise (three one-pole stages, Paul Kellet's economy
            // filter) sounds fuller than white for risers.
            const float white = pState->random[c].bipolar();
            float* p = pState->pink[c];
            p[0] = 0.99765f * p[0] + white * 0.0990460f;
            p[1] = 0.96300f * p[1] + white * 0.2965164f;
            p[2] = 0.57000f * p[2] + white * 1.0526913f;
            const float noise = 0.25f * (p[0] + p[1] + p[2] + white * 0.1848f);
            pState->filter[c].setCoeffs(hz, q, rate);
            const auto out = pState->filter[c].process(noise);
            // Resonance raises the peak; normalize so level stays usable.
            const float shaped = (lowPass ? out.lp : out.hp) / std::sqrt(q / 0.707f);
            pOutput[2 * i + c] = pInput[2 * i + c] + shaped * level;
        }
    }
}
