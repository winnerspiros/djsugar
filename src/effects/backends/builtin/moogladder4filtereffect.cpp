#include "effects/backends/builtin/moogladder4filtereffect.h"

namespace {

enum Mode {
    kSine,
    kTriangle,
    kRampUp,
    kRampDown,
    kSquare,
    kEnvelope,
};

} // namespace

QString MoogLadder4FilterEffect::getId() {
    return QStringLiteral("org.mixxx.effects.moogladder4filter");
}

EffectManifestPointer MoogLadder4FilterEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Auto Filter"),
            QObject::tr("AutoFlt"),
            QObject::tr("Analog ladder low-pass that sweeps by itself in time with "
                        "the beat, or opens with the dynamics of the track like "
                        "an auto-wah."));

    djfx::division(pManifest,
            QStringLiteral("period"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Length of one sweep in beats."),
            4.0);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Depth of the sweep."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("cutoff"),
            QObject::tr("Cutoff"),
            QObject::tr("Cutoff"),
            QObject::tr("Lowest point of the sweep."),
            0.0,
            0.25,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("resonance"),
            QObject::tr("Resonance"),
            QObject::tr("Res"),
            QObject::tr("Emphasis at the cutoff; near full it starts to sing."),
            0.0,
            0.55,
            1.0);
    djfx::choice(pManifest,
            QStringLiteral("mode"),
            QObject::tr("Shape"),
            QObject::tr("Shape"),
            QObject::tr("Sweep shape, or Envelope to follow the track dynamics."),
            {QObject::tr("Sine"),
                    QObject::tr("Triangle"),
                    QObject::tr("Ramp Up"),
                    QObject::tr("Ramp Down"),
                    QObject::tr("Square"),
                    QObject::tr("Envelope")});
    djfx::knob(pManifest,
            QStringLiteral("drive"),
            QObject::tr("Drive"),
            QObject::tr("Drive"),
            QObject::tr("Pushes the ladder into saturation."),
            0.0,
            0.3,
            1.0);
    return pManifest;
}

MoogLadder4FilterGroupState::MoogLadder4FilterGroupState(
        const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    envelope.setup(0.004f, 0.15f, m_sampleRate);
    amount.setTime(0.02f, m_sampleRate);
    // Smooths the cutoff path: rounds square steps and ramp resets.
    cutoff.setTime(0.004f, m_sampleRate);
}

void MoogLadder4FilterEffect::processChannel(MoogLadder4FilterGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    const float base = param(kCutoff);
    if (enableState == EffectEnableState::Enabling) {
        pState->ladder[0].reset();
        pState->ladder[1].reset();
        pState->envelope.reset();
        pState->amount.reset(param(kAmount));
        pState->cutoff.reset(base);
    }
    pState->amount.setTarget(param(kAmount));
    const double period = paramBeats(kRate);
    const float resonance = param(kResonance);
    const int mode = paramInt(kMode);
    const float driveKnob = param(kDrive);
    const float rate = pState->sampleRate();

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float phase = pState->clock.tick(period);
        const float env = pState->envelope.process(0.5f * (inL + inR));
        float mod;
        switch (mode) {
        case kTriangle:
            mod = fxdsp::lfo(fxdsp::LfoShape::Triangle, phase);
            break;
        case kRampUp:
            mod = fxdsp::lfo(fxdsp::LfoShape::RampUp, phase);
            break;
        case kRampDown:
            mod = fxdsp::lfo(fxdsp::LfoShape::RampDown, phase);
            break;
        case kSquare:
            mod = fxdsp::lfo(fxdsp::LfoShape::SmoothSquare, phase, 0.02f);
            break;
        case kEnvelope:
            mod = fxdsp::clampf(env * 3.0f, 0.0f, 1.0f);
            break;
        case kSine:
        default:
            mod = fxdsp::lfo(fxdsp::LfoShape::Sine, phase);
            break;
        }
        const float amount = pState->amount.next();
        // At zero amount the filter rests fully open; amount pulls the
        // resting point down to the cutoff knob and adds the sweep.
        const float position = fxdsp::lerp(1.0f, base + (1.0f - base) * mod, amount);
        pState->cutoff.setTarget(position);
        const float hz = fxdsp::knobToHz(pState->cutoff.next(), 60.0f, 0.47f * rate);
        const float res = resonance * std::min(1.0f, amount * 3.0f);
        // Ladder input gain: 0.3 is nearly linear (no coloring at rest),
        // up to 3.3 saturates hard. It follows amount so a closed effect
        // is clean.
        const float drive = 0.3f + 3.0f * driveKnob * amount;
        pState->ladder[0].setCoeffs(hz, res, rate);
        pState->ladder[1].setCoeffs(hz, res, rate);
        const float yL = pState->ladder[0].process(inL, drive);
        const float yR = pState->ladder[1].process(inR, drive);
        // A four-pole low-pass even fully open dulls the top octave, so
        // the first stretch of the amount knob fades in from dry.
        const float wet = std::min(1.0f, amount * 5.0f);
        pOutput[2 * i] = fxdsp::lerp(inL, yL, wet);
        pOutput[2 * i + 1] = fxdsp::lerp(inR, yR, wet);
    }
}
