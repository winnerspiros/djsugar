#include "controllers/midi/androidusbmidicontroller.h"

#include "controllers/legacycontrollermapping.h"
#include "controllers/midi/legacymidicontrollermappingfilehandler.h"
#include "util/logger.h"

namespace {

const mixxx::Logger kLogger("AndroidUsbMidiController");

// USB-MIDI packets are 4 bytes
constexpr int kUsbMidiPacketSize = 4;

// USB-MIDI Code Index Numbers
constexpr uint8_t kCINNoteOn = 0x09;
constexpr uint8_t kCINNoteOff = 0x08;
constexpr uint8_t kCINControlChange = 0x0B;
constexpr uint8_t kCINProgramChange = 0x0C;
constexpr uint8_t kCINPitchBend = 0x0E;
constexpr uint8_t kCINSysExStart = 0x04;
constexpr uint8_t kCINSysExContinue = 0x07;
constexpr uint8_t kCINSysExEnd1 = 0x05;
constexpr uint8_t kCINSysExEnd2 = 0x06;

} // namespace

AndroidUsbMidiController::AndroidUsbMidiController(const QString& name,
        int vendorId,
        int productId,
        QObject* parent)
        : MidiController(parent),
          m_vendorId(vendorId),
          m_productId(productId),
          m_pIoThread(nullptr) {
}

AndroidUsbMidiController::~AndroidUsbMidiController() {
    close();
}

void AndroidUsbMidiController::setUsbDevice(QJniObject usbDevice, QJniObject usbManager) {
    m_usbDevice = usbDevice;
    m_usbManager = usbManager;
}

int AndroidUsbMidiController::open(const QString& resourcePath) {
    if (isOpen()) {
        return 0;
    }

    if (!m_usbDevice.isValid() || !m_usbManager.isValid()) {
        qCWarning(kLogger) << "AndroidUsbMidiController: No USB device/manager set";
        return -1;
    }

    // Open device connection
    m_usbConnection = m_usbManager.callObjectMethod(
            "openDevice",
            "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
            m_usbDevice.object<jobject>());

    if (!m_usbConnection.isValid()) {
        qCWarning(kLogger) << "AndroidUsbMidiController: Cannot open USB device";
        return -1;
    }

    // Find the MIDI interface (class=1, subclass=3)
    int ifaceCount = m_usbDevice.callMethod<jint>("getInterfaceCount");
    for (int i = 0; i < ifaceCount; i++) {
        QJniObject iface = m_usbDevice.callMethod<jobject>(
                "getInterface", "(I)Landroid/hardware/usb/UsbInterface;", i);
        jint ifaceClass = iface.callMethod<jint>("getInterfaceClass");
        jint ifaceSubclass = iface.callMethod<jint>("getInterfaceSubclass");
        if (ifaceClass == 1 && ifaceSubclass == 3) { // Audio/MIDI
            m_usbInterface = iface;
            break;
        }
    }

    if (!m_usbInterface.isValid()) {
        qCWarning(kLogger) << "AndroidUsbMidiController: No MIDI interface found";
        return -1;
    }

    // Claim the interface
    bool claimed = m_usbConnection.callMethod<jboolean>(
            "claimInterface",
            "(Landroid/hardware/usb/UsbInterface;Z)Z",
            m_usbInterface.object<jobject>(),
            true);

    if (!claimed) {
        qCWarning(kLogger) << "AndroidUsbMidiController: Cannot claim MIDI interface";
        return -1;
    }

    // Find MIDI IN and OUT endpoints
    int epCount = m_usbInterface.callMethod<jint>("getEndpointCount");
    for (int i = 0; i < epCount; i++) {
        QJniObject ep = m_usbInterface.callMethod<jobject>(
                "getEndpoint", "(I)Landroid/hardware/usb/UsbEndpoint;", i);
        jint direction = ep.callMethod<jint>("getDirection");
        jint type = ep.callMethod<jint>("getType");
        // direction: 0x80 = IN, 0x00 = OUT
        // type: 0x03 = Interrupt (for MIDI)
        if (direction == 0x80) {
            m_endpointIn = ep;
        } else if (direction == 0x00) {
            m_endpointOut = ep;
        }
    }

    qCInfo(kLogger) << "AndroidUsbMidiController: Opening" << getName()
                    << "VID: 0x" << QString::number(m_vendorId, 16)
                    << "PID: 0x" << QString::number(m_productId, 16)
                    << "IN endpoint:" << m_endpointIn.isValid()
                    << "OUT endpoint:" << m_endpointOut.isValid();

    // Start IO thread for reading MIDI input
    m_pIoThread = new IoThread(this);
    m_pIoThread->start();

    return 0;
}

