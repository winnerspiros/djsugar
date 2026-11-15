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

/// USB drives Android didn't mount (e.g. FAT32 without partition table) are
/// read by Mixxx, mirrored on demand to local folders for Rekordbox/Engine DJ readers.

/// Looks for USB drives, returns local folder stand-ins. May block (USB permission).
/// Must not be called on the main thread.
QFileInfoList driveRoots();

/// Copies drive files to the local cache if missing or outdated.
void prepareFiles(const QString& root, const QStringList& relativePaths);

/// Reset USB permission state so the next scan re-requests unapproved drives.
void allowPermissionRequests();

/// Ensure a drive file is copied locally. Blocks — not on the main thread.
bool fetchFile(const QString& localPath);

struct DriveFile {
    /// Relative to the drive root, with the names as stored on the drive
    QString relativePath;
    qint64 size;
    QDateTime modified;
};

/// Recursively list all files on a drive. Blocks. Sets *pComplete=false on read errors.
std::optional<QList<DriveFile>> listFiles(const QString& root, bool* pComplete);

/// Copy a drive file directly to destination (no local cache). Blocks.
bool copyFileTo(const QString& root, const QString& relativePath, const QString& destination);

/// Serial number of the drive, e.g. "1A2B-3C4D". Empty if not a recognised drive.
QString driveSerial(const QString& root);

/// True if the path is inside the local folder of a drive
bool isOnDrive(const QString& localPath);

/// Makes sure a track of a drive has been copied, then calls `then` on the
/// thread of `pContext`. Tracks that are not on a drive continue at once.
void fetchTrack(const QString& localPath, QObject* pContext, std::function<void()> then);

} // namespace usbdrive
} // namespace mixxx
