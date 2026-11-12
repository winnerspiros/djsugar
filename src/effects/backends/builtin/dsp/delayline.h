#pragma once

// Mono circular delay line with fractional reads. Power-of-two length so
// wrapping is a mask. allocate() must run outside the audio callback, except
// when a larger allocate() came first: assign() then reuses the storage.

#include <vector>

#include "effects/backends/builtin/dsp/fxmath.h"

namespace fxdsp {

class DelayLine {
  public:
    void allocate(int maxDelaySamples) {
        int size = 1;
        while (size < maxDelaySamples + 4) {
            size <<= 1;
        }
        m_buffer.assign(static_cast<std::size_t>(size), 0.0f);
        m_mask = size - 1;
        m_write = 0;
    }

    void clear() {
        std::fill(m_buffer.begin(), m_buffer.end(), 0.0f);
    }

    int capacity() const {
        return m_mask - 3;
    }

    void write(float x) {
        m_write = (m_write + 1) & m_mask;
        m_buffer[static_cast<std::size_t>(m_write)] = x;
    }

    // Sample written `delay` writes ago (0 = most recent).
    float tap(int delay) const {
        return m_buffer[static_cast<std::size_t>((m_write - delay) & m_mask)];
    }

    float readLinear(float delay) const {
        const float d = clampf(delay, 0.0f, static_cast<float>(capacity()));
        const int i = static_cast<int>(d);
        const float f = d - static_cast<float>(i);
        return lerp(tap(i), tap(i + 1), f);
    }

    // 4-point, 3rd-order Hermite interpolation: flat response to about
    // 0.8 * Nyquist and no zipper when the delay time glides, which linear
    // interpolation turns into audible high-frequency loss.
    float readHermite(float delay) const {
        const float d = clampf(delay, 1.0f, static_cast<float>(capacity() - 1));
        const int i = static_cast<int>(d);
        const float f = d - static_cast<float>(i);
        const float xm1 = tap(i - 1);
        const float x0 = tap(i);
        const float x1 = tap(i + 1);
        const float x2 = tap(i + 2);
        const float c1 = 0.5f * (x1 - xm1);
        const float c2 = xm1 - 2.5f * x0 + 2.0f * x1 - 0.5f * x2;
        const float c3 = 0.5f * (x2 - xm1) + 1.5f * (x0 - x1);
        return ((c3 * f + c2) * f + c1) * f + x0;
    }

  private:
    std::vector<float> m_buffer;
    int m_mask = 0;
    int m_write = 0;
};

} // namespace fxdsp
