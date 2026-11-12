#include "effects/backends/builtin/bitcrushereffect.h"

QString BitCrusherEffect::getId() {
    return QStringLiteral("org.mixxx.effects.bitcrusher");
}

EffectManifestPointer BitCrusherEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Crush"),
            QObject::tr("Crush"),
            QObject::tr("Lo-fi crusher: one knob takes the track from vintage "
                        "sampler to broken video game."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Crush"),
            QObject::tr("Crush"),
            QObject::tr("Lowers the sample rate and the bit depth together."),
            0.0,
            0.5,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("bit_depth"),
            QObject::tr("Bits"),
            QObject::tr("Bits"),
            QObject::tr("Extra bit depth reduction on top of Crush."),
            0.0,
            0.3,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("tone"),
            QObject::tr("Tone"),
            QObject::tr("Tone"),
            QObject::tr("Low-pass after the crusher: tames the fizz."),
            0.0,
            0.75,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Blend with the clean track."),
            0.0,
            1.0,
            1.0);
    return pManifest;
}

BitCrusherGroupState::BitCrusherGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters),
          random(0xC0FFEEu) {
    amount.setTime(0.03f, m_sampleRate);
}

void BitCrusherEffect::processChannel(BitCrusherGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        pState->tone[0].reset();
        pState->tone[1].reset();
        pState->hold[0] = pInput[0];
        pState->hold[1] = pInput[1];
        pState->counter = 0.0f;
        pState->amount.reset(param(kAmount));
    }
    pState->amount.setTarget(param(kAmount));
    const float extraBits = param(kBits);
    const float rate = pState->sampleRate();
    const float toneHz = fxdsp::knobToHz(param(kTone), 800.0f, 20000.0f);
    pState->tone[0].setCoeffs(toneHz, 0.6f, rate);
    pState->tone[1].setCoeffs(toneHz, 0.6f, rate);
    const float mix = param(kMix);

    for (int i = 0; i < frames; ++i) {
        const float amount = pState->amount.next();
        // Target sample rate from full rate down to 1.5 kHz, exponential.
        const float targetRate = fxdsp::knobToHz(1.0f - amount, 1500.0f, rate);
        // Bits from 16 down to 4 (3 with extra bits).
        const float bits = 16.0f - 12.0f * amount - 2.0f * extraBits;
        const float steps = std::exp2(bits - 1.0f);
        pState->counter += targetRate / rate;
        const bool sample = pState->counter >= 1.0f;
        if (sample) {
            pState->counter -= std::floor(pState->counter);
        }
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            if (sample) {
                // Triangular dither of one step keeps the noise floor even.
                const float dither =
                        (pState->random.unipolar() - pState->random.unipolar()) / steps;
                pState->hold[c] = std::round((x + dither) * steps) / steps;
            }
            const float y = pState->tone[c].process(pState->hold[c]).lp;
            pOutput[2 * i + c] = fxdsp::lerp(x, y, mix * std::min(1.0f, amount * 8.0f));
        }
    }
}
