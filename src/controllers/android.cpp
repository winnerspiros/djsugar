#include "android.h"

#include <android/api-level.h>
#include <android/log.h>
#include <qjnitypes.h>

#include <QtJniTypes>
#include <cstddef>
#include <functional>
#include <map>
#include <optional>

#include <QCoreApplication>
#include <QMetaObject>
#include <QDebug>
#include <QSemaphore>
#include <memory>

namespace mixxx {
namespace android {
std::mutex s_androidLock = {};
std::condition_variable s_grantingWaitCond = {};
std::vector<std::pair<QJniObject, bool>> s_grantingResult = {};
QJniObject s_intent = {};
QJniObject s_usbManager = {};

namespace {

/// UsbDevice objects received from a broadcast are different Java objects
/// than the ones returned by UsbManager.getDeviceList(), so devices are
/// identified by their device name (e.g. /dev/bus/usb/001/002).
QString usbDeviceName(const QJniObject& device) {
    return device.callMethod<jstring>("getDeviceName").toString();
}

} // namespace

const QJniObject& getIntent() {
    __android_log_print(ANDROID_LOG_VERBOSE, "mixxx", "about to get intent");
    std::unique_lock lock(s_androidLock);
    if (s_intent.isValid()) {
        return s_intent;
    }
    // QNativeInterface::QAndroidApplication::runOnAndroidMainThread([]() {
    if (!QNativeInterface::QAndroidApplication::isActivityContext()) {
        __android_log_print(ANDROID_LOG_WARN,
                "mixxx",
                "current context doesn't refer to an activity!");
    }

    QJniObject context = QNativeInterface::QAndroidApplication::context();

    s_usbManager = QJniObject("org/mixxx/UsbPermission");
    // UsbManager fills in EXTRA_DEVICE and EXTRA_PERMISSION_GRANTED, so the
    // PendingIntent must be mutable. Since Android 12 (API 31) this has to be
    // requested explicitly with FLAG_MUTABLE, and since Android 14 a mutable
    // PendingIntent must use an explicit Intent (setPackage).
    jint apiLevel = QJniObject::getStaticField<jint>(
            "android/os/Build$VERSION", "SDK_INT");
    jint pendingIntentFlag = 0;
    if (apiLevel >= 34) {
        pendingIntentFlag = QJniObject::getStaticField<jint>(
                "android/app/PendingIntent", "FLAG_IMMUTABLE");
    } else if (apiLevel >= 31) {
        pendingIntentFlag = QJniObject::getStaticField<jint>(
                "android/app/PendingIntent", "FLAG_MUTABLE");
    }
    QtJniTypes::String ACTION_USB_PERMISSION =
            QJniObject::fromString("org.mixxx.permissions.USB_PERMISSION");
    QJniObject intent = QJniObject("android/content/Intent",
            "(Ljava/lang/String;)V",
            ACTION_USB_PERMISSION.object<jstring>());
    if (intent.isValid()) {
        QJniObject packageName = context.callObjectMethod<jstring>("getPackageName");
        intent.callObjectMethod("setPackage",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                packageName.object<jstring>());
    }
    if (!intent.isValid()) {
        __android_log_print(ANDROID_LOG_WARN, "mixxx", "pending intent is invalid!");
    }
    s_intent =
            QJniObject::callStaticMethod<jobject>("android/app/PendingIntent",
                    "getBroadcast",
                    "(Landroid/content/Context;ILandroid/content/"
                    "Intent;I)Landroid/app/PendingIntent;",
                    context,
                    0,
                    intent.object(),
                    pendingIntentFlag);

    if (!s_intent.isValid()) {
        __android_log_print(ANDROID_LOG_WARN, "mixxx", "pending intent is invalid!");
    }

    __android_log_print(ANDROID_LOG_VERBOSE,
            "mixxx",
            "about to register the the receiver %d",
            s_usbManager.isValid());
    auto success = s_usbManager.callMethod<jboolean>("registerServiceBroadcastReceiver",
            "(Landroid/content/Context;)Z",
            context.object<jobject>());
    if (!success) {
        __android_log_print(ANDROID_LOG_WARN, "mixxx", "failed to registered the receiver!");
    }
    // });
    return s_intent;
}

bool waitForPermission(const QJniObject& device) {
    __android_log_print(ANDROID_LOG_VERBOSE, "mixxx", "about to wait for perm");
    const QString deviceName = usbDeviceName(device);
    // Leave enough time for the user to read and answer the system dialog.
    // Note: the event loop must not be run here, otherwise a queued rescan
    // would re-enter the controller setup while we hold the lock.
    constexpr auto kTimeout = std::chrono::seconds(30);

    std::unique_lock lock(s_androidLock);
    std::optional<bool> granted;
    const bool gotResult = s_grantingWaitCond.wait_for(lock, kTimeout, [&] {
        auto it = std::find_if(s_grantingResult.begin(),
                s_grantingResult.end(),
                [&deviceName](const auto& resultPair) {
                    return usbDeviceName(resultPair.first) == deviceName;
                });
        if (it == s_grantingResult.end()) {
            return false;
        }
        granted = it->second;
        s_grantingResult.erase(it);
        return true;
    });
    if (!gotResult) {
        __android_log_print(ANDROID_LOG_WARN, "mixxx", "wait for perm timeout");
        qWarning() << "Timeout reached when waiting for Android permission to USB device";
        return false;
    }
    __android_log_print(ANDROID_LOG_VERBOSE, "mixxx", "got perm result");
    return granted.value_or(false);
}

void usbDeviceAccessResult(QJniObject device, bool granted) {
    {
        std::unique_lock lock(s_androidLock);
        __android_log_print(ANDROID_LOG_WARN, "mixxx", "received permission result: %d", granted);
        // Results nobody waits for (permission requested on hotplug) must not
        // pile up
        if (s_grantingResult.size() > 16) {
            s_grantingResult.erase(s_grantingResult.begin());
        }
        s_grantingResult.push_back(std::make_pair<>(device, granted));
        s_grantingWaitCond.notify_all();
    }
    if (granted) {
        // Lets devices whose permission was requested asynchronously be set up
        notifyDevicesChanged();
    }
}

namespace {

std::mutex s_devicesChangedLock;
std::map<int, std::function<void()>> s_devicesChangedListeners;
int s_nextListenerId = 1;

std::mutex s_saveBeforeExitLock;
std::function<void()> s_saveBeforeExit;
// Long enough to write the settings and a few tracks, short enough not to
// hang the closing app if the main thread is blocked.
constexpr int kSaveBeforeExitTimeoutMillis = 4000;

} // namespace

int addDevicesChangedListener(std::function<void()> callback) {
    std::lock_guard lock(s_devicesChangedLock);
    const int id = s_nextListenerId++;
    s_devicesChangedListeners.emplace(id, std::move(callback));
    return id;
}

void removeDevicesChangedListener(int id) {
    std::lock_guard lock(s_devicesChangedLock);
    s_devicesChangedListeners.erase(id);
}

void notifyDevicesChanged() {
    std::lock_guard lock(s_devicesChangedLock);
    for (const auto& [id, callback] : s_devicesChangedListeners) {
        callback();
    }
}

void setSaveBeforeExit(std::function<void()> save) {
    std::lock_guard lock(s_saveBeforeExitLock);
    s_saveBeforeExit = std::move(save);
}

void saveBeforeExit() {
    auto* pApp = QCoreApplication::instance();
    if (!pApp) {
        return;
    }
    auto pDone = std::make_shared<QSemaphore>();
    QMetaObject::invokeMethod(
            pApp,
            [pDone]() {
                std::function<void()> save;
                {
                    std::lock_guard lock(s_saveBeforeExitLock);
                    save = s_saveBeforeExit;
                }
                if (save) {
                    save();
                }
                pDone->release();
            },
            Qt::QueuedConnection);
    if (!pDone->tryAcquire(1, kSaveBeforeExitTimeoutMillis)) {
        qWarning() << "Saving before exit timed out";
    }
}
} // namespace android
} // namespace mixxx

