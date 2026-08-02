#ifdef __ANDROID__

#include "controllers/midi/androidmidienumerator.h"

#include <QJniObject>
#include <QtJniTypes>

#include "controllers/defs_controllers.h"
#include "controllers/midi/androidmidicontroller.h"
#include "moc_androidmidienumerator.cpp"

namespace {
const mixxx::Logger kLogger("AndroidMidiEnumerator");
}

AndroidMidiEnumerator::AndroidMidiEnumerator()
        : MidiEnumerator() {
}

AndroidMidiEnumerator::~AndroidMidiEnumerator() {
    while (!m_devices.isEmpty()) {
        delete m_devices.takeLast();
    }
}

QList<Controller*> AndroidMidiEnumerator::queryDevices() {
    qCInfo(kLogger) << "Scanning Android MIDI devices";

    while (!m_devices.isEmpty()) {
        delete m_devices.takeLast();
    }

    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        qCWarning(kLogger) << "Android context is null";
        return {};
    }

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
        return {};
    }

    auto deviceInfoArray = midiManager.callObjectMethod(
            "getDevices", "()[Landroid/media/midi/MidiDeviceInfo;");

    if (!deviceInfoArray.isValid()) {
        qCInfo(kLogger) << "No Android MIDI devices found";
        return {};
    }

    QJniEnvironment env;
    jsize count = env->GetArrayLength(
            static_cast<jobjectArray>(deviceInfoArray.object()));
    qCInfo(kLogger) << "Found" << count << "Android MIDI devices";

    for (jsize i = 0; i < count; i++) {
        QJniObject deviceInfo = env->GetObjectArrayElement(
                static_cast<jobjectArray>(deviceInfoArray.object()), i);

        if (!deviceInfo.isValid()) {
            continue;
        }

        // Get input/output port counts
        jint inputPorts = deviceInfo.callMethod<jint>(
                "getInputPortCount");
        jint outputPorts = deviceInfo.callMethod<jint>(
                "getOutputPortCount");

        // Get device name
        QJniObject props = deviceInfo.callObjectMethod(
                "getProperties", "()Landroid/os/Bundle;");
        QString name;
        if (props.isValid()) {
            QJniObject nameStr = props.callObjectMethod(
                    "getString",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    QJniObject::fromString(
                            "android.media.midi.extra.PROPERTY_NAME")
                            .object());
            if (nameStr.isValid()) {
                name = nameStr.toString();
            }
        }
        if (name.isEmpty()) {
            name = QStringLiteral("Android MIDI Device %1").arg(i);
        }

        qCInfo(kLogger) << "MIDI device:" << name
                         << "inputs:" << inputPorts
                         << "outputs:" << outputPorts;

        // Use the first available input and output ports
        int inputIdx = inputPorts > 0 ? 0 : -1;
        int outputIdx = outputPorts > 0 ? 0 : -1;

        auto* controller = new AndroidMidiController(
                name, deviceInfo, inputIdx, outputIdx);
        m_devices.push_back(controller);
    }

    return m_devices;
}

#endif // __ANDROID__
