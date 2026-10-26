#pragma once

#include <portaudio.h>

#include <QList>

#include "preferences/usersettings.h"
#include "soundio/sounddevice.h"
#include "soundio/sounddeviceenumerator.h"
#include "soundio/sounddeviceportaudio.h"
#ifdef __ANDROID_USB_AUDIO__
#include "soundio/androidusbsounddevices.h"
#endif

class PortAudioEnumerator : public SoundDeviceEnumerator {
  public:
    PortAudioEnumerator(UserSettingsPointer config,
            SoundManager* sm);
    ~PortAudioEnumerator() override;

    std::vector<SoundDevicePointer> queryDevices() const override;

    QList<QString> getAPIs() const override {
        return m_apis;
    }

    QList<mixxx::audio::SampleRate> getSampleRates(bool jackSampleRate) const override;

    void initialize() override;
    void deinitialize() override;

#ifdef __ANDROID_USB_AUDIO__
    /// Rescans the USB sound cards driven directly by Mixxx. Returns true if
    /// the set changed; the devices that disappeared are added to pRemoved.
    bool refreshAndroidUsbDevices(
            std::vector<SoundDevicePointer>* pRemoved, bool waitForPermission);
    /// True if the output of a USB sound card driven directly stopped by itself
    bool androidUsbOutputFailed() const {
        return m_androidUsbDevices.outputFailed();
    }
#endif

  private:
    void setJACKName() const;

    std::vector<QSharedPointer<SoundDevicePortAudio>> m_devices;
#ifdef __ANDROID_USB_AUDIO__
    AndroidUsbSoundDevices m_androidUsbDevices;
#endif
    mixxx::audio::SampleRate m_jackSampleRate;

    UserSettingsPointer m_pConfig;
    SoundManager* m_pSoundManager;
    QList<QString> m_apis;
};
