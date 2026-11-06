#pragma once

#include <QFileInfoList>
#include <QList>
#include <QString>

namespace mixxx {

#if defined(__ANDROID__)
/// A storage volume Android has mounted
struct AndroidStorageVolume {
    enum class Kind {
        Internal,
        SdCard,
        UsbDrive,
    };
    /// e.g. /storage/emulated/0 or /storage/1A2B-3C4D
    QString path;
    /// The name Android shows it with, e.g. "Tarjeta SD SanDisk"
    QString name;
    Kind kind;
};

/// The internal shared storage, SD cards and USB drives Android has mounted
/// (org.mixxx.StorageHelper). Android doesn't tell SD cards and USB drives
/// apart: a removable volume whose name doesn't say SD counts as a USB
/// drive.
QList<AndroidStorageVolume> androidStorageVolumes();
#endif

/// Root folders of the external drives (USB sticks, SD cards...) that are
/// currently mounted. They are scanned for databases exported by other DJ
/// software. On Android the internal shared storage is included as well, the
/// drives Android didn't mount, and the copies of USB sticks: each subfolder
/// of /storage/emulated/0/Music/Mixxx/Rekordbox is read like a stick named
/// after the folder. Looking for drives may wait for the USB permission, so
/// this must not be called on the main thread.
QFileInfoList removableStorageRoots();

/// The name a drive found by removableStorageRoots() is shown with. Copies
/// of sticks are marked as backups, so a stick and its backup can be told
/// apart (Rekordbox identifies the drives by this name).
QString removableStorageLabel(const QFileInfo& root);

} // namespace mixxx
