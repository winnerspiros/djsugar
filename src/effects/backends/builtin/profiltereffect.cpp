#include "effects/backends/builtin/profiltereffect.h"

#include <algorithm>
#include <cmath>

#include "effects/backends/effectmanifest.h"

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

namespace {
// Filter modes
enum FilterMode {
    LPF = 0,  // Low-pass
    HPF = 1,  // High-pass
    BPF = 2,  // Band-pass
    NOTCH = 3, // Notch
    PEAK = 4,  // Peaking EQ
    NUM_MODES = 5
};

// Smoothing coefficient for parameter interpolation
constexpr double kSmoothCoeff = 0.05;

} // anonymous namespace

void ProFilterEffect::computeBiquadCoefficients(
        double cutoff,
        double resonance,
        double gain,
        double* b0, double* b1, double* b2,
        double* a1, double* a2,
        int mode, double sampleRate) {
    // Clamp cutoff to avoid instability
    double fc = std::clamp(cutoff, 20.0, 20000.0) / sampleRate;
    double Q = std::clamp(resonance, 0.1, 20.0);
    double A = std::pow(10.0, gain / 40.0); // gain in dB

    double w0 = 2.0 * M_PI * fc;
    double cosw0 = std::cos(w0);
    double sinw0 = std::sin(w0);
    double alpha = sinw0 / (2.0 * Q);

    double a0;

    switch (mode) {
    case LPF:
        *b0 = (1.0 - cosw0) / 2.0;
        *b1 = 1.0 - cosw0;
        *b2 = (1.0 - cosw0) / 2.0;
        a0 = 1.0 + alpha;
        *a1 = -2.0 * cosw0;
        *a2 = 1.0 - alpha;
        break;
    case HPF:
        *b0 = (1.0 + cosw0) / 2.0;
        *b1 = -(1.0 + cosw0);
        *b2 = (1.0 + cosw0) / 2.0;
        a0 = 1.0 + alpha;
        *a1 = -2.0 * cosw0;
        *a2 = 1.0 - alpha;
        break;
    case BPF:
        *b0 = alpha;
        *b1 = 0.0;
        *b2 = -alpha;
        a0 = 1.0 + alpha;
        *a1 = -2.0 * cosw0;
        *a2 = 1.0 - alpha;
        break;
    case NOTCH:
        *b0 = 1.0;
        *b1 = -2.0 * cosw0;
        *b2 = 1.0;
        a0 = 1.0 + alpha;
        *a1 = -2.0 * cosw0;
        *a2 = 1.0 - alpha;
        break;
    case PEAK:
        *b0 = 1.0 + alpha * A;
        *b1 = -2.0 * cosw0;
        *b2 = 1.0 - alpha * A;
        a0 = 1.0 + alpha / A;
        *a1 = -2.0 * cosw0;
        *a2 = 1.0 - alpha / A;
        break;
    default:
        // Identity (bypass)
        *b0 = 1.0;
        *b1 = 0.0;
        *b2 = 0.0;
        a0 = 1.0;
        *a1 = 0.0;
        *a2 = 0.0;
        break;
    }

    // Normalize by a0
    *b0 /= a0;
    *b1 /= a0;
    *b2 /= a0;
    *a1 /= a0;
    *a2 /= a0;
}

QString ProFilterEffect::getId() {
    return QStringLiteral("org.mixxx.effects.profilter");
}

