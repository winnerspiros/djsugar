#pragma once

#include <algorithm>
#include <array>
#include <cmath>

namespace fxdsp {

/// The motion of a back spin, in times the normal speed, at t = 0..1 of
/// its length. The hand catches the record and throws it back (the speed
/// glides through zero, the "zip" of a real spin), then the platter coasts
/// to a stop against the friction of the mat.
class BackSpinMotion {
  public:
    /// Part of the spin the hand takes to throw the record back
    static constexpr double kCatch = 0.08;

    /// peak: top backward speed
    static double velocity(double t, double peak) {
        if (t >= 1.0) {
            return 0.0;
        }
        if (t < kCatch) {
            const double u = t / kCatch;
            const double smooth = u * u * (3.0 - 2.0 * u);
            return 1.0 - (1.0 + peak) * smooth;
        }
        const double u = (t - kCatch) / (1.0 - kCatch);
        return -peak * (1.0 - u) * (1.0 - u);
    }
    /// How far back the spin travels, in frames, for a length in frames
    /// and a peak speed: the throw (the speed eases from 1 to -peak) and
    /// the coast (a third of peak x its part)
    static double distance(double frames, double peak) {
        return frames * (peak * kBackPerPeak - kCatch / 2.0);
    }
    /// The highest peak speed that stays within the audio there is
    static double peakFor(double availableFrames, double frames, double maxPeak) {
        return std::clamp(
                (availableFrames / frames + kCatch / 2.0) / kBackPerPeak, 0.0, maxPeak);
    }

  private:
    static constexpr double kBackPerPeak = (1.0 - kCatch) / 3.0 + kCatch / 2.0;
};

/// How a record played at any speed sounds through a cartridge, as in the
/// scratch engine (EngineBufferScaleLinear::shapeScratch): the level
/// follows the stylus velocity (slow is quiet, a stop is silent), and a
/// low pass follows the speed: below 1x it removes the images a slowed
/// record does not have, above 1x the aliasing of reading faster.
class NeedleShaper {
  public:
    void reset() {
        m_state = {};
    }
    /// Shapes a stereo frame read at velocity (times the normal speed)
    void process(float* pLeft, float* pRight, double velocity) {
        constexpr double kPiDouble = 3.14159265358979323846;
        const double speed = std::fabs(velocity);
        const float level = static_cast<float>(std::min(speed, 1.0));
        const float g = static_cast<float>(std::tan(kPiDouble *
                std::clamp(0.45 * std::min(speed, 1.0 / std::max(speed, 1e-3)),
                        0.002,
                        0.45)));
        // Two 2-pole state variable stages (24 dB/oct, Butterworth)
        constexpr float kDamping = 1.41421356f;
        const float a1 = 1.f / (1.f + g * (g + kDamping));
        float* channels[2] = {pLeft, pRight};
        for (int c = 0; c < 2; ++c) {
            float x = *channels[c];
            for (int stage = 0; stage < 2; ++stage) {
                float& s1 = m_state[c][2 * stage];
                float& s2 = m_state[c][2 * stage + 1];
                const float v1 = a1 * (s1 + g * (x - s2));
                const float v2 = s2 + g * v1;
                s1 = 2.f * v1 - s1;
                s2 = 2.f * v2 - s2;
                x = v2;
            }
            *channels[c] = level * x;
        }
    }

  private:
    std::array<std::array<float, 4>, 2> m_state{};
};

} // namespace fxdsp
