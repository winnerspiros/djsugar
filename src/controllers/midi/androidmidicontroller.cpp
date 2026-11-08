#include "controllers/midi/androidmidicontroller.h"

#include <dlfcn.h>
#include <jni.h>

#include <QJniEnvironment>
#include <cstring>
#include <vector>

#include "controllers/midi/midiutils.h"
#include "moc_androidmidicontroller.cpp"
#include "soundio/androidusbaudiodevice.h"
#include "util/time.h"

namespace {

// Values from <amidi/AMidi.h>
constexpr int32_t kAMidiOpcodeData = 1;
constexpr int32_t kMediaStatusOk = 0;

// android.media.midi.MidiDeviceInfo.TYPE_BLUETOOTH
constexpr int kMidiDeviceInfoTypeBluetooth = 3;

constexpr int kOpenTimeoutMillis = 5000;
// Upper bound for the number of packets read in a single poll
constexpr int kMaxPacketsPerPoll = 256;
constexpr size_t kReceiveBufferSize = 1024;

/// The AMidi NDK API is only available from API level 29, while Mixxx still
/// supports API level 28, so the symbols are resolved at runtime.
struct AMidiApi {
    using PFN_DeviceFromJava = int32_t (*)(JNIEnv*, jobject, AMidiDevice**);
    using PFN_DeviceRelease = int32_t (*)(const AMidiDevice*);
    using PFN_OutputPortOpen = int32_t (*)(const AMidiDevice*, int32_t, AMidiOutputPort**);
    using PFN_OutputPortReceive = ssize_t (*)(const AMidiOutputPort*,
            int32_t*,
            uint8_t*,
            size_t,
            size_t*,
            int64_t*);
    using PFN_OutputPortClose = void (*)(const AMidiOutputPort*);
    using PFN_InputPortOpen = int32_t (*)(const AMidiDevice*, int32_t, AMidiInputPort**);
    using PFN_InputPortSend = ssize_t (*)(const AMidiInputPort*, const uint8_t*, size_t);
    using PFN_InputPortClose = void (*)(const AMidiInputPort*);

    PFN_DeviceFromJava deviceFromJava = nullptr;
    PFN_DeviceRelease deviceRelease = nullptr;
    PFN_OutputPortOpen outputPortOpen = nullptr;
    PFN_OutputPortReceive outputPortReceive = nullptr;
    PFN_OutputPortClose outputPortClose = nullptr;
    PFN_InputPortOpen inputPortOpen = nullptr;
    PFN_InputPortSend inputPortSend = nullptr;
    PFN_InputPortClose inputPortClose = nullptr;

    bool isValid() const {
        return deviceFromJava && deviceRelease && outputPortOpen &&
                outputPortReceive && outputPortClose && inputPortOpen &&
                inputPortSend && inputPortClose;
    }
};

const AMidiApi& amidi() {
    static const AMidiApi api = []() {
        AMidiApi result;
        void* pLibrary = dlopen("libamidi.so", RTLD_NOW);
        if (!pLibrary) {
            qWarning() << "Unable to load libamidi.so:" << dlerror();
            return result;
        }
        result.deviceFromJava = reinterpret_cast<AMidiApi::PFN_DeviceFromJava>(
                dlsym(pLibrary, "AMidiDevice_fromJava"));
        result.deviceRelease = reinterpret_cast<AMidiApi::PFN_DeviceRelease>(
                dlsym(pLibrary, "AMidiDevice_release"));
        result.outputPortOpen = reinterpret_cast<AMidiApi::PFN_OutputPortOpen>(
                dlsym(pLibrary, "AMidiOutputPort_open"));
        result.outputPortReceive = reinterpret_cast<AMidiApi::PFN_OutputPortReceive>(
                dlsym(pLibrary, "AMidiOutputPort_receive"));
        result.outputPortClose = reinterpret_cast<AMidiApi::PFN_OutputPortClose>(
                dlsym(pLibrary, "AMidiOutputPort_close"));
        result.inputPortOpen = reinterpret_cast<AMidiApi::PFN_InputPortOpen>(
                dlsym(pLibrary, "AMidiInputPort_open"));
        result.inputPortSend = reinterpret_cast<AMidiApi::PFN_InputPortSend>(
                dlsym(pLibrary, "AMidiInputPort_send"));
        result.inputPortClose = reinterpret_cast<AMidiApi::PFN_InputPortClose>(
                dlsym(pLibrary, "AMidiInputPort_close"));
        if (!result.isValid()) {
            qWarning() << "libamidi.so is missing some symbols";
        }
        return result;
    }();
    return api;
}

QString callStringHelper(const char* pMethod, const QJniObject& deviceInfo) {
    return QJniObject::callStaticMethod<jstring>("org/mixxx/MidiHelper",
            pMethod,
            "(Landroid/media/midi/MidiDeviceInfo;)Ljava/lang/String;",
            deviceInfo.object())
            .toString();
}

std::optional<uint16_t> callUsbIdHelper(const char* pMethod, const QJniObject& deviceInfo) {
    const jint id = QJniObject::callStaticMethod<jint>("org/mixxx/MidiHelper",
            pMethod,
            "(Landroid/media/midi/MidiDeviceInfo;)I",
            deviceInfo.object());
    if (id < 0) {
        return std::nullopt;
    }
    return static_cast<uint16_t>(id);
}

/// Number of data bytes following a status byte
int dataLength(unsigned char status) {
    if (status < 0xF0) {
        const unsigned char opCode = status & 0xF0;
        return (opCode == 0xC0 || opCode == 0xD0) ? 1 : 2;
    }
    switch (status) {
    case 0xF1: // MIDI time code quarter frame
    case 0xF3: // Song select
        return 1;
    case 0xF2: // Song position pointer
        return 2;
    default:
        return 0;
    }
}

} // namespace

