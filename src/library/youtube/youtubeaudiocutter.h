#pragma once

#include <QList>
#include <QString>

namespace mixxx {

struct SponsorSegment;

/// Cuts time ranges from audio via libavformat stream-copy (no re-encode).
/// After cutting, duration/beat-grid/BPM/waveform reflect music-only length.
/// Replaces `path` in-place on success; leaves it on failure.
bool cutAudioRanges(const QString& path, const QList<SponsorSegment>& segments);

} // namespace mixxx
