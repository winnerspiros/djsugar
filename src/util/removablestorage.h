#pragma once

#include <QFileInfoList>

namespace mixxx {

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
