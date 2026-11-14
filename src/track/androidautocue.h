#pragma once

#include "track/track_decl.h"

class PlayerManager;
class Track;

namespace mixxx {
namespace android {

/// Moves the cue on the first sound of a track onto the nearest beat.
/// The analysis puts the main cue where the track first exceeds -60 dB,
/// often a few ms before the first kick. If a beatgrid beat is within a
/// quarter beat of that position, the cue is moved onto it. User-moved
/// cues (not at the first-sound position) are left alone.
bool snapAutoCueToBeat(Track* pTrack);

/// Applies snapAutoCueToBeat() to every track that is loaded into a deck,
/// before the deck jumps to the main cue.
void snapAutoCuesOnLoad(PlayerManager* pPlayerManager);

} // namespace android
} // namespace mixxx
