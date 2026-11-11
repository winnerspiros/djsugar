#include "effects/backends/builtin/autopaneffect.h"

QString AutoPanEffect::getId() {
    return QStringLiteral("org.mixxx.effects.autopan");
}

EffectManifestPointer AutoPanEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Auto Pan"),
            QObject::tr("AutoPan"),
            QObject::tr("Beat-synced panning. Bass stays centered."));

    djfx::division(pManifest,
            QStringLiteral("period"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Length of one left-right cycle in beats."),
            2.0);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("width"),
            QObject::tr("Width"),
            QObject::tr("Width"),
            QObject::tr("How far the sound travels."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("smoothing"),
            QObject::tr("Shape"),
            QObject::tr("Shape"),
            QObject::tr("Smooth sweep to hard hops between the sides."),
            0.0,
            0.0,
            1.0);
    return pManifest;
}

AutoPanGroupState::AutoPanGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    for (auto& f : crossover) {
        f.setCutoff(150.0f, m_sampleRate);
    }
    amount.setTime(0.02f, m_sampleRate);
}

void AutoPanEffect::processChannel(AutoPanGroupState* pState,
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
        pState->amount.reset(param(kAmount));
    }
    pState->amount.setTarget(param(kAmount));
    const double period = paramBeats(kRate);
    const float shape = param(kShape);

    for (int i = 0; i < frames; ++i) {
        const float phase = pState->clock.tick(period);
        // Blend a sine with a rounded square for the hop shape.
        const float sine = fxdsp::lfo(fxdsp::LfoShape::Sine, phase);
        const float hop = fxdsp::lfo(fxdsp::LfoShape::SmoothSquare,
                phase < 0.75f ? phase + 0.25f : phase - 0.75f,
                0.04f);
        const float position = fxdsp::lerp(sine, hop, shape); // 0 = left
        const float pan = 0.5f + (position - 0.5f) * pState->amount.next();
        // Balance law: unity at center, the far side fades out, the near
        // side never rises above unity, so panning cannot clip.
        const float gl = std::min(1.0f, 2.0f * (1.0f - pan));
        const float gr = std::min(1.0f, 2.0f * pan);
        const float lowL = pState->crossover[0].lowpass(pInput[2 * i]);
        const float lowR = pState->crossover[1].lowpass(pInput[2 * i + 1]);
        const float highL = pInput[2 * i] - lowL;
        const float highR = pInput[2 * i + 1] - lowR;
        pOutput[2 * i] = lowL + highL * gl;
        pOutput[2 * i + 1] = lowR + highR * gr;
    }
}
