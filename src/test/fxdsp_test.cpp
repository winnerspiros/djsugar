#include <gtest/gtest.h>

#include <cmath>
#include <vector>

#include "effects/backends/builtin/dsp/delayline.h"
#include "effects/backends/builtin/dsp/fdnreverb.h"
#include "effects/backends/builtin/dsp/filters.h"
#include "effects/backends/builtin/dsp/modulation.h"
#include "effects/backends/builtin/dsp/saturation.h"

namespace {

constexpr float kRate = 48000.0f;

std::vector<float> sine(float hz, int n, float amp = 1.0f, float rate = kRate) {
    std::vector<float> out(static_cast<std::size_t>(n));
    for (int i = 0; i < n; ++i) {
        out[static_cast<std::size_t>(i)] =
                amp * std::sin(fxdsp::kTwoPi * hz * static_cast<float>(i) / rate);
    }
    return out;
}

double rms(const std::vector<float>& x, std::size_t from = 0) {
    double sum = 0.0;
    for (std::size_t i = from; i < x.size(); ++i) {
        sum += static_cast<double>(x[i]) * x[i];
    }
    return std::sqrt(sum / static_cast<double>(x.size() - from));
}

// Magnitude of one frequency (Goertzel), normalized to sine amplitude.
double magnitudeAt(const std::vector<float>& x, double hz, double rate) {
    const double w = 2.0 * M_PI * hz / rate;
    const double c = 2.0 * std::cos(w);
    double s1 = 0.0;
    double s2 = 0.0;
    for (float v : x) {
        const double s = v + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    const double power = s1 * s1 + s2 * s2 - c * s1 * s2;
    return 2.0 * std::sqrt(std::max(power, 0.0)) / static_cast<double>(x.size());
}

bool allFinite(const std::vector<float>& x) {
    for (float v : x) {
        if (!std::isfinite(v)) {
            return false;
        }
    }
    return true;
}

TEST(FxDspTest, SvfLowpassResponse) {
    fxdsp::Svf svf;
    svf.setCoeffs(1000.0f, 0.707f, kRate);
    auto low = sine(100.0f, 48000);
    auto high = sine(10000.0f, 48000);
    for (float& v : low) {
        v = svf.process(v).lp;
    }
    svf.reset();
    for (float& v : high) {
        v = svf.process(v).lp;
    }
    EXPECT_NEAR(rms(low, 4800), M_SQRT1_2, 0.02);
    // 12 dB/oct: about -40 dB at 10x the cutoff.
    EXPECT_LT(rms(high, 4800), M_SQRT1_2 * 0.015);
}

TEST(FxDspTest, SvfSurvivesAudioRateSweepAtHighResonance) {
    fxdsp::Svf svf;
    auto x = sine(220.0f, 96000, 1.0f);
    for (int i = 0; i < 96000; ++i) {
        const float lfo = 0.5f + 0.5f * std::sin(fxdsp::kTwoPi * 50.0f * i / kRate);
        svf.setCoeffs(fxdsp::knobToHz(lfo, 20.0f, 20000.0f), 20.0f, kRate);
        x[static_cast<std::size_t>(i)] =
                svf.processDriven(x[static_cast<std::size_t>(i)], 2.0f).bp;
    }
    ASSERT_TRUE(allFinite(x));
    for (float v : x) {
        EXPECT_LT(std::fabs(v), 8.0f);
    }
}

TEST(FxDspTest, LadderSelfOscillationIsBounded) {
    fxdsp::Ladder ladder;
    ladder.setCoeffs(800.0f, 1.0f, kRate);
    auto x = sine(55.0f, 48000, 2.0f);
    for (float& v : x) {
        v = ladder.process(v, 1.5f);
    }
    ASSERT_TRUE(allFinite(x));
    for (float v : x) {
        EXPECT_LT(std::fabs(v), 4.0f);
    }
}

TEST(FxDspTest, AdaaReducesAliasing) {
    // A 5 kHz tone driven hard at 44.1 kHz: the 7th and 9th harmonics fold
    // back to 9.1 kHz and 0.9 kHz. ADAA must cut them clearly.
    const float rate = 44100.0f;
    auto x = sine(5000.0f, 44100, 8.0f, rate);
    std::vector<float> naive(x.size());
    std::vector<float> adaa(x.size());
    fxdsp::AdaaTanh shaper;
    for (std::size_t i = 0; i < x.size(); ++i) {
        naive[i] = std::tanh(x[i]);
        adaa[i] = shaper.process(x[i]);
    }
    const double naiveAlias = magnitudeAt(naive, 9100.0, rate) + magnitudeAt(naive, 900.0, rate);
    const double adaaAlias = magnitudeAt(adaa, 9100.0, rate) + magnitudeAt(adaa, 900.0, rate);
    EXPECT_LT(adaaAlias, naiveAlias * 0.5);
    // The fundamental survives.
    EXPECT_GT(magnitudeAt(adaa, 5000.0, rate), 0.8);
}

TEST(FxDspTest, HermiteFractionalDelay) {
    fxdsp::DelayLine line;
    line.allocate(1024);
    auto x = sine(1000.0f, 2000);
    double maxErr = 0.0;
    for (int i = 0; i < 2000; ++i) {
        line.write(x[static_cast<std::size_t>(i)]);
        if (i > 200) {
            const float y = line.readHermite(100.5f);
            const float expected = std::sin(fxdsp::kTwoPi * 1000.0f *
                    (static_cast<float>(i) - 100.5f) / kRate);
            maxErr = std::max(maxErr, static_cast<double>(std::fabs(y - expected)));
        }
    }
    EXPECT_LT(maxErr, 2e-3);
}

TEST(FxDspTest, ReverbDecaysAtRt60AndFreezeHolds) {
    fxdsp::FdnReverb reverb;
    reverb.setup(kRate);
    reverb.setSize(0.6f, true);
    reverb.setDecay(1.0f);
    reverb.setDamping(20000.0f);
    reverb.setLowCut(10.0f);
    float l = 0.0f;
    float r = 0.0f;
    std::vector<float> out;
    out.reserve(96000);
    for (int i = 0; i < 96000; ++i) {
        const float in = i < 2000 ? 0.5f * std::sin(static_cast<float>(i) * 0.37f) : 0.0f;
        reverb.process(in, in, &l, &r);
        out.push_back(l);
    }
    ASSERT_TRUE(allFinite(out));
    auto window = [&](int from) {
        return rms(std::vector<float>(out.begin() + from, out.begin() + from + 4800));
    };
    const double early = window(4800);
    const double atRt60 = window(4800 + 48000);
    const double drop = 20.0 * std::log10(atRt60 / early);
    EXPECT_LT(drop, -45.0);
    EXPECT_GT(drop, -80.0);

    // Freeze: energy stays put for two seconds.
    reverb.clear();
    for (int i = 0; i < 4000; ++i) {
        const float in = 0.5f * std::sin(static_cast<float>(i) * 0.21f);
        reverb.process(in, in, &l, &r);
    }
    reverb.setFreeze(true);
    for (int i = 0; i < 4800; ++i) {
        reverb.process(0.0f, 0.0f, &l, &r);
    }
    const float e0 = reverb.energy();
    for (int i = 0; i < 96000; ++i) {
        reverb.process(1.0f, 1.0f, &l, &r); // input is ignored while frozen
    }
    EXPECT_GT(reverb.energy(), e0 * 0.5f);
    EXPECT_LT(reverb.energy(), e0 * 2.0f);
}

TEST(FxDspTest, PitchShifterOctaveUp) {
    fxdsp::PitchShifter shifter;
    shifter.setup(kRate, 0.05f);
    auto x = sine(300.0f, 48000);
    for (float& v : x) {
        v = shifter.process(v, 2.0f);
    }
    std::vector<float> tail(x.begin() + 9600, x.end());
    EXPECT_GT(magnitudeAt(tail, 600.0, kRate), 4.0 * magnitudeAt(tail, 300.0, kRate));
}

TEST(FxDspTest, BeatClockFollowsGrid) {
    fxdsp::BeatClock clock;
    const int frames = 512;
    const double beatSeconds = 0.5;
    double position = 0.0; // beats
    float lastPhase = 0.0f;
    int wraps = 0;
    for (int buffer = 0; buffer < 400; ++buffer) {
        position += frames / (beatSeconds * kRate);
        clock.update(beatSeconds, position - std::floor(position), frames, kRate);
        for (int i = 0; i < frames; ++i) {
            const float phase = clock.tick(4.0);
            if (phase < lastPhase) {
                ++wraps;
            }
            lastPhase = phase;
        }
    }
    // 400 * 512 frames = 4.27 s = 8.53 beats = 2 full four-beat cycles.
    EXPECT_EQ(wraps, 2);
    EXPECT_TRUE(clock.synced());
}

} // namespace
