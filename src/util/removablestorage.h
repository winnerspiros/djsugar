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
    /// The name Android shows it with
    QString name;
    Kind kind;
};

/// Internal storage, SD cards and USB drives Android has mounted
/// (org.mixxx.StorageHelper). A removable volume whose name doesn't say
/// SD counts as a USB drive.
QList<AndroidStorageVolume> androidStorageVolumes();
#endif

/// Root folders of mounted external drives (USB sticks, SD cards...).
/// Scanned for databases exported by other DJ software. On Android includes
/// internal storage, drives Android didn't mount, and USB stick copies
/// (each subfolder of /storage/emulated/0/Music/Mixxx/Rekordbox). Looking
/// for drives may wait for USB permission: not for the main thread.
QFileInfoList removableStorageRoots();

/// The label shown for a drive found by removableStorageRoots(). Stick
/// copies are marked as backups, so a stick and its backup can be told apart.
QString removableStorageLabel(const QFileInfo& root);

} // namespace mixxx