EffectManifestPointer ProFilterEffect::getManifest() {
    auto pManifest = EffectManifestPointer::create();
    pManifest->setId(getId());
    pManifest->setName(QObject::tr("Pro Filter"));
    pManifest->setAuthor("DJ Sugar");
    pManifest->setVersion("1.0");
    pManifest->setDescription(QObject::tr(
        "Professional multi-mode filter with LPF, HPF, BPF, Notch, and Peaking modes. "
        "High resonance for dramatic sweeps. Comparable to Rekordbox/Serato filters."));

    auto pCutoff = pManifest->addParameter();
    pCutoff->setId("cutoff");
    pCutoff->setName(QObject::tr("Cutoff"));
    pCutoff->setDescription(QObject::tr("Filter cutoff frequency"));
    pCutoff->setControlHint(EffectManifestParameter::ControlHint::KNOB_LOGARITHMIC);
    pCutoff->setSemanticHint(EffectManifestParameter::SemanticHint::UNKNOWN);
    pCutoff->setUnitsHint(EffectManifestParameter::UnitsHint::UNKNOWN);
    pCutoff->setDefault(0.5);
    pCutoff->setMinimum(0.0);
    pCutoff->setMaximum(1.0);

    auto pResonance = pManifest->addParameter();
    pResonance->setId("resonance");
    pResonance->setName(QObject::tr("Resonance"));
    pResonance->setDescription(QObject::tr("Filter resonance (Q factor)"));
    pResonance->setControlHint(EffectManifestParameter::ControlHint::KNOB_LINEAR);
    pResonance->setSemanticHint(EffectManifestParameter::SemanticHint::UNKNOWN);
    pResonance->setUnitsHint(EffectManifestParameter::UnitsHint::UNKNOWN);
    pResonance->setDefault(0.3);
    pResonance->setMinimum(0.0);
    pResonance->setMaximum(1.0);

    auto pMode = pManifest->addParameter();
    pMode->setId("mode");
    pMode->setName(QObject::tr("Mode"));
    pMode->setDescription(QObject::tr("Filter mode: 0=LPF, 1=HPF, 2=BPF, 3=Notch, 4=Peak"));
    pMode->setControlHint(EffectManifestParameter::ControlHint::KNOB_LINEAR);
    pMode->setSemanticHint(EffectManifestParameter::SemanticHint::UNKNOWN);
    pMode->setUnitsHint(EffectManifestParameter::UnitsHint::UNKNOWN);
    pMode->setDefault(0.0);
    pMode->setMinimum(0.0);
    pMode->setMaximum(4.0);

    auto pGain = pManifest->addParameter();
    pGain->setId("gain");
    pGain->setName(QObject::tr("Gain"));
    pGain->setDescription(QObject::tr("Gain (for peaking mode, in dB)"));
    pGain->setControlHint(EffectManifestParameter::ControlHint::KNOB_LINEAR);
    pGain->setSemanticHint(EffectManifestParameter::SemanticHint::UNKNOWN);
    pGain->setUnitsHint(EffectManifestParameter::UnitsHint::UNKNOWN);
    pGain->setDefault(0.0);
    pGain->setMinimum(-1.0);
    pGain->setMaximum(1.0);

    return pManifest;
}

void ProFilterEffect::loadEngineEffectParameters(
        const QMap<QString, EngineEffectParameterPointer>& parameters) {
    m_pCutoffParameter = parameters.value("cutoff");
    m_pResonanceParameter = parameters.value("resonance");
    m_pModeParameter = parameters.value("mode");
    m_pGainParameter = parameters.value("gain");
}

void ProFilterEffect::processChannel(
        ProFilterGroupState* pState,
        const CSAMPLE* pInput,
        CSAMPLE* pOutput,
        const mixxx::EngineParameters& engineParameters,
        const EffectEnableState enableState,
        const GroupFeatureState& groupFeatures) {
    const int sampleRate = engineParameters.sampleRate();
    const int numSamples = engineParameters.framesPerBuffer();

    // Get parameters
    double cutoff_raw = m_pCutoffParameter->value();
    double resonance_raw = m_pResonanceParameter->value();
    int mode = static_cast<int>(m_pModeParameter->value());
    double gain_raw = m_pGainParameter->value();

    // Clamp mode
    mode = std::clamp(mode, 0, NUM_MODES - 1);

    // Map cutoff from 0-1 to 20Hz-20kHz (logarithmic)
    double cutoff = 20.0 * std::pow(1000.0, cutoff_raw);

    // Map resonance from 0-1 to 0.1-20
    double resonance = 0.1 + resonance_raw * 19.9;

    // Map gain from -1 to 1 to -12dB to +12dB
    double gain = gain_raw * 12.0;

    // Smooth parameters
    double smoothCutoff = pState->prev_cutoff + kSmoothCoeff * (cutoff - pState->prev_cutoff);
    double smoothResonance = pState->prev_resonance + kSmoothCoeff * (resonance - pState->prev_resonance);
    double smoothGain = pState->prev_gain + kSmoothCoeff * (gain - pState->prev_gain);

    pState->prev_cutoff = smoothCutoff;
    pState->prev_resonance = smoothResonance;
    pState->prev_gain = smoothGain;

    // Compute biquad coefficients
    double b0, b1, b2, a1, a2;
    computeBiquadCoefficients(
        smoothCutoff, smoothResonance, smoothGain,
        &b0, &b1, &b2, &a1, &a2, mode, sampleRate);

    // Process filter for each channel
    for (int i = 0; i < numSamples; ++i) {
        int ch = i % 2; // Stereo interleaved

        double x0 = static_cast<double>(pInput[i]);
        double y0 = b0 * x0 + b1 * pState->x1[ch] + b2 * pState->x2[ch]
                           - a1 * pState->y1[ch] - a2 * pState->y2[ch];

        // Update filter state
        pState->x2[ch] = pState->x1[ch];
        pState->x1[ch] = x0;
        pState->y2[ch] = pState->y1[ch];
        pState->y1[ch] = y0;

        pOutput[i] = static_cast<CSAMPLE>(y0);
    }
}
