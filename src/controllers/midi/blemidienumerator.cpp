#include "blemidienumerator.h"

#include "controllers/controller.h"
#include "util/logger.h"

#ifdef Q_OS_ANDROID
#include <QtCore/private/qandroidextras_p.h>

#include <QJniEnvironment>
#include <QJniObject>
#endif

namespace {

const mixxx::Logger kLogger("BleMidiEnumerator");

} // namespace

BleMidiEnumerator::BleMidiEnumerator(UserSettingsPointer pConfig)
        : m_pConfig(pConfig),
          m_pScanTimer(new QTimer(this)),
          m_scanning(false) {
    m_pScanTimer->setSingleShot(true);
    m_pScanTimer->setInterval(10000);
    connect(m_pScanTimer, &QTimer::timeout, this, &BleMidiEnumerator::slotOnScanTimeout);
}

BleMidiEnumerator::~BleMidiEnumerator() {
    m_devices.clear();
}

QList<Controller*> BleMidiEnumerator::queryDevices() {
    QMutexLocker locker(&m_mutex);
    return m_devices;
}

void BleMidiEnumerator::startScan() {
#ifdef Q_OS_ANDROID
    constexpr const char* kBleMidiServiceUuid = "03B80E5A-EDE8-4B33-A751-6CE34EC4C700";
    constexpr const char* kBleScannerClass = "org/mixxx/BleMidiScanner";

    if (m_scanning) {
        return;
    }
    m_scanning = true;

    QJniEnvironment env;
    if (!env.isValid()) {
        kLogger.warning() << "BLE scan: JNI environment not available";
        m_scanning = false;
        return;
    }

    QJniObject activity = QJniObject::callStaticObjectMethod(
            "org/qtproject/qt/android/QtNative",
            "activity",
            "()Landroid/app/Activity;");
    if (!activity.isValid()) {
        kLogger.warning() << "BLE scan: activity not available";
        m_scanning = false;
        return;
    }

    // Create BleMidiScanner instance
    m_scanner = QJniObject(
            kBleScannerClass,
            "(Landroid/content/Context;)V",
            activity.object<jobject>());
    if (!m_scanner.isValid()) {
        kLogger.warning() << "BLE scan: failed to create BleMidiScanner";
        m_scanning = false;
        return;
    }

    jboolean result = m_scanner.callMethod<jboolean>(
            "startScan",
            "(Landroid/content/Context;Ljava/lang/String;)Z",
            activity.object<jobject>(),
            QJniObject::fromString(QString(kBleMidiServiceUuid)).object<jstring>());

    if (!result) {
        kLogger.info() << "BLE scan: scanner returned no results or permissions denied";
        m_scanning = false;
        return;
    }

    m_pScanTimer->start();
    kLogger.info() << "BLE scan: started for MIDI service";
#else
    Q_UNUSED(m_pConfig);
    kLogger.info() << "BLE scan: not supported on this platform";
#endif
}

void BleMidiEnumerator::rescan() {
    startScan();
}

bool BleMidiEnumerator::isConnected() const {
#ifdef Q_OS_ANDROID
    if (!m_scanner.isValid()) {
        return false;
    }
    jboolean result = m_scanner.callMethod<jboolean>("isConnected", "()Z");
    return result;
#else
    return false;
#endif
}

void BleMidiEnumerator::slotOnScanTimeout() {
#ifdef Q_OS_ANDROID
    m_scanning = false;

    if (!m_scanner.isValid()) {
        return;
    }

    QJniObject devices = m_scanner.callStaticMethod<jobject>(
            "getDiscoveredDevices",
            "()Ljava/util/List;");

    if (!devices.isValid()) {
        return;
    }

    kLogger.info() << "BLE scan: completed, checking results";
#endif
}

#include "moc_blemidienumerator.cpp"
