#include "controllers/midi/androidusbmidicontroller.h"

#ifdef Q_OS_ANDROID
#include <unistd.h>

#include <QJniEnvironment>
#include <QJniObject>

#include "util/logger.h"

namespace {

const mixxx::Logger kLogger("AndroidUsbMidiController");
constexpr int kBufferSize = 64; // USB MIDI event packet size
constexpr int kPollTimeoutMs = 10;

} // namespace

// --- MidiIoThread ---

AndroidUsbMidiController::MidiIoThread::MidiIoThread(
        AndroidUsbMidiController* parent)
        : QThread(parent),
          m_parent(parent) {
    m_stopRequested.storeRelaxed(0);
}

void AndroidUsbMidiController::MidiIoThread::stop() {
    m_stopRequested.storeRelaxed(1);
}

void AndroidUsbMidiController::MidiIoThread::setAndroidDevice(
        QJniObject&& usbDevice,
        QJniObject&& usbConnection,
        QJniObject&& bulkInEndpoint,
        QJniObject&& bulkOutEndpoint) {
    m_usbDevice = std::move(usbDevice);
    m_usbConnection = std::move(usbConnection);
    m_bulkInEndpoint = std::move(bulkInEndpoint);
    m_bulkOutEndpoint = std::move(bulkOutEndpoint);
}

void AndroidUsbMidiController::MidiIoThread::run() {
    if (!m_usbConnection.isValid() || !m_bulkInEndpoint.isValid()) {
        kLogger.warning() << "MIDI IO thread: no valid connection or endpoint";
        return;
    }

    QJniEnvironment env;
    jbyteArray byteArray = env->NewByteArray(kBufferSize);

    // Verify the connection is valid for I/O
    int fd = m_usbConnection.callMethod<jint>("getFileDescriptor");
    kLogger.info() << "MIDI IO thread started, FD=" << fd;

    while (!m_stopRequested.loadRelaxed()) {
        // Read MIDI data from the bulk IN endpoint
        auto bytesRead = m_usbConnection.callMethod<jint>(
                "bulkTransfer",
                "(Landroid/hardware/usb/UsbEndpoint;[BIII)I",
                m_bulkInEndpoint.object(),
                byteArray,
                0,
                kBufferSize,
                kPollTimeoutMs);

        if (bytesRead > 0) {
            // Convert jbyteArray to QByteArray
            jbyte* elements = env->GetByteArrayElements(byteArray, nullptr);
            if (elements) {
                QByteArray midiData(
                        reinterpret_cast<const char*>(elements),
                        bytesRead);
                env->ReleaseByteArrayElements(byteArray, elements, JNI_ABORT);

                // Process MIDI data
                // USB MIDI event packets are 4 bytes each:
                // [cin(b4) | cable(b4)] [byte1] [byte2] [byte3]
                // CIN (Code Index Number) identifies the message type
                for (int i = 0; i + 3 < bytesRead; i += 4) {
                    unsigned char cin = static_cast<unsigned char>(
                            elements[i] & 0x0F);
                    unsigned char midiStatus = static_cast<unsigned char>(
                            elements[i + 1]);
                    unsigned char midiByte1 = static_cast<unsigned char>(
                            elements[i + 2]);
                    unsigned char midiByte2 = static_cast<unsigned char>(
                            elements[i + 3]);

                    // Strip USB framing — pass raw MIDI bytes
                    QByteArray rawMidi;
                    rawMidi.append(static_cast<char>(midiStatus));
                    if (cin >= 0x2 && cin <= 0x6) {
                        rawMidi.append(static_cast<char>(midiByte1));
                    }
                    if (cin == 0x3 || cin == 0x6) {
                        rawMidi.append(static_cast<char>(midiByte2));
                    }

                    if (!rawMidi.isEmpty()) {
                        m_parent->receive(rawMidi, mixxx::Duration());
                    }
                }
            }
        } else if (bytesRead < 0) {
            // bulkTransfer error (e.g. -1 if interface not claimed)
            // Sleep briefly to avoid busy-looping on persistent errors
            usleep(10000); // 10ms
        }
        // bytesRead == 0 is a timeout — loop back and try again
    }

    env->DeleteLocalRef(byteArray);
    kLogger.info() << "MIDI IO thread finished";
}

// --- AndroidUsbMidiController ---

AndroidUsbMidiController::AndroidUsbMidiController(
        const QString& name,
        int vendorId,
        int productId,
        const QString& vendorString,
        const QString& productString)
        : MidiController(name),
          m_pIoThread(nullptr),
          m_vendorId(vendorId),
          m_productId(productId),
          m_vendorString(vendorString),
          m_productString(productString) {
}

AndroidUsbMidiController::~AndroidUsbMidiController() {
    if (m_pIoThread) {
        m_pIoThread->stop();
        m_pIoThread->wait(1000);
        delete m_pIoThread;
    }
}

void AndroidUsbMidiController::setAndroidDevice(
        QJniObject&& usbDevice,
        QJniObject&& usbConnection,
        QJniObject&& bulkInEndpoint,
        QJniObject&& bulkOutEndpoint) {
    m_pIoThread = new MidiIoThread(this);
    m_pIoThread->setAndroidDevice(
            std::move(usbDevice),
            std::move(usbConnection),
            std::move(bulkInEndpoint),
            std::move(bulkOutEndpoint));
}

