#pragma once

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

/// True when a USB permission was recently granted (e.g. via USB_DEVICE_ATTACHED).
/// HID enumerator should re-scan when this is set.
extern std::atomic<bool> s_usbPermissionGranted;

} // namespace android
} // namespace mixxx
