#pragma once

#include <QDateTime>
#include <QList>
#include <QRegularExpression>
#include <QSqlDatabase>
#include <QString>
#include <QVariant>
#include <optional>

#include "track/track_decl.h"
#include "track/trackid.h"

class PlayerManager;
class QAbstractItemModel;
class RekordboxFeature;

namespace mixxx {
namespace android {

/// Characters not allowed in file names stripped.
inline QString safeFileName(QString name) {
    static const QRegularExpression kInvalid(QStringLiteral("[/\\\\:*?\"<>|\\x00-\\x1F]"));
    name.replace(kInvalid, QStringLiteral("_"));
    name = name.trimmed();
    // Android's FAT and exFAT file systems drop trailing dots
    while (name.endsWith(QChar('.'))) {
        name.chop(1);
    }
    return name;
}

/// A track of a Rekordbox playlist with the data exported for it.
struct RekordboxTrack {
    QString location;
    /// The ANLZ file with the beatgrid and the cues
    QString analyzePath;
    QString key;
    QVariant color;
};

/// The tracks of the Rekordbox playlist that `pModel` shows, in playlist
/// order regardless of the current search. Returns nothing if not a
/// Rekordbox playlist.
std::optional<QList<RekordboxTrack>> rekordboxPlaylistTracks(QAbstractItemModel* pModel);

/// True if `pModel` shows a Rekordbox playlist
bool isRekordboxPlaylist(QAbstractItemModel* pModel);

/// Applies the beatgrid, cues, key and color Rekordbox exported for
/// `rekordboxTrack` to `pTrack` (a copy). Same as loading from a Rekordbox drive.
void applyRekordboxData(const TrackPointer& pTrack, const RekordboxTrack& rekordboxTrack);

/// Applies Rekordbox data to tracks of Rekordbox drives and their backups
/// when a deck loads them. Without this, QML-loaded tracks get Mixxx's own
/// beatgrid instead of Rekordbox's. Applied once so changes in Mixxx are kept.
/// Must be called before other code that reacts to loaded tracks.
void applyRekordboxDataOnLoad(PlayerManager* pPlayerManager, const QSqlDatabase& database);

/// Date added to the Rekordbox collection for a track on a Rekordbox drive
/// (by location on the drive or its copy), or invalid if not a Rekordbox track.
QDateTime rekordboxDateAdded(const QSqlDatabase& database, const QString& location);

/// Library track id for `location`, or invalid.
TrackId libraryTrackId(const QSqlDatabase& database, const QString& location);

/// Sets the "date added" of a track of the library
bool setTrackDateAdded(
        const QSqlDatabase& database, TrackId trackId, const QDateTime& dateAdded);

/// Re-scans for USB devices a few seconds after plug/unplug.
void watchUsbDevices(RekordboxFeature* pFeature);

} // namespace android
} // namespace mixxx
