#pragma once

#include <optional>
#include <vector>

/// Beatmatch maths for Auto DJ transitions. Pure functions, unit-testable.
namespace beatmatch {

/// Seconds the incoming track takes to glide back to its own tempo after the
/// mix; slow enough to go unnoticed.
constexpr double kGlideSeconds = 30.0;

/// The tempo ratio to play the incoming track at (1.0 = own speed). Half/double
/// time considered (e.g. 72 BPM under 144 BPM). Nullopt when change exceeds
/// tolerancePct or a BPM is unknown.
std::optional<double> matchRatio(double fromBpm, double toTrackBpm, double tolerancePct);

/// Glide ratio: straight line from startRatio to 1.0 over glideSeconds.
double glideRatio(double startRatio, double elapsedSeconds, double glideSeconds = kGlideSeconds);

/// Which bass (low EQ) is cut at crossfade position 0..1. Hard swap at the
/// middle so two basslines never play together.
struct BassState {
    bool fromLowKilled = false;
    bool toLowKilled = false;
};
BassState bassSwap(double progress);

/// Full EQ transition on top of the bass swap. Mids/highs cross over gradually:
/// incoming rises from kEqBlendFloor to full, outgoing falls. Values multiply
/// the DJ's own EQ setting (1 = unchanged).
constexpr double kEqBlendFloor = 0.3; // about -10 dB
struct EqBlend {
    double fromMidHigh = 1.0;
    double toMidHigh = 1.0;
};
EqBlend eqBlend(double progress);

/// Stem mix: when both songs have parts (drums, bass, instrumental, vocals),
/// the parts cross over instead of EQ. Two vocals never play together; two
/// basslines never clash. Equal-power curves.
struct StemBlend {
    double fromVocals = 1.0;
    double fromInstrumental = 1.0;
    double fromDrums = 1.0;
    double fromBass = 1.0;
    double toVocals = 0.0;
    double toInstrumental = 0.0;
    double toDrums = 0.0;
    double toBass = 0.0;
};
/// fadeDrums: drums cross over gradually instead; bass still swaps at the middle.
StemBlend stemBlend(double progress, bool fadeDrums = false);

/// Who sings during the mix and when the outgoing singer ends a line (mix progress 0..1).
struct VocalPlan {
    bool fromSings = true;
    bool toSings = true;
    double swapAt = 0.5;
};
/// Stem mix with vocals timed by singing. A song that does not sing keeps
/// vocals with instrumental. Only when both sing are vocals handed over at
/// swapAt, never together.
StemBlend stemBlend(double progress, const VocalPlan& plan, bool fadeDrums = false);

/// Beat lock for a track with a beat map: continuously set the incoming speed
/// so its beats stay on the outgoing beats.
/// @param fromBeat outgoing track: beat number now (fractional)
/// @param fromBeatRealSec outgoing track: length of its beat now in real
/// seconds (at deck speed)
/// @param toBeat incoming track: beat number now (fractional)
/// @param toBeatTrackSec incoming track: length of its beat now at own speed
/// @return the incoming tempo ratio that gives it the same beat length, nudged
/// up to kMaxLockNudge to pull a slipped beat back in line. Slips under
/// kLockDeadBeats are left alone.
constexpr double kLockBeats = 2.0;
constexpr double kMaxLockNudge = 0.06;  // 6 % (key lock keeps the pitch)
constexpr double kLockDeadBeats = 0.01; // 5 ms at 120 BPM
/// Early in the mix (the incoming song still quiet), a slip bigger than
/// this is not pulled back slowly: the incoming song jumps in line.
constexpr double kResyncBeats = 0.05;
double followRatio(double fromBeat,
        double fromBeatRealSec,
        double toBeat,
        double toBeatTrackSec,
        double* pSlipBeats = nullptr);

/// Where the kick really starts after a song's beat lines: analysis can place
/// lines 8-41 ms before the kick, so two locked songs can still clash. Beat
/// lock lines up the kicks instead.
/// @param level drum level (0..1), rate values per second (waveform: ~441/s)
/// @param beatTimes the song's beat lines in seconds
/// @return middle offset in seconds (kick after line = positive), or nothing
/// when fewer than 16 beats have a clear hit
std::optional<double> kickOffset(const std::vector<float>& level,
        double rate,
        const std::vector<double>& beatTimes);

} // namespace beatmatch
