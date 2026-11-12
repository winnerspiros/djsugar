#include "effects/backends/builtin/filtereffect.h"

namespace {

// Half-width of the bypass zone around the knob center.
constexpr float kDeadZone = 0.02f;

} // namespace

QString FilterEffect::getId() {
    return QStringLiteral("org.mixxx.effects.filter");
}

EffectManifestPointer FilterEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Filter"),
            QObject::tr("Filter"),
            QObject::tr("One-knob DJ filter: low-pass to the left, high-pass to the "
                        "right, bypassed at center. Analog style resonance that "
                        "sings instead of whistling."),
            0.5);

    auto pFilter = djfx::knob(pManifest,
            QStringLiteral("filter"),
            QObject::tr("Filter"),
            QObject::tr("Filter"),
            QObject::tr("Left: low-pass sweeps down. Right: high-pass sweeps up. "
                        "Center: off."),
            0.0,
            0.5,
            1.0);
    djfx::linkToMetaknob(pFilter, 0.5);

    djfx::knob(pManifest,
            QStringLiteral("q"),
            QObject::tr("Resonance"),
            QObject::tr("Res"),
            QObject::tr("Emphasis at the cutoff frequency."),
            0.0,
            0.35,
            1.0);

    djfx::knob(pManifest,
            QStringLiteral("drive"),
            QObject::tr("Drive"),
            QObject::tr("Drive"),
            QObject::tr("Analog style saturation inside the filter. Fattens the "
                        "resonance peak."),
            0.0,
            0.2,
            1.0);

    return pManifest;
}

FilterGroupState::FilterGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    position.setTime(0.012f, m_sampleRate);
    resonance.setTime(0.03f, m_sampleRate);
    drive.setTime(0.03f, m_sampleRate);
    clear();
}

void FilterGroupState::clear() {
    for (int c = 0; c < 2; ++c) {
        lp[c].reset();
        hp[c].reset();
    }
}

void FilterEffect::loadEngineEffectParameters(
        const QMap<QString, EngineEffectParameterPointer>& parameters) {
    m_pFilter = parameters.value(QStringLiteral("filter"));
    m_pResonance = parameters.value(QStringLiteral("q"));
    m_pDrive = parameters.value(QStringLiteral("drive"));
}

void FilterEffect::processChannel(FilterGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const float position = djfx::value(m_pFilter);
    const float resonance = djfx::value(m_pResonance);
    const float drive = djfx::value(m_pDrive);
    if (enableState == EffectEnableState::Enabling) {
        pState->clear();
        pState->position.reset(position);
        pState->resonance.reset(resonance);
        pState->drive.reset(drive);
    }
    pState->position.setTarget(position);
    pState->resonance.setTarget(resonance);
    pState->drive.setTarget(drive);

    const float rate = pState->sampleRate();
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    for (int i = 0; i < frames; ++i) {
        const float pos = pState->position.next();
        const float res = pState->resonance.next();
        const float drv = pState->drive.next();
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];

        // Distance from center outside the dead zone, 0..1.
        const float offset = pos - 0.5f;
        const float amount = fxdsp::clampf(
                (std::fabs(offset) - kDeadZone) / (0.5f - kDeadZone), 0.0f, 1.0f);
        if (amount <= 0.0f) {
            // Keep both filters running at their open positions so that
            // leaving the dead zone is seamless.
            pState->lp[0].setCoeffs(20000.0f, 0.707f, rate);
            pState->lp[1].setCoeffs(20000.0f, 0.707f, rate);
            pState->hp[0].setCoeffs(16.0f, 0.707f, rate);
            pState->hp[1].setCoeffs(16.0f, 0.707f, rate);
            pState->lp[0].process(inL);
            pState->lp[1].process(inR);
            pState->hp[0].process(inL);
            pState->hp[1].process(inR);
            pOutput[2 * i] = inL;
            pOutput[2 * i + 1] = inR;
            continue;
        }

        // Resonance fades in over the first part of the sweep so the
        // filter does not ring at its open end.
        const float q = 0.707f + res * res * 14.0f * std::min(1.0f, amount * 4.0f);
        // Saturation limit of the resonant integrator: lower means more
        // compression of the peak.
        const float limit = 3.0f - 2.4f * drv;
        const float inGain = 1.0f + 1.5f * drv;
        float l;
        float r;
        if (offset < 0.0f) {
            // Low-pass from 20 kHz down to 60 Hz, perceptually even.
            const float hz = fxdsp::knobToHz(1.0f - amount, 60.0f, 20000.0f);
            pState->lp[0].setCoeffs(hz, q, rate);
            pState->lp[1].setCoeffs(hz, q, rate);
            l = pState->lp[0].processDriven(inL * inGain, limit).lp;
            r = pState->lp[1].processDriven(inR * inGain, limit).lp;
            pState->hp[0].process(inL);
            pState->hp[1].process(inR);
        } else {
            // High-pass from 16 Hz up to 12 kHz.
            const float hz = fxdsp::knobToHz(amount, 16.0f, 12000.0f);
            pState->hp[0].setCoeffs(hz, q, rate);
            pState->hp[1].setCoeffs(hz, q, rate);
            l = pState->hp[0].processDriven(inL * inGain, limit).hp;
            r = pState->hp[1].processDriven(inR * inGain, limit).hp;
            pState->lp[0].process(inL);
            pState->lp[1].process(inR);
        }
        // Undo the input gain so drive adds harmonics, not level.
        const float outGain = 1.0f / inGain;
        pOutput[2 * i] = l * outGain;
        pOutput[2 * i + 1] = r * outGain;
    }
}
