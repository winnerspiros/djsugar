#pragma once

#include <condition_variable>
#include <functional>
#include <mutex>
#include <vector>

struct libusb_context;

namespace mixxx {
namespace android {

const QJniObject& getIntent();
bool waitForPermission(const QJniObject& device);
void usbDeviceAccessResult(QJniObject device, bool granted);

extern std::mutex s_androidLock;
extern std::condition_variable s_grantingWaitCond;
extern std::vector<std::pair<QJniObject, bool>> s_grantingResult;
extern QJniObject s_intent;
extern QJniObject s_usbManager;

/// Registers a function called when a USB or MIDI device is plugged in or
/// out (from JaviLabarumDJ/mixxx-android android-stable: UsbPermission and
/// MidiHelper call notifyDevicesChanged on hotplug). Invoked from Java and
/// USB threads. Returns an id for removeDevicesChangedListener().
int addDevicesChangedListener(std::function<void()> callback);
void removeDevicesChangedListener(int id);
void notifyDevicesChanged();

/// What has to be saved before the process ends, called on the main thread
/// when the activity closes. Empty to unset.
void setSaveBeforeExit(std::function<void()> save);

/// True when a USB permission was recently granted (e.g. via USB_DEVICE_ATTACHED).
/// HID enumerator should re-scan when this is set.
extern std::atomic<bool> s_usbPermissionGranted;

} // namespace android
} // namespace mixxx
