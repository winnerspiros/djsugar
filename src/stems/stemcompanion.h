#pragma once

#include <QString>

namespace mixxx {
namespace stems {

/// The stem file of a track, saved next to it in a hidden ".stems" folder,
/// e.g. "Music/.stems/Song.mp3.stem.mp4". The library scanner skips hidden
/// folders, so the stem files don't show up as tracks.
QString companionPath(const QString& trackLocation);

/// Where the stems of a track go when its folder can't be written, e.g. a
/// read-only drive: the app's own storage
QString fallbackPath(const QString& trackLocation);

/// The stem file of a track if it was separated, otherwise an empty string
QString findStemFile(const QString& trackLocation);

} // namespace stems
} // namespace mixxx
