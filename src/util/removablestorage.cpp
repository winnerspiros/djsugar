#include "util/removablestorage.h"

#include <QCoreApplication>
#include <QDir>
#include <QStorageInfo>

#if defined(__ANDROID__)
#include <QJniEnvironment>
#include <QJniObject>
#endif
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

namespace mixxx {

namespace {

#if defined(__ANDROID__)
const QString kDriveCopiesFolder = QStringLiteral("/storage/emulated/0/Music/Mixxx/Rekordbox");
#endif

} // namespace

QFileInfoList removableStorageRoots() {
    QFileInfoList roots;
#if defined(__ANDROID__)
    // Android mounts USB drives and SD cards at /storage/<volume id>.
    // "emulated" holds the internal storage of each user and "self" links to
    // the one of the current user.
    const QFileInfoList volumes = QDir(QStringLiteral("/storage"))
                                          .entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
    for (const QFileInfo& volume : volumes) {
        if (volume.fileName() != QLatin1String("emulated") &&
                volume.fileName() != QLatin1String("self")) {
            // Android takes a few seconds to mount a drive after it was
            // plugged in (or after granting all-files access). The drive
            // entry in /storage already exists, but it is not readable yet.
            // Skip unready volumes: the device features re-scan on the next
            // activate/open, by which time the mount has finished. Waiting
            // here is not an option: this also runs on the UI thread.
            if (!QStorageInfo(volume.absoluteFilePath()).isReady()) {
                continue;
            }
            roots.append(volume);
        }
    }
    roots.append(QFileInfo(QStringLiteral("/storage/emulated/0")));
    // Copies of USB sticks, read as if the stick was plugged in
    roots += QDir(kDriveCopiesFolder).entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
#ifdef __ANDROID_USB_DRIVE__
    // Drives Android didn't mount, read by Mixxx itself
    roots += mixxx::usbdrive::driveRoots();
#endif
#elif defined(__WINDOWS__)
    const auto volumes = QStorageInfo::mountedVolumes();
    for (const QStorageInfo& volume : volumes) {
        if (volume.isValid() && volume.isReady()) {
            roots.append(QFileInfo(volume.rootPath()));
        }
    }
#elif defined(__APPLE__)
    roots = QDir(QStringLiteral("/Volumes")).entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
#else
    const QString user = QString::fromLocal8Bit(qgetenv("USER"));
    roots += QDir(QStringLiteral("/media")).entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
    roots += QDir(QStringLiteral("/media/") + user)
                     .entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
    roots += QDir(QStringLiteral("/run/media/") + user)
                     .entryInfoList(QDir::AllDirs | QDir::NoDotAndDotDot);
#endif
    return roots;
}

QString removableStorageLabel(const QFileInfo& root) {
#if defined(__ANDROID__)
    if (root.absolutePath() == kDriveCopiesFolder) {
        return QCoreApplication::translate("RemovableStorage", "%1 (backup)").arg(root.fileName());
    }
#endif
    return root.fileName();
}

#if defined(__ANDROID__)
QList<AndroidStorageVolume> androidStorageVolumes() {
    QList<AndroidStorageVolume> volumes;
    const QJniObject lines = QJniObject::callStaticMethod<jobjectArray>(
            "org/mixxx/StorageHelper",
            "volumes",
            "(Landroid/content/Context;)[Ljava/lang/String;",
            QNativeInterface::QAndroidApplication::context().object());
    QJniEnvironment env;
    if (!lines.isValid()) {
        env.checkAndClearExceptions();
        return volumes;
    }
    const auto array = lines.object<jobjectArray>();
    const jsize count = env->GetArrayLength(array);
    for (jsize i = 0; i < count; ++i) {
        const QJniObject line = QJniObject::fromLocalRef(env->GetObjectArrayElement(array, i));
        const QStringList fields = line.toString().split(QChar('\t'));
        if (fields.size() < 3 || fields.at(0).isEmpty()) {
            continue;
        }
        AndroidStorageVolume volume;
        volume.path = fields.at(0);
        volume.name = fields.at(1);
        if (fields.at(2) == QLatin1String("internal")) {
            volume.kind = AndroidStorageVolume::Kind::Internal;
        } else if (fields.at(2) == QLatin1String("sd")) {
            volume.kind = AndroidStorageVolume::Kind::SdCard;
        } else {
            volume.kind = AndroidStorageVolume::Kind::UsbDrive;
        }
        volumes.append(volume);
    }
    return volumes;
}
#endif

} // namespace mixxx
