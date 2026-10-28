#pragma once

#include <functional>
#include <vector>

struct libusb_context;

namespace mixxx {
namespace android {

const QJniObject& getIntent();
bool waitForPermission(const QJniObject& device);
void usbDeviceAccessResult(QJniObject device, bool granted);

/// Registers a function called when a USB or MIDI device is plugged in or
/// out. It is invoked from Java and USB threads. Returns an id for
/// removeDevicesChangedListener().
int addDevicesChangedListener(std::function<void()> callback);
void removeDevicesChangedListener(int id);
void notifyDevicesChanged();

/// What has to be saved before the process ends, called on the main thread
/// when the activity closes (see MainActivity.onDestroy()). Empty to unset.
void setSaveBeforeExit(std::function<void()> save);

extern std::mutex s_androidLock;
extern std::condition_variable s_grantingWaitCond;
extern std::vector<std::pair<QJniObject, bool>> s_grantingResult;
extern QJniObject s_intent;
extern QJniObject s_usbManager;

} // namespace android
} // namespace mixxx
