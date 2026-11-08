#pragma once

#include <QHash>

#include "controllers/controllerenumerator.h"

struct libusb_context;

/// Locate supported USB bulk controllers
class BulkEnumerator : public ControllerEnumerator {
    Q_OBJECT
  public:
    explicit BulkEnumerator();
    virtual ~BulkEnumerator();

    QList<Controller*> queryDevices() override;

  private:
    QList<Controller*> m_devices;
#ifdef __ANDROID__
    /// Devices by Android USB device name, so a replugged controller gets a
    /// fresh Controller instead of reusing the stale USB handle.
    QHash<QString, Controller*> m_androidDevicesByKey;
    /// Unplugged devices are only deleted on shutdown, because the GUI may
    /// still hold pointers to them until it processed devicesChanged().
    QList<Controller*> m_removedDevices;
#endif
#ifndef __ANDROID__
    libusb_context* m_pContext;
#endif
};
