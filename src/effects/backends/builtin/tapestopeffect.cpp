#include "effects/backends/builtin/tapestopeffect.h"

namespace {

// The read head never gets ahead of real time, so a brake needs at most
// its own length of audio. Longer brakes are clamped to this.
constexpr float kMaxSeconds = 8.0f;

} // namespace

QString TapeStopEffect::getId() {
    return QStringLiteral("org.mixxx.effects.tapestop");
}

EffectManifestPointer TapeStopEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Brake"),
            QObject::tr("Brake"),
            QObject::tr("Winds the deck down to a stop like a turntable with the "
                        "motor off or a tape losing power, synced to the beat."));

    djfx::division(pManifest,
            QStringLiteral("time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("How long the stop takes, in beats."),
            2.0);
    auto pMix = djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Brake level against the live track."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pMix);
    djfx::knob(pManifest,
            QStringLiteral("curve"),
            QObject::tr("Curve"),
            QObject::tr("Curve"),
            QObject::tr("Left: tape, slows evenly. Right: turntable, holds speed "
                        "then drops off."),
            0.0,
            0.6,
            1.0);
    djfx::toggle(pManifest,
            QStringLiteral("quantize"),
            QObject::tr("Quantize"),
            QObject::tr("Quant"),
            QObject::tr("Starts the brake on the next beat."),
            true);
    return pManifest;
}

TapeStopGroupState::TapeStopGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    capacityFrames = static_cast<int>(m_sampleRate * kMaxSeconds);
    history.assign(2 * static_cast<std::size_t>(capacityFrames), 0.0f);
}

void TapeStopGroupState::restart() {
    written = 0;
    readPos = 0.0;
    elapsed = 0.0;
    started = false;
    lastPhase = 0.0f;
}

void TapeStopEffect::processChannel(TapeStopGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->restart();
        pState->started = !paramBool(kQuantize);
    }
    const double stopFrames = std::clamp(paramBeats(kTime) * pState->clock.samplesPerBeat(),
            256.0,
            static_cast<double>(pState->capacityFrames));
    const float mix = param(kMix);
    const float curve = param(kCurve);
    const float* history = pState->history.data();

    for (int i = 0; i < frames; ++i) {
        const float x[2] = {pInput[2 * i], pInput[2 * i + 1]};
        const float phase = pState->clock.tick(1.0);
        if (!pState->started) {
            if (phase < pState->lastPhase) {
                pState->started = true;
            }
            pState->lastPhase = phase;
            pOutput[2 * i] = x[0];
            pOutput[2 * i + 1] = x[1];
            continue;
        }
        if (pState->written < pState->capacityFrames) {
            const std::size_t at = 2 * static_cast<std::size_t>(pState->written);
            pState->history[at] = x[0];
            pState->history[at + 1] = x[1];
            ++pState->written;
        }
        const double t = std::min(1.0, pState->elapsed / stopFrames);
        // Speed curve: tape slows linearly; a turntable keeps its inertia
        // and then drops off (1 - t^p with p growing with the curve).
        const double power = 1.0 + 3.0 * curve;
        const double speed = std::max(0.0, 1.0 - std::pow(t, power));
        // Fade the last stretch so the stop lands on silence, not a
        // frozen DC value.
        const float tail = static_cast<float>(std::min(1.0, (1.0 - t) * 12.0));
        const int index = static_cast<int>(pState->readPos);
        const float frac = static_cast<float>(pState->readPos - index);
        const int next = std::min(index + 1, std::max(0, pState->written - 1));
        for (int c = 0; c < 2; ++c) {
            const float a = history[2 * static_cast<std::size_t>(index) + static_cast<std::size_t>(c)];
            const float b = history[2 * static_cast<std::size_t>(next) + static_cast<std::size_t>(c)];
            const float y = fxdsp::lerp(a, b, frac) * tail;
            pOutput[2 * i + c] = fxdsp::lerp(x[c], y, mix);
        }
        pState->readPos = std::min(pState->readPos + speed,
                static_cast<double>(std::max(0, pState->written - 1)));
        pState->elapsed += 1.0;
    }
}
