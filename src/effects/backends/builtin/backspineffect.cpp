#include "effects/backends/builtin/backspineffect.h"

namespace {

// Audio kept for the spin; it never reads more than it recorded
constexpr float kMaxSeconds = 8.0f;
// Top speed of a hand pulling the record back
constexpr double kMaxRate = 4.0;
// Without quantize, this much of a beat is grabbed before the spin
constexpr double kGrabBeats = 0.25;

} // namespace

QString BackSpinEffect::getId() {
    return QStringLiteral("org.mixxx.effects.backspin");
}

EffectManifestPointer BackSpinEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Back Spin"),
            QObject::tr("Spin"),
            QObject::tr("Pulls the record back by hand: fast at first, slowing "
                        "down while the track fades out, synced to the beat."));

    djfx::division(pManifest,
            QStringLiteral("time"),
            QObject::tr("Time"),
            QObject::tr("Time"),
            QObject::tr("How long the spin takes, in beats."),
            1.0);
    auto pMix = djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Spin level against the live track."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pMix);
    djfx::toggle(pManifest,
            QStringLiteral("quantize"),
            QObject::tr("Quantize"),
            QObject::tr("Quant"),
            QObject::tr("Starts the spin on the next beat."),
            true);
    return pManifest;
}

BackSpinGroupState::BackSpinGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    capacityFrames = static_cast<int>(m_sampleRate * kMaxSeconds);
    history.assign(2 * static_cast<std::size_t>(capacityFrames), 0.0f);
}

void BackSpinGroupState::restart() {
    written = 0;
    readPos = 0.0;
    elapsed = 0.0;
    peakRate = 0.0;
    started = false;
    lastPhase = 0.0f;
}

void BackSpinEffect::processChannel(BackSpinGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        pState->restart();
    }
    const double samplesPerBeat = pState->clock.samplesPerBeat();
    const double spinFrames = std::clamp(paramBeats(kTime) * samplesPerBeat,
            256.0,
            static_cast<double>(pState->capacityFrames));
    const float mix = param(kMix);
    const bool quantize = paramBool(kQuantize);
    const float* history = pState->history.data();

    for (int i = 0; i < frames; ++i) {
        const float x[2] = {pInput[2 * i], pInput[2 * i + 1]};
        const float phase = pState->clock.tick(1.0);
        if (!pState->started) {
            // The track plays on while it is recorded for the spin
            if (pState->written < pState->capacityFrames) {
                const std::size_t at = 2 * static_cast<std::size_t>(pState->written);
                pState->history[at] = x[0];
                pState->history[at + 1] = x[1];
                ++pState->written;
            }
            const bool onBeat = phase < pState->lastPhase && pState->written > 1;
            const bool grabbed = pState->written >= kGrabBeats * samplesPerBeat;
            pState->lastPhase = phase;
            pOutput[2 * i] = x[0];
            pOutput[2 * i + 1] = x[1];
            if (quantize ? onBeat : grabbed) {
                pState->started = true;
                pState->readPos = pState->written - 1;
                // The speed falls as (1 - t)^2, so the spin travels a third
                // of peak * length: never further back than recorded
                pState->peakRate = std::min(kMaxRate, 3.0 * pState->readPos / spinFrames);
            }
            continue;
        }
        const double t = std::min(1.0, pState->elapsed / spinFrames);
        const double rate = pState->peakRate * (1.0 - t) * (1.0 - t);
        const float level = static_cast<float>(std::min(1.0, 4.0 * (1.0 - t)));
        const int index = static_cast<int>(pState->readPos);
        const float frac = static_cast<float>(pState->readPos - index);
        const int next = std::min(index + 1, pState->written - 1);
        for (int c = 0; c < 2; ++c) {
            const float a = history[2 * static_cast<std::size_t>(index) +
                    static_cast<std::size_t>(c)];
            const float b = history[2 * static_cast<std::size_t>(next) +
                    static_cast<std::size_t>(c)];
            const float y = fxdsp::lerp(a, b, frac) * level;
            pOutput[2 * i + c] = fxdsp::lerp(x[c], y, mix);
        }
        pState->readPos = std::max(0.0, pState->readPos - rate);
        pState->elapsed += 1.0;
    }
}
