#include "effects/backends/builtin/glitcheffect.h"

namespace {

constexpr float kHistorySeconds = 4.0f;
constexpr float kFadeSeconds = 0.003f;

} // namespace

QString GlitchEffect::getId() {
    return QStringLiteral("org.mixxx.effects.glitch");
}

EffectManifestPointer GlitchEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Stutter"),
            QObject::tr("Stutter"),
            QObject::tr("Beat-locked glitch edits: every step may repeat, "
                        "reverse or half-speed the previous one."));

    djfx::division(pManifest,
            QStringLiteral("step"),
            QObject::tr("Step"),
            QObject::tr("Step"),
            QObject::tr("Grid step in beats."),
            0.25);
    auto pChance = djfx::knob(pManifest,
            QStringLiteral("chance"),
            QObject::tr("Chance"),
            QObject::tr("Chance"),
            QObject::tr("How often a step gets glitched."),
            0.0,
            0.5,
            1.0);
    djfx::linkToMetaknob(pChance);
    djfx::knob(pManifest,
            QStringLiteral("reverse"),
            QObject::tr("Reverse"),
            QObject::tr("Rev"),
            QObject::tr("Share of glitches played backwards."),
            0.0,
            0.25,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("half_speed"),
            QObject::tr("Half Speed"),
            QObject::tr("Half"),
            QObject::tr("Share of glitches played an octave down."),
            0.0,
            0.15,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Glitch level against the live track."),
            0.0,
            1.0,
            1.0);
    return pManifest;
}

GlitchGroupState::GlitchGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters),
          random(0x6117C4u) {
    capacityFrames = static_cast<int>(m_sampleRate * kHistorySeconds);
    history.assign(2 * static_cast<std::size_t>(capacityFrames), 0.0f);
    mix.setTime(0.01f, m_sampleRate);
}

void GlitchGroupState::restart() {
    written = 0;
    stepStart = 0;
    previousStepStart = 0;
    current = Voice();
    fading = Voice();
    fadeLeft = 0;
    lastPhase = 0.0f;
}

namespace {

// Reads one channel of a voice. Pass voices read the live input.
float readVoice(const GlitchGroupState& state,
        const GlitchGroupState::Voice& voice,
        int channel,
        float live,
        int stepFrames) {
    double offset;
    switch (voice.action) {
    case GlitchGroupState::Action::Pass:
        return live;
    case GlitchGroupState::Action::Reverse:
        offset = static_cast<double>(stepFrames - 1) - voice.position;
        break;
    case GlitchGroupState::Action::Repeat:
    case GlitchGroupState::Action::HalfSpeed:
    default:
        offset = voice.position;
        break;
    }
    offset = std::clamp(offset, 0.0, static_cast<double>(stepFrames - 1));
    const int64_t frame = voice.start + static_cast<int64_t>(offset);
    // Never read audio that has not been written or was overwritten.
    if (frame >= state.written || frame < state.written - state.capacityFrames + 1) {
        return live;
    }
    const float frac = static_cast<float>(offset - std::floor(offset));
    const int64_t nextFrame = std::min(frame + 1, state.written - 1);
    const auto at = [&](int64_t f) {
        const std::size_t index = static_cast<std::size_t>(f % state.capacityFrames);
        return state.history[2 * index + static_cast<std::size_t>(channel)];
    };
    return fxdsp::lerp(at(frame), at(nextFrame), frac);
}

} // namespace

void GlitchEffect::processChannel(GlitchGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->restart();
        pState->mix.reset(param(kMix));
    }
    pState->mix.setTarget(param(kMix));
    const double beats = paramBeats(kStep);
    const int stepFrames = std::clamp(
            static_cast<int>(beats * pState->clock.samplesPerBeat()),
            64,
            pState->capacityFrames / 2);
    const float chance = param(kChance);
    const float reverse = param(kReverse);
    const float halfSpeed = param(kHalfSpeed);
    const int fadeFrames = std::max(1, static_cast<int>(kFadeSeconds * pState->sampleRate()));

    for (int i = 0; i < frames; ++i) {
        const float x[2] = {pInput[2 * i], pInput[2 * i + 1]};
        const std::size_t slot = static_cast<std::size_t>(pState->written % pState->capacityFrames);
        pState->history[2 * slot] = x[0];
        pState->history[2 * slot + 1] = x[1];
        ++pState->written;

        const float gridPhase = pState->clock.tick(beats);
        if (gridPhase < pState->lastPhase) {
            // New step: pick what it does.
            pState->previousStepStart = pState->stepStart;
            pState->stepStart = pState->written - 1;
            GlitchGroupState::Voice next;
            const bool havePrevious = pState->stepStart - pState->previousStepStart >= stepFrames / 2;
            if (havePrevious && pState->random.unipolar() < chance) {
                const float pick = pState->random.unipolar();
                if (pick < reverse) {
                    next.action = GlitchGroupState::Action::Reverse;
                } else if (pick < reverse + halfSpeed) {
                    next.action = GlitchGroupState::Action::HalfSpeed;
                } else {
                    next.action = GlitchGroupState::Action::Repeat;
                }
                // Back-to-back glitches stay on the same source half of the
                // time (a stutter run), otherwise grab the step just played
                // by the track, so high chance never freezes into a loop.
                const bool stay = pState->current.action != GlitchGroupState::Action::Pass &&
                        pState->random.unipolar() < 0.5f;
                next.start = stay ? pState->current.start : pState->previousStepStart;
            }
            if (next.action != GlitchGroupState::Action::Pass ||
                    pState->current.action != GlitchGroupState::Action::Pass) {
                pState->fading = pState->current;
                pState->fadeLeft = fadeFrames;
            }
            pState->current = next;
        }
        pState->lastPhase = gridPhase;

        const float mix = pState->mix.next();
        for (int c = 0; c < 2; ++c) {
            float y = readVoice(*pState, pState->current, c, x[c], stepFrames);
            if (pState->fadeLeft > 0) {
                const float t = static_cast<float>(pState->fadeLeft) / static_cast<float>(fadeFrames);
                float a;
                float b;
                fxdsp::equalPower(t, &b, &a); // t = 1: all old voice
                const float old = readVoice(*pState, pState->fading, c, x[c], stepFrames);
                y = old * a + y * b;
            }
            pOutput[2 * i + c] = fxdsp::lerp(x[c], y, mix);
        }
        const double speed = pState->current.action == GlitchGroupState::Action::HalfSpeed ? 0.5 : 1.0;
        // Grid steps are fractional, so a voice may run a frame past its
        // source; readVoice() holds the last frame until the next step's
        // crossfade instead of wrapping (which would click).
        pState->current.position += speed;
        if (pState->fadeLeft > 0) {
            pState->fading.position += pState->fading.action == GlitchGroupState::Action::HalfSpeed
                    ? 0.5
                    : 1.0;
            --pState->fadeLeft;
        }
    }
}
