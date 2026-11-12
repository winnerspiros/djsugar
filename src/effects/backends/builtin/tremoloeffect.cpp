#include "effects/backends/builtin/tremoloeffect.h"

namespace {

enum Shape {
    kGate,
    kSine,
    kPump,
    kRampUp,
    kRampDown,
};

} // namespace

QString TremoloEffect::getId() {
    return QStringLiteral("org.mixxx.effects.tremolo");
}

EffectManifestPointer TremoloEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Trans"),
            QObject::tr("Trans"),
            QObject::tr("Beat-locked volume chopping: hard gate, smooth tremolo, "
                        "sidechain pump or ramps. Click-free edges."));

    djfx::division(pManifest,
            QStringLiteral("rate"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Length of one cycle in beats."),
            0.25);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("depth"),
            QObject::tr("Depth"),
            QObject::tr("Depth"),
            QObject::tr("How far the volume drops."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::choice(pManifest,
            QStringLiteral("shape"),
            QObject::tr("Shape"),
            QObject::tr("Shape"),
            QObject::tr("Gate chops, Sine pulses, Pump ducks like a sidechain "
                        "compressor, ramps fade in or out each cycle."),
            {QObject::tr("Gate"),
                    QObject::tr("Sine"),
                    QObject::tr("Pump"),
                    QObject::tr("Ramp Up"),
                    QObject::tr("Ramp Down")});
    djfx::knob(pManifest,
            QStringLiteral("duty"),
            QObject::tr("Width"),
            QObject::tr("Width"),
            QObject::tr("Share of each cycle that stays open (Gate) or how long "
                        "the duck lasts (Pump)."),
            0.1,
            0.5,
            0.9);
    djfx::knob(pManifest,
            QStringLiteral("phase"),
            QObject::tr("Offset"),
            QObject::tr("Offset"),
            QObject::tr("Shifts the pattern against the beat grid."),
            0.0,
            0.0,
            1.0);
    return pManifest;
}

TremoloGroupState::TremoloGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    amount.setTime(0.02f, m_sampleRate);
    // 1.5 ms: fast enough to stay punchy, slow enough to never click.
    gain.setTime(0.0015f, m_sampleRate);
}

void TremoloEffect::processChannel(TremoloGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->amount.reset(param(kAmount));
        pState->gain.reset(1.0f);
    }
    pState->amount.setTarget(param(kAmount));
    const double period = paramBeats(kRate);
    const int shape = paramInt(kShape);
    const float duty = param(kDuty);
    const float offset = param(kOffset);

    for (int i = 0; i < frames; ++i) {
        float phase = pState->clock.tick(period) + offset;
        phase -= std::floor(phase);
        float open;
        switch (shape) {
        case kSine:
            open = 1.0f - fxdsp::lfo(fxdsp::LfoShape::Sine, phase);
            break;
        case kPump: {
            // Ducks at the start of each cycle and recovers with an
            // exponential-like curve whose length follows Width.
            const float t = fxdsp::clampf(phase / duty, 0.0f, 1.0f);
            open = 1.0f - (1.0f - t) * (1.0f - t);
            break;
        }
        case kRampUp:
            open = phase;
            break;
        case kRampDown:
            open = 1.0f - phase;
            break;
        case kGate:
        default:
            open = phase < duty ? 1.0f : 0.0f;
            break;
        }
        const float amount = pState->amount.next();
        pState->gain.setTarget(1.0f - amount * (1.0f - open));
        const float g = pState->gain.next();
        pOutput[2 * i] = pInput[2 * i] * g;
        pOutput[2 * i + 1] = pInput[2 * i + 1] * g;
    }
}
