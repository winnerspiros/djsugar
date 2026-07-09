#include "controllers/midi/usbmidienumerator.h"

#ifdef Q_OS_ANDROID

#include <QJniObject>

#include "controllers/midi/usbmidicontroller.h"
#include "util/logger.h"

namespace {

const mixxx::Logger kLogger("UsbMidiEnumerator");
constexpr const char* kUsbMidiJavaClass = "org/mixxx/UsbMidiDevice";

} // namespace

UsbMidiEnumerator::UsbMidiEnumerator()
        : MidiEnumerator() {
}

UsbMidiEnumerator::~UsbMidiEnumerator() {
    m_devices.clear();
}

QList<Controller*> UsbMidiEnumerator::queryDevices() {
    m_devices.clear();

    // Initialize the Android MIDI manager
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        kLogger.warning() << "No Android context available for USB MIDI";
        return m_devices;
    }

    bool initialized = QJniObject::callStaticMethod<jboolean>(
            kUsbMidiJavaClass,
            "initialize",
            "(Landroid/content/Context;)Z",
            context.object<jobject>());

    if (!initialized) {
        kLogger.warning() << "Failed to initialize Android MIDI manager";
        return m_devices;
    }

    // Enumerate USB MIDI devices.
    // This calls back to UsbMidiController::onDeviceConnected for each device,
    // which creates UsbMidiController instances and registers them in a static map.
    jint deviceCount = QJniObject::callStaticMethod<jint>(
            kUsbMidiJavaClass,
            "enumerateDevices",
            "()I");

    kLogger.info() << "Found" << deviceCount << "USB MIDI device(s)";

    // We don't directly collect controllers here — the Java callback
    // creates them as a side effect. Since queryDevices() is called
    // periodically, the controllers will be available on the next call.
    // For the first call, we just trigger the enumeration. The controller
    // manager will get them eventually.

    return m_devices;
}

#else // !Q_OS_ANDROID

UsbMidiEnumerator::UsbMidiEnumerator()
        : MidiEnumerator() {
}

UsbMidiEnumerator::~UsbMidiEnumerator() = default;

QList<Controller*> UsbMidiEnumerator::queryDevices() {
    return {};
}

#endif