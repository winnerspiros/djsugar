#ifdef __ANDROID__

#include "controllers/midi/androidmidicontroller.h"

#include <android/log.h>
#include <unistd.h>

#include <QJniEnvironment>
#if __has_include(<QNativeInterface/QAndroidApplication>)
#include <QNativeInterface/QAndroidApplication>
#endif

#include "controllers/android.h"
#include "controllers/defs_controllers.h"
#include "controllers/midi/midiutils.h"
#include "moc_androidmidicontroller.cpp"

namespace {
const mixxx::Logger kLogger("AndroidMidiController");
constexpr int kUsbMidiPacketSize = 64;
constexpr int kPollTimeoutMs = 10;
constexpr int kMidiMsgSize = 3;
} // namespace

AndroidMidiController::AndroidMidiController(const QString& name,
        const QJniObject& usbDevice,
        int interfaceNumber,
        uint16_t vendorId,
        uint16_t productId,
        const QString& vendorStr,
        const QString& productStr)
        : MidiController(name),
          m_usbDevice(usbDevice),
          m_interfaceNumber(interfaceNumber),
          m_vendorId(vendorId),
          m_productId(productId),
          m_vendor(vendorStr),
          m_product(productStr) {
}

AndroidMidiController::~AndroidMidiController() {
    close();
}

int AndroidMidiController::open(const QString& resourcePath) {
    Q_UNUSED(resourcePath);
    if (m_pIoThread && m_pIoThread->isRunning()) {
        return 0;
    }

    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        kLogger.warning() << "No Android context";
        return 1;
    }

    QJniObject USB_SERVICE =
            QJniObject::getStaticObjectField("android/content/Context",
                    "USB_SERVICE",
                    "Ljava/lang/String;");
    auto usbManager = context.callObjectMethod("getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            USB_SERVICE.object());

    if (!usbManager.isValid()) {
        kLogger.warning() << "No UsbManager";
        return 1;
    }

    // Request USB permission
    if (!usbManager.callMethod<jboolean>("hasPermission",
                "(Landroid/hardware/usb/UsbDevice;)Z",
                m_usbDevice.object())) {
        kLogger.info() << "Requesting USB permission";
        auto pendingIntent = mixxx::android::getIntent();
        usbManager.callMethod<void>("requestPermission",
                "(Landroid/hardware/usb/UsbDevice;Landroid/app/PendingIntent;)V",
                m_usbDevice.object(),
                pendingIntent.object());
        if (!mixxx::android::waitForPermission(m_usbDevice)) {
            kLogger.warning() << "USB permission denied";
            return 1;
        }
    }

    // Open USB device
    auto usbConnection = usbManager.callObjectMethod("openDevice",
            "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
            m_usbDevice.object());

    if (!usbConnection.isValid()) {
        kLogger.warning() << "Cannot open USB device";
        return 1;
    }

    // Get file descriptor
    jint usbFd = usbConnection.callMethod<jint>("getFileDescriptor");
    if (usbFd < 0) {
        kLogger.warning() << "No file descriptor";
        return 1;
    }

    // Claim the MIDI interface
    auto usbInterface = m_usbDevice.callObjectMethod("getInterface",
            "(I)Landroid/hardware/usb/UsbInterface;",
            m_interfaceNumber);
    if (usbInterface.isValid()) {
        bool claimed = usbConnection.callMethod<jboolean>("claimInterface",
                "(Landroid/hardware/usb/UsbInterface;Z)Z",
                usbInterface.object(),
                true);
        if (!claimed) {
            kLogger.warning() << "Cannot claim MIDI interface";
            return 1;
        }
        kLogger.info() << "MIDI interface" << m_interfaceNumber << "claimed";
    }

    // Start I/O thread
    m_pIoThread = new IoThread(m_usbDevice,
            usbFd,
            m_interfaceNumber,
            this);
    m_pIoThread->start();

    kLogger.info() << "Android MIDI controller opened:"
                   << getProductString() << "USB FD:" << usbFd;
    return 0;
}

int AndroidMidiController::close() {
    if (m_pIoThread) {
        m_pIoThread->stop();
        m_pIoThread->wait(2000);
        delete m_pIoThread;
        m_pIoThread = nullptr;
    }
    return MidiController::close();
}

