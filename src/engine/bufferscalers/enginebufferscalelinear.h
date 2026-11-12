#pragma once

#include <array>

#include "engine/bufferscalers/enginebufferscale.h"
#include "util/samplebuffer.h"

class ReadAheadManager;

/** Number of samples to read ahead */
constexpr int kiLinearScaleReadAheadLength = 10240;

class EngineBufferScaleLinear : public EngineBufferScale  {
    Q_OBJECT
  public:
    explicit EngineBufferScaleLinear(
            ReadAheadManager* pReadAheadManager);
    ~EngineBufferScaleLinear() override;

    double scaleBuffer(
            CSAMPLE* pOutputBuffer,
            SINT iOutputBufferSize) override;
    void clear() override;

    void setScaleParameters(double base_rate,
                            double* pTempoRatio,
                             double* pPitchRatio) override;

    /// Scratching, spin backs and throws: shape the sound like a needle on
    /// a moving record (see smoothSlowPlayback). Off: plain playback.
    void setScratching(bool scratching) {
        m_scratching = scratching;
    }

  private:
    void onSignalChanged() override;

    double do_scale(CSAMPLE* buf, SINT buf_size);
    SINT do_copy(CSAMPLE* buf, SINT buf_size);
    void shapeScratch(CSAMPLE* buf, SINT buf_size, double rate, double speed);

    // The read-ahead manager that we use to fetch samples
    ReadAheadManager* m_pReadAheadManager;

    // Buffer for handling calls to ReadAheadManager
    CSAMPLE* m_bufferInt;
    SINT m_bufferIntSize;

    mixxx::SampleBuffer m_floorSampleOld;
    mixxx::SampleBuffer m_floorSample;
    mixxx::SampleBuffer m_ceilSample;

    bool m_bClear;
    double m_dRate;
    double m_dOldRate;

    double m_dCurrentFrame;
    double m_dNextFrame;

    double m_dBaseRate = 1.0;
    bool m_scratching = false;

    // Scratch shaping state, see shapeScratch()
    static constexpr int kMaxScratchChannels = 8;
    std::array<std::array<float, 4>, kMaxScratchChannels> m_scratchFilter{};
    float m_scratchCutoff = 0.f; // tan(pi * fc / fs) of the last buffer
    float m_scratchMix = 0.f;    // low pass amount of the last buffer
    float m_scratchGain = 1.f;   // needle velocity gain of the last buffer
};