Q_DECLARE_JNI_CLASS(UsbPermissionClass, "org/mixxx/UsbPermission")
Q_DECLARE_JNI_CLASS(MidiHelperClass, "org/mixxx/MidiHelper")
Q_DECLARE_JNI_CLASS(MainActivityClass, "org/mixxx/MainActivity")
Q_DECLARE_JNI_CLASS(AndroidMidiHelperClass, "org/mixxx/AndroidMidiHelper")

void usbDeviceAccessResult(JNIEnv*, jobject, jobject device, jboolean granted) {
    mixxx::android::usbDeviceAccessResult(device, granted);
}
Q_DECLARE_JNI_NATIVE_METHOD(usbDeviceAccessResult)

void usbDevicesChanged(JNIEnv*, jclass) {
    mixxx::android::notifyDevicesChanged();
}
Q_DECLARE_JNI_NATIVE_METHOD(usbDevicesChanged)

void midiDevicesChanged(JNIEnv*, jclass) {
    mixxx::android::notifyDevicesChanged();
}
Q_DECLARE_JNI_NATIVE_METHOD(midiDevicesChanged)

void saveBeforeExit(JNIEnv*, jclass) {
    mixxx::android::saveBeforeExit();
}
Q_DECLARE_JNI_NATIVE_METHOD(saveBeforeExit)

// Native callback from AndroidMidiHelper.midiReceive()
static void midiReceive(JNIEnv*,
        jobject,
        jint controllerId,
        jbyteArray data,
        jint offset,
        jint count,
        jlong timestamp) {
    // Forward to the active AndroidMidiController
    // TODO: implement controller lookup by ID
    Q_UNUSED(controllerId);
    Q_UNUSED(data);
    Q_UNUSED(offset);
    Q_UNUSED(count);
    Q_UNUSED(timestamp);
}
Q_DECLARE_JNI_NATIVE_METHOD(midiReceive)

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM*, void*) {
    QJniEnvironment env;
    env.registerNativeMethods<QtJniTypes::UsbPermissionClass>({
            Q_JNI_NATIVE_METHOD(usbDeviceAccessResult),
            Q_JNI_NATIVE_METHOD(usbDevicesChanged),
    });
    env.registerNativeMethods<QtJniTypes::MidiHelperClass>({
            Q_JNI_NATIVE_METHOD(midiDevicesChanged),
    });
    env.registerNativeMethods<QtJniTypes::MainActivityClass>({
            Q_JNI_NATIVE_METHOD(saveBeforeExit),
    });
    env.registerNativeMethods<QtJniTypes::AndroidMidiHelperClass>({
            Q_JNI_NATIVE_METHOD(midiReceive),
    });
    return JNI_VERSION_1_6;
}
