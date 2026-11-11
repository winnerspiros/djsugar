#include "effects/backends/builtin/proflangereffect.h"

namespace {

constexpr float kBaseMs = 12.0f;
constexpr float kMaxDepthMs = 6.0f;
constexpr float kBassCrossoverHz = 180.0f;
// Each voice runs at a slightly different rate so they never line up.
constexpr float kVoiceRate[ProFlangerGroupState::kVoices] = {1.0f, 1.13f, 0.87f};

} // namespace

QString ProFlangerEffect::getId() {
    return QStringLiteral("org.mixxx.effects.proflanger");
}

EffectManifestPointer ProFlangerEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Chorus"),
            QObject::tr("Chorus"),
            QObject::tr("Lush three-voice ensemble chorus. Bass stays mono and "
                        "dry so the low end stays tight."));

    auto pAmount = djfx::knob(pManifest,
            QStringLiteral("amount"),
            QObject::tr("Amount"),
            QObject::tr("Amount"),
            QObject::tr("Level of the chorus voices."),
            0.0,
            1.0,
            1.0);
    djfx::linkToMetaknob(pAmount);
    djfx::knob(pManifest,
            QStringLiteral("rate"),
            QObject::tr("Rate"),
            QObject::tr("Rate"),
            QObject::tr("Speed of the modulation."),
            0.05,
            0.6,
            6.0,
            djfx::Scaler::Logarithmic,
            djfx::Units::Hertz);
    djfx::knob(pManifest,
            QStringLiteral("depth"),
            QObject::tr("Depth"),
            QObject::tr("Depth"),
            QObject::tr("Amount of pitch wobble."),
            0.0,
            0.5,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("width"),
            QObject::tr("Width"),
            QObject::tr("Width"),
            QObject::tr("Stereo spread of the voices."),
            0.0,
            1.0,
            1.0);
    return pManifest;
}

ProFlangerGroupState::ProFlangerGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    const int length = static_cast<int>(m_sampleRate * (kBaseMs + kMaxDepthMs) * 0.001f) + 16;
    for (int c = 0; c < 2; ++c) {
        line[c].allocate(length);
        crossover[c].setCutoff(kBassCrossoverHz, m_sampleRate);
    }
    amount.setTime(0.02f, m_sampleRate);
    depth.setTime(0.05f, m_sampleRate);
}

void ProFlangerEffect::processChannel(ProFlangerGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        for (int c = 0; c < 2; ++c) {
            pState->line[c].clear();
            pState->crossover[c].reset();
        }
        pState->amount.reset(param(kAmount));
        pState->depth.reset(param(kDepth));
    }
    pState->amount.setTarget(param(kAmount));
    pState->depth.setTarget(param(kDepth));
    const float rate = pState->sampleRate();
    const float lfoHz = param(kRate);
    const float width = param(kWidth);
    const float msToSamples = rate * 0.001f;
    // Voice pan positions, scaled by width: left, center, right.
    const float pan[ProFlangerGroupState::kVoices] = {-width, 0.0f, width};

    for (int i = 0; i < frames; ++i) {
        const float amount = pState->amount.next();
        const float depth = pState->depth.next() * kMaxDepthMs * msToSamples;
        pState->jitterPhase += 0.071f / rate;
        pState->jitterPhase -= std::floor(pState->jitterPhase);
        const float jitter = 1.0f + 0.15f * std::sin(fxdsp::kTwoPi * pState->jitterPhase);

        float bass[2];
        float high[2];
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            bass[c] = pState->crossover[c].lowpass(x);
            high[c] = x - bass[c];
            pState->line[c].write(high[c]);
        }
        float wet[2] = {0.0f, 0.0f};
        for (int v = 0; v < ProFlangerGroupState::kVoices; ++v) {
            pState->phase[v] += lfoHz * kVoiceRate[v] * jitter / rate;
            pState->phase[v] -= std::floor(pState->phase[v]);
            const float mod = fxdsp::lfo(fxdsp::LfoShape::Sine, pState->phase[v]);
            const float d = kBaseMs * msToSamples + depth * mod;
            // Each voice reads both channels, panned by its position.
            const float l = pState->line[0].readHermite(d);
            const float r = pState->line[1].readHermite(d + 0.37f * msToSamples);
            const float voice = 0.5f * (l + r);
            const float gl = std::sqrt(0.5f * (1.0f - pan[v]));
            const float gr = std::sqrt(0.5f * (1.0f + pan[v]));
            wet[0] += voice * gl;
            wet[1] += voice * gr;
        }
        for (int c = 0; c < 2; ++c) {
            const float voices = wet[c] * 0.55f;
            // Dry highs plus the chorus voices; bass passes untouched.
            pOutput[2 * i + c] = bass[c] + high[c] * (1.0f - 0.3f * amount) + voices * amount;
        }
    }
}
