#pragma once

#include <QImage>
#include <QSize>
#include <QString>

namespace mixxx {
namespace qml {

/// Loads the cover art of a library track from the information stored in
/// the database, without loading the track itself (which would have to
/// happen on the main thread), and keeps the recently shown covers in a
/// cache. Tracks that aren't in the library get no cover.
///
/// Returns false if the database can't be read, then the caller has to load
/// the cover from the track.
bool loadLibraryCover(const QString& id,
        const QString& trackLocation,
        const QSize& requestedSize,
        QImage* pImage);

} // namespace qml
} // namespace mixxx
