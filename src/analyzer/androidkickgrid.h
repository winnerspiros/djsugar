#pragma once

#include <vector>

#include "audio/types.h"
#include "track/beats.h"
#include "util/types.h"

namespace mixxx {
namespace android {

/// Lines a constant beatgrid up with the kicks.
///
/// The beat tracker often follows the off-beat hi-hats or claps of house and
/// techno rather than the kick, which leaves the grid half a beat off, and
/// sometimes doubles the BPM. While the track is analyzed, this keeps the
/// onsets of its low frequencies (below about 120 Hz) for the first two
/// minutes. Afterwards:
///
/// - from 200 BPM on, the BPM is halved when one half of the beats carries
///   much more kick than the other (doubled house and techno);
/// - the grid is moved by half a beat if its off-beats carry clearly more
///   low-frequency onsets than its beats and the first onset of the track
///   lies on an off-beat too (tracks start on a beat). The beat tracker is
///   precise, its beats are only kept or moved by exactly half a beat, as
///   where the low frequencies rise depends on the kick.
///
/// Otherwise, e.g. with a bass louder than the kick, the grid is left as the
/// beat tracker made it.
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
