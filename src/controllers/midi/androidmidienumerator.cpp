#ifdef __ANDROID__

#include "controllers/midi/androidmidienumerator.h"

#include <QJniObject>
#include <QtJniTypes>

#include "controllers/defs_controllers.h"
#include "controllers/midi/androidmidicontroller.h"
#include "moc_androidmidienumerator.cpp"

namespace {
const mixxx::Logger kLogger("AndroidMidiEnumerator");
} // namespace

AndroidMidiEnumerator::AndroidMidiEnumerator()
        : MidiEnumerator() {
}

AndroidMidiEnumerator::~AndroidMidiEnumerator() {
    while (!m_devices.isEmpty()) {
        delete m_devices.takeLast();
    }
}

QList<Controller*> AndroidMidiEnumerator::queryDevices() {
    kLogger.info() << "Scanning Android MIDI devices";

    while (!m_devices.isEmpty()) {
        delete m_devices.takeLast();
    }

    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        kLogger.warning() << "Android context is null";
        return {};
    }

    QJniObject MIDI_SERVICE =
            QJniObject::getStaticObjectField("android/content/Context",
                    "MIDI_SERVICE",
                    "Ljava/lang/String;");
    auto midiManager = context.callObjectMethod("getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            MIDI_SERVICE.object());

    if (!midiManager.isValid()) {
        kLogger.warning() << "Cannot get MidiManager";
        return {};
    }

    auto deviceInfoArray = midiManager.callObjectMethod(
            "getDevices", "()[Landroid/media/midi/MidiDeviceInfo;");

    if (!deviceInfoArray.isValid()) {
        kLogger.info() << "No Android MIDI devices found";
        return {};
    }

    QJniEnvironment env;
    jsize count = env->GetArrayLength(
            static_cast<jobjectArray>(deviceInfoArray.object()));
    kLogger.info() << "Found" << count << "Android MIDI devices";

    for (jsize i = 0; i < count; i++) {
        QJniObject deviceInfo = env->GetObjectArrayElement(
                static_cast<jobjectArray>(deviceInfoArray.object()), i);

        if (!deviceInfo.isValid()) {
            continue;
        }

        jint inputPorts = deviceInfo.callMethod<jint>("getInputPortCount");
        jint outputPorts = deviceInfo.callMethod<jint>("getOutputPortCount");

        QJniObject props = deviceInfo.callObjectMethod(
                "getProperties", "()Landroid/os/Bundle;");
        QString name;
        QString vendorStr;
        QString productStr;
        uint16_t vendorId = 0;
        uint16_t productId = 0;
        QJniObject usbDevice;

        if (props.isValid()) {
            QJniObject nameStr = props.callObjectMethod("getString",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    QJniObject::fromString(
                            "android.media.midi.extra.PROPERTY_NAME")
                            .object());
            if (nameStr.isValid()) {
                name = nameStr.toString();
            }

            QJniObject mfrStr = props.callObjectMethod("getString",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    QJniObject::fromString(
                            "android.media.midi.extra.PROPERTY_MANUFACTURER")
                            .object());
            if (mfrStr.isValid()) {
                vendorStr = mfrStr.toString();
            }

            QJniObject prodStr = props.callObjectMethod("getString",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    QJniObject::fromString(
                            "android.media.midi.extra.PROPERTY_PRODUCT")
                            .object());
            if (prodStr.isValid()) {
                productStr = prodStr.toString();
            }

            // Get USB device for bulk transfer
            usbDevice = props.callObjectMethod("getParcelable",
                    "(Ljava/lang/String;)Landroid/os/Parcelable;",
                    QJniObject::fromString(
                            "android.media.midi.extra.PROPERTY_USB_DEVICE")
                            .object());
            if (usbDevice.isValid()) {
                vendorId = static_cast<uint16_t>(
                        usbDevice.callMethod<jint>("getVendorId"));
                productId = static_cast<uint16_t>(
                        usbDevice.callMethod<jint>("getProductId"));
            }
        }

        if (name.isEmpty()) {
            name = QStringLiteral("Android MIDI Device %1").arg(i);
        }

        kLogger.info() << "MIDI device:" << name
                       << "inputs:" << inputPorts
                       << "outputs:" << outputPorts
                       << QStringLiteral("VID:0x%1 PID:0x%2")
                                  .arg(vendorId, 4, 16, QChar('0'))
                                  .arg(productId, 4, 16, QChar('0'));

        // Check USB interfaces for MIDI streaming (class 1, subclass 3)
        int ifaceNum = -1;
        uint8_t bulkInEp = 0x81;  // default IN endpoint
        uint8_t bulkOutEp = 0x01; // default OUT endpoint

        if (usbDevice.isValid()) {
            jint ifaceCount = usbDevice.callMethod<jint>("getInterfaceCount");
            for (jint ifIdx = 0; ifIdx < ifaceCount; ifIdx++) {
                auto iface = usbDevice.callMethod<QJniObject>("getInterface",
                        "(I)Landroid/hardware/usb/UsbInterface;",
                        ifIdx);
                jint ifaceClass = iface.callMethod<jint>("getInterfaceClass");
                jint ifaceSubclass =
                        iface.callMethod<jint>("getInterfaceSubclass");
                if (ifaceClass == 1 && ifaceSubclass == 3) {
                    ifaceNum = ifIdx;
                    kLogger.info()
                            << "Found MIDI interface" << ifIdx
                            << "on" << name;
                    // Try to get actual endpoint addresses
                    jint epCount = iface.callMethod<jint>("getEndpointCount");
                    for (jint ep = 0; ep < epCount; ep++) {
                        auto endpoint =
                                iface.callMethod<QJniObject>("getEndpoint",
                                        "(I)Landroid/hardware/usb/UsbEndpoint;",
                                        ep);
                        jint epAddr =
                                endpoint.callMethod<jint>("getEndpointAddress");
                        jint epDir =
                                endpoint.callMethod<jint>("getDirection");
                        if (epDir == 0x80) { // IN
                            bulkInEp = static_cast<uint8_t>(epAddr);
                        } else { // OUT
                            bulkOutEp = static_cast<uint8_t>(epAddr);
                        }
                    }
                    break;
                }
            }
        }

        if (ifaceNum < 0) {
            kLogger.info() << "No USB MIDI interface found for" << name
                           << "- skipping (Bluetooth MIDI?)";
            continue;
        }

        auto* controller = new AndroidMidiController(
                name, usbDevice, ifaceNum, bulkInEp, bulkOutEp, vendorId, productId, vendorStr, productStr);
        m_devices.push_back(controller);
    }

    kLogger.info() << "Enumerated" << m_devices.size()
                   << "Android MIDI controllers";
    return m_devices;
}

#endif // __ANDROID__
