#pragma once

#include <vector>

#include "audio/types.h"
#include "track/beats.h"
#include "util/types.h"

namespace mixxx {
namespace android {

/// Lines a constant beatgrid up with the kicks.
///
/// The beat tracker often follows off-beat hi-hats or claps rather than the
/// kick, leaving the grid half a beat off, and sometimes doubles the BPM.
/// Keeps low-frequency onsets (below ~120 Hz) for the first two minutes.
/// Afterwards: halves BPM (200+) when one half carries much more kick energy;
/// shifts grid by half a beat if off-beats have clearly more low-frequency
/// onsets and the first onset is on an off-beat (tracks start on a beat).
/// Otherwise leaves the grid as-is.
class KickGrid {
  public:
    void initialize(audio::SampleRate sampleRate);
    /// Interleaved samples of the analysis
    void process(const CSAMPLE* pSamples, SINT frames, int channels);
    /// The corrected beats, or pBeats if nothing needs to change
    BeatsPointer correct(const BeatsPointer& pBeats) const;

  private:
    static constexpr SINT kHopFrames = 256;
    static constexpr double kSecondsKept = 120;

    audio::SampleRate m_sampleRate;
    double m_lowPassCoefficient = 0;
    double m_lowPass1 = 0;
    double m_lowPass2 = 0;
    double m_hopEnergy = 0;
    SINT m_hopFrames = 0;
    SINT m_maxHops = 0;
    /// RMS of the low frequencies per hop
    std::vector<float> m_envelope;
};

} // namespace android
} // namespace mixxx
