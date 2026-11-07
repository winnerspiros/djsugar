#include "controllers/midi/blemidienumerator.h"

#ifdef Q_OS_ANDROID

#include <QtCore/private/qandroidextras_p.h>

#include <QJniEnvironment>
#include <QJniObject>
#include <QSet>

#include "controllers/android.h"
#include "controllers/midi/blemidicontroller.h"
#include "util/logger.h"

namespace {

const mixxx::Logger kLogger("BleMidiEnumerator");

} // namespace

QMutex BleMidiEnumerator::s_connectedMutex;
QHash<QString, QString> BleMidiEnumerator::s_connectedGattDevices;

BleMidiEnumerator::BleMidiEnumerator(UserSettingsPointer pConfig)
        : m_pConfig(pConfig),
          m_pScanTimer(new QTimer(this)),
          m_scanning(false) {
    m_pScanTimer->setSingleShot(true);
    m_pScanTimer->setInterval(5000);
    connect(m_pScanTimer, &QTimer::timeout, this, &BleMidiEnumerator::slotOnScanTimeout);
}

BleMidiEnumerator::~BleMidiEnumerator() {
    qDeleteAll(m_devices);
    m_devices.clear();
    qDeleteAll(m_removedDevices);
    m_removedDevices.clear();
    m_devicesByAddress.clear();
}

// static
void BleMidiEnumerator::notifyBleDeviceConnected(
        const QString& address, const QString& name) {
    if (address.isEmpty()) {
        return;
    }
    QMutexLocker locker(&s_connectedMutex);
    s_connectedGattDevices.insert(address, name.isEmpty() ? address : name);
}

// static
void BleMidiEnumerator::notifyBleDeviceDisconnected(const QString& address) {
    if (address.isEmpty()) {
        return;
    }
    QMutexLocker locker(&s_connectedMutex);
    s_connectedGattDevices.remove(address);
}

QList<Controller*> BleMidiEnumerator::queryDevices() {
    QHash<QString, QString> gattConnected;
    {
        QMutexLocker connLock(&s_connectedMutex);
        gattConnected = s_connectedGattDevices;
    }

    QMutexLocker locker(&m_mutex);
    QSet<QString> presentAddresses;

    for (auto it = gattConnected.constBegin(); it != gattConnected.constEnd(); ++it) {
        const QString& address = it.key();
        const QString& rawName = it.value();

        // If Android's MidiManager opened this Bluetooth device directly,
        // AndroidMidiEnumerator already exposes it as an AndroidMidiController.
        const bool handledByMidiManager = QJniObject::callStaticMethod<jboolean>(
                "org/mixxx/MidiHelper",
                "isBluetoothDeviceOpen",
                "(Ljava/lang/String;)Z",
                QJniObject::fromString(address).object<jstring>());
        if (handledByMidiManager) {
            continue;
        }

        presentAddresses.insert(address);
        if (m_devicesByAddress.contains(address)) {
            continue;
        }

        auto* pController = new BleMidiController(rawName, address);
        m_devicesByAddress.insert(address, pController);
        m_devices.push_back(pController);
        kLogger.info() << "Registered GATT BLE MIDI controller:" << rawName
                       << "at" << address;
    }

    for (auto it = m_devicesByAddress.begin(); it != m_devicesByAddress.end();) {
        if (presentAddresses.contains(it.key())) {
            ++it;
            continue;
        }
        BleMidiController* pRemoved = it.value();
        m_devices.removeAll(pRemoved);
        m_removedDevices.append(pRemoved);
        it = m_devicesByAddress.erase(it);
    }

    return m_devices;
}

void BleMidiEnumerator::startScan() {
    constexpr const char* kBleMidiServiceUuid = "03B80E5A-EDE8-4B33-A751-6CE34EC4C700";
    constexpr const char* kBleScannerClass = "org/mixxx/BleMidiScanner";

    if (m_scanning) {
        return;
    }
    m_scanning = true;

    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        kLogger.warning() << "BLE scan: Android context not available";
        m_scanning = false;
        return;
    }

    jboolean result = QJniObject::callStaticMethod<jboolean>(
            kBleScannerClass,
            "startScan",
            "(Landroid/content/Context;Ljava/lang/String;)Z",
            context.object<jobject>(),
            QJniObject::fromString(QString::fromLatin1(kBleMidiServiceUuid))
                    .object<jstring>());

    if (!result) {
        kLogger.info() << "BLE scan: scanner not started (permissions or BT off)";
        m_scanning = false;
        return;
    }

    m_pScanTimer->start();
    kLogger.info() << "BLE scan: started for MIDI service";
}

void BleMidiEnumerator::rescan() {
    startScan();
}

bool BleMidiEnumerator::isConnected() const {
    constexpr const char* kBleScannerClass = "org/mixxx/BleMidiScanner";
    return QJniObject::callStaticMethod<jboolean>(
            kBleScannerClass, "isConnected", "()Z");
}

void BleMidiEnumerator::slotOnScanTimeout() {
    constexpr const char* kBleScannerClass = "org/mixxx/BleMidiScanner";
    m_scanning = false;
    QJniObject::callStaticMethod<void>(kBleScannerClass, "stopScan", "()V");
    mixxx::android::notifyDevicesChanged();
}

#include "moc_blemidienumerator.cpp"

#endif // Q_OS_ANDROID
