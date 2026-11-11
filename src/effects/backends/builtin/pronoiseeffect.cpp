#include "effects/backends/builtin/pronoiseeffect.h"

namespace {

constexpr float kDeadZone = 0.02f;

} // namespace

QString ProNoiseEffect::getId() {
    return QStringLiteral("org.mixxx.effects.pronoise");
}

EffectManifestPointer ProNoiseEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Space"),
            QObject::tr("Space"),
            QObject::tr("One-knob color reverb. Right: a bright hall that thins "
                        "out the low end. Left: a dark room that pushes the track "
                        "into the distance. Center: off, the tail rings out."),
            0.5);

    auto pSpace = djfx::knob(pManifest,
            QStringLiteral("space"),
            QObject::tr("Space"),
            QObject::tr("Space"),
            QObject::tr("Left: dark room. Right: bright hall. Center: off."),
            0.0,
            0.5,
            1.0);
    djfx::linkToMetaknob(pSpace, 0.5);
    djfx::knob(pManifest,
            QStringLiteral("decay"),
            QObject::tr("Decay"),
            QObject::tr("Decay"),
            QObject::tr("Length of the tail."),
            0.0,
            0.5,
            1.0);
    return pManifest;
}

ProNoiseGroupState::ProNoiseGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    reverb.setup(m_sampleRate);
    reverb.setSize(0.8f, true);
    reverb.setModulation(0.6f);
    position.setTime(0.03f, m_sampleRate);
}

void ProNoiseEffect::processChannel(ProNoiseGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        pState->reverb.clear();
        for (int c = 0; c < 2; ++c) {
            pState->sendFilter[c].reset();
            pState->dryFilter[c].reset();
        }
        pState->position.reset(param(kSpace));
    }
    pState->position.setTarget(param(kSpace));
    const float decay = param(kDecay);
    if (decay != pState->lastDecay) {
        // 1.2 s to 9 s RT60.
        pState->reverb.setDecay(1.2f * std::pow(7.5f, decay));
        pState->lastDecay = decay;
    }
    const float rate = pState->sampleRate();

    // Tone settings change per buffer from the smoothed position; the
    // per-sample amount below keeps the motion click-free.
    const float offset = pState->position.value() - 0.5f;
    const float amount = fxdsp::clampf(
            (std::fabs(offset) - kDeadZone) / (0.5f - kDeadZone), 0.0f, 1.0f);
    const bool bright = offset >= 0.0f;
    // Bright: send high-passed 150 Hz -> 1.2 kHz, reverb damping open.
    // Dark: send low-passed 6 kHz -> 900 Hz, heavy damping.
    const float sendHz = bright ? fxdsp::knobToHz(amount, 150.0f, 1200.0f)
                                : fxdsp::knobToHz(1.0f - amount, 900.0f, 6000.0f);
    pState->reverb.setDamping(bright ? 12000.0f : fxdsp::knobToHz(1.0f - amount, 1500.0f, 6000.0f));
    // The dry track gets a gentle version of the same tilt so the effect
    // reads as a move, not only an added wash.
    const float dryHz = bright ? fxdsp::knobToHz(amount, 20.0f, 300.0f)
                               : fxdsp::knobToHz(1.0f - amount, 2500.0f, 20000.0f);
    for (int c = 0; c < 2; ++c) {
        pState->sendFilter[c].setCoeffs(sendHz, 0.707f, rate);
        pState->dryFilter[c].setCoeffs(dryHz, 0.707f, rate);
    }

    for (int i = 0; i < frames; ++i) {
        const float a = fxdsp::clampf(
                (std::fabs(pState->position.next() - 0.5f) - kDeadZone) / (0.5f - kDeadZone),
                0.0f,
                1.0f);
        float send[2];
        float dry[2];
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            const auto s = pState->sendFilter[c].process(x);
            const auto d = pState->dryFilter[c].process(x);
            send[c] = (bright ? s.hp : s.lp) * a;
            dry[c] = fxdsp::lerp(x, bright ? d.hp : d.lp, a);
        }
        float wetL;
        float wetR;
        pState->reverb.process(send[0], send[1], &wetL, &wetR);
        // Wet sits a little under the dry at full turn, like a mixer send.
        pOutput[2 * i] = dry[0] + 0.8f * wetL;
        pOutput[2 * i + 1] = dry[1] + 0.8f * wetR;
    }
}
