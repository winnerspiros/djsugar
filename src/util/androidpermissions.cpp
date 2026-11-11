#include "util/androidpermissions.h"

#include <QtCore/private/qandroidextras_p.h>
#include <android/api-level.h>

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QJniObject>
#include <QStandardPaths>

#include "util/assert.h"

namespace {

bool requestPermission(const QString& permission) {
    if (QtAndroidPrivate::checkPermission(permission).result() ==
            QtAndroidPrivate::Authorized) {
        return true;
    }
    // Qt runs main() on a dedicated thread, not on the Android UI thread, so
    // waiting for the dialog result here does not block the dialog itself.
    const auto result = QtAndroidPrivate::requestPermission(permission).result();
    if (result != QtAndroidPrivate::Authorized) {
        qWarning() << "Android permission denied:" << permission;
        return false;
    }
    return true;
}

} // namespace

namespace mixxx {
namespace android {

bool requestMusicLibraryPermissions() {
    const int apiLevel = android_get_device_api_level();
    bool granted = false;
    if (apiLevel >= 33) {
        granted = requestPermission(
                QStringLiteral("android.permission.READ_MEDIA_AUDIO"));
    } else {
        granted = requestPermission(
                QStringLiteral("android.permission.READ_EXTERNAL_STORAGE"));
    }
    if (apiLevel <= 29) {
        // Writing to shared storage is only possible with this permission on
        // legacy storage devices. A denial is not fatal, Mixxx then falls back
        // to its app-private directory.
        requestPermission(
                QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE"));
    }
    return granted;
}

bool requestMicrophonePermission() {
    return requestPermission(QStringLiteral("android.permission.RECORD_AUDIO"));
}

bool hasFullExternalStorageAccess() {
    if (android_get_device_api_level() >= 30) {
        return QJniObject::callStaticMethod<jboolean>(
                       "android/os/Environment", "isExternalStorageManager") ==
                JNI_TRUE;
    }
    return QtAndroidPrivate::checkPermission(
                   QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE"))
                   .result() == QtAndroidPrivate::Authorized;
}

void requestFullExternalStorageAccess() {
    if (android_get_device_api_level() < 30) {
        requestPermission(
                QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE"));
        return;
    }
    if (hasFullExternalStorageAccess()) {
        return;
    }
    qDebug() << "requesting ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION";
    const QJniObject action = QJniObject::getStaticObjectField(
            "android/provider/Settings",
            "ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION",
            "Ljava/lang/String;");
    QJniObject intent("android/content/Intent",
            "(Ljava/lang/String;)V",
            action.object());
    const QJniObject jniPath = QJniObject::fromString(
            QStringLiteral("package:%1").arg(QStringLiteral(ANDROID_PACKAGE_NAME)));
    const QJniObject jniUri = QJniObject::callStaticObjectMethod(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            jniPath.object<jstring>());
    intent.callObjectMethod("setData",
            "(Landroid/net/Uri;)Landroid/content/Intent;",
            jniUri.object<jobject>());
    QtAndroidPrivate::startActivity(intent, 0);
}

namespace {

// Environment.getExternalStoragePublicDirectory(): the shared folder the
// phone's own apps use. Qt's MusicLocation/DownloadLocation are the
// app-private folders under Android/data, where no user songs live.
QString publicDirectory(const char* type) {
    const QJniObject jniType = QJniObject::getStaticObjectField(
            "android/os/Environment", type, "Ljava/lang/String;");
    const QJniObject dir = QJniObject::callStaticObjectMethod(
            "android/os/Environment",
            "getExternalStoragePublicDirectory",
            "(Ljava/lang/String;)Ljava/io/File;",
            jniType.object<jstring>());
    return dir.isValid()
            ? dir.callObjectMethod("getAbsolutePath", "()Ljava/lang/String;").toString()
            : QString();
}

} // namespace

QStringList sharedAudioDirectories() {
    QStringList dirs;
    for (const char* type : {"DIRECTORY_MUSIC",
                 "DIRECTORY_DOWNLOADS",
                 "DIRECTORY_PODCASTS",
                 "DIRECTORY_RECORDINGS"}) {
        // DIRECTORY_RECORDINGS is API 31+: the field lookup fails before
        const QString path = publicDirectory(type);
        if (!path.isEmpty() && QFileInfo::exists(path)) {
            dirs.append(QDir::cleanPath(path));
        }
    }
    return dirs;
}

QString defaultMusicDirectory() {
    QString path = publicDirectory("DIRECTORY_MUSIC");
    if (path.isEmpty()) {
        path = QStringLiteral("/storage/emulated/0/Music");
    }
    QDir dir(path);
    if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
        qWarning() << "Could not create music directory" << path
                   << "- falling back to the app private directory";
        const QStringList appPaths =
                QStandardPaths::standardLocations(QStandardPaths::AppDataLocation);
        VERIFY_OR_DEBUG_ASSERT(!appPaths.isEmpty()) {
            return path;
        }
        path = appPaths.first() + QStringLiteral("/Music");
        QDir().mkpath(path);
    }
    return path;
}

} // namespace android
} // namespace mixxx