bool AndroidUsbMidiController::isPolling() const {
    return false;
}

int AndroidUsbMidiController::open(const QString& resourcePath) {
    Q_UNUSED(resourcePath);
    kLogger.info() << "Opening Android USB MIDI device" << getName();

    if (!m_pIoThread) {
        kLogger.warning() << "No IO thread — setAndroidDevice not called";
        return -1;
    }

    // Reset stop flag before starting (may be 1 from prior stop())
    m_pIoThread->resetStopFlag();
    m_pIoThread->start();
    startEngine();
    setOpen(true);
    return 0;
}

int AndroidUsbMidiController::close() {
    kLogger.info() << "Closing Android USB MIDI device" << getName();

    if (m_pIoThread) {
        m_pIoThread->stop();
        m_pIoThread->wait(1000);
    }

    stopEngine();
    setOpen(false);
    return 0;
}

PhysicalTransportProtocol AndroidUsbMidiController::
        getPhysicalTransportProtocol() const {
    return PhysicalTransportProtocol::USB;
}

QString AndroidUsbMidiController::getVendorString() const {
    return m_vendorString;
}

QString AndroidUsbMidiController::getProductString() const {
    return m_productString;
}

std::optional<uint16_t> AndroidUsbMidiController::getVendorId() const {
    return static_cast<uint16_t>(m_vendorId);
}

std::optional<uint16_t> AndroidUsbMidiController::getProductId() const {
    return static_cast<uint16_t>(m_productId);
}

QString AndroidUsbMidiController::getSerialNumber() const {
    return {};
}

std::optional<uint8_t> AndroidUsbMidiController::getUsbInterfaceNumber()
        const {
    return std::nullopt;
}

bool AndroidUsbMidiController::poll() {
    return false;
}

void AndroidUsbMidiController::sendShortMsg(unsigned char status,
        unsigned char byte1,
        unsigned char byte2) {
    QByteArray data;
    data.append(static_cast<char>(status));
    data.append(static_cast<char>(byte1));
    data.append(static_cast<char>(byte2));
    sendBytes(data);
}

bool AndroidUsbMidiController::sendBytes(const QByteArray& data) {
    if (!m_pIoThread || data.isEmpty())
        return false;

    // Wrap raw MIDI bytes in USB MIDI event packet
    // Format: [cin(b4) | cable(b4)] [status] [byte1] [byte2]
    // CIN 0x3 = 3-byte note/control message
    QJniEnvironment env;
    jbyteArray jData = env->NewByteArray(4);
    jbyte packet[4] = {0x03, 0, 0, 0};
    packet[1] = static_cast<jbyte>(data[0]);
    if (data.size() > 1)
        packet[2] = static_cast<jbyte>(data[1]);
    if (data.size() > 2)
        packet[3] = static_cast<jbyte>(data[2]);
    env->SetByteArrayRegion(jData, 0, 4, packet);

    if (m_pIoThread->m_bulkOutEndpoint.isValid()) {
        m_pIoThread->m_usbConnection.callMethod<jint>(
                "bulkTransfer",
                "(Landroid/hardware/usb/UsbEndpoint;[BIII)I",
                m_pIoThread->m_bulkOutEndpoint.object(),
                jData,
                0,
                4,
                10);
    }

    env->DeleteLocalRef(jData);
    return true;
}

#else // !Q_OS_ANDROID

AndroidUsbMidiController::AndroidUsbMidiController(
        const QString& name,
        int vendorId,
        int productId,
        const QString& vendorString,
        const QString& productString)
        : MidiController(name),
          m_vendorId(vendorId),
          m_productId(productId),
          m_vendorString(vendorString),
          m_productString(productString) {
}

AndroidUsbMidiController::~AndroidUsbMidiController() = default;

bool AndroidUsbMidiController::isPolling() const {
    return false;
}
int AndroidUsbMidiController::open(const QString&) {
    return -1;
}
int AndroidUsbMidiController::close() {
    return 0;
}
PhysicalTransportProtocol AndroidUsbMidiController::getPhysicalTransportProtocol() const {
    return PhysicalTransportProtocol::USB;
}
QString AndroidUsbMidiController::getVendorString() const {
    return m_vendorString;
}
QString AndroidUsbMidiController::getProductString() const {
    return m_productString;
}
std::optional<uint16_t> AndroidUsbMidiController::getVendorId() const {
    return static_cast<uint16_t>(m_vendorId);
}
std::optional<uint16_t> AndroidUsbMidiController::getProductId() const {
    return static_cast<uint16_t>(m_productId);
}
QString AndroidUsbMidiController::getSerialNumber() const {
    return {};
}
std::optional<uint8_t> AndroidUsbMidiController::getUsbInterfaceNumber() const {
    return std::nullopt;
}
bool AndroidUsbMidiController::poll() {
    return false;
}
void AndroidUsbMidiController::sendShortMsg(unsigned char, unsigned char, unsigned char) {
}
bool AndroidUsbMidiController::sendBytes(const QByteArray&) {
    return false;
}

#endif
