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

    jsize count = 0;
    if (deviceInfoArray.isValid()) {
        QJniEnvironment env;
        count = env->GetArrayLength(
                static_cast<jobjectArray>(deviceInfoArray.object()));
    }

    // ── FALLBACK: MidiManager sees nothing when audio HAL claims the device ──
    // Scan UsbManager directly for USB devices with MIDI interfaces (class 1, subclass 3).
    if (count == 0) {
        kLogger.info() << "MidiManager returned 0 devices — falling back to UsbManager scan";
        QJniObject USB_SERVICE = QJniObject::getStaticObjectField(
                "android/content/Context", "USB_SERVICE", "Ljava/lang/String;");
        auto usbManager = context.callObjectMethod("getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                USB_SERVICE.object());

        if (usbManager.isValid()) {
            auto usbDeviceMap = usbManager.callObjectMethod(
                    "getDeviceList", "()Ljava/util/HashMap;");
            if (usbDeviceMap.isValid()) {
                auto values = usbDeviceMap.callObjectMethod(
                        "values", "()Ljava/util/Collection;");
                auto valuesArray = values.callObjectMethod(
                        "toArray", "()[Ljava/lang/Object;");

                if (valuesArray.isValid()) {
                    QJniEnvironment env2;
                    jsize usbCount = env2->GetArrayLength(
                            static_cast<jobjectArray>(valuesArray.object()));
                    kLogger.info() << "UsbManager found" << usbCount << "USB devices";

                    for (jsize j = 0; j < usbCount; j++) {
                        QJniObject usbDevice = env2->GetObjectArrayElement(
                                static_cast<jobjectArray>(valuesArray.object()), j);
                        if (!usbDevice.isValid())
                            continue;

                        jint vid = usbDevice.callMethod<jint>("getVendorId");
                        jint pid = usbDevice.callMethod<jint>("getProductId");
                        jint ifaceCount = usbDevice.callMethod<jint>("getInterfaceCount");

                        // Scan interfaces for MIDI class (class=1, subclass=3)
                        int midiIface = -1;
                        for (jint ifIdx = 0; ifIdx < ifaceCount; ifIdx++) {
                            auto iface = usbDevice.callObjectMethod("getInterface",
                                    "(I)Landroid/hardware/usb/UsbInterface;",
                                    ifIdx);
                            jint ifaceClass = iface.callMethod<jint>("getInterfaceClass");
                            jint ifaceSubclass = iface.callMethod<jint>("getInterfaceSubclass");
                            if (ifaceClass == 1 && ifaceSubclass == 3) {
                                midiIface = ifIdx;
                                break;
                            }
                        }

                        if (midiIface < 0)
                            continue;

                        // Try to get product name via USB device descriptor
                        QString name;
                        QString vendorStr;
                        QString productStr;
                        jint productIdJni = usbDevice.callMethod<jint>("getProductId");
                        jint vendorIdJni = usbDevice.callMethod<jint>("getVendorId");

                        // Try getProductName via JNI (custom method may not exist on all devices)
                        QJniObject devName = usbDevice.callObjectMethod(
                                "getProductName", "()Ljava/lang/String;");
                        if (devName.isValid()) {
                            productStr = devName.toString();
                        }
                        QJniObject mfrName = usbDevice.callObjectMethod(
                                "getManufacturerName", "()Ljava/lang/String;");
                        if (mfrName.isValid()) {
                            vendorStr = mfrName.toString();
                        }

                        if (productStr.isEmpty() && vendorStr.isEmpty()) {
                            productStr = QStringLiteral("USB MIDI %1:%2")
                                                 .arg(vid, 4, 16, QChar('0'))
                                                 .arg(pid, 4, 16, QChar('0'));
                        }
                        name = vendorStr.isEmpty()
                                ? productStr
                                : (productStr.isEmpty()
                                                  ? vendorStr
                                                  : QStringLiteral("%1 %2").arg(
                                                            vendorStr,
                                                            productStr));

                        kLogger.info() << "USB MIDI device (UsbManager):" << name
                                       << QStringLiteral("VID:0x%1 PID:0x%2")
                                                  .arg(vid, 4, 16, QChar('0'))
                                                  .arg(pid, 4, 16, QChar('0'))
                                       << "interface:" << midiIface;

                        auto* controller = new AndroidMidiController(name,
                                usbDevice,
                                midiIface,
                                static_cast<uint16_t>(vid),
                                static_cast<uint16_t>(pid),
                                vendorStr,
                                productStr);
                        m_devices.push_back(controller);
                    }
                }
            }
        }

        kLogger.info() << "Enumerated" << m_devices.size()
                       << "Android MIDI controllers (UsbManager fallback)";
        return m_devices;
    }

    if (!deviceInfoArray.isValid()) {
        kLogger.info() << "No Android MIDI devices found";
        return {};
    }

    QJniEnvironment env;
    count = env->GetArrayLength(
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

        if (usbDevice.isValid()) {
            jint ifaceCount = usbDevice.callMethod<jint>("getInterfaceCount");
            for (jint ifIdx = 0; ifIdx < ifaceCount; ifIdx++) {
                auto iface = usbDevice.callObjectMethod("getInterface",
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
                    break;
                }
            }
        }

        if (ifaceNum < 0) {
            kLogger.info() << "No USB MIDI interface found for" << name
                           << "- skipping (Bluetooth MIDI?)";
            continue;
        }

        auto* controller = new AndroidMidiController(name,
                usbDevice,
                ifaceNum,
                vendorId,
                productId,
                vendorStr,
                productStr);
        m_devices.push_back(controller);
    }

    kLogger.info() << "Enumerated" << m_devices.size()
                   << "Android MIDI controllers";
    return m_devices;
}

#endif // __ANDROID__
