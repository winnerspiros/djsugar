#include "controllers/midi/usbmidicontroller.h"

#ifdef Q_OS_ANDROID

#include <QtCore/private/qandroidextras_p.h>

#include <QJniEnvironment>
#include <QJniObject>

#include "controllers/midi/midioutputhandler.h"
#include "moc_usbmidicontroller.cpp"
#include "util/logger.h"

namespace {

const mixxx::Logger kLogger("UsbMidiController");
constexpr const char* kUsbMidiJavaClass = "org/mixxx/UsbMidiDevice";

// Registry of USB MIDI controllers created by the enumerator.
// Keyed by (vendorId << 16) | productId to deduplicate across queryDevices() calls.
QHash<int, UsbMidiController*> s_controllers;

int deviceKey(int vendorId, int productId) {
    return (vendorId << 16) | productId;
}

} // namespace

UsbMidiController* UsbMidiController::s_pInstance = nullptr;

UsbMidiController::UsbMidiController(const QString& deviceName,
        int vendorId,
        int productId,
        const QString& vendorString,
        const QString& productString)
        : MidiController(deviceName),
          m_vendorId(vendorId),
          m_productId(productId),
          m_deviceId(-1),
          m_vendorString(vendorString),
          m_productString(productString),
          m_connected(false) {
}

UsbMidiController::~UsbMidiController() {
    if (s_pInstance == this) {
        s_pInstance = nullptr;
    }
    int key = deviceKey(m_vendorId, m_productId);
    s_controllers.remove(key);
}

bool UsbMidiController::isPolling() const {
    return false;
}

int UsbMidiController::open(const QString& resourcePath) {
    Q_UNUSED(resourcePath);
    kLogger.info() << "Opening USB MIDI device" << getName();

    s_pInstance = this;

    // Open the device via Java
    QJniObject jDevice = QJniObject::callStaticObjectMethod(
            kUsbMidiJavaClass,
            "openDevice",
            "(II)Lorg/mixxx/UsbMidiDevice;",
            m_vendorId,
            m_productId);

    if (!jDevice.isValid()) {
        kLogger.warning() << "Failed to open USB MIDI device" << getName();
        return -1;
    }

    m_deviceId = jDevice.callMethod<jint>("getDeviceId");
    kLogger.info() << "Opened USB MIDI device id=" << m_deviceId;
    m_connected = true;

    startEngine();
    setOpen(true);
    return 0;
}

int UsbMidiController::close() {
    kLogger.info() << "Closing USB MIDI device" << getName();

    if (m_deviceId >= 0) {
        QJniObject::callStaticMethod<void>(
                kUsbMidiJavaClass,
                "closeDevice",
                "(I)V",
                m_deviceId);
        m_deviceId = -1;
    }

    if (s_pInstance == this) {
        s_pInstance = nullptr;
    }

    stopEngine();
    m_connected = false;
    setOpen(false);
    return 0;
}

PhysicalTransportProtocol UsbMidiController::getPhysicalTransportProtocol() const {
    return PhysicalTransportProtocol::USB;
}

QString UsbMidiController::getVendorString() const {
    return m_vendorString;
}

QString UsbMidiController::getProductString() const {
    return m_productString;
}

std::optional<uint16_t> UsbMidiController::getVendorId() const {
    return static_cast<uint16_t>(m_vendorId);
}

std::optional<uint16_t> UsbMidiController::getProductId() const {
    return static_cast<uint16_t>(m_productId);
}

QString UsbMidiController::getSerialNumber() const {
    return {};
}

std::optional<uint8_t> UsbMidiController::getUsbInterfaceNumber() const {
    return std::nullopt;
}

bool UsbMidiController::poll() {
    return false;
}

void UsbMidiController::sendShortMsg(unsigned char status,
        unsigned char byte1,
        unsigned char byte2) {
    if (!m_connected || m_deviceId < 0)
        return;

    QByteArray data;
    data.append(static_cast<char>(status));
    data.append(static_cast<char>(byte1));
    data.append(static_cast<char>(byte2));

    sendBytes(data);
}

bool UsbMidiController::sendBytes(const QByteArray& data) {
    if (!m_connected || m_deviceId < 0 || data.isEmpty())
        return false;

    QJniEnvironment env;
    jbyteArray jData = env->NewByteArray(data.size());
    env->SetByteArrayRegion(jData,
            0,
            data.size(),
            reinterpret_cast<const jbyte*>(data.constData()));

    jboolean result = QJniObject::callStaticMethod<jboolean>(
            kUsbMidiJavaClass,
            "sendMidiData",
            "(I[BII)Z",
            m_deviceId,
            jData,
            0,
            static_cast<jint>(data.size()));

    env->DeleteLocalRef(jData);
    return result;
}

void UsbMidiController::onMidiDataReceived(int deviceId, const QByteArray& data) {
    if (!s_pInstance || s_pInstance->m_deviceId != deviceId || data.isEmpty())
        return;

    kLogger.info() << "USB MIDI received" << data.size() << "bytes";
    s_pInstance->receive(data, mixxx::Duration());
}

void UsbMidiController::onDeviceConnected(int vendorId,
        int productId,
        const QString& manufacturer,
        const QString& product,
        int interfaceNumber) {
    Q_UNUSED(interfaceNumber);
    int key = ::deviceKey(vendorId, productId);
    if (s_controllers.contains(key))
        return; // Already created

    // Build a device name
    QString deviceName = product;
    if (deviceName.isEmpty()) {
        deviceName = QStringLiteral("USB MIDI Device");
    }

    auto* controller = new UsbMidiController(
            deviceName, vendorId, productId, manufacturer, product);

    s_controllers.insert(key, controller);
    kLogger.info() << "Created USB MIDI controller:" << deviceName
                   << QString::number(vendorId, 16)
                   << QString::number(productId, 16);
}

#endif // Q_OS_ANDROID
