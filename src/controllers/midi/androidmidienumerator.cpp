#include "controllers/midi/androidmidienumerator.h"

#include <QJniEnvironment>
#include <QJniObject>
#include <QSet>

#include "controllers/midi/androidmidicontroller.h"
#include "moc_androidmidienumerator.cpp"
#include "soundio/androidusbaudiodevice.h"

AndroidMidiEnumerator::AndroidMidiEnumerator()
        : MidiEnumerator() {
}

AndroidMidiEnumerator::~AndroidMidiEnumerator() {
    qDebug() << "Deleting Android MIDI devices...";
    qDeleteAll(m_devices);
    m_devices.clear();
    qDeleteAll(m_usbDevices);
    m_usbDevices.clear();
    qDeleteAll(m_removedDevices);
    m_removedDevices.clear();
}

QList<Controller*> AndroidMidiEnumerator::queryDevices() {
    qDebug() << "Scanning Android MIDI devices:";
    if (!AndroidMidiController::isSupported()) {
        qWarning() << "Android MIDI is not supported on this device (requires Android 10)";
        return {};
    }

    QJniObject devices = QJniObject::callStaticMethod<jobjectArray>(
            "org/mixxx/MidiHelper",
            "getDevices",
            "()[Landroid/media/midi/MidiDeviceInfo;");
    QJniEnvironment env;
    const jsize count = devices.isValid()
            ? env->GetArrayLength(devices.object<jobjectArray>())
            : 0;

    QSet<int> presentIds;
    QSet<QString> usedNames;
    // Directly driven USB devices first, they get the plain product name
    updateUsbDevices(&usedNames);
    QSet<QString> usbDeviceNames = usedNames;
    for (const auto* pDevice : std::as_const(m_devices)) {
        usedNames.insert(pDevice->getName());
    }
    for (jsize i = 0; i < count; ++i) {
        QJniObject deviceInfo = QJniObject::fromLocalRef(
                env->GetObjectArrayElement(devices.object<jobjectArray>(), i));
        if (!deviceInfo.isValid()) {
            continue;
        }
        const int id = deviceInfo.callMethod<jint>("getId");
        if (usbDeviceNames.contains(QJniObject::callStaticMethod<jstring>(
                    "org/mixxx/MidiHelper",
                    "getName",
                    "(Landroid/media/midi/MidiDeviceInfo;)Ljava/lang/String;",
                    deviceInfo.object())
                            .toString())) {
            // Stale entry of a device that Mixxx now drives over USB directly
            continue;
        }
        presentIds.insert(id);

        const bool alreadyKnown = std::any_of(m_devices.cbegin(),
                m_devices.cend(),
                [id](const AndroidMidiController* pDevice) {
                    return pDevice->androidDeviceId() == id;
                });
        if (alreadyKnown) {
            continue;
        }
        if (deviceInfo.callMethod<jint>("getInputPortCount") == 0 &&
                deviceInfo.callMethod<jint>("getOutputPortCount") == 0) {
            continue;
        }

        // The name is used as configuration key, so it must be stable. Only
        // disambiguate when two identical devices are connected.
        const QString baseName = QJniObject::callStaticMethod<jstring>(
                "org/mixxx/MidiHelper",
                "getName",
                "(Landroid/media/midi/MidiDeviceInfo;)Ljava/lang/String;",
                deviceInfo.object())
                                         .toString();
        QString name = baseName;
        for (int n = 2; usedNames.contains(name); ++n) {
            name = QStringLiteral("%1 #%2").arg(baseName, QString::number(n));
        }
        usedNames.insert(name);

        qInfo() << "Found Android MIDI device:" << name << "id" << id;
        m_devices.append(new AndroidMidiController(deviceInfo, name));
    }

    for (int i = m_devices.size() - 1; i >= 0; --i) {
        AndroidMidiController* pDevice = m_devices.at(i);
        if (presentIds.contains(pDevice->androidDeviceId())) {
            continue;
        }
        // ControllerManager closes controllers that disappeared from the list
        qInfo() << "Android MIDI device removed:" << pDevice->getName();
        m_devices.removeAt(i);
        m_removedDevices.append(pDevice);
    }

    QList<Controller*> result;
    result.reserve(m_usbDevices.size() + m_devices.size());
    for (auto* pDevice : std::as_const(m_usbDevices)) {
        result.append(pDevice);
    }
    for (auto* pDevice : std::as_const(m_devices)) {
        result.append(pDevice);
    }
    return result;
}

void AndroidMidiEnumerator::updateUsbDevices(QSet<QString>* pUsedNames) {
    const auto usbAudioDevices = mixxx::androidusb::AndroidUsbAudioDevice::devices();
    QSet<QString> present;
    for (const auto& pUsbDevice : usbAudioDevices) {
        if (!pUsbDevice->isClaimed() || !pUsbDevice->midiInterface().isValid()) {
            continue;
        }
        present.insert(pUsbDevice->usbDeviceName());
        const bool alreadyKnown = std::any_of(m_usbDevices.cbegin(),
                m_usbDevices.cend(),
                [&](const AndroidUsbMidiController* pDevice) {
                    return pDevice->usbDevice() == pUsbDevice;
                });
        if (!alreadyKnown) {
            qInfo() << "Found direct USB MIDI device:" << pUsbDevice->productName();
            m_usbDevices.append(new AndroidUsbMidiController(pUsbDevice));
        }
    }
    for (int i = m_usbDevices.size() - 1; i >= 0; --i) {
        AndroidUsbMidiController* pDevice = m_usbDevices.at(i);
        if (present.contains(pDevice->usbDevice()->usbDeviceName())) {
            pUsedNames->insert(pDevice->getName());
            continue;
        }
        // ControllerManager closes controllers that disappeared from the list
        qInfo() << "Direct USB MIDI device removed:" << pDevice->getName();
        m_usbDevices.removeAt(i);
        m_removedDevices.append(pDevice);
    }
}
