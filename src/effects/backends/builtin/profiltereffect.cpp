#include "effects/backends/builtin/profiltereffect.h"

namespace {

constexpr float kDeadZone = 0.02f;

} // namespace

QString ProFilterEffect::getId() {
    return QStringLiteral("org.mixxx.effects.profilter");
}

EffectManifestPointer ProFilterEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Sweep"),
            QObject::tr("Sweep"),
            QObject::tr("Right: a resonant band-pass sweeps up through the track. "
                        "Left: a transient gate cuts the tails and leaves only "
                        "the hits. Center: off."),
            0.5);

    auto pSweep = djfx::knob(pManifest,
            QStringLiteral("sweep"),
            QObject::tr("Sweep"),
            QObject::tr("Sweep"),
            QObject::tr("Left: gate. Right: band-pass. Center: off."),
            0.0,
            0.5,
            1.0);
    djfx::linkToMetaknob(pSweep, 0.5);
    djfx::knob(pManifest,
            QStringLiteral("resonance"),
            QObject::tr("Resonance"),
            QObject::tr("Res"),
            QObject::tr("Narrowness of the band-pass."),
            0.0,
            0.5,
            1.0);
    return pManifest;
}

ProFilterGroupState::ProFilterGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    fast.setup(0.0005f, 0.02f, m_sampleRate);
    slow.setup(0.02f, 0.25f, m_sampleRate);
    position.setTime(0.012f, m_sampleRate);
    gateGain.setTime(0.003f, m_sampleRate);
}

void ProFilterEffect::processChannel(ProFilterGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        pState->band[0].reset();
        pState->band[1].reset();
        pState->fast.reset();
        pState->slow.reset();
        pState->position.reset(param(kSweep));
        pState->gateGain.reset(1.0f);
    }
    pState->position.setTarget(param(kSweep));
    const float resonance = param(kResonance);
    const float rate = pState->sampleRate();

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        const float pos = pState->position.next();
        const float offset = pos - 0.5f;
        const float amount = fxdsp::clampf(
                (std::fabs(offset) - kDeadZone) / (0.5f - kDeadZone), 0.0f, 1.0f);
        // Envelopes always run so switching sides is seamless.
        const float mono = 0.5f * (std::fabs(inL) + std::fabs(inR));
        const float fast = pState->fast.process(mono);
        const float slow = pState->slow.process(mono);

        if (offset >= 0.0f) {
            // Band-pass: center 80 Hz -> 5 kHz, Q from wide to narrow as
            // the knob turns, scaled by resonance. Blended in from dry so
            // leaving center is smooth.
            const float hz = fxdsp::knobToHz(amount, 80.0f, 5000.0f);
            const float q = 0.5f + amount * (0.8f + 2.5f * resonance);
            float y[2];
            for (int c = 0; c < 2; ++c) {
                pState->band[c].setCoeffs(hz, q, rate);
                // The SVF band output has unity gain at the peak when
                // scaled by k.
                const auto o = pState->band[c].process(pInput[2 * i + c]);
                y[c] = o.bp * pState->band[c].k();
            }
            const float blend = std::min(1.0f, amount * 6.0f);
            pState->gateGain.setTarget(1.0f);
            pState->gateGain.next();
            pOutput[2 * i] = fxdsp::lerp(inL, y[0], blend);
            pOutput[2 * i + 1] = fxdsp::lerp(inR, y[1], blend);
        } else {
            pState->band[0].process(inL);
            pState->band[1].process(inR);
            // Gate: open while the fast envelope is above the slow one
            // (a transient), close on the sustain. Amount sets how far
            // the tail is pushed down and how strict the detector is.
            const float ratio = fast / (slow + 1e-6f);
            const float threshold = 1.0f + 0.4f * amount;
            const float open = fxdsp::clampf((ratio - threshold + 0.25f) * 4.0f, 0.0f, 1.0f);
            const float floorGain = 1.0f - amount;
            pState->gateGain.setTarget(fxdsp::lerp(floorGain, 1.0f, open));
            const float g = pState->gateGain.next();
            pOutput[2 * i] = inL * g;
            pOutput[2 * i + 1] = inR * g;
        }
    }
}
