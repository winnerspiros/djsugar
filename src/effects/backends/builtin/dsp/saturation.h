#pragma once

// Waveshapers. The Adaa* classes use first-order antiderivative
// anti-aliasing (Parker, Zavalishin, Le Bivic, "Reducing the aliasing of
// nonlinear waveshaping using continuous-time convolution", DAFx 2016):
// y[n] = (F(x[n]) - F(x[n-1])) / (x[n] - x[n-1]) with F the antiderivative
// of the shaper. This removes most of the harsh fold-back aliasing of
// naive clipping at the cost of half a sample of delay, with no
// oversampling.

#include <cmath>

namespace fxdsp {

// Smooth cubic clip, unity slope at 0, saturates at +-2/3 * 1.5 = 1.
inline float softClip(float x) {
    if (x <= -1.5f) {
        return -1.0f;
    }
    if (x >= 1.5f) {
        return 1.0f;
    }
    return x - (4.0f / 27.0f) * x * x * x;
}

// Rational tanh approximation (Pade 7/6), error below 2e-4 and exactly
// bounded to [-1, 1].
inline float fastTanh(float x) {
    if (x <= -4.97f) {
        return -1.0f;
    }
    if (x >= 4.97f) {
        return 1.0f;
    }
    const float x2 = x * x;
    const float a = x * (135135.0f + x2 * (17325.0f + x2 * (378.0f + x2)));
    const float b = 135135.0f + x2 * (62370.0f + x2 * (3150.0f + x2 * 28.0f));
    const float y = a / b;
    return y > 1.0f ? 1.0f : (y < -1.0f ? -1.0f : y);
}

// Antiderivative of tanh, overflow safe: log(cosh(x)).
inline double logCosh(double x) {
    const double ax = std::fabs(x);
    return ax + std::log1p(std::exp(-2.0 * ax)) - 0.69314718055994530942;
}

class AdaaTanh {
  public:
    void reset() {
        m_x1 = 0.0;
        m_F1 = 0.0;
    }
    float process(float xf) {
        const double x = xf;
        const double F = logCosh(x);
        const double dx = x - m_x1;
        double y;
        if (std::fabs(dx) < 1e-5) {
            y = std::tanh(0.5 * (x + m_x1));
        } else {
            y = (F - m_F1) / dx;
        }
        m_x1 = x;
        m_F1 = F;
        return static_cast<float>(y);
    }

  private:
    double m_x1 = 0.0;
    double m_F1 = 0.0;
};

// Hard clip at +-1 with ADAA. F(x) = x^2 / 2 inside, |x| - 1/2 outside.
class AdaaHardClip {
  public:
    void reset() {
        m_x1 = 0.0;
        m_F1 = 0.0;
    }
    float process(float xf) {
        const double x = xf;
        const double ax = std::fabs(x);
        const double F = ax <= 1.0 ? 0.5 * x * x : ax - 0.5;
        const double dx = x - m_x1;
        double y;
        if (std::fabs(dx) < 1e-5) {
            const double m = 0.5 * (x + m_x1);
            y = m > 1.0 ? 1.0 : (m < -1.0 ? -1.0 : m);
        } else {
            y = (F - m_F1) / dx;
        }
        m_x1 = x;
        m_F1 = F;
        return static_cast<float>(y);
    }

  private:
    double m_x1 = 0.0;
    double m_F1 = 0.0;
};

// Sine wavefolder with ADAA: f(x) = sin(x), F(x) = -cos(x).
class AdaaSineFold {
  public:
    void reset() {
        m_x1 = 0.0;
        m_F1 = -1.0;
    }
    float process(float xf) {
        const double x = xf;
        const double F = -std::cos(x);
        const double dx = x - m_x1;
        double y;
        if (std::fabs(dx) < 1e-5) {
            y = std::sin(0.5 * (x + m_x1));
        } else {
            y = (F - m_F1) / dx;
        }
        m_x1 = x;
        m_F1 = F;
        return static_cast<float>(y);
    }

  private:
    double m_x1 = 0.0;
    double m_F1 = -1.0;
};

} // namespace fxdsp
