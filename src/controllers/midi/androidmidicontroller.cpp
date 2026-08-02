#ifdef __ANDROID__

#include "controllers/midi/androidmidicontroller.h"

#include <android/log.h>

#include <QCoreApplication>
#include <QtJniTypes>

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
        kLogger.warning() << "Cannot get MidiManager";
        return 1;
    }

    midiManager.callMethod<void>(
            "openDevice",
            "(Landroid/media/midi/MidiDeviceInfo;"
            "Landroid/media/midi/MidiManager$OnDeviceOpenedListener;"
            "Landroid/os/Handler;)V",
            m_deviceInfo.object(),
            nullptr,
            nullptr);

    QJniObject helper("org/mixxx/AndroidMidiHelper", "()V");
    if (!helper.isValid()) {
        kLogger.warning() << "Cannot create AndroidMidiHelper";
        return 1;
    }

    helper.callMethod<jboolean>(
            "open",
            "(Landroid/media/midi/MidiManager;"
            "Landroid/media/midi/MidiDeviceInfo;I)Z",
            midiManager.object(),
            m_deviceInfo.object(),
            static_cast<jint>(0));

    for (int i = 0; i < 20; i++) {
        QCoreApplication::processEvents();
        QThread::msleep(50);
    }

    helper.callMethod<jboolean>(
            "openPorts", "(II)Z", static_cast<jint>(m_inputPortIndex), static_cast<jint>(m_outputPortIndex));

    m_pIoThread = new IoThread(this);
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
        AndroidMidiController* /*parent*/) {
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
    __android_log_print(ANDROID_LOG_INFO, "mixxx", "AndroidMidiController I/O thread started");

    QJniEnvironment env;
    while (!m_stop.loadRelaxed()) {
        {
            QMutexLocker lock(&m_sendMutex);
            if (m_hasPending && m_outputPort.isValid()) {
                jbyteArray jdata = env->NewByteArray(
                        m_pendingSend.size());
                if (jdata) {
                    env->SetByteArrayRegion(jdata, 0, m_pendingSend.size(), reinterpret_cast<const jbyte*>(m_pendingSend.constData()));
                    m_outputPort.callMethod<void>(
                            "write", "([BII)V", jdata, static_cast<jint>(0), static_cast<jint>(m_pendingSend.size()));
                    env->DeleteLocalRef(jdata);
                }
                m_hasPending = false;
            }
        }
        msleep(5);
    }

    if (m_inputPort.isValid()) {
        m_inputPort.callMethod<void>("close");
    }
    if (m_outputPort.isValid()) {
        m_outputPort.callMethod<void>("close");
    }
    __android_log_print(ANDROID_LOG_INFO, "mixxx", "AndroidMidiController I/O thread stopped");
}

#endif // __ANDROID__
