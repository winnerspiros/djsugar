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

/// `name` without the characters that are not allowed in file names
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

/// A track of a Rekordbox playlist with the data Rekordbox exported for it
struct RekordboxTrack {
    QString location;
    /// The ANLZ file with the beatgrid and the cues
    QString analyzePath;
    QString key;
    QVariant color;
};

/// The tracks of the Rekordbox playlist that `pModel` shows, in the order
/// of the playlist and regardless of the current search. `pModel` is a
/// track list of the QML UI or a Rekordbox playlist model. Returns nothing
/// if it doesn't show a Rekordbox playlist.
std::optional<QList<RekordboxTrack>> rekordboxPlaylistTracks(QAbstractItemModel* pModel);

/// True if `pModel` shows a Rekordbox playlist
bool isRekordboxPlaylist(QAbstractItemModel* pModel);

/// Applies the beatgrid, cues, key and color that Rekordbox exported for
/// `rekordboxTrack` to `pTrack`, a copy of its file. Does the same as
/// loading the track from the Rekordbox drive.
void applyRekordboxData(const TrackPointer& pTrack, const RekordboxTrack& rekordboxTrack);

/// Applies the Rekordbox data to the tracks of Rekordbox drives and their
/// backups when a deck loads them. The QML library loads tracks by their
/// location, without the Rekordbox track model that does this on desktop, so
/// without it these tracks would get the beatgrid of Mixxx's own analysis
/// instead of the one Rekordbox exported for them. A beatgrid of Rekordbox is
/// only applied once, so changes made to it in Mixxx are kept.
/// Must be called before other code that reacts to tracks being loaded,
/// which should see the beatgrid and the cues of Rekordbox.
void applyRekordboxDataOnLoad(PlayerManager* pPlayerManager, const QSqlDatabase& database);

/// The day a track of a Rekordbox drive (by its location on the drive or
/// the drive's copy) was added to the Rekordbox collection, or an invalid
/// date if it is not a track of a Rekordbox drive that was opened
QDateTime rekordboxDateAdded(const QSqlDatabase& database, const QString& location);

/// The id of the library track with the file `location`, or an invalid id
TrackId libraryTrackId(const QSqlDatabase& database, const QString& location);

/// Sets the "date added" of a track of the library
bool setTrackDateAdded(
        const QSqlDatabase& database, TrackId trackId, const QDateTime& dateAdded);

/// Makes `pFeature` look for devices again a few seconds after a USB device
/// was plugged in or out, so that sticks appear and disappear by themselves
void watchUsbDevices(RekordboxFeature* pFeature);

} // namespace android
} // namespace mixxx
