#include "effects/backends/builtin/flangereffect.h"

namespace {

// Center of the sweep: the dry path is delayed by this much, so the wet
// delay sweeping 0..2x passes through zero relative delay.
constexpr float kCenterMs = 4.0f;

} // namespace

QString FlangerEffect::getId() {
    return QStringLiteral("org.mixxx.effects.flanger");
}

EffectManifestPointer FlangerEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Flanger"),
            QObject::tr("Flanger"),
            QObject::tr("Beat-synced through-zero flanger with the deep jet sweep "
                        "of tape flanging. Feedback left of center sounds "
                        "hollow, right of center metallic."));

    djfx::division(pManifest,
            QStringLiteral("period"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Length of one sweep in beats."),
            16.0);
    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Depth of the flanging."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("feedback"),
            QObject::tr("Feedback"),
            QObject::tr("Feedback"),
            QObject::tr("Left: hollow. Right: metallic. Center: plain sweep."),
            -1.0,
            0.6,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("depth"),
            QObject::tr("Sweep"),
            QObject::tr("Sweep"),
            QObject::tr("Range of the sweep."),
            0.0,
            1.0,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("manual"),
            QObject::tr("Manual"),
            QObject::tr("Manual"),
            QObject::tr("Shifts the sweep. With Sweep at zero, plays the flanger "
                        "by hand."),
            0.0,
            0.5,
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

FlangerGroupState::FlangerGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    const int length = static_cast<int>(m_sampleRate * kCenterMs * 0.001f * 2.5f) + 8;
    for (int c = 0; c < 2; ++c) {
        wet[c].allocate(length);
        dry[c].allocate(length);
        dc[c].setup(m_sampleRate);
    }
    amount.setTime(0.02f, m_sampleRate);
    feedback.setTime(0.05f, m_sampleRate);
    depth.setTime(0.05f, m_sampleRate);
    manual.setTime(0.03f, m_sampleRate);
}

void FlangerEffect::processChannel(FlangerGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    pState->updateClock(groupFeatures, frames);
    if (enableState == EffectEnableState::Enabling) {
        for (int c = 0; c < 2; ++c) {
            pState->wet[c].clear();
            pState->dry[c].clear();
            pState->dc[c].reset();
            pState->last[c] = 0.0f;
        }
        pState->amount.reset(param(kAmount));
        pState->feedback.reset(param(kFeedback));
        pState->depth.reset(param(kDepth));
        pState->manual.reset(param(kManual));
    }
    pState->amount.setTarget(param(kAmount));
    pState->feedback.setTarget(param(kFeedback));
    pState->depth.setTarget(param(kDepth));
    pState->manual.setTarget(param(kManual));
    const double period = paramBeats(kRate);
    const float stereo = param(kStereo);
    const float center = kCenterMs * 0.001f * pState->sampleRate();

    for (int i = 0; i < frames; ++i) {
        const float phase = pState->clock.tick(period);
        const float amount = pState->amount.next();
        const float feedback = 0.9f * pState->feedback.next();
        const float depth = pState->depth.next();
        const float manual = pState->manual.next();
        for (int c = 0; c < 2; ++c) {
            float p = phase + (c == 1 ? stereo : 0.0f);
            p -= std::floor(p);
            // Sweep position in [0, 1]; triangle sounds most even for
            // flanging. Manual offsets it, depth scales it.
            const float sweep = fxdsp::clampf(
                    manual + depth * (fxdsp::lfo(fxdsp::LfoShape::Triangle, p) - 0.5f),
                    0.0f,
                    1.0f);
            // Squared so equal knob travel gives equal pitch of the comb.
            const float d = 1.0f + 2.0f * center * sweep * sweep;
            const float x = pInput[2 * i + c];
            pState->dry[c].write(x);
            const float delayedDry = pState->dry[c].tap(static_cast<int>(center));
            pState->wet[c].write(fxdsp::flush(
                    fxdsp::softClip(x + feedback * pState->last[c])));
            const float w = pState->dc[c].process(pState->wet[c].readHermite(d));
            pState->last[c] = fxdsp::sanitize(w);
            // Feedback raises the level, scale back to match the dry.
            const float y = 0.5f * (delayedDry + w) * (1.0f - 0.35f * std::fabs(feedback));
            pOutput[2 * i + c] = fxdsp::lerp(x, y * 1.35f, amount);
        }
    }
}
