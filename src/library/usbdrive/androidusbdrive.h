#pragma once

#include <QDateTime>
#include <QFileInfoList>
#include <QList>
#include <QString>
#include <QStringList>
#include <functional>
#include <optional>

class QObject;

namespace mixxx {
namespace usbdrive {

/// USB drives that Android didn't mount (e.g. FAT32 drives formatted without
/// a partition table, which Pioneer players accept) are read by Mixxx
/// itself, read-only. Their files are mirrored on demand to a local folder
/// that stands in for the drive, so the existing Rekordbox and Engine DJ
/// readers and the decks can use ordinary file paths.

/// Looks for USB drives and returns the local folders standing in for them.
/// May ask for the USB permission and block until it is answered, so it
/// must not be called on the main thread.
QFileInfoList driveRoots();

/// Copies files of a drive to its local folder, if they exist on the drive
/// and the local copy is missing or outdated. `relativePaths` are relative
/// to the drive root; "folder/*.db" copies all .db files of a folder.
/// Database files are refreshed whenever they changed on the drive.
void prepareFiles(const QString& root, const QStringList& relativePaths);

/// The USB permission of a drive is asked for once per plug-in. After this
/// call, the next search for drives asks again for the drives that still
/// lack it, e.g. because the user opened the drives explicitly.
void allowPermissionRequests();

/// Makes sure a file of a drive has been copied to its local folder. Blocks
/// while copying, so it must not be called on the main thread. Returns true
/// if the file is there, also for files that are not on a drive.
bool fetchFile(const QString& localPath);

struct DriveFile {
    /// Relative to the drive root, with the names as stored on the drive
    QString relativePath;
    qint64 size;
    QDateTime modified;
};

/// All files of the drive whose local folder is `root`, recursively.
/// Returns nothing if `root` is not a drive that Mixxx reads itself. Blocks
/// while reading the drive. Sets *pComplete to false if a part of the drive
/// could not be read, in which case files may be missing from the list.
std::optional<QList<DriveFile>> listFiles(const QString& root, bool* pComplete);

/// Copies a file of the drive whose local folder is `root` to `destination`,
/// directly, without the local cache. Blocks while copying.
bool copyFileTo(const QString& root, const QString& relativePath, const QString& destination);

/// The serial number of the drive whose local folder is `root`, e.g.
/// "1A2B-3C4D", or an empty string if `root` is not a drive Mixxx reads
QString driveSerial(const QString& root);

/// True if the path is inside the local folder of a drive
bool isOnDrive(const QString& localPath);

/// Makes sure a track of a drive has been copied, then calls `then` on the
/// thread of `pContext`. Tracks that are not on a drive continue at once.
void fetchTrack(const QString& localPath, QObject* pContext, std::function<void()> then);

} // namespace usbdrive
} // namespace mixxx