bool AndroidMidiController::poll() {
    return m_pIoThread && m_pIoThread->isRunning();
}

bool AndroidMidiController::isPolling() const {
    return m_pIoThread && m_pIoThread->isRunning();
}

void AndroidMidiController::sendShortMsg(
        unsigned char status, unsigned char byte1, unsigned char byte2) {
    QByteArray data(kMidiMsgSize, '\0');
    data[0] = static_cast<char>(status);
    data[1] = static_cast<char>(byte1);
    data[2] = static_cast<char>(byte2);
    sendBytes(data);
}

bool AndroidMidiController::sendBytes(const QByteArray& data) {
    if (m_pIoThread && m_pIoThread->isRunning()) {
        m_pIoThread->send(data);
        return true;
    }
    return false;
}

// ── IoThread ────────────────────────────────────────────────────

AndroidMidiController::IoThread::IoThread(
        const QJniObject& usbDevice,
        jint usbFd,
        int interfaceNumber,
        AndroidMidiController* controller)
        : m_usbDevice(usbDevice),
          m_usbFd(usbFd),
          m_interfaceNumber(interfaceNumber),
          m_controller(controller) {
}

void AndroidMidiController::IoThread::stop() {
    m_stop.storeRelaxed(1);
}

void AndroidMidiController::IoThread::send(const QByteArray& data) {
    QMutexLocker lock(&m_sendMutex);
    m_pendingSend = data;
    m_hasPending = true;
}

void AndroidMidiController::IoThread::processUsbMidiPacket(
        const unsigned char* packet, int len) {
    // USB MIDI packet: 4-byte words: [CIN|MIDI_0|MIDI_1|MIDI_2]
    // CIN (Code Index Number) tells us the MIDI message type and cable number
    for (int i = 0; i + 3 < len; i += 4) {
        unsigned char cin = packet[i] & 0x0F;
        unsigned char midi0 = packet[i + 1];
        unsigned char midi1 = packet[i + 2];
        unsigned char midi2 = packet[i + 3];

        if (cin == 0x0) {
            continue; // miscellaneous / reserved
        }

        // Standard MIDI messages in USB: cin 0x2—0xF map to MIDI status bytes
        // cin 0x8 = Note Off, 0x9 = Note On, 0xA = Poly Pressure,
        // 0xB = CC, 0xC = Program Change, 0xD = Channel Pressure,
        // 0xE = Pitch Bend, 0xF = System
        m_controller->receive(QByteArray(1, static_cast<char>(midi0)),
                mixxx::Duration::fromMillis(0));
        if (cin >= 0x8 && cin <= 0xE) {
            // 3-byte message: [status+channel, data1, data2]
            unsigned char statusByte = (cin << 4) | (midi0 & 0x0F);
            m_controller->receivedShortMessage(
                    statusByte, midi1, midi2, mixxx::Duration::fromMillis(0));
        } else if (cin == 0x2 || cin == 0x3 || cin == 0x4 || cin == 0x6 || cin == 0x7) {
            // 2-byte system common messages (MTC, Song Position, Song Select, etc.)
            m_controller->receive(
                    QByteArray(1, static_cast<char>(midi0)),
                    mixxx::Duration::fromMillis(0));
        } else if (cin == 0x5) {
            // System exclusive — variable length
            // For now just forward the raw bytes
            QByteArray sysex;
            sysex.append(static_cast<char>(0xF0));
            int sysexLen = (midi0 == 0xF0) ? 3 : 0;
            Q_UNUSED(sysexLen);
            m_controller->receive(
                    QByteArray(1, static_cast<char>(midi0)),
                    mixxx::Duration::fromMillis(0));
        }
    }
}

