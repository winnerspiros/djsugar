#include "effects/backends/builtin/phasereffect.h"

namespace {

// Relative spread of the stage frequencies, like component tolerance in
// an analog phaser.
constexpr float kStageSpread[PhaserGroupState::kMaxStages] = {
        1.0f, 1.11f, 0.93f, 1.21f, 0.87f, 1.05f, 1.33f, 0.97f, 1.17f, 0.9f, 1.26f, 1.02f};

} // namespace

QString PhaserEffect::getId() {
    return QStringLiteral("org.mixxx.effects.phaser");
}

EffectManifestPointer PhaserEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Phaser"),
            QObject::tr("Phaser"),
            QObject::tr("Beat-synced analog style phaser with 4 to 12 stages, "
                        "resonance and stereo spread."));

    djfx::division(pManifest,
            QStringLiteral("period"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Length of one sweep in beats."),
            8.0);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Depth of the phasing."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("feedback"),
            QObject::tr("Resonance"),
            QObject::tr("Res"),
            QObject::tr("Sharpens the notches into whistling peaks."),
            0.0,
            0.5,
            1.0);
    djfx::choice(pManifest,
            QStringLiteral("stages"),
            QObject::tr("Stages"),
            QObject::tr("Stages"),
            QObject::tr("More stages give more notches and a denser sweep."),
            {QStringLiteral("4"), QStringLiteral("6"), QStringLiteral("8"), QStringLiteral("12")},
            2);
    djfx::knob(pManifest,
            QStringLiteral("range"),
            QObject::tr("Range"),
            QObject::tr("Range"),
            QObject::tr("Width of the sweep."),
            0.0,
            0.8,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("stereo"),
            QObject::tr("Stereo"),
            QObject::tr("Stereo"),
            QObject::tr("Offsets the right channel sweep for width."),
            0.0,
            0.25,
            0.5);
    return pManifest;
}

PhaserGroupState::PhaserGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.05f, m_sampleRate);
}

void PhaserEffect::processChannel(PhaserGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        for (auto& channel : pState->stages) {
            for (auto& stage : channel) {
                stage.reset();
            }
        }
        pState->last[0] = 0.0f;
        pState->last[1] = 0.0f;
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));
    const double period = paramBeats(kRate);
    constexpr int kStageCounts[] = {4, 6, 8, 12};
    const int stageCount = kStageCounts[std::clamp(paramInt(kStages), 0, 3)];
    const float range = param(kRange);
    const float stereo = param(kStereo);
    const float rate = pState->sampleRate();
    // Sweep between 200 Hz and up to 200 Hz * 2^(6 * range).
    const float lowHz = 200.0f;
    const float octaves = 1.0f + 6.0f * range;

    for (int i = 0; i < frames; ++i) {
        const float phase = pState->clock.tick(period);
        const float amount = pState->amount.next();
        const float feedback = 0.85f * pState->feedback.next();
        for (int c = 0; c < 2; ++c) {
            float p = phase + (c == 1 ? stereo : 0.0f);
            p -= std::floor(p);
            const float hz = lowHz * std::exp2(octaves * fxdsp::lfo(fxdsp::LfoShape::Sine, p));
            const float x = pInput[2 * i + c];
            float y = x + feedback * pState->last[c];
            for (int s = 0; s < stageCount; ++s) {
                // Cutoff updates are cheap for a one-pole; doing it every
                // sample keeps fast sweeps smooth.
                pState->stages[c][s].setCutoff(
                        std::min(hz * kStageSpread[s], 0.45f * rate), rate);
                y = pState->stages[c][s].allpass(y);
            }
            pState->last[c] = fxdsp::sanitize(fxdsp::softClip(y));
            // Equal dry and wet gives the deepest notches.
            const float out = 0.5f * (x + y) * (1.0f - 0.35f * feedback);
            pOutput[2 * i + c] = fxdsp::lerp(x, out, amount);
        }
    }
}
