#include "effects/backends/builtin/pitchshifteffect.h"

QString PitchShiftEffect::getId() {
    return QStringLiteral("org.mixxx.effects.pitchshift");
}

EffectManifestPointer PitchShiftEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Pitch"),
            QObject::tr("Pitch"),
            QObject::tr("Shifts the deck up or down without changing the tempo. "
                        "Center is no shift; the knob glides smoothly."),
            0.5);

    auto pPitch = djfx::knob(pManifest,
            QStringLiteral("pitch"),
            QObject::tr("Pitch"),
            QObject::tr("Pitch"),
            QObject::tr("Left lowers, right raises the pitch. Center is off."),
            -1.0,
            0.0,
            1.0);
    djfx::linkToMetaknob(pPitch, 0.5);
    djfx::choice(pManifest,
            QStringLiteral("range"),
            QObject::tr("Range"),
            QObject::tr("Range"),
            QObject::tr("Shift at the ends of the knob."),
            {QObject::tr("1 octave"), QObject::tr("5 semitones"), QObject::tr("2 octaves")});
    djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Blend with the original for harmonies."),
            0.0,
            1.0,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("grain"),
            QObject::tr("Grain"),
            QObject::tr("Grain"),
            QObject::tr("Short grains suit drums, long grains suit vocals and pads."),
            0.0,
            0.5,
            1.0);
    return pManifest;
}

PitchShiftGroupState::PitchShiftGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    for (auto& s : shifter) {
        s.setup(m_sampleRate, 0.1f);
    }
    semitones.setTime(0.06f, m_sampleRate);
    mix.setTime(0.02f, m_sampleRate);
}

void PitchShiftEffect::processChannel(PitchShiftGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    constexpr float kRanges[] = {12.0f, 5.0f, 24.0f};
    const float range = kRanges[std::clamp(paramInt(kRange), 0, 2)];
    const float target = param(kPitch) * range;
    // Grain (window) 25..100 ms; the shifters are allocated for the
    // longest one.
    const float window = 0.025f + 0.075f * param(kGrain);
    if (enableState == EffectEnableState::Enabling || window != pState->lastWindow) {
        if (enableState == EffectEnableState::Enabling || pState->lastWindow < 0.0f) {
            pState->semitones.reset(target);
            pState->mix.reset(param(kMix));
        }
        for (auto& shifter : pState->shifter) {
            shifter.setWindow(window, pState->sampleRate());
        }
        pState->lastWindow = window;
    }
    pState->semitones.setTarget(target);
    pState->mix.setTarget(param(kMix));

    for (int i = 0; i < frames; ++i) {
        const float semitones = pState->semitones.next();
        const float ratio = std::exp2(semitones / 12.0f);
        // Fade the shifter in over the first quarter semitone so the dead
        // center is truly dry and leaving it is seamless.
        const float engage = fxdsp::clampf(std::fabs(semitones) * 4.0f, 0.0f, 1.0f);
        const float mix = pState->mix.next() * engage;
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            const float y = pState->shifter[c].process(x, ratio);
            pOutput[2 * i + c] = fxdsp::lerp(x, y, mix);
        }
    }
}
