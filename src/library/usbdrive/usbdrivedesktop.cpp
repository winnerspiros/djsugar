// Desktop stub for the Android USB-direct drive reader
// (from JaviLabarumDJ/mixxx-android: androidusbdrive.cpp is JNI/libusb and
// Android-only). On desktop the OS mounts USB drives itself, so there is no
// drive that Mixxx reads itself: all queries answer "not a drive".
#include <QFileInfo>
#include <QTimer>

#include "library/usbdrive/androidusbdrive.h"

namespace mixxx {
namespace usbdrive {

QFileInfoList driveRoots() {
    return {};
}

void prepareFiles(const QString&, const QStringList&) {
}

void allowPermissionRequests() {
}

bool fetchFile(const QString& localPath) {
    return QFileInfo::exists(localPath);
}

std::optional<QList<DriveFile>> listFiles(const QString&, bool* pComplete) {
    if (pComplete) {
        *pComplete = true;
    }
    return std::nullopt;
}

bool copyFileTo(const QString&, const QString&, const QString&) {
    return false;
}

QString driveSerial(const QString&) {
    return {};
}

bool isOnDrive(const QString&) {
    return false;
}

void fetchTrack(const QString&, QObject* pContext, std::function<void()> then) {
    QTimer::singleShot(0, pContext, std::move(then));
}

} // namespace usbdrive
} // namespace mixxx
