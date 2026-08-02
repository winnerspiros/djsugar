#include "controllers/midi/androidusbmidicontroller.h"

#ifdef Q_OS_ANDROID
#include <hidapi_libusb.h>
#include <libusb.h>
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
    m_usbHandle = nullptr;
}

void AndroidUsbMidiController::MidiIoThread::stop() {
    m_stopRequested.storeRelaxed(1);
}

void AndroidUsbMidiController::MidiIoThread::setAndroidDevice(
        QJniObject&& usbDevice,
        jint fd,
        jint interfaceNumber,
        uint8_t bulkInEndpoint,
        uint8_t bulkOutEndpoint,
        uint16_t vendorId,
        uint16_t productId) {
    m_usbDevice = std::move(usbDevice);
    m_usbFd = fd;
    m_interfaceNumber = interfaceNumber;
    m_bulkInEpAddress = bulkInEndpoint;
    m_bulkOutEpAddress = bulkOutEndpoint;
    m_vendorId = vendorId;
    m_productId = productId;
}

void AndroidUsbMidiController::MidiIoThread::run() {
    if (m_usbFd < 0) {
        kLogger.warning() << "MIDI IO thread: no valid file descriptor";
        return;
    }

    // Initialize libusb context
    libusb_context* ctx = nullptr;
    int rc = libusb_init(&ctx);
    if (rc != LIBUSB_SUCCESS) {
        kLogger.warning() << "MIDI IO thread: libusb_init failed:" << rc;
        return;
    }

    // Wrap the file descriptor that was already opened via JNI's
    // UsbManager.openDevice(). On Android, we cannot call
    // libusb_open_device_with_vid_pid because native libusb open
    // requires root or a udev rule — only the Java USB API can
    // open USB devices in unprivileged apps. Wrapping the FD
    // reuses the already-opened kernel handle from JNI.
    bool useLibusb = false;
    rc = libusb_wrap_sys_device(ctx,
            static_cast<intptr_t>(m_usbFd),
            &m_usbHandle);
    if (rc != LIBUSB_SUCCESS || !m_usbHandle) {
        kLogger.warning()
                << "MIDI IO thread: libusb_wrap_sys_device failed (rc="
                << rc << ") - falling back to JNI claimInterface";
        libusb_exit(ctx);
    } else {
        // Primary path: libusb bulk_transfer
        // Detach kernel driver if present (e.g., audio subsystem owns it)
        int rcd = libusb_detach_kernel_driver(m_usbHandle, m_interfaceNumber);
        if (rcd != LIBUSB_SUCCESS && rcd != LIBUSB_ERROR_NOT_FOUND) {
            // LIBUSB_ERROR_NOT_FOUND means no kernel driver to detach — fine
            kLogger.warning()
                    << "MIDI IO thread: libusb_detach_kernel_driver for iface"
                    << m_interfaceNumber << ":" << rcd;
        }

        // Claim the MIDI interface
        rc = libusb_claim_interface(m_usbHandle, m_interfaceNumber);
        if (rc != LIBUSB_SUCCESS) {
            kLogger.warning()
                    << "MIDI IO thread: libusb_claim_interface failed for iface"
                    << m_interfaceNumber << ":" << rc
                    << "- audio subsystem likely owns it, falling back to JNI";
            libusb_close(m_usbHandle);
            m_usbHandle = nullptr;
            libusb_exit(ctx);
        } else {
            useLibusb = true;
        }
    }

    if (useLibusb) {
        kLogger.info()
                << "MIDI IO thread started (libusb), FD=" << m_usbFd
                << "iface=" << m_interfaceNumber
                << "bulkIn=0x" << Qt::hex << m_bulkInEpAddress
                << "bulkOut=0x" << m_bulkOutEpAddress;

        // Send Pioneer DDJ-FLX4 init SysEx to put controller in MIDI mode.
        // Required for the controller to start sending MIDI data on the
        // streaming interface. Without this, no input events arrive.
        if (m_vendorId == 0x2b73 || m_vendorId == 0x08e4) {
            // Wrap Pioneer init SysEx in 4 USB MIDI SysEx packets
            struct {
                unsigned char cin;
                unsigned char d[3];
            } sysexPkts[] = {
                    {0x04, {0xF0, 0x00, 0x40}}, // start
                    {0x04, {0x05, 0x00, 0x00}}, // continue
                    {0x04, {0x04, 0x05, 0x00}}, // continue
                    {0x07, {0x50, 0x02, 0xF7}}, // end (3 bytes)
            };
            for (const auto& pkt : sysexPkts) {
                unsigned char buf[4] = {pkt.cin, pkt.d[0], pkt.d[1], pkt.d[2]};
                int transferred = 0;
                libusb_bulk_transfer(m_usbHandle, m_bulkOutEpAddress, buf, 4, &transferred, 100);
            }
            kLogger.info() << "Sent Pioneer init SysEx to iface"
                           << m_interfaceNumber;
        }

        unsigned char buffer[kBufferSize];

        while (!m_stopRequested.loadRelaxed()) {
            int transferred = 0;
            rc = libusb_bulk_transfer(m_usbHandle,
                    m_bulkInEpAddress,
                    buffer,
                    kBufferSize,
                    &transferred,
                    kPollTimeoutMs);

            if (rc == LIBUSB_SUCCESS && transferred > 0) {
                QByteArray midiData(
                        reinterpret_cast<const char*>(buffer), transferred);
                kLogger.debug()
                        << "MIDI IO thread: libusb_bulk_transfer returned"
                        << transferred << "bytes:" << midiData.toHex();

                // Process USB MIDI event packets (4 bytes each)
                for (int i = 0; i + 3 < transferred; i += 4) {
                    unsigned char cin = static_cast<unsigned char>(
                            static_cast<unsigned char>(buffer[i]) & 0x0F);
                    unsigned char midiStatus = buffer[i + 1];
                    unsigned char midiByte1 = buffer[i + 2];
                    unsigned char midiByte2 = buffer[i + 3];

                    QByteArray rawMidi;
                    rawMidi.append(static_cast<char>(midiStatus));
                    if (cin >= 0x2 && cin <= 0x6)
                        rawMidi.append(static_cast<char>(midiByte1));
                    if (cin == 0x3 || cin == 0x6)
                        rawMidi.append(static_cast<char>(midiByte2));

                    if (!rawMidi.isEmpty())
                        m_parent->receive(rawMidi, mixxx::Duration());
                }
            } else if (rc == LIBUSB_ERROR_TIMEOUT) {
                // Timeout is normal — no data available, loop back
                continue;
            } else if (rc < 0) {
                kLogger.warning()
                        << "MIDI IO thread: libusb_bulk_transfer error:"
                        << rc << "- sleeping 10ms";
                usleep(10000);
            }
        }

        // Clean up
        libusb_release_interface(m_usbHandle, m_interfaceNumber);
        libusb_close(m_usbHandle);
        m_usbHandle = nullptr;
        libusb_exit(ctx);

        kLogger.info() << "MIDI IO thread finished (libusb)";
    } else {
        // ── JNI fallback path ──
        // Used when libusb_wrap_sys_device or claim_interface fails.
        // On Android, the audio subsystem often claims composite USB devices
        // before we can, so this path uses JNI UsbDeviceConnection directly
        // with force-claim (passing force=true to claimInterface).
        QJniEnvironment env;
        jbyteArray byteArray = env->NewByteArray(kBufferSize);

        QJniObject context =
                QNativeInterface::QAndroidApplication::context();
        QJniObject javaUsbManager = context.callObjectMethod(
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                QJniObject::getStaticObjectField(
                        "android/content/Context",
                        "USB_SERVICE",
                        "Ljava/lang/String;")
                        .object());
        auto usbConnection = javaUsbManager.callMethod<QJniObject>(
                "openDevice",
                "(Landroid/hardware/usb/UsbDevice;)"
                "Landroid/hardware/usb/UsbDeviceConnection;",
                m_usbDevice);
        if (!usbConnection.isValid()) {
            kLogger.warning() << "MIDI IO thread: JNI openDevice failed";
            env->DeleteLocalRef(byteArray);
            return;
        }

        auto usbInterface = m_usbDevice.callObjectMethod(
                "getInterface",
                "(I)Landroid/hardware/usb/UsbInterface;",
                m_interfaceNumber);
        if (usbInterface.isValid()) {
            bool claimed = usbConnection.callMethod<jboolean>("claimInterface",
                    "(Landroid/hardware/usb/UsbInterface;Z)Z",
                    usbInterface,
                    true);
            if (!claimed) {
                kLogger.warning()
                        << "MIDI IO thread: JNI claimInterface failed"
                        << "- giving up";
                env->DeleteLocalRef(byteArray);
                return;
            }
            kLogger.info()
                    << "MIDI IO thread: JNI claimInterface succeeded"
                    << "(fallback path)";
        }

        kLogger.info() << "MIDI IO thread started (JNI fallback), FD="
                       << m_usbFd;

        // Read loop using JNI bulkTransfer
        while (!m_stopRequested.loadRelaxed()) {
            int bytesRead = usbConnection.callMethod<jint>("bulkTransfer",
                    "(Landroid/hardware/usb/UsbEndpoint;[BIII)I",
                    m_usbDevice
                            .callObjectMethod("getInterface",
                                    "(I)Landroid/hardware/usb/UsbInterface;",
                                    m_interfaceNumber)
                            .callObjectMethod("getEndpoint",
                                    "(I)Landroid/hardware/usb/UsbEndpoint;",
                                    0),
                    byteArray,
                    0,
                    kBufferSize,
                    kPollTimeoutMs);

            if (bytesRead > 0) {
                jbyte* elements = env->GetByteArrayElements(byteArray, nullptr);
                if (elements) {
                    QByteArray midiData(
                            reinterpret_cast<const char*>(elements), bytesRead);
                    kLogger.debug() << "MIDI IO thread: JNI bulkTransfer returned"
                                    << bytesRead << "bytes:" << midiData.toHex();
                    env->ReleaseByteArrayElements(byteArray, elements, JNI_ABORT);

                    for (int i = 0; i + 3 < bytesRead; i += 4) {
                        unsigned char cin = static_cast<unsigned char>(
                                static_cast<unsigned char>(midiData.at(i)) & 0x0F);
                        unsigned char midiStatus = static_cast<unsigned char>(
                                midiData.at(i + 1));
                        unsigned char midiByte1 = static_cast<unsigned char>(
                                midiData.at(i + 2));
                        unsigned char midiByte2 = static_cast<unsigned char>(
                                midiData.at(i + 3));

                        QByteArray rawMidi;
                        rawMidi.append(static_cast<char>(midiStatus));
                        if (cin >= 0x2 && cin <= 0x6)
                            rawMidi.append(static_cast<char>(midiByte1));
                        if (cin == 0x3 || cin == 0x6)
                            rawMidi.append(static_cast<char>(midiByte2));

                        if (!rawMidi.isEmpty())
                            m_parent->receive(rawMidi, mixxx::Duration());
                    }
                }
            } else if (bytesRead < 0) {
                kLogger.warning()
                        << "MIDI IO thread: JNI bulkTransfer returned"
                        << bytesRead << "- sleeping 10ms";
                usleep(10000);
            }
        }

        env->DeleteLocalRef(byteArray);
        kLogger.info() << "MIDI IO thread finished (JNI fallback)";
    }
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
        jint fd,
        jint interfaceNumber,
        uint8_t bulkInEndpoint,
        uint8_t bulkOutEndpoint,
        uint16_t vendorId,
        uint16_t productId) {
    m_pIoThread = new MidiIoThread(this);
    m_pIoThread->setAndroidDevice(
            std::move(usbDevice),
            fd,
            interfaceNumber,
            bulkInEndpoint,
            bulkOutEndpoint,
            vendorId,
            productId);
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

    // Send via libusb bulk transfer
    if (m_pIoThread->m_usbHandle && m_pIoThread->m_bulkOutEpAddress) {
        // Build USB MIDI event packets (4 bytes each)
        // For standard 3-byte MIDI: 1 packet with [CIN|CN, S, D1, D2]
        // For SysEx: split into 3-byte chunks with CIN 0x4/0x5/0x6/0x7
        const int dataLen = data.size();
        int pos = 0;

        while (pos < dataLen) {
            unsigned char packet[4] = {0, 0, 0, 0};
            unsigned char status = static_cast<unsigned char>(data[pos]);
            int remaining = dataLen - pos;

            if (status == 0xF0) {
                // SysEx — split across multiple packets
                if (remaining <= 3) {
                    // Single packet: all remaining bytes
                    packet[0] = (remaining == 1) ? 0x05 : (remaining == 2) ? 0x06
                                                                           : 0x07;
                    for (int j = 0; j < remaining && j < 3; j++)
                        packet[1 + j] = static_cast<unsigned char>(data[pos + j]);
                    pos += remaining;
                } else {
                    // Start/continue: 3 bytes with CIN 0x4
                    packet[0] = 0x04;
                    for (int j = 0; j < 3; j++)
                        packet[1 + j] = static_cast<unsigned char>(data[pos + j]);
                    pos += 3;
                }
            } else {
                // Standard channel voice / system message
                // CIN = status >> 4 for channel messages (0x8-0xE)
                unsigned char cin = (status >= 0xF0) ? 0x02
                                                     : (status >> 4);
                packet[0] = cin;
                packet[1] = status;
                if (remaining > 1)
                    packet[2] = static_cast<unsigned char>(data[pos + 1]);
                if (remaining > 2)
                    packet[3] = static_cast<unsigned char>(data[pos + 2]);
                pos += qMin(remaining, 3);
            }

            int transferred = 0;
            int rc = libusb_bulk_transfer(
                    m_pIoThread->m_usbHandle,
                    m_pIoThread->m_bulkOutEpAddress,
                    packet,
                    4,
                    &transferred,
                    10);
            if (rc != LIBUSB_SUCCESS) {
                kLogger.warning()
                        << "sendBytes: libusb_bulk_transfer error:" << rc;
                return false;
            }
        }
        return true;
    }

    return false;
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
