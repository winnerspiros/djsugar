#include "effects/backends/builtin/distortioneffect.h"

namespace {

enum Mode {
    kTube,
    kTape,
    kClip,
    kFold,
};

} // namespace

QString DistortionEffect::getId() {
    return QStringLiteral("org.mixxx.effects.distortion");
}

EffectManifestPointer DistortionEffect::getManifest() {
    auto pManifest = djfx::manifest(getId(),
            QObject::tr("Saturator"),
            QObject::tr("Saturate"),
            QObject::tr("Anti-aliased saturation: warm tube, tape, hard clip or "
                        "wavefolder. Level stays matched while the drive adds "
                        "grit."));

    djfx::choice(pManifest,
            QStringLiteral("mode"),
            QObject::tr("Mode"),
            QObject::tr("Mode"),
            QObject::tr("Tube is warm and asymmetric, Tape is soft and "
                        "compressing, Clip is hard and loud, Fold is metallic."),
            {QObject::tr("Tube"), QObject::tr("Tape"), QObject::tr("Clip"), QObject::tr("Fold")});
    auto pDrive = djfx::knob(pManifest,
            QStringLiteral("drive"),
            QObject::tr("Drive"),
            QObject::tr("Drive"),
            QObject::tr("Amount of saturation."),
            0.0,
            0.0,
            1.0);
    djfx::linkToMetaknob(pDrive);
    djfx::knob(pManifest,
            QStringLiteral("tone"),
            QObject::tr("Tone"),
            QObject::tr("Tone"),
            QObject::tr("Brightness after the saturator."),
            0.0,
            0.7,
            1.0);
    djfx::knob(pManifest,
            QStringLiteral("mix"),
            QObject::tr("Mix"),
            QObject::tr("Mix"),
            QObject::tr("Blend with the clean track, for parallel saturation."),
            0.0,
            1.0,
            1.0);
    return pManifest;
}

DistortionGroupState::DistortionGroupState(const mixxx::EngineParameters& engineParameters)
        : DjFxState(engineParameters) {
    for (int c = 0; c < 2; ++c) {
        dc[c].setup(m_sampleRate);
        preEmphasis[c].setCutoff(700.0f, m_sampleRate);
    }
    drive.setTime(0.03f, m_sampleRate);
    mix.setTime(0.03f, m_sampleRate);
    powerCoeff = fxdsp::onePoleCoeff(0.3f, m_sampleRate);
    makeup.setTime(0.05f, m_sampleRate);
}

void DistortionEffect::processChannel(DistortionGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        [[maybe_unused]] const GroupFeatureState& groupFeatures) {
    const int frames = static_cast<int>(engineParameters.framesPerBuffer());
    if (enableState == EffectEnableState::Enabling) {
        for (int c = 0; c < 2; ++c) {
            pState->tanh[c].reset();
            pState->clip[c].reset();
            pState->fold[c].reset();
            pState->dc[c].reset();
            pState->tone[c].reset();
            pState->preEmphasis[c].reset();
        }
        pState->drive.reset(param(kDrive));
        pState->mix.reset(param(kMix));
        pState->inputPower = 0.0f;
        pState->outputPower = 0.0f;
        pState->makeup.reset(1.0f);
    }
    pState->drive.setTarget(param(kDrive));
    pState->mix.setTarget(param(kMix));
    const int mode = std::clamp(paramInt(kMode), 0, 3);
    const float toneHz = fxdsp::knobToHz(param(kTone), 1500.0f, 20000.0f);
    for (auto& f : pState->tone) {
        f.setCoeffs(toneHz, 0.6f, pState->sampleRate());
    }

    for (int i = 0; i < frames; ++i) {
        const float drive = pState->drive.next();
        const float mix = pState->mix.next() * std::min(1.0f, drive * 20.0f);
        // 0 dB to +36 dB of drive, exponential feel.
        const float gain = fxdsp::dbToGain(36.0f * drive);
        const float makeup = pState->makeup.next();
        float shaped[2];
        for (int c = 0; c < 2; ++c) {
            const float x = pInput[2 * i + c];
            float y;
            switch (mode) {
            case kTape: {
                // Tape: saturation is mostly in the mids and highs; bass
                // passes a bit cleaner.
                const float low = pState->preEmphasis[c].lowpass(x);
                y = low * 0.6f + pState->tanh[c].process((x - 0.6f * low) * gain);
                break;
            }
            case kClip:
                y = pState->clip[c].process(x * gain);
                break;
            case kFold:
                y = pState->fold[c].process(x * gain * 1.57f);
                break;
            case kTube:
            default:
                // A DC bias before tanh makes even harmonics; the DC
                // blocker removes the offset afterwards.
                y = pState->tanh[c].process(x * gain + 0.2f * drive) - 0.2f * drive;
                break;
            }
            y = pState->dc[c].process(y);
            shaped[c] = pState->tone[c].process(y).lp;
        }
        // Level match: the saturated signal follows the loudness of the
        // input, so the knob changes the character, not the volume.
        const float k = pState->powerCoeff;
        const float xl = pInput[2 * i];
        const float xr = pInput[2 * i + 1];
        pState->inputPower = fxdsp::flush(
                k * pState->inputPower + (1.0f - k) * 0.5f * (xl * xl + xr * xr));
        pState->outputPower = fxdsp::flush(k * pState->outputPower +
                (1.0f - k) * 0.5f * (shaped[0] * shaped[0] + shaped[1] * shaped[1]));
        if (pState->outputPower > 1e-8f) {
            pState->makeup.setTarget(fxdsp::clampf(
                    std::sqrt(pState->inputPower / pState->outputPower), 0.05f, 2.0f));
        }
        for (int c = 0; c < 2; ++c) {
            pOutput[2 * i + c] = fxdsp::lerp(pInput[2 * i + c], shaped[c] * makeup, mix);
        }
    }
}
