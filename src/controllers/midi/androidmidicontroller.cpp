#ifdef __ANDROID__

#include "controllers/midi/androidmidicontroller.h"

#include <QCoreApplication>
#include <QtJniTypes>
#include <android/log.h>

#include "controllers/defs_controllers.h"
#include "moc_androidmidicontroller.cpp"

namespace {
const mixxx::Logger kLogger("AndroidMidiController");
}

AndroidMidiController::AndroidMidiController(const QString& name,
        const QJniObject& midiDeviceInfo,
        int inputPortIndex,
        int outputPortIndex)
        : MidiController(name),
          m_deviceInfo(midiDeviceInfo),
          m_inputPortIndex(inputPortIndex),
          m_outputPortIndex(outputPortIndex) {
    // Extract device properties from MidiDeviceInfo
    QJniObject props = midiDeviceInfo.callObjectMethod(
            "getProperties", "()Landroid/os/Bundle;");
    if (props.isValid()) {
        QJniObject nameStr = props.callObjectMethod(
                "getString",
                "(Ljava/lang/String;)Ljava/lang/String;",
                QJniObject::fromString(
                        "android.media.midi.extra.PROPERTY_NAME")
                        .object());
        m_product = nameStr.isValid() ? nameStr.toString() : name;

        QJniObject mfrStr = props.callObjectMethod(
                "getString",
                "(Ljava/lang/String;)Ljava/lang/String;",
                QJniObject::fromString(
                        "android.media.midi.extra.PROPERTY_MANUFACTURER")
                        .object());
        m_vendor = mfrStr.isValid() ? mfrStr.toString() : QString();

        // Try to get USB VID/PID from the USB device property
        QJniObject usbDev = props.callObjectMethod(
                "getParcelable",
                "(Ljava/lang/String;)Landroid/os/Parcelable;",
                QJniObject::fromString(
                        "android.media.midi.extra.PROPERTY_USB_DEVICE")
                        .object());
        if (usbDev.isValid()) {
            m_vendorId = static_cast<uint16_t>(
                    usbDev.callMethod<jint>("getVendorId"));
            m_productId = static_cast<uint16_t>(
                    usbDev.callMethod<jint>("getProductId"));
        }
    }
}

AndroidMidiController::~AndroidMidiController() {
    close();
}

int AndroidMidiController::open(const QString& resourcePath) {
    Q_UNUSED(resourcePath);
    if (m_pIoThread && m_pIoThread->isRunning()) {
        return 0;
    }

    // Get MidiManager from Android context
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    QJniObject MIDI_SERVICE =
            QJniObject::getStaticObjectField(
                    "android/content/Context",
                    "MIDI_SERVICE",
                    "Ljava/lang/String;");
    auto midiManager = context.callObjectMethod(
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            MIDI_SERVICE.object());

    if (!midiManager.isValid()) {
        qCWarning(kLogger) << "Cannot get MidiManager";
        return 1;
    }

    // Open the device
    midiManager.callMethod<void>(
            "openDevice",
            "(Landroid/media/midi/MidiDeviceInfo;"
            "Landroid/media/midi/MidiManager$OnDeviceOpenedListener;"
            "Landroid/os/Handler;)V",
            m_deviceInfo.object(),
            nullptr,  // listener — we'll use a Java helper instead
            nullptr); // handler

    // Use Java helper for port management
    QJniObject helper("org/mixxx/AndroidMidiHelper", "()V");
    if (!helper.isValid()) {
        qCWarning(kLogger) << "Cannot create AndroidMidiHelper";
        return 1;
    }

    // Open device via helper (Java handles the async callback)
    helper.callMethod<jboolean>(
            "open",
            "(Landroid/media/midi/MidiManager;"
            "Landroid/media/midi/MidiDeviceInfo;I)Z",
            midiManager.object(),
            m_deviceInfo.object(),
            static_cast<jint>(0)); // controller ID placeholder

    // Give Android time to open the device asynchronously
    for (int i = 0; i < 20; i++) {
        QCoreApplication::processEvents();
        QThread::msleep(50);
    }

    // Open ports
    helper.callMethod<jboolean>(
            "openPorts", "(II)Z",
            static_cast<jint>(m_inputPortIndex),
            static_cast<jint>(m_outputPortIndex));

    // Start I/O thread for reading
    m_pIoThread = new IoThread(this, m_deviceInfo,
            m_inputPortIndex, m_outputPortIndex);
    m_pIoThread->start();

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
    // I/O thread handles receiving; nothing to do here
    return m_pIoThread && m_pIoThread->isRunning();
}

bool AndroidMidiController::isPolling() const {
    return m_pIoThread && m_pIoThread->isRunning();
}

void AndroidMidiController::sendShortMsg(
        unsigned char status, unsigned char byte1, unsigned char byte2) {
    QByteArray data(3, '\0');
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
        AndroidMidiController* parent,
        const QJniObject& midiDevice,
        int inputPortIndex,
        int outputPortIndex)
        : m_parent(parent),
          m_inputPortIndex(inputPortIndex) {
    Q_UNUSED(midiDevice);
}

void AndroidMidiController::IoThread::stop() {
    m_stop.storeRelaxed(1);
}

void AndroidMidiController::IoThread::send(const QByteArray& data) {
    QMutexLocker lock(&m_sendMutex);
    m_pendingSend = data;
    m_hasPending = true;
}

void AndroidMidiController::IoThread::run() {
    __android_log_print(ANDROID_LOG_INFO, "mixxx",
            "AndroidMidiController I/O thread started");

    // The Java callback (midiReceive) handles incoming data.
    // This thread just processes sends and keeps alive.
    while (!m_stop.loadRelaxed()) {
        // Process pending sends
        {
            QMutexLocker lock(&m_sendMutex);
            if (m_hasPending && m_outputPort.isValid()) {
                QJniObject data = QJniObject::fromLocalBuffer(
                        m_pendingSend.constData(),
                        m_pendingSend.size());
                m_outputPort.callMethod<void>(
                        "write", "([BII)V",
                        data.object(),
                        static_cast<jint>(0),
                        static_cast<jint>(m_pendingSend.size()));
                m_hasPending = false;
            }
        }
        msleep(5);
    }

    // Close ports
    if (m_inputPort.isValid()) {
        m_inputPort.callMethod<void>("close");
    }
    if (m_outputPort.isValid()) {
        m_outputPort.callMethod<void>("close");
    }
    __android_log_print(ANDROID_LOG_INFO, "mixxx",
            "AndroidMidiController I/O thread stopped");
}

#endif // __ANDROID__