void AndroidUsbMidiController::IoThread::stop() {
    m_stop = true;
}

AndroidUsbMidiController::IoThread::IoThread(AndroidUsbMidiController* pController)
        : m_pController(pController),
          m_stop(false) {
}

void AndroidUsbMidiController::IoThread::run() {
    if (!m_pController->m_endpointIn.isValid()) {
        return;
    }

    QByteArray buffer(kUsbMidiPacketSize, 0);
    while (!m_stop) {
        // Read from MIDI IN endpoint (interrupt transfer)
        // Android UsbDeviceConnection.interruptTransfer() for reading
        QJniObject connection = m_pController->m_usbConnection;
        QJniObject endpoint = m_pController->m_endpointIn;
        if (!connection.isValid() || !endpoint.isValid()) {
            break;
        }

        jint maxPacketSize = endpoint.callMethod<jint>("getMaxPacketSize");
        if (maxPacketSize > buffer.size()) {
            buffer.resize(maxPacketSize);
        }

        jint bytesRead = connection.callMethod<jint>(
                "interruptTransfer",
                "(Landroid/hardware/usb/UsbEndpoint;[BIII)I",
                endpoint.object<jobject>(),
                buffer.data(),
                0,
                buffer.size(),
                100);

        if (bytesRead >= kUsbMidiPacketSize) {
            m_pController->parseUsbMidiPacket(
                    QByteArray(buffer.constData(), bytesRead));
        } else if (bytesRead < 0) {
            // Error or timeout, don't spin
            QThread::msleep(1);
        }
    }
}

void AndroidUsbMidiController::parseUsbMidiPacket(const QByteArray& packet) {
    if (packet.size() < kUsbMidiPacketSize) {
        return;
    }

    uint8_t header = static_cast<uint8_t>(packet[0]);
    uint8_t cin = header & 0x0F;
    // uint8_t cable = (header >> 4) & 0x0F; // usually 0 for single cable

    uint8_t byte1 = static_cast<uint8_t>(packet[1]);
    uint8_t byte2 = static_cast<uint8_t>(packet[2]);
    uint8_t byte3 = static_cast<uint8_t>(packet[3]);

    (void)cin;
    (void)byte1;
    (void)byte2;
    (void)byte3;
    // TODO: Feed MIDI bytes into the MidiController input handler
    // This requires access to the MIDI input engine
}

void AndroidUsbMidiController::sendMidiMessage(uint8_t cin, uint8_t status, uint8_t data1, uint8_t data2) {
    if (!m_endpointOut.isValid() || !m_usbConnection.isValid()) {
        return;
    }

    QByteArray packet(kUsbMidiPacketSize, 0);
    packet[0] = static_cast<char>((0 << 4) | (cin & 0x0F)); // Cable 0 + CIN
    packet[1] = static_cast<char>(status);
    packet[2] = static_cast<char>(data1);
    packet[3] = static_cast<char>(data2);

    m_usbConnection.callMethod<jint>(
            "bulkTransfer",
            "(Landroid/hardware/usb/UsbEndpoint;[BIII)I",
            m_endpointOut.object<jobject>(),
            packet.data(), 0, packet.size(), 1000);
}

void AndroidUsbMidiController::sendShortMsg(unsigned char status,
        unsigned char byte1,
        unsigned char byte2) {
    // USB-MIDI 4-byte packet: [CIN<<4 | cable, status, data1, data2]
    uint8_t cin;
    uint8_t msgType = status & 0xF0;
    switch (msgType) {
    case 0x80: cin = kCINNoteOff; break;
    case 0x90: cin = kCINNoteOn; break;
    case 0xA0: cin = 0x0A; break; // Poly Key Pressure
    case 0xB0: cin = kCINControlChange; break;
    case 0xC0: cin = kCINProgramChange; break;
    case 0xD0: cin = 0x0D; break; // Channel Pressure
    case 0xE0: cin = kCINPitchBend; break;
    default:   cin = 0x0F; break; // Single Byte
    }

    sendMidiMessage(static_cast<uint8_t>(cin), status, byte1, byte2);
}

#include "moc_androidusbmidicontroller.cpp"