// static
bool AndroidMidiController::isSupported() {
    return amidi().isValid();
}

AndroidMidiController::AndroidMidiController(
        const QJniObject& deviceInfo, const QString& deviceName)
        : AndroidMidiControllerBase(deviceName),
          m_deviceInfo(deviceInfo),
          m_pDevice(nullptr),
          m_pOutputPort(nullptr),
          m_pInputPort(nullptr),
          m_androidDeviceId(deviceInfo.callMethod<jint>("getId")),
          m_androidDeviceType(deviceInfo.callMethod<jint>("getType")),
          m_manufacturer(callStringHelper("getManufacturer", deviceInfo)),
          m_product(callStringHelper("getName", deviceInfo)),
          m_serialNumber(callStringHelper("getSerialNumber", deviceInfo)),
          m_vendorId(callUsbIdHelper("getUsbVendorId", deviceInfo)),
          m_productId(callUsbIdHelper("getUsbProductId", deviceInfo)) {
    // From the point of view of the Android API, an "output port" sends data
    // out of the device (to Mixxx) and an "input port" receives data from Mixxx.
    setInputDevice(deviceInfo.callMethod<jint>("getOutputPortCount") > 0);
    setOutputDevice(deviceInfo.callMethod<jint>("getInputPortCount") > 0);
}

AndroidMidiController::~AndroidMidiController() {
    if (isOpen()) {
        close();
    }
}

PhysicalTransportProtocol AndroidMidiController::getPhysicalTransportProtocol() const {
    if (m_vendorId.has_value()) {
        return PhysicalTransportProtocol::USB;
    }
    if (m_androidDeviceType == kMidiDeviceInfoTypeBluetooth) {
        return PhysicalTransportProtocol::BlueTooth;
    }
    return PhysicalTransportProtocol::UNKNOWN;
}

int AndroidMidiController::open(const QString& resourcePath) {
    if (isOpen()) {
        qCWarning(m_logBase) << "Android MIDI device" << getName() << "already open";
        return -1;
    }
    const AMidiApi& api = amidi();
    if (!api.isValid()) {
        return -1;
    }

    m_midiDevice = QJniObject::callStaticMethod<jobject>("org/mixxx/MidiHelper",
            "openDevice",
            "(Landroid/media/midi/MidiDeviceInfo;I)Landroid/media/midi/MidiDevice;",
            m_deviceInfo.object(),
            kOpenTimeoutMillis);
    if (!m_midiDevice.isValid()) {
        qCWarning(m_logBase) << "Unable to open Android MIDI device" << getName();
        return -2;
    }

    QJniEnvironment env;
    if (api.deviceFromJava(env.jniEnv(), m_midiDevice.object(), &m_pDevice) !=
                    kMediaStatusOk ||
            !m_pDevice) {
        qCWarning(m_logBase) << "AMidiDevice_fromJava failed for" << getName();
        m_pDevice = nullptr;
        closePorts();
        return -2;
    }

    if (isInputDevice() &&
            api.outputPortOpen(m_pDevice, 0, &m_pOutputPort) != kMediaStatusOk) {
        qCWarning(m_logBase) << "Unable to open the MIDI input of" << getName();
        m_pOutputPort = nullptr;
    }
    if (isOutputDevice() &&
            api.inputPortOpen(m_pDevice, 0, &m_pInputPort) != kMediaStatusOk) {
        qCWarning(m_logBase) << "Unable to open the MIDI output of" << getName();
        m_pInputPort = nullptr;
    }
    if (!m_pOutputPort && !m_pInputPort) {
        closePorts();
        return -2;
    }

    qCInfo(m_logBase) << "Opened Android MIDI device" << getName()
                      << "input:" << (m_pOutputPort != nullptr)
                      << "output:" << (m_pInputPort != nullptr);

    resetParser();

    // Open before applying the mapping: its init script and the output
    // handlers send the initial LED states, which are dropped otherwise
    setOpen(true);
    startEngine();
    applyMapping(resourcePath);
    return 0;
}

