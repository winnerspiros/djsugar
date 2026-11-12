#include "effects/backends/builtin/reverbeffect.h"

QString ReverbEffect::getId() {
    return QStringLiteral("org.mixxx.effects.reverb");
}

EffectManifestPointer ReverbEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Reverb"),
            QObject::tr("Reverb"),
            QObject::tr("Dense, smooth reverb from small room to endless hall, "
                        "with pre-delay, tone controls and freeze. Low cut keeps "
                        "the kick and bass dry."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("send_amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the reverb."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("size"),
            QObject::tr("Size"),
            QObject::tr("Size"),
            QObject::tr("From small room to large hall."),
            0.0,
            0.6,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("decay"),
            QObject::tr("Decay"),
            QObject::tr("Decay"),
            QObject::tr("Reverb time (RT60)."),
            0.3,
            3.0,
            20.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Seconds);
    djfx::knob(pManifest,
            QStringLiteral("damping"),
            QObject::tr("Damping"),
            QObject::tr("Damp"),
            QObject::tr("How quickly the high frequencies die away. More is "
                        "darker."),
            0.0,
            0.4,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("low_cut"),
            QObject::tr("Low Cut"),
            QObject::tr("LowCut"),
            QObject::tr("Removes low end from the reverb so the kick stays "
                        "tight."),
            20.0,
            200.0,
            1000.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Hertz);
    djfx::knob(pManifest,
            QStringLiteral("predelay"),
            QObject::tr("Pre-Delay"),
            QObject::tr("PreDly"),
            QObject::tr("Gap between the sound and the reverb, keeps the attack "
                        "clear."),
            0.0,
            0.02,
            0.2,
            djfx::Scaler::Linear,
            djfx::Units::Seconds);
    djfx::toggle(pManifest,
            QStringLiteral("freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Freeze"),
            QObject::tr("Holds the current reverb forever and closes the input."));
    return pManifest;
}

ReverbGroupState::ReverbGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    reverb.setup(m_sampleRate);
    reverb.setModulation(0.9f);
    amount.setTime(0.02f, m_sampleRate);
}

void ReverbEffect::processChannel(ReverbGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    auto& reverb = pState->reverb;
    if (enableState == EffectEnableState::Enabling) {
        reverb.clear();
        pState->amount.reset(param(kAmount));
        pState->lastSize = -1.0f;
    }
    const float size = param(kSize);
    if (size != pState->lastSize) {
        reverb.setSize(size, pState->lastSize < 0.0f);
        pState->lastSize = size;
        pState->lastDecay = -1.0f; // gains depend on the line lengths
    }
    const float decay = param(kDecay);
    if (decay != pState->lastDecay) {
        reverb.setDecay(decay);
        pState->lastDecay = decay;
    }
    const float damping = param(kDamping);
    if (damping != pState->lastDamping) {
        reverb.setDamping(fxdsp::knobToHz(1.0f - damping, 1500.0f, 18000.0f));
        pState->lastDamping = damping;
    }
    const float lowCut = param(kLowCut);
    if (lowCut != pState->lastLowCut) {
        reverb.setLowCut(lowCut);
        pState->lastLowCut = lowCut;
    }
    const float preDelay = param(kPreDelay);
    if (preDelay != pState->lastPreDelay) {
        reverb.setPreDelay(preDelay);
        pState->lastPreDelay = preDelay;
    }
    reverb.setFreeze(paramBool(kFreeze));
    pState->amount.setTarget(param(kAmount));

    for (int i = 0; i < frames; ++i) {
        const float inL = pInput[2 * i];
        const float inR = pInput[2 * i + 1];
        float wetL;
        float wetR;
        reverb.process(inL, inR, &wetL, &wetR);
        const float gain = pState->amount.next();
        pOutput[2 * i] = inL + wetL * gain;
        pOutput[2 * i + 1] = inR + wetR * gain;
    }
}
