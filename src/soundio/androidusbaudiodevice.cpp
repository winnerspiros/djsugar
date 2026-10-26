#include "soundio/androidusbaudiodevice.h"

#include <QtCore/private/qandroidextras_p.h>
#include <libusb.h>
#include <pthread.h>
#include <sys/resource.h>
#include <unistd.h>

#include <QDebug>
#include <QJniEnvironment>
#include <QSet>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstring>

#include "controllers/android.h"

namespace mixxx {
namespace androidusb {

namespace {

constexpr uint8_t kRequestCur = 0x01;
constexpr uint8_t kRequestRange = 0x02;
// UAC1 has separate request codes for reading
constexpr uint8_t kRequestGetCurUac1 = 0x81;
// UAC1 endpoint control / UAC2 clock source control
constexpr uint16_t kSamplingFreqControl = 0x0100;

constexpr int kStopTimeoutMillis = 1000;
constexpr int kControlTimeoutMillis = 1000;
constexpr int kFeedbackTransfers = 2;
constexpr int kFeedbackPacketsPerTransfer = 4;
constexpr int kFeedbackPacketSize = 4;
constexpr size_t kMaxImplicitQueue = 1024;

const int kCommonSampleRates[] = {44100, 48000, 88200, 96000, 32000, 176400, 192000};

std::mutex s_registryMutex;
std::vector<std::shared_ptr<AndroidUsbAudioDevice>> s_registry;
// USB device names (e.g. /dev/bus/usb/001/005) for which the permission was
// already requested, to not ask again on every rescan. A device gets a new
// name when it is plugged in again.
QSet<QString> s_permissionRequested;

QJniObject usbManager() {
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    QJniObject usbService = QJniObject::getStaticObjectField(
            "android/content/Context", "USB_SERVICE", "Ljava/lang/String;");
    return context.callObjectMethod("getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            usbService.object());
}

/// Cheap check through the Java API, which doesn't require the USB permission
bool hasAudioStreamingInterface(const QJniObject& usbDevice) {
    const jint interfaceCount = usbDevice.callMethod<jint>("getInterfaceCount");
    for (jint i = 0; i < interfaceCount; ++i) {
        QJniObject usbInterface = usbDevice.callObjectMethod("getInterface",
                "(I)Landroid/hardware/usb/UsbInterface;",
                i);
        // UsbConstants.USB_CLASS_AUDIO = 1, audio streaming subclass = 2
        if (usbInterface.callMethod<jint>("getInterfaceClass") == 1 &&
                usbInterface.callMethod<jint>("getInterfaceSubclass") == 2) {
            return true;
        }
    }
    return false;
}

uint32_t readLittleEndian(const uint8_t* p, int bytes) {
    uint32_t value = 0;
    for (int i = bytes - 1; i >= 0; --i) {
        value = (value << 8) | p[i];
    }
    return value;
}

void raiseThreadPriority() {
    // THREAD_PRIORITY_URGENT_AUDIO; best effort, Android may refuse it
    if (setpriority(PRIO_PROCESS, gettid(), -19) != 0) {
        setpriority(PRIO_PROCESS, gettid(), -16);
    }
#if defined(__aarch64__)
    // The audio engine runs on this thread: enable flush-to-zero (FPCR bit
    // 24) to avoid the denormal performance penalty, like the other sound
    // devices do.
    int64_t fpcr;
    asm volatile("mrs %[fpcr], FPCR" : [fpcr] "=r"(fpcr));
    asm volatile("msr FPCR, %[src]" : : [src] "r"(fpcr | (1 << 24)));
#endif
}

} // namespace

AndroidUsbAudioDevice::AndroidUsbAudioDevice(const QJniObject& usbDevice)
        : m_usbDevice(usbDevice),
          m_productName(usbDevice.callMethod<jstring>("getProductName").toString()),
          m_usbDeviceName(usbDevice.callMethod<jstring>("getDeviceName").toString()),
          m_manufacturerName(usbDevice.callMethod<jstring>("getManufacturerName").toString()),
          m_vendorId(static_cast<uint16_t>(usbDevice.callMethod<jint>("getVendorId"))),
          m_productId(static_cast<uint16_t>(usbDevice.callMethod<jint>("getProductId"))),
          m_pContext(nullptr),
          m_pHandle(nullptr),
          m_hasImplicitFeedback(false),
          m_claimed(false),
          m_disconnected(false),
          m_eventThreadRunning(false),
          m_outputRunning(false),
          m_outputTransfersPending(0),
          m_framesInFlight(0),
          m_sampleRate(0),
          m_packetsPerSecond(1000),
          m_frameAccumulator(0),
          m_nominalFramesPerPacket(0),
          m_framesPerPacket(0),
          m_fractionalFrames(0),
          m_feedbackTransfersPending(0),
          m_measuredFrames16_16(0),
          m_measuredPackets(0),
          m_clockLogged(false),
          m_pMidiInTransfer(nullptr),
          m_midiInRunning(false),
          m_midiInPending(false) {
    if (m_productName.trimmed().isEmpty()) {
        m_productName = QStringLiteral("USB Audio");
    }
}

AndroidUsbAudioDevice::~AndroidUsbAudioDevice() {
    stopOutput();
    stopMidiInput();
    stopEventThread();
    if (m_pHandle) {
        for (int interfaceNumber : m_claimedInterfaces) {
            libusb_release_interface(m_pHandle, interfaceNumber);
        }
        libusb_close(m_pHandle);
        m_pHandle = nullptr;
    }
    if (m_pContext) {
        libusb_exit(m_pContext);
        m_pContext = nullptr;
    }
    if (m_connection.isValid()) {
        m_connection.callMethod<void>("close");
    }
}

// static
std::vector<std::shared_ptr<AndroidUsbAudioDevice>> AndroidUsbAudioDevice::scan(
        DirectMode mode, bool waitForPermission) {
    std::vector<std::shared_ptr<AndroidUsbAudioDevice>> previous;
    {
        std::lock_guard lock(s_registryMutex);
        previous = s_registry;
    }
    std::vector<std::shared_ptr<AndroidUsbAudioDevice>> found;
    QJniObject manager = usbManager();
    if (mode == DirectMode::Never || !manager.isValid()) {
        // Devices already claimed stay with us, the kernel driver only comes
        // back when they are plugged in again
        for (const auto& pDevice : previous) {
            if (pDevice->isConnected() && pDevice->isClaimed()) {
                found.push_back(pDevice);
            }
        }
        std::lock_guard lock(s_registryMutex);
        s_registry = found;
        return found;
    }
    QJniObject deviceMap = manager.callObjectMethod("getDeviceList", "()Ljava/util/HashMap;");
    QJniObject values = deviceMap.callObjectMethod("values", "()Ljava/util/Collection;");
    QJniObject array = values.callObjectMethod("toArray", "()[Ljava/lang/Object;");
    QJniEnvironment env;
    const jsize count = array.isValid() ? env->GetArrayLength(array.object<jobjectArray>()) : 0;

    for (jsize i = 0; i < count; ++i) {
        QJniObject usbDevice = QJniObject::fromLocalRef(
                env->GetObjectArrayElement(array.object<jobjectArray>(), i));
        if (!usbDevice.isValid() || !hasAudioStreamingInterface(usbDevice)) {
            continue;
        }
        const QString deviceName = usbDevice.callMethod<jstring>("getDeviceName").toString();
        auto it = std::find_if(previous.begin(), previous.end(), [&](const auto& pDevice) {
            return pDevice->usbDeviceName() == deviceName && pDevice->isConnected();
        });
        if (it != previous.end()) {
            found.push_back(*it);
            continue;
        }
        std::shared_ptr<AndroidUsbAudioDevice> pDevice(new AndroidUsbAudioDevice(usbDevice));
        if (pDevice->open(mode, waitForPermission)) {
            found.push_back(pDevice);
        }
    }
    {
        std::lock_guard lock(s_registryMutex);
        s_registry = found;
    }
    return found;
}

// static
std::vector<std::shared_ptr<AndroidUsbAudioDevice>> AndroidUsbAudioDevice::devices() {
    std::lock_guard lock(s_registryMutex);
    std::vector<std::shared_ptr<AndroidUsbAudioDevice>> result;
    for (const auto& pDevice : s_registry) {
        if (pDevice->isConnected()) {
            result.push_back(pDevice);
        }
    }
    return result;
}

bool AndroidUsbAudioDevice::open(DirectMode mode, bool waitForPermission) {
    QJniObject manager = usbManager();
    if (!manager.isValid()) {
        return false;
    }
    if (!manager.callMethod<jboolean>("hasPermission",
                "(Landroid/hardware/usb/UsbDevice;)Z",
                m_usbDevice.object())) {
        if (s_permissionRequested.contains(m_usbDeviceName) && !waitForPermission) {
            // Still waiting for the answer, or denied
            return false;
        }
        s_permissionRequested.insert(m_usbDeviceName);
        qInfo() << "Requesting USB permission for" << m_productName;
        const QJniObject& pendingIntent = mixxx::android::getIntent();
        manager.callMethod<void>("requestPermission",
                "(Landroid/hardware/usb/UsbDevice;Landroid/app/PendingIntent;)V",
                m_usbDevice.object(),
                pendingIntent.object());
        if (!waitForPermission) {
            // The answer triggers a rescan (see usbDeviceAccessResult)
            return false;
        }
        if (!mixxx::android::waitForPermission(m_usbDevice)) {
            qWarning() << "USB permission denied for" << m_productName;
            return false;
        }
    }
    m_connection = manager.callObjectMethod("openDevice",
            "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
            m_usbDevice.object());
    if (!m_connection.isValid()) {
        qWarning() << "Unable to open USB device" << m_productName;
        return false;
    }

    QJniObject rawDescriptors = m_connection.callObjectMethod("getRawDescriptors", "()[B");
    if (!rawDescriptors.isValid()) {
        return false;
    }
    QJniEnvironment env;
    const jsize length = env->GetArrayLength(rawDescriptors.object<jbyteArray>());
    std::vector<uint8_t> bytes(length);
    env->GetByteArrayRegion(rawDescriptors.object<jbyteArray>(),
            0,
            length,
            reinterpret_cast<jbyte*>(bytes.data()));
    m_description = usbaudio::parse(bytes.data(), bytes.size());

    const usbaudio::StreamFormat* pOutput = usbaudio::chooseOutput(m_description);
    if (!pOutput) {
        for (const auto& format : m_description.streams) {
            if (format.isOutput && format.altSetting > 0) {
                qInfo() << m_productName << "unsupported output:"
                        << QString::fromStdString(usbaudio::describe(format));
            }
        }
        qInfo() << m_productName << "has no supported USB audio output, using Android audio";
        return false;
    }
    m_outputFormat = *pOutput;
    if (const auto* pImplicit = usbaudio::implicitFeedbackSource(m_description, m_outputFormat)) {
        m_implicitFormat = *pImplicit;
        m_hasImplicitFeedback = true;
    }
    if (mode == DirectMode::Auto && m_outputFormat.channels <= 2) {
        qInfo() << m_productName << "is a stereo sound card, using Android audio";
        return false;
    }
    if (m_outputFormat.uacVersion == 2 && m_outputFormat.sampleRates.empty()) {
        // Provisional, the clock is only queried once the device is claimed
        m_outputFormat.sampleRates = {44100, 48000, 88200, 96000};
    }

    const intptr_t fileDescriptor = m_connection.callMethod<jint>("getFileDescriptor");
    libusb_set_option(nullptr, LIBUSB_OPTION_NO_DEVICE_DISCOVERY);
    int error = libusb_init(&m_pContext);
    if (error < 0) {
        qWarning() << "libusb_init failed:" << libusb_error_name(error);
        m_pContext = nullptr;
        return false;
    }
    error = libusb_wrap_sys_device(m_pContext, fileDescriptor, &m_pHandle);
    if (error < 0) {
        qWarning() << "libusb_wrap_sys_device failed:" << libusb_error_name(error);
        m_pHandle = nullptr;
        return false;
    }
    qInfo() << "Direct USB audio device:" << m_productName
            << QString::fromStdString(usbaudio::describe(m_outputFormat))
            << (isHighSpeed() ? "high speed" : "full speed")
            << "MIDI:" << m_description.midi.isValid();
    if (m_hasImplicitFeedback) {
        qInfo() << "  clocked by" << QString::fromStdString(usbaudio::describe(m_implicitFormat));
    }
    return true;
}

bool AndroidUsbAudioDevice::isHighSpeed() const {
    return m_pHandle &&
            libusb_get_device_speed(libusb_get_device(m_pHandle)) >= LIBUSB_SPEED_HIGH;
}

QList<int> AndroidUsbAudioDevice::sampleRates() const {
    QList<int> rates(m_outputFormat.sampleRates.begin(), m_outputFormat.sampleRates.end());
    if (m_outputFormat.minRate > 0) {
        for (int rate : kCommonSampleRates) {
            if (m_outputFormat.supportsRate(rate) && !rates.contains(rate)) {
                rates.append(rate);
            }
        }
    }
    return rates;
}

bool AndroidUsbAudioDevice::claimInterface(int interfaceNumber) {
    // UsbDeviceConnection.claimInterface(force) detaches the kernel driver.
    // libusb then claims the same interface on the same file descriptor,
    // which is a no-op for usbfs.
    const jint interfaceCount = m_usbDevice.callMethod<jint>("getInterfaceCount");
    for (jint i = 0; i < interfaceCount; ++i) {
        QJniObject usbInterface = m_usbDevice.callObjectMethod("getInterface",
                "(I)Landroid/hardware/usb/UsbInterface;",
                i);
        if (usbInterface.callMethod<jint>("getId") != interfaceNumber ||
                usbInterface.callMethod<jint>("getAlternateSetting") != 0) {
            continue;
        }
        if (!m_connection.callMethod<jboolean>("claimInterface",
                    "(Landroid/hardware/usb/UsbInterface;Z)Z",
                    usbInterface.object(),
                    jboolean(true))) {
            qWarning() << "Unable to claim USB interface" << interfaceNumber << "of"
                       << m_productName;
            return false;
        }
        const int error = libusb_claim_interface(m_pHandle, interfaceNumber);
        if (error < 0) {
            qWarning() << "libusb_claim_interface" << interfaceNumber
                       << "failed:" << libusb_error_name(error);
            return false;
        }
        m_claimedInterfaces.push_back(interfaceNumber);
        return true;
    }
    return false;
}

bool AndroidUsbAudioDevice::claim() {
    if (m_claimed) {
        return true;
    }
    if (!m_pHandle || m_disconnected) {
        return false;
    }
    // Class requests to the audio control interface (UAC2 clocks) need it
    // claimed too; UAC1 devices work without it
    const usbaudio::ControlInterface* pControl = m_description.controlFor(m_outputFormat);
    if (pControl && !claimInterface(pControl->interfaceNumber) &&
            m_outputFormat.uacVersion == 2) {
        return false;
    }
    if (!claimInterface(m_outputFormat.interfaceNumber)) {
        return false;
    }
    if (m_hasImplicitFeedback &&
            m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber &&
            !claimInterface(m_implicitFormat.interfaceNumber)) {
        return false;
    }
    // The kernel driver releases the whole sound card, so the MIDI interface
    // has to be driven by us too.
    if (m_description.midi.isValid() && !claimInterface(m_description.midi.interfaceNumber)) {
        m_description.midi = usbaudio::MidiInterface();
    }
    m_claimed = true;
    qInfo() << "Claimed" << m_productName << "from the kernel driver";
    if (m_outputFormat.uacVersion == 2) {
        queryUac2SampleRates();
    }
    startEventThread();
    // The Android MIDI device disappears, let the controller manager pick up
    // the direct USB-MIDI one.
    mixxx::android::notifyDevicesChanged();
    return true;
}

void AndroidUsbAudioDevice::queryUac2SampleRates() {
    const usbaudio::ControlInterface* pControl = m_description.controlFor(m_outputFormat);
    if (!pControl) {
        return;
    }
    const int clockId = pControl->clockSourceForTerminal(m_outputFormat.terminalLink);
    if (clockId < 0) {
        return;
    }
    const uint16_t index = static_cast<uint16_t>((clockId << 8) | pControl->interfaceNumber);
    // wNumSubRanges followed by (dMIN, dMAX, dRES) triplets of 4 bytes
    unsigned char buffer[2 + 12 * 32] = {};
    const int received = libusb_control_transfer(m_pHandle,
            static_cast<int>(LIBUSB_ENDPOINT_IN) |
                    static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                    static_cast<int>(LIBUSB_RECIPIENT_INTERFACE),
            kRequestRange,
            kSamplingFreqControl,
            index,
            buffer,
            sizeof(buffer),
            kControlTimeoutMillis);
    if (received < 2) {
        qWarning() << "Querying the sample rates of" << m_productName
                   << "failed:" << libusb_error_name(received);
        return;
    }
    const int ranges = std::min<int>(buffer[0] | (buffer[1] << 8), (received - 2) / 12);
    std::vector<int> rates;
    for (int i = 0; i < ranges; ++i) {
        const uint8_t* pRange = buffer + 2 + 12 * i;
        const int minRate = static_cast<int>(readLittleEndian(pRange, 4));
        const int maxRate = static_cast<int>(readLittleEndian(pRange + 4, 4));
        const int resolution = static_cast<int>(readLittleEndian(pRange + 8, 4));
        if (minRate == maxRate) {
            rates.push_back(minRate);
            continue;
        }
        for (int rate : kCommonSampleRates) {
            if (rate >= minRate && rate <= maxRate &&
                    (resolution <= 0 || (rate - minRate) % resolution == 0)) {
                rates.push_back(rate);
            }
        }
    }
    if (!rates.empty()) {
        m_outputFormat.sampleRates = rates;
        if (m_hasImplicitFeedback) {
            m_implicitFormat.sampleRates = rates;
        }
        qInfo() << m_productName << "supports" << QList<int>(rates.begin(), rates.end()) << "Hz";
    }
}

bool AndroidUsbAudioDevice::setSampleRate(const usbaudio::StreamFormat& format, int sampleRate) {
    int error = 0;
    if (format.uacVersion == 2) {
        const usbaudio::ControlInterface* pControl = m_description.controlFor(format);
        const int clockId = pControl ? pControl->clockSourceForTerminal(format.terminalLink) : -1;
        if (clockId < 0) {
            qWarning() << "No clock source found for" << m_productName;
            return true;
        }
        unsigned char data[4];
        for (int i = 0; i < 4; ++i) {
            data[i] = static_cast<unsigned char>((sampleRate >> (8 * i)) & 0xff);
        }
        error = libusb_control_transfer(m_pHandle,
                static_cast<int>(LIBUSB_ENDPOINT_OUT) |
                        static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                        static_cast<int>(LIBUSB_RECIPIENT_INTERFACE),
                kRequestCur,
                kSamplingFreqControl,
                static_cast<uint16_t>(
                        (clockId << 8) | pControl->interfaceNumber),
                data,
                sizeof(data),
                kControlTimeoutMillis);
    } else if (format.hasSampleRateControl || format.sampleRates.size() > 1 ||
            format.minRate > 0) {
        unsigned char data[3];
        for (int i = 0; i < 3; ++i) {
            data[i] = static_cast<unsigned char>((sampleRate >> (8 * i)) & 0xff);
        }
        error = libusb_control_transfer(m_pHandle,
                static_cast<int>(LIBUSB_ENDPOINT_OUT) |
                        static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                        static_cast<int>(LIBUSB_RECIPIENT_ENDPOINT),
                kRequestCur,
                kSamplingFreqControl,
                format.endpoint,
                data,
                sizeof(data),
                kControlTimeoutMillis);
    }
    if (error < 0) {
        qWarning() << "Setting" << sampleRate << "Hz on" << m_productName
                   << "failed:" << libusb_error_name(error);
        return false;
    }
    return true;
}

int AndroidUsbAudioDevice::readSampleRate(const usbaudio::StreamFormat& format) {
    unsigned char data[4] = {};
    int received = 0;
    if (format.uacVersion == 2) {
        const usbaudio::ControlInterface* pControl = m_description.controlFor(format);
        const int clockId = pControl ? pControl->clockSourceForTerminal(format.terminalLink) : -1;
        if (clockId < 0) {
            return 0;
        }
        received = libusb_control_transfer(m_pHandle,
                static_cast<int>(LIBUSB_ENDPOINT_IN) |
                        static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                        static_cast<int>(LIBUSB_RECIPIENT_INTERFACE),
                kRequestCur,
                kSamplingFreqControl,
                static_cast<uint16_t>(
                        (clockId << 8) | pControl->interfaceNumber),
                data,
                4,
                kControlTimeoutMillis);
    } else {
        received = libusb_control_transfer(m_pHandle,
                static_cast<int>(LIBUSB_ENDPOINT_IN) |
                        static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                        static_cast<int>(LIBUSB_RECIPIENT_ENDPOINT),
                kRequestGetCurUac1,
                kSamplingFreqControl,
                format.endpoint,
                data,
                3,
                kControlTimeoutMillis);
    }
    if (received < 3) {
        return 0;
    }
    return static_cast<int>(readLittleEndian(data, std::min(received, 4)));
}

void AndroidUsbAudioDevice::applySampleRate(int sampleRate) {
    // UAC2: the clock is set before the interface, UAC1: the endpoint control
    // needs the altsetting to be active
    if (m_outputFormat.uacVersion == 2) {
        setSampleRate(m_outputFormat, sampleRate);
    }
    libusb_set_interface_alt_setting(
            m_pHandle, m_outputFormat.interfaceNumber, m_outputFormat.altSetting);
    if (m_outputFormat.uacVersion == 1) {
        setSampleRate(m_outputFormat, sampleRate);
    }
    if (m_hasImplicitFeedback) {
        if (m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber) {
            libusb_set_interface_alt_setting(m_pHandle,
                    m_implicitFormat.interfaceNumber,
                    m_implicitFormat.altSetting);
        }
        if (m_implicitFormat.uacVersion == 1) {
            setSampleRate(m_implicitFormat, sampleRate);
        }
    }
}

void AndroidUsbAudioDevice::measureClock(uint64_t framesPerPacket16_16) {
    if (m_clockLogged) {
        return;
    }
    m_measuredFrames16_16 += framesPerPacket16_16;
    ++m_measuredPackets;
    if (m_measuredPackets < m_packetsPerSecond) {
        return;
    }
    m_clockLogged = true;
    const double rate = static_cast<double>(m_measuredFrames16_16) / 65536.0 *
            m_packetsPerSecond / m_measuredPackets;
    if (std::abs(rate - m_sampleRate) > m_sampleRate * 0.02) {
        qWarning() << m_productName << "plays at about" << qRound(rate) << "Hz instead of"
                   << m_sampleRate << "Hz: the music is too fast or too slow";
    } else {
        qInfo() << m_productName << "plays at about" << qRound(rate) << "Hz";
    }
}

void AndroidUsbAudioDevice::startEventThread() {
    if (m_eventThreadRunning) {
        return;
    }
    m_eventThreadRunning = true;
    m_eventThread = std::thread([this]() {
        pthread_setname_np(pthread_self(), "MixxxUsbAudio");
        raiseThreadPriority();
        while (m_eventThreadRunning) {
            timeval timeout{0, 50000};
            const int error = libusb_handle_events_timeout_completed(
                    m_pContext, &timeout, nullptr);
            if (error == LIBUSB_ERROR_NO_DEVICE) {
                markDisconnected();
                break;
            }
        }
    });
}

void AndroidUsbAudioDevice::stopEventThread() {
    if (!m_eventThreadRunning) {
        return;
    }
    m_eventThreadRunning = false;
    if (m_eventThread.joinable()) {
        m_eventThread.join();
    }
}

void AndroidUsbAudioDevice::markDisconnected() {
    if (!m_disconnected.exchange(true)) {
        qInfo() << "USB audio device" << m_productName << "disconnected";
        m_outputRunning = false;
        m_midiInRunning = false;
        mixxx::android::notifyDevicesChanged();
    }
}

bool AndroidUsbAudioDevice::startOutput(int sampleRate,
        int transferMillis,
        int numTransfers,
        RenderCallback renderCallback) {
    std::lock_guard lock(m_mutex);
    if (m_outputRunning) {
        return true;
    }
    if (!claim()) {
        return false;
    }
    if (!m_outputFormat.supportsRate(sampleRate)) {
        qWarning() << m_productName << "does not support" << sampleRate << "Hz";
        return false;
    }
    // Start from stopped streams: the device keeps its state, e.g. the rate
    // Android's own driver or the last session set, until it is unplugged
    libusb_set_interface_alt_setting(m_pHandle, m_outputFormat.interfaceNumber, 0);
    if (m_hasImplicitFeedback &&
            m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber) {
        libusb_set_interface_alt_setting(m_pHandle, m_implicitFormat.interfaceNumber, 0);
    }
    if (m_outputFormat.uacVersion == 2) {
        setSampleRate(m_outputFormat, sampleRate);
    }
    int error = libusb_set_interface_alt_setting(
            m_pHandle, m_outputFormat.interfaceNumber, m_outputFormat.altSetting);
    if (error < 0) {
        qWarning() << "Unable to select the audio altsetting:" << libusb_error_name(error);
        return false;
    }
    if (m_outputFormat.uacVersion == 1) {
        setSampleRate(m_outputFormat, sampleRate);
    }
    if (m_hasImplicitFeedback) {
        if (m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber) {
            error = libusb_set_interface_alt_setting(m_pHandle,
                    m_implicitFormat.interfaceNumber,
                    m_implicitFormat.altSetting);
            if (error < 0) {
                qWarning() << "Unable to select the capture altsetting:"
                           << libusb_error_name(error);
                return false;
            }
        }
        if (m_implicitFormat.uacVersion == 1) {
            setSampleRate(m_implicitFormat, sampleRate);
        }
    }
    // A device that didn't take the rate plays everything too fast or too
    // slow, so check it and try once more
    int deviceRate = readSampleRate(m_outputFormat);
    if (deviceRate > 0 && deviceRate != sampleRate) {
        qWarning() << m_productName << "is at" << deviceRate << "Hz instead of" << sampleRate
                   << "Hz, setting the rate again";
        libusb_set_interface_alt_setting(m_pHandle, m_outputFormat.interfaceNumber, 0);
        if (m_hasImplicitFeedback &&
                m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber) {
            libusb_set_interface_alt_setting(m_pHandle, m_implicitFormat.interfaceNumber, 0);
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
        applySampleRate(sampleRate);
        deviceRate = readSampleRate(m_outputFormat);
    }
    qInfo() << m_productName << "reports a rate of" << deviceRate << "Hz (0: unknown)";
    m_measuredFrames16_16 = 0;
    m_measuredPackets = 0;
    m_clockLogged = false;

    m_packetsPerSecond = usbaudio::packetsPerSecond(m_outputFormat.bInterval, isHighSpeed());
    const int packetsPerTransfer = std::max(1, transferMillis * m_packetsPerSecond / 1000);
    m_sampleRate = sampleRate;
    m_frameAccumulator = 0;
    m_nominalFramesPerPacket = static_cast<uint32_t>(
            (static_cast<uint64_t>(sampleRate) << 16) / m_packetsPerSecond);
    m_framesPerPacket = m_nominalFramesPerPacket;
    m_fractionalFrames = 0;
    m_implicitPacketFrames.clear();
    m_framesInFlight = 0;
    m_renderCallback = std::move(renderCallback);
    qInfo() << "Starting USB audio output on" << m_productName << sampleRate << "Hz,"
            << m_packetsPerSecond << "packets/s," << numTransfers << "x"
            << packetsPerTransfer << "packets";

    m_outputRunning = true;

    // Clock source of asynchronous outputs
    if (m_outputFormat.syncType == usbaudio::SyncType::Asynchronous) {
        const bool implicit = m_hasImplicitFeedback;
        const uint8_t endpoint = implicit ? m_implicitFormat.endpoint
                                          : m_outputFormat.feedbackEndpoint;
        const int transfers = implicit ? numTransfers : kFeedbackTransfers;
        const int packets = implicit ? packetsPerTransfer : kFeedbackPacketsPerTransfer;
        const int packetSize = implicit ? m_implicitFormat.maxPacketSize : kFeedbackPacketSize;
        for (int i = 0; i < transfers; ++i) {
            libusb_transfer* pTransfer = libusb_alloc_transfer(packets);
            if (!pTransfer) {
                break;
            }
            auto* pBuffer = static_cast<unsigned char*>(std::calloc(packets * packetSize, 1));
            libusb_fill_iso_transfer(pTransfer,
                    m_pHandle,
                    endpoint,
                    pBuffer,
                    packets * packetSize,
                    packets,
                    implicit ? implicitTransferCallback : feedbackTransferCallback,
                    this,
                    0);
            libusb_set_iso_packet_lengths(pTransfer, packetSize);
            pTransfer->flags = LIBUSB_TRANSFER_FREE_BUFFER;
            if (libusb_submit_transfer(pTransfer) < 0) {
                libusb_free_transfer(pTransfer);
                continue;
            }
            m_feedbackTransfers.push_back(pTransfer);
            ++m_feedbackTransfersPending;
        }
        if (m_feedbackTransfers.empty()) {
            qWarning() << "Unable to read the clock of" << m_productName
                       << ", playing at the nominal rate";
        }
    }

    const int bufferSize = packetsPerTransfer * m_outputFormat.maxPacketSize;
    for (int i = 0; i < numTransfers; ++i) {
        libusb_transfer* pTransfer = libusb_alloc_transfer(packetsPerTransfer);
        if (!pTransfer) {
            break;
        }
        auto* pBuffer = static_cast<unsigned char*>(std::calloc(bufferSize, 1));
        libusb_fill_iso_transfer(pTransfer,
                m_pHandle,
                m_outputFormat.endpoint,
                pBuffer,
                bufferSize,
                packetsPerTransfer,
                outputTransferCallback,
                this,
                0);
        pTransfer->flags = LIBUSB_TRANSFER_FREE_BUFFER;
        m_outputTransfers.push_back(pTransfer);
    }
    // The first transfers are silent: the engine must not run on this thread
    for (libusb_transfer* pTransfer : m_outputTransfers) {
        fillOutputTransfer(pTransfer, true);
        error = libusb_submit_transfer(pTransfer);
        if (error < 0) {
            qWarning() << "Submitting an USB audio transfer failed:" << libusb_error_name(error);
            continue;
        }
        ++m_outputTransfersPending;
    }
    if (m_outputTransfersPending == 0) {
        m_outputRunning = false;
        cancelAndFree(&m_feedbackTransfers, &m_feedbackTransfersPending);
        for (libusb_transfer* pTransfer : m_outputTransfers) {
            libusb_free_transfer(pTransfer);
        }
        m_outputTransfers.clear();
        return false;
    }
    return true;
}

int AndroidUsbAudioDevice::nextPacketFrames() {
    if (!m_implicitPacketFrames.empty()) {
        // Implicit feedback: play as many frames as the device captured
        const int frames = m_implicitPacketFrames.front();
        m_implicitPacketFrames.pop_front();
        return frames;
    }
    if (m_outputFormat.syncType == usbaudio::SyncType::Asynchronous) {
        m_fractionalFrames += m_framesPerPacket;
        const int frames = static_cast<int>(m_fractionalFrames >> 16);
        m_fractionalFrames &= 0xffff;
        return frames;
    }
    // Synchronous/adaptive endpoints play exactly sampleRate frames per
    // second, distributed over the packets (44/45 frames per ms at 44.1 kHz)
    m_frameAccumulator += m_sampleRate;
    const int frames = m_frameAccumulator / m_packetsPerSecond;
    m_frameAccumulator -= frames * m_packetsPerSecond;
    return frames;
}

void AndroidUsbAudioDevice::fillOutputTransfer(libusb_transfer* pTransfer, bool silent) {
    const int frameBytes = m_outputFormat.frameBytes();
    const int maxFramesPerPacket = m_outputFormat.maxPacketSize / frameBytes;
    int totalFrames = 0;
    for (int p = 0; p < pTransfer->num_iso_packets; ++p) {
        const int frames = std::min(nextPacketFrames(), maxFramesPerPacket);
        pTransfer->iso_packet_desc[p].length = frames * frameBytes;
        totalFrames += frames;
    }
    pTransfer->length = totalFrames * frameBytes;
    if (silent) {
        std::memset(pTransfer->buffer, 0, pTransfer->length);
    } else {
        m_renderCallback(pTransfer->buffer, totalFrames);
    }
    m_framesInFlight += totalFrames;
}

// static
void AndroidUsbAudioDevice::outputTransferCallback(libusb_transfer* pTransfer) {
    auto* pThis = static_cast<AndroidUsbAudioDevice*>(pTransfer->user_data);
    pThis->m_framesInFlight -= pTransfer->length / pThis->m_outputFormat.frameBytes();
    if (pTransfer->status == LIBUSB_TRANSFER_NO_DEVICE) {
        pThis->markDisconnected();
    }
    if (pThis->m_outputRunning && pTransfer->status != LIBUSB_TRANSFER_CANCELLED &&
            pTransfer->status != LIBUSB_TRANSFER_NO_DEVICE) {
        pThis->fillOutputTransfer(pTransfer, false);
        if (libusb_submit_transfer(pTransfer) == 0) {
            return;
        }
        qWarning() << "Resubmitting an USB audio transfer failed";
    }
    --pThis->m_outputTransfersPending;
}

// static
void AndroidUsbAudioDevice::feedbackTransferCallback(libusb_transfer* pTransfer) {
    auto* pThis = static_cast<AndroidUsbAudioDevice*>(pTransfer->user_data);
    if (pTransfer->status == LIBUSB_TRANSFER_COMPLETED) {
        for (int p = 0; p < pTransfer->num_iso_packets; ++p) {
            const libusb_iso_packet_descriptor& packet = pTransfer->iso_packet_desc[p];
            if (packet.status != LIBUSB_TRANSFER_COMPLETED || packet.actual_length < 3) {
                continue;
            }
            const int bytes = std::min<int>(packet.actual_length, 4);
            const uint32_t raw = readLittleEndian(
                    libusb_get_iso_packet_buffer_simple(pTransfer, p), bytes);
            const uint32_t framesPerPacket = usbaudio::feedbackToFramesPerPacket(
                    raw, bytes, pThis->m_nominalFramesPerPacket);
            if (framesPerPacket != 0) {
                pThis->m_framesPerPacket = framesPerPacket;
                pThis->measureClock(framesPerPacket);
            }
        }
    } else if (pTransfer->status == LIBUSB_TRANSFER_NO_DEVICE) {
        pThis->markDisconnected();
    }
    if (pThis->m_outputRunning && pTransfer->status != LIBUSB_TRANSFER_CANCELLED &&
            pTransfer->status != LIBUSB_TRANSFER_NO_DEVICE &&
            libusb_submit_transfer(pTransfer) == 0) {
        return;
    }
    --pThis->m_feedbackTransfersPending;
}

// static
void AndroidUsbAudioDevice::implicitTransferCallback(libusb_transfer* pTransfer) {
    auto* pThis = static_cast<AndroidUsbAudioDevice*>(pTransfer->user_data);
    if (pTransfer->status == LIBUSB_TRANSFER_COMPLETED) {
        const int frameBytes = pThis->m_implicitFormat.frameBytes();
        for (int p = 0; p < pTransfer->num_iso_packets; ++p) {
            const libusb_iso_packet_descriptor& packet = pTransfer->iso_packet_desc[p];
            if (packet.status != LIBUSB_TRANSFER_COMPLETED || frameBytes <= 0) {
                continue;
            }
            const int frames = static_cast<int>(packet.actual_length) / frameBytes;
            pThis->measureClock(static_cast<uint64_t>(frames) << 16);
            if (pThis->m_implicitPacketFrames.size() < kMaxImplicitQueue) {
                pThis->m_implicitPacketFrames.push_back(frames);
            }
        }
    } else if (pTransfer->status == LIBUSB_TRANSFER_NO_DEVICE) {
        pThis->markDisconnected();
    }
    if (pThis->m_outputRunning && pTransfer->status != LIBUSB_TRANSFER_CANCELLED &&
            pTransfer->status != LIBUSB_TRANSFER_NO_DEVICE &&
            libusb_submit_transfer(pTransfer) == 0) {
        return;
    }
    --pThis->m_feedbackTransfersPending;
}

void AndroidUsbAudioDevice::cancelAndFree(
        std::vector<libusb_transfer*>* pTransfers, std::atomic<int>* pPending) {
    if (pTransfers->empty()) {
        return;
    }
    for (libusb_transfer* pTransfer : *pTransfers) {
        libusb_cancel_transfer(pTransfer);
    }
    const auto deadline = std::chrono::steady_clock::now() +
            std::chrono::milliseconds(kStopTimeoutMillis);
    while (*pPending > 0 && std::chrono::steady_clock::now() < deadline) {
        if (m_eventThreadRunning) {
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        } else {
            timeval timeout{0, 10000};
            libusb_handle_events_timeout_completed(m_pContext, &timeout, nullptr);
        }
    }
    if (*pPending > 0) {
        qWarning() << "USB transfers of" << m_productName << "did not finish, leaking them";
    } else {
        for (libusb_transfer* pTransfer : *pTransfers) {
            libusb_free_transfer(pTransfer);
        }
    }
    pTransfers->clear();
    *pPending = 0;
}

void AndroidUsbAudioDevice::stopOutput() {
    std::lock_guard lock(m_mutex);
    if (m_outputTransfers.empty() && m_feedbackTransfers.empty()) {
        return;
    }
    m_outputRunning = false;
    cancelAndFree(&m_outputTransfers, &m_outputTransfersPending);
    cancelAndFree(&m_feedbackTransfers, &m_feedbackTransfersPending);
    m_framesInFlight = 0;
    if (!m_disconnected) {
        // Zero bandwidth altsettings
        libusb_set_interface_alt_setting(m_pHandle, m_outputFormat.interfaceNumber, 0);
        if (m_hasImplicitFeedback &&
                m_implicitFormat.interfaceNumber != m_outputFormat.interfaceNumber) {
            libusb_set_interface_alt_setting(m_pHandle, m_implicitFormat.interfaceNumber, 0);
        }
    }
}

bool AndroidUsbAudioDevice::startMidiInput(MidiCallback callback) {
    std::lock_guard lock(m_mutex);
    if (m_midiInRunning) {
        return true;
    }
    const usbaudio::MidiInterface& midi = m_description.midi;
    if (!midi.isValid() || midi.inEndpoint == 0 || !claim()) {
        return false;
    }
    m_midiCallback = std::move(callback);
    if (!m_pMidiInTransfer) {
        m_pMidiInTransfer = libusb_alloc_transfer(0);
        const int size = std::max(64, midi.inMaxPacketSize);
        auto* pBuffer = static_cast<unsigned char*>(std::calloc(size, 1));
        libusb_fill_bulk_transfer(m_pMidiInTransfer,
                m_pHandle,
                midi.inEndpoint,
                pBuffer,
                size,
                midiInTransferCallback,
                this,
                0);
        m_pMidiInTransfer->flags = LIBUSB_TRANSFER_FREE_BUFFER;
    }
    m_midiInRunning = true;
    if (libusb_submit_transfer(m_pMidiInTransfer) < 0) {
        m_midiInRunning = false;
        return false;
    }
    m_midiInPending = true;
    return true;
}

// static
void AndroidUsbAudioDevice::midiInTransferCallback(libusb_transfer* pTransfer) {
    auto* pThis = static_cast<AndroidUsbAudioDevice*>(pTransfer->user_data);
    if (pTransfer->status == LIBUSB_TRANSFER_COMPLETED) {
        for (int i = 0; i + 4 <= pTransfer->actual_length; i += 4) {
            const uint8_t* pPacket = pTransfer->buffer + i;
            if (pPacket[0] != 0 && pThis->m_midiCallback) {
                pThis->m_midiCallback(pPacket);
            }
        }
    } else if (pTransfer->status == LIBUSB_TRANSFER_NO_DEVICE) {
        pThis->markDisconnected();
    }
    if (pThis->m_midiInRunning && pTransfer->status != LIBUSB_TRANSFER_CANCELLED &&
            pTransfer->status != LIBUSB_TRANSFER_NO_DEVICE &&
            libusb_submit_transfer(pTransfer) == 0) {
        return;
    }
    pThis->m_midiInPending = false;
}

void AndroidUsbAudioDevice::stopMidiInput() {
    std::lock_guard lock(m_mutex);
    if (!m_pMidiInTransfer) {
        return;
    }
    m_midiInRunning = false;
    if (m_midiInPending) {
        libusb_cancel_transfer(m_pMidiInTransfer);
        const auto deadline = std::chrono::steady_clock::now() +
                std::chrono::milliseconds(kStopTimeoutMillis);
        while (m_midiInPending && std::chrono::steady_clock::now() < deadline) {
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
    }
    if (!m_midiInPending) {
        libusb_free_transfer(m_pMidiInTransfer);
    }
    m_pMidiInTransfer = nullptr;
    m_midiCallback = nullptr;
}

bool AndroidUsbAudioDevice::sendMidi(const uint8_t* pData, int length) {
    const uint8_t endpoint = m_description.midi.outEndpoint;
    if (!m_claimed || m_disconnected || endpoint == 0) {
        return false;
    }
    std::vector<unsigned char> buffer(pData, pData + length);
    int transferred = 0;
    const int error = libusb_bulk_transfer(m_pHandle,
            endpoint,
            buffer.data(),
            length,
            &transferred,
            100);
    if (error == LIBUSB_ERROR_NO_DEVICE) {
        markDisconnected();
    }
    return error == 0 && transferred == length;
}

} // namespace androidusb
} // namespace mixxx