void AndroidMidiController::closePorts() {
    const AMidiApi& api = amidi();
    if (m_pOutputPort) {
        api.outputPortClose(m_pOutputPort);
        m_pOutputPort = nullptr;
    }
    if (m_pInputPort) {
        api.inputPortClose(m_pInputPort);
        m_pInputPort = nullptr;
    }
    if (m_pDevice) {
        api.deviceRelease(m_pDevice);
        m_pDevice = nullptr;
    }
    if (m_midiDevice.isValid()) {
        QJniObject::callStaticMethod<void>("org/mixxx/MidiHelper",
                "closeDevice",
                "(Landroid/media/midi/MidiDevice;)V",
                m_midiDevice.object());
        m_midiDevice = QJniObject();
    }
}

int AndroidMidiController::close() {
    if (!isOpen()) {
        qCWarning(m_logBase) << "Android MIDI device" << getName() << "already closed";
        return -1;
    }

    stopEngine();
    MidiController::close();
    closePorts();
    setOpen(false);
    return 0;
}

bool AndroidMidiController::poll() {
    if (!m_pOutputPort) {
        return false;
    }
    const AMidiApi& api = amidi();
    uint8_t buffer[kReceiveBufferSize];
    bool received = false;
    for (int i = 0; i < kMaxPacketsPerPoll; ++i) {
        int32_t opCode = 0;
        size_t numBytes = 0;
        int64_t timestampNs = 0;
        const ssize_t numMessages = api.outputPortReceive(m_pOutputPort,
                &opCode,
                buffer,
                sizeof(buffer),
                &numBytes,
                &timestampNs);
        if (numMessages < 0) {
            qCWarning(m_logInput) << "AMidiOutputPort_receive error" << numMessages;
            return received;
        }
        if (numMessages == 0) {
            break;
        }
        if (opCode == kAMidiOpcodeData && numBytes > 0) {
            parseBytes(buffer, numBytes, mixxx::Time::elapsed());
            received = true;
        }
    }
    return received;
}

AndroidMidiControllerBase::AndroidMidiControllerBase(const QString& deviceName)
        : MidiController(deviceName),
          m_runningStatus(0),
          m_data{0, 0},
          m_dataCount(0),
          m_bInSysex(false) {
}

void AndroidMidiControllerBase::resetParser() {
    m_runningStatus = 0;
    m_dataCount = 0;
    m_bInSysex = false;
    m_sysexBuffer.clear();
}

