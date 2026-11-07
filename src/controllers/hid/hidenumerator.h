#pragma once

#include <QHash>

#include "controllers/controllerenumerator.h"
#include "controllers/hid/hiddevice.h"

/// This class handles discovery and enumeration of DJ controllers that use the
/// USB-HID protocol.
class HidEnumerator : public ControllerEnumerator {
    Q_OBJECT
  public:
    HidEnumerator() = default;
    ~HidEnumerator() override;

    QList<Controller*> queryDevices() override;

  private:
    QList<Controller*> m_devices;
#ifdef __ANDROID__
    /// Devices by "<usbDeviceName>#<interfaceNumber>", so a replugged
    /// controller gets a fresh Controller instead of reusing the stale USB
    /// handle.
    QHash<QString, Controller*> m_androidDevicesByKey;
    /// Unplugged devices are only deleted on shutdown, because the GUI may
    /// still hold pointers to them until it processed devicesChanged().
    QList<Controller*> m_removedDevices;
#endif
};
