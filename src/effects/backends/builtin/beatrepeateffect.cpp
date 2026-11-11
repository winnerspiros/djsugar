#include "effects/backends/builtin/beatrepeateffect.h"

namespace {

constexpr float kMaxSeconds = 4.0f;
constexpr float kEdgeSeconds = 0.002f;

} // namespace

QString BeatRepeatEffect::getId() {
    return QStringLiteral("org.mixxx.effects.beatrepeat");
}

EffectManifestPointer BeatRepeatEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Roll"),
            QObject::tr("Roll"),
            QObject::tr("Beat-quantized loop roll. Shrink the division while it "
                        "rolls for a build-up; repeats can fade and drop in "
                        "pitch."));

    djfx::division(pManifest,
            QStringLiteral("division"),
            QObject::tr("Division"),
            QObject::tr("Div"),
            QObject::tr("Length of the rolled slice in beats."),
            0.5);
    auto pMix = djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Roll level against the live track."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pMix);
    djfx::knob(pManifest,
            QStringLiteral("decay"),
            QObject::tr("Decay"),
            QObject::tr("Decay"),
            QObject::tr("Each repeat gets quieter."),
            0.0,
            0.0,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("pitch"),
            QObject::tr("Pitch Drop"),
            QObject::tr("Pitch"),
            QObject::tr("Semitones each repeat drops (or rises when negative)."),
            -2.0,
            0.0,
            2.0);
    djfx::knob(pManifest,
            QStringLiteral("gate"),
            QObject::tr("Gate"),
            QObject::tr("Gate"),
            QObject::tr("Plays only the front part of every slice."),
            0.1,
            1.0,
            1.0);
    return pManifest;
}

BeatRepeatGroupState::BeatRepeatGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    capacityFrames = static_cast<int>(m_sampleRate * kMaxSeconds);
    buffer.assign(2 * static_cast<std::size_t>(capacityFrames), 0.0f);
    mix.setTime(0.01f, m_sampleRate);
}

void BeatRepeatGroupState::restart() {
    phase = Phase::Armed;
    captured = 0;
    readPos = 0.0;
    playFrame = 0;
    repeat = 0;
    lastPhase = 0.0f;
}

void BeatRepeatEffect::processChannel(BeatRepeatGroupState* pState,
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
    const double beats = paramBeats(kDivision);
    int loopFrames = static_cast<int>(beats * pState->clock.samplesPerBeat());
    loopFrames = std::clamp(loopFrames, 64, pState->capacityFrames);
    const float decay = param(kDecay);
    const float pitch = param(kPitch);
    const float gate = param(kGate);
    const int edge = std::max(1, static_cast<int>(kEdgeSeconds * pState->sampleRate()));
    const float* buf = pState->buffer.data();

    for (int i = 0; i < frames; ++i) {
        const float x[2] = {pInput[2 * i], pInput[2 * i + 1]};
        const float gridPhase = pState->clock.tick(beats);
        const bool boundary = gridPhase < pState->lastPhase;
        pState->lastPhase = gridPhase;
        const float mix = pState->mix.next();

        if (pState->phase == BeatRepeatGroupState::Phase::Armed && boundary) {
            pState->phase = BeatRepeatGroupState::Phase::Capturing;
            pState->captured = 0;
        }
        if (pState->phase == BeatRepeatGroupState::Phase::Capturing) {
            const std::size_t at = 2 * static_cast<std::size_t>(pState->captured);
            pState->buffer[at] = x[0];
            pState->buffer[at + 1] = x[1];
            ++pState->captured;
            // The first pass of the slice is the live audio itself, faded
            // out over the last edge so the first repeat starts cleanly.
            const int left = std::min(loopFrames, pState->capacityFrames) - pState->captured;
            const float fade = std::min(1.0f, static_cast<float>(left) / static_cast<float>(edge));
            pOutput[2 * i] = fxdsp::lerp(x[0], x[0] * fade, mix);
            pOutput[2 * i + 1] = fxdsp::lerp(x[1], x[1] * fade, mix);
            if (pState->captured >= loopFrames ||
                    pState->captured >= pState->capacityFrames) {
                pState->phase = BeatRepeatGroupState::Phase::Playing;
                pState->loopFrames = pState->captured;
                pState->playFrame = 0;
                pState->readPos = 0.0;
                pState->repeat = 1;
            }
            continue;
        }
        if (pState->phase != BeatRepeatGroupState::Phase::Playing) {
            pOutput[2 * i] = x[0];
            pOutput[2 * i + 1] = x[1];
            continue;
        }

        // A shorter division while rolling shrinks the loop in place.
        if (loopFrames < pState->loopFrames) {
            pState->loopFrames = loopFrames;
            if (pState->playFrame >= loopFrames) {
                pState->playFrame = 0;
                pState->readPos = 0.0;
                ++pState->repeat;
            }
        }
        const int slice = pState->loopFrames;
        const int open = std::max(2 * edge, static_cast<int>(gate * static_cast<float>(slice)));
        const int t = pState->playFrame;
        float env = 0.0f;
        if (t < open) {
            env = std::min({1.0f,
                    static_cast<float>(t + 1) / static_cast<float>(edge),
                    static_cast<float>(open - t) / static_cast<float>(edge)});
        }
        const float level = std::pow(1.0f - 0.6f * decay, static_cast<float>(pState->repeat - 1));
        const double rate = std::exp2(-pitch * static_cast<float>(pState->repeat - 1) / 12.0f);
        const int index = static_cast<int>(pState->readPos);
        const float frac = static_cast<float>(pState->readPos - index);
        const int next = std::min(index + 1, pState->captured - 1);
        for (int c = 0; c < 2; ++c) {
            const float a = buf[2 * static_cast<std::size_t>(index) + static_cast<std::size_t>(c)];
            const float b = buf[2 * static_cast<std::size_t>(next) + static_cast<std::size_t>(c)];
            const float y = fxdsp::lerp(a, b, frac) * env * level;
            pOutput[2 * i + c] = fxdsp::lerp(x[c], y, mix);
        }
        pState->readPos = std::min(pState->readPos + rate,
                static_cast<double>(pState->captured - 1));
        if (++pState->playFrame >= slice) {
            pState->playFrame = 0;
            pState->readPos = 0.0;
            ++pState->repeat;
        }
    }
}