void AndroidMidiControllerBase::parseBytes(
        const uint8_t* pData, size_t length, mixxx::Duration timestamp) {
    for (size_t i = 0; i < length; ++i) {
        const unsigned char byte = pData[i];

        if (byte >= 0xF8) {
            // Real-time messages may appear anywhere, even inside SysEx
            receivedShortMessage(byte, 0, 0, timestamp);
            continue;
        }

        if (m_bInSysex) {
            if (byte == MidiUtils::opCodeValue(MidiOpCode::EndOfExclusive)) {
                m_sysexBuffer.append(static_cast<char>(byte));
                m_bInSysex = false;
                receive(m_sysexBuffer, timestamp);
                m_sysexBuffer.clear();
                continue;
            }
            if (byte < 0x80) {
                if (m_sysexBuffer.size() < MIXXX_ANDROID_MIDI_SYSEX_BUFFER_LEN) {
                    m_sysexBuffer.append(static_cast<char>(byte));
                }
                continue;
            }
            // A status byte interrupts the SysEx message: drop it and handle
            // the new status byte below.
            qCWarning(m_logInput) << "Buggy MIDI device: SysEx interrupted!";
            m_bInSysex = false;
            m_sysexBuffer.clear();
        }

        if (byte == 0xF0) {
            m_bInSysex = true;
            m_runningStatus = 0;
            m_dataCount = 0;
            m_sysexBuffer.clear();
            m_sysexBuffer.append(static_cast<char>(byte));
            continue;
        }

        if (byte >= 0x80) {
            m_dataCount = 0;
            if (dataLength(byte) == 0) {
                // Tune request and undefined system common messages
                m_runningStatus = 0;
                receivedShortMessage(byte, 0, 0, timestamp);
            } else {
                m_runningStatus = byte;
            }
            continue;
        }

        // Data byte, possibly using running status
        if (m_runningStatus == 0) {
            continue;
        }
        m_data[m_dataCount++] = byte;
        const int needed = dataLength(m_runningStatus);
        if (m_dataCount < needed) {
            continue;
        }
        receivedShortMessage(m_runningStatus,
                m_data[0],
                needed > 1 ? m_data[1] : 0,
                timestamp);
        m_dataCount = 0;
        if (m_runningStatus >= 0xF0) {
            // System common messages do not support running status
            m_runningStatus = 0;
        }
    }
}

void AndroidMidiController::sendShortMsg(
        unsigned char status, unsigned char byte1, unsigned char byte2) {
    if (!m_pInputPort) {
        return;
    }
    const uint8_t message[3] = {status, byte1, byte2};
    const size_t length = (status >= 0xF8) ? 1 : 1 + dataLength(status);
    const ssize_t sent = amidi().inputPortSend(m_pInputPort, message, length);
    if (sent == static_cast<ssize_t>(length)) {
        qCDebug(m_logOutput) << QStringLiteral("outgoing: ")
                             << MidiUtils::formatMidiOpCode(getName(),
                                        status,
                                        byte1,
                                        byte2,
                                        MidiUtils::channelFromStatus(status),
                                        MidiUtils::opCodeFromStatus(status));
    } else {
        qCWarning(m_logOutput) << "Error sending short message"
                               << MidiUtils::formatMidiOpCode(getName(),
                                          status,
                                          byte1,
                                          byte2,
                                          MidiUtils::channelFromStatus(status),
                                          MidiUtils::opCodeFromStatus(status));
    }
}

bool AndroidMidiController::sendBytes(const QByteArray& data) {
    if (!m_pInputPort || data.isEmpty()) {
        return false;
    }
    const ssize_t sent = amidi().inputPortSend(m_pInputPort,
            reinterpret_cast<const uint8_t*>(data.constData()),
            static_cast<size_t>(data.size()));
    if (sent != data.size()) {
        qCWarning(m_logOutput) << "Error sending SysEx message:"
                               << MidiUtils::formatSysexMessage(getName(), data);
        return false;
    }
    qCDebug(m_logOutput) << QStringLiteral("outgoing: ")
                         << MidiUtils::formatSysexMessage(getName(), data);
    return true;
}

namespace {

/// USB-MIDI Code Index Number of a short message
uint8_t codeIndexNumber(unsigned char status) {
    if (status < 0xF0) {
        return status >> 4;
    }
    switch (status) {
    case 0xF1: // MIDI time code quarter frame
    case 0xF3: // Song select
        return 0x2;
    case 0xF2: // Song position pointer
        return 0x3;
    case 0xF6: // Tune request
        return 0x5;
    default:
        // Real-time and undefined single bytes
        return 0xF;
    }
}

/// Number of MIDI bytes carried by a USB-MIDI event packet
int packetLength(uint8_t codeIndex) {
    switch (codeIndex) {
    case 0x5: // Single-byte system common or SysEx end
    case 0xF: // Single byte
        return 1;
    case 0x2: // Two-byte system common
    case 0x6: // SysEx ends with two bytes
    case 0xC: // Program change
    case 0xD: // Channel pressure
        return 2;
    case 0x0: // Reserved
    case 0x1: // Reserved (cable events)
        return 0;
    default:
        return 3;
    }
}

// Packets received and not yet handled by the controller thread: a few
// seconds of turning every knob at once
constexpr size_t kMaxPendingUsbMidiPackets = 4096;

} // namespace

AndroidUsbMidiController::AndroidUsbMidiController(
        std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> pDevice)
        // Same name as the Android MIDI device, so the configuration and the
        // mapping carry over
        : AndroidMidiControllerBase(pDevice->productName()),
          m_pDevice(std::move(pDevice)),
          m_packets(kMaxPendingUsbMidiPackets) {
    setInputDevice(m_pDevice->midiInterface().inEndpoint != 0);
    setOutputDevice(m_pDevice->midiInterface().outEndpoint != 0);
}

