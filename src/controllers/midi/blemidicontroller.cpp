#include "controllers/midi/blemidicontroller.h"

#ifdef Q_OS_ANDROID

#include <jni.h>

#include <QJniEnvironment>
#include <QJniObject>
#include <QMutexLocker>

#include "controllers/android.h"
#include "controllers/midi/blemidienumerator.h"
#include "moc_blemidicontroller.cpp"
#include "util/logger.h"
#include "util/time.h"

namespace {
const mixxx::Logger kLogger("BleMidiController");
constexpr const char* kBleScannerClass = "org/mixxx/BleMidiScanner";
} // namespace

QMutex BleMidiController::s_instancesMutex;
QMap<QString, BleMidiController*> BleMidiController::s_instances;

BleMidiController::BleMidiController(const QString& deviceName,
        const QString& deviceAddress)
        : AndroidMidiControllerBase(deviceName),
          m_deviceAddress(deviceAddress),
          m_productString(deviceName),
          m_serialNumber(deviceAddress),
          m_connected(false) {
    setInputDevice(true);
    setOutputDevice(true);
    QMutexLocker locker(&s_instancesMutex);
    s_instances.insert(m_deviceAddress, this);
}

BleMidiController::~BleMidiController() {
    {
        QMutexLocker locker(&s_instancesMutex);
        if (s_instances.value(m_deviceAddress) == this) {
            s_instances.remove(m_deviceAddress);
        }
    }
    if (isOpen()) {
        close();
    }
}

bool BleMidiController::isPolling() const {
    return false;
}

int BleMidiController::open(const QString& resourcePath) {
    if (isOpen()) {
        kLogger.info() << "BLE MIDI device" << getName() << "already open";
        return -1;
    }
    kLogger.info() << "Opening BLE MIDI device" << getName()
                   << "address:" << m_deviceAddress;

    {
        QMutexLocker locker(&s_instancesMutex);
        s_instances.insert(m_deviceAddress, this);
    }

    connectDevice();
    resetParser();

    setOpen(true);
    startEngine();
    applyMapping(resourcePath);
    return 0;
}

int BleMidiController::close() {
    if (!isOpen()) {
        return 0;
    }
    kLogger.info() << "Closing BLE MIDI device" << getName();

    stopEngine();
    MidiController::close();
    m_connected = false;
    setOpen(false);
    return 0;
}

PhysicalTransportProtocol BleMidiController::getPhysicalTransportProtocol() const {
    return PhysicalTransportProtocol::BlueTooth;
}

QString BleMidiController::getVendorString() const {
    return m_vendorString;
}

QString BleMidiController::getProductString() const {
    return m_productString;
}

std::optional<uint16_t> BleMidiController::getVendorId() const {
    return std::nullopt;
}

std::optional<uint16_t> BleMidiController::getProductId() const {
    return std::nullopt;
}

QString BleMidiController::getSerialNumber() const {
    return m_serialNumber;
}

std::optional<uint8_t> BleMidiController::getUsbInterfaceNumber() const {
    return std::nullopt;
}

bool BleMidiController::connectDevice() {
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        return false;
    }
    QJniObject addressObj = QJniObject::fromString(m_deviceAddress);
    QJniObject nameObj = QJniObject::fromString(getName());

    jboolean result = QJniObject::callStaticMethod<jboolean>(
            kBleScannerClass,
            "connectDevice",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Z",
            context.object<jobject>(),
            addressObj.object<jstring>(),
            nameObj.object<jstring>());

    m_connected = result;
    return result;
}

void BleMidiController::disconnectDevice() {
    QJniObject addressObj = QJniObject::fromString(m_deviceAddress);
    QJniObject::callStaticMethod<void>(
            kBleScannerClass,
            "disconnectDevice",
            "(Ljava/lang/String;)V",
            addressObj.object<jstring>());
    m_connected = false;
}

// static
void BleMidiController::dispatchMidiData(
        const QString& address, const QByteArray& data) {
    if (data.isEmpty()) {
        return;
    }
    BleMidiController* pController = nullptr;
    {
        QMutexLocker locker(&s_instancesMutex);
        pController = s_instances.value(address, nullptr);
        if (!pController && !s_instances.isEmpty()) {
            pController = s_instances.first();
        }
    }
    if (!pController || !pController->isOpen()) {
        return;
    }
    pController->parseBytes(
            reinterpret_cast<const uint8_t*>(data.constData()),
            static_cast<size_t>(data.size()),
            mixxx::Time::elapsed());
}

bool BleMidiController::poll() {
    return false;
}

void BleMidiController::sendShortMsg(unsigned char status,
        unsigned char byte1,
        unsigned char byte2) {
    QByteArray raw;
    if (status >= 0xF8) {
        raw.append(static_cast<char>(status));
    } else if ((status & 0xF0) == 0xC0 || (status & 0xF0) == 0xD0 ||
            status == 0xF1 || status == 0xF3) {
        raw.append(static_cast<char>(status));
        raw.append(static_cast<char>(byte1));
    } else {
        raw.append(static_cast<char>(status));
        raw.append(static_cast<char>(byte1));
        raw.append(static_cast<char>(byte2));
    }
    sendBytes(raw);
}

bool BleMidiController::sendBytes(const QByteArray& data) {
    if (data.isEmpty()) {
        return false;
    }
    QJniObject jAddress = QJniObject::fromString(m_deviceAddress);
    QJniObject jData = QJniObject::fromString(QString::fromLatin1(data.toHex()));
    QJniObject::callStaticMethod<void>(
            kBleScannerClass,
            "writeMidiData",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            jAddress.object<jstring>(),
            jData.object<jstring>());
    return true;
}

extern "C" {

JNIEXPORT void JNICALL __attribute__((used, visibility("default")))
Java_org_mixxx_BleMidiController_nativeOnMidiDataReceived(
        JNIEnv* env, jclass, jstring address, jbyteArray data) {
    if (!env || !data) {
        return;
    }
    const QString addr = address ? QJniObject(address).toString() : QString();
    const jsize len = env->GetArrayLength(data);
    if (len <= 0) {
        return;
    }
    QByteArray bytes(len, Qt::Uninitialized);
    env->GetByteArrayRegion(
            data, 0, len, reinterpret_cast<jbyte*>(bytes.data()));
    BleMidiController::dispatchMidiData(addr, bytes);
}

JNIEXPORT void JNICALL __attribute__((used, visibility("default")))
Java_org_mixxx_BleMidiController_nativeOnDeviceConnected(
        JNIEnv*, jclass, jstring address, jstring name) {
    const QString addr = address ? QJniObject(address).toString() : QString();
    const QString devName = name ? QJniObject(name).toString() : QStringLiteral("BLE MIDI");
    BleMidiEnumerator::notifyBleDeviceConnected(addr, devName);
    mixxx::android::notifyDevicesChanged();
}

JNIEXPORT void JNICALL __attribute__((used, visibility("default")))
Java_org_mixxx_BleMidiController_nativeOnDeviceDisconnected(
        JNIEnv*, jclass, jstring address) {
    const QString addr = address ? QJniObject(address).toString() : QString();
    BleMidiEnumerator::notifyBleDeviceDisconnected(addr);
    mixxx::android::notifyDevicesChanged();
}

} // extern "C"

#endif // Q_OS_ANDROID