void AndroidMidiController::IoThread::run() {
    __android_log_print(ANDROID_LOG_INFO,
            "mixxx",
            "AndroidMidiController I/O thread starting, FD=%d",
            m_usbFd);

    QJniEnvironment env;
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    auto usbManager = context.callObjectMethod("getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            QJniObject::getStaticObjectField("android/content/Context",
                    "USB_SERVICE",
                    "Ljava/lang/String;")
                    .object());

    auto usbConnection = usbManager.callObjectMethod("openDevice",
            "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
            m_usbDevice.object());

    if (!usbConnection.isValid()) {
        __android_log_print(ANDROID_LOG_ERROR, "mixxx", "IoThread: openDevice failed");
        return;
    }

    auto usbInterface = m_usbDevice.callObjectMethod("getInterface",
            "(I)Landroid/hardware/usb/UsbInterface;",
            m_interfaceNumber);

    // Claim the MIDI interface on this thread's USB connection
    if (usbInterface.isValid()) {
        jboolean claimed = usbConnection.callMethod<jboolean>("claimInterface",
                "(Landroid/hardware/usb/UsbInterface;Z)Z",
                usbInterface.object(),
                true);
        if (!claimed) {
            __android_log_print(ANDROID_LOG_WARN, "mixxx",
                    "IoThread: claimInterface failed — trying bulkTransfer anyway");
            // Fall through — some devices work without explicit claim
        }
    }

    // Scan endpoints by direction instead of hardcoded indices
    QJniObject bulkInEndpoint;
    QJniObject bulkOutEndpoint;
    jint epCount = usbInterface.callMethod<jint>("getEndpointCount");
    for (jint ep = 0; ep < epCount; ep++) {
        auto endpoint = usbInterface.callObjectMethod("getEndpoint",
                "(I)Landroid/hardware/usb/UsbEndpoint;", ep);
        jint dir = endpoint.callMethod<jint>("getDirection");
        // DIRECTION_IN = 0x80, DIRECTION_OUT = 0x00 in UsbConstants
        if (dir == 0x80 && !bulkInEndpoint.isValid()) {
            bulkInEndpoint = endpoint;
        } else if (dir == 0x00 && !bulkOutEndpoint.isValid()) {
            bulkOutEndpoint = endpoint;
        }
    }

    if (!bulkInEndpoint.isValid() && !bulkOutEndpoint.isValid()) {
        __android_log_print(ANDROID_LOG_ERROR, "mixxx",
                "IoThread: no bulk endpoints found on interface %d (epCount=%d)",
                m_interfaceNumber, epCount);
        return;
    }

    __android_log_print(ANDROID_LOG_INFO, "mixxx",
            "IoThread: endpoints — IN=%s OUT=%s",
            bulkInEndpoint.isValid() ? "yes" : "no",
            bulkOutEndpoint.isValid() ? "yes" : "no");

    jbyteArray readBuffer = env->NewByteArray(kUsbMidiPacketSize);

    while (!m_stop.loadRelaxed()) {
        // Process sends
        {
            QMutexLocker lock(&m_sendMutex);
            if (m_hasPending && bulkOutEndpoint.isValid()) {
                jbyteArray writeBuf = env->NewByteArray(m_pendingSend.size());
                env->SetByteArrayRegion(writeBuf,
                        0,
                        m_pendingSend.size(),
                        reinterpret_cast<const jbyte*>(
                                m_pendingSend.constData()));
                usbConnection.callMethod<jint>("bulkTransfer",
                        "(Landroid/hardware/usb/UsbEndpoint;[BII)I",
                        bulkOutEndpoint.object(),
                        writeBuf,
                        0,
                        m_pendingSend.size(),
                        kPollTimeoutMs);
                env->DeleteLocalRef(writeBuf);
                m_hasPending = false;
            }
        }

        // Read incoming data
        jint bytesRead = usbConnection.callMethod<jint>("bulkTransfer",
                "(Landroid/hardware/usb/UsbEndpoint;[BII)I",
                bulkInEndpoint.object(),
                readBuffer,
                0,
                kUsbMidiPacketSize,
                kPollTimeoutMs);

        if (bytesRead > 0) {
            jbyte* elements = env->GetByteArrayElements(readBuffer, nullptr);
            if (elements) {
                processUsbMidiPacket(
                        reinterpret_cast<const unsigned char*>(elements),
                        bytesRead);
                env->ReleaseByteArrayElements(readBuffer, elements, JNI_ABORT);
            }
        }

        msleep(5);
    }

    env->DeleteLocalRef(readBuffer);
    __android_log_print(ANDROID_LOG_INFO, "mixxx", "AndroidMidiController I/O thread stopped");
}

#endif // __ANDROID__