AndroidUsbMidiController::~AndroidUsbMidiController() {
    if (isOpen()) {
        close();
    }
}

QString AndroidUsbMidiController::getVendorString() const {
    return m_pDevice->manufacturerName();
}

QString AndroidUsbMidiController::getProductString() const {
    return m_pDevice->productName();
}

std::optional<uint16_t> AndroidUsbMidiController::getVendorId() const {
    return m_pDevice->vendorId();
}

std::optional<uint16_t> AndroidUsbMidiController::getProductId() const {
    return m_pDevice->productId();
}

int AndroidUsbMidiController::open(const QString& resourcePath) {
    if (isOpen()) {
        qCWarning(m_logBase) << "USB MIDI device" << getName() << "already open";
        return -1;
    }
    while (m_packets.front()) {
        m_packets.pop();
    }
    if (isInputDevice() && !m_pDevice->startMidiInput([this](const uint8_t* pPacket) {
            MidiPacket packet;
            std::memcpy(packet.bytes, pPacket, sizeof(packet.bytes));
            // Timed on arrival, not when the controller thread polls
            packet.timestamp = mixxx::Time::elapsed();
            // Dropped if the controller thread is that far behind
            static_cast<void>(m_packets.try_push(packet));
        })) {
        qCWarning(m_logBase) << "Unable to start the USB MIDI input of" << getName();
        return -2;
    }
    resetParser();
    qCInfo(m_logBase) << "Opened direct USB MIDI device" << getName();
    // Open before applying the mapping: its init script and the output
    // handlers send the initial LED states, which are dropped otherwise
    setOpen(true);
    startEngine();
    applyMapping(resourcePath);
    return 0;
}

int AndroidUsbMidiController::close() {
    if (!isOpen()) {
        qCWarning(m_logBase) << "USB MIDI device" << getName() << "already closed";
        return -1;
    }
    stopEngine();
    MidiController::close();
    m_pDevice->stopMidiInput();
    setOpen(false);
    return 0;
}

bool AndroidUsbMidiController::poll() {
    bool received = false;
    while (const MidiPacket* pPacket = m_packets.front()) {
        const int length = packetLength(pPacket->bytes[0] & 0x0F);
        if (length > 0) {
            parseBytes(pPacket->bytes + 1, length, pPacket->timestamp);
        }
        m_packets.pop();
        received = true;
    }
    return received;
}

void AndroidUsbMidiController::sendShortMsg(
        unsigned char status, unsigned char byte1, unsigned char byte2) {
    const uint8_t packet[4] = {codeIndexNumber(status), status, byte1, byte2};
    if (!m_pDevice->sendMidi(packet, sizeof(packet))) {
        qCWarning(m_logOutput) << "Error sending short message"
                               << MidiUtils::formatMidiOpCode(getName(),
                                          status,
                                          byte1,
                                          byte2,
                                          MidiUtils::channelFromStatus(status),
                                          MidiUtils::opCodeFromStatus(status));
    }
}

bool AndroidUsbMidiController::sendBytes(const QByteArray& data) {
    if (data.isEmpty()) {
        return false;
    }
    // SysEx is split into 3 byte packets: CIN 0x4 while it continues, then
    // 0x5/0x6/0x7 for the last 1/2/3 bytes
    std::vector<uint8_t> packets;
    packets.reserve((data.size() / 3 + 1) * 4);
    const auto* pBytes = reinterpret_cast<const uint8_t*>(data.constData());
    qsizetype offset = 0;
    while (offset < data.size()) {
        const qsizetype remaining = data.size() - offset;
        const int count = remaining > 3 ? 3 : static_cast<int>(remaining);
        uint8_t codeIndex = 0x4;
        if (remaining <= 3) {
            codeIndex = static_cast<uint8_t>(0x4 + count);
        }
        packets.push_back(codeIndex);
        for (int i = 0; i < 3; ++i) {
            packets.push_back(i < count ? pBytes[offset + i] : 0);
        }
        offset += count;
    }
    if (!m_pDevice->sendMidi(packets.data(), static_cast<int>(packets.size()))) {
        qCWarning(m_logOutput) << "Error sending SysEx message:"
                               << MidiUtils::formatSysexMessage(getName(), data);
        return false;
    }
    return true;
}
