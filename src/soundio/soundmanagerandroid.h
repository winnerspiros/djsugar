#pragma once

#include <QObject>
#include <QSet>
#include <vector>

#include "preferences/usersettings.h"
#include "soundio/sounddevice.h"

class QTimer;
class SoundManager;

/// The Android specific behavior of SoundManager: automatic routing to DJ
/// controllers, fallback to the built-in speaker, and hotplug of USB sound
/// cards driven directly by Mixxx. Lives in its own file so merging
/// upstream stays conflict-free.
class SoundManagerAndroid : public QObject {
    Q_OBJECT
  public:
    /// See mixxx::androidusb::DirectMode (0 = auto, 1 = always, 2 = never).
    /// Takes effect after restart.
    static const ConfigKey kUsbDirectModeKey;
    /// When enabled (default), a USB sound card with >=4 output channels
    /// (a DJ controller) gets Main on channels 1-2 and Headphones on 3-4
    /// as soon as present. Disabled when user picks another routing.
    static const ConfigKey kAutoRouteControllerKey;

    /// Called by the SoundManager constructor once the configuration has
    /// been read
    explicit SoundManagerAndroid(SoundManager* pSoundManager);
    ~SoundManagerAndroid() override;

    /// Called at the start of SoundManager::setupDevices()
    void prepareSetup();
    /// Called when a configured device cannot be opened. Returns true if
    /// SoundManager should carry on without it.
    bool skipFailedDevice(const SoundDevicePointer& pDevice, SoundDevicePointer* pMainClockRef);
    /// Called once the configured devices have been opened
    void finishSetup(const std::vector<SoundDevicePointer>& devices,
            SoundDevicePointer* pMainClockRef,
            int* pOutputDevicesOpened,
            QSet<SoundDeviceId>* pDevicesNotFound);

  private slots:
    /// Reconfigures the audio when a directly driven USB sound card is
    /// plugged in or out
    void slotUsbDevicesChanged();

  private:
    /// Routes the outputs to a connected DJ controller sound card, see
    /// kAutoRouteControllerKey. Modifies and saves the SoundManagerConfig.
    void autoRouteToController();
    /// Opens the built-in speaker (or any other output) if no physical
    /// output could be opened, so Mixxx never ends up without sound
    void openFallbackOutput(const std::vector<SoundDevicePointer>& devices,
            SoundDevicePointer* pMainClockRef,
            int* pOutputDevicesOpened,
            QSet<SoundDeviceId>* pDevicesNotFound);

    SoundManager* m_pSoundManager;
    /// Debounces USB hotplug notifications
    QTimer* m_pUsbTimer = nullptr;
    int m_usbListenerId = 0;
};
