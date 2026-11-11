#pragma once

#include <atomic>
#include <memory>
#include <vector>

#include "control/pollingcontrolproxy.h"
#include "soundio/sounddevice.h"
#include "util/duration.h"
#include "util/fifo.h"
#include "util/performancetimer.h"

namespace mixxx {
namespace androidusb {
class AndroidUsbAudioDevice;
}
} // namespace mixxx

/// Sound device for a USB audio interface driven directly by Mixxx on Android
/// (see AndroidUsbAudioDevice). When it is the clock reference, the engine is
/// processed on the USB event thread whenever the USB transfers need data.
class SoundDeviceAndroidUsb : public SoundDevice {
  public:
    /// instance tells identical sound cards apart (1 for the first)
    SoundDeviceAndroidUsb(UserSettingsPointer config,
            SoundManager* sm,
            std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> pDevice,
            int instance = 1);
    ~SoundDeviceAndroidUsb() override;

    SoundDeviceStatus open(bool isClkRefDevice, int syncBuffers) override;
    bool isOpen() const override;
    SoundDeviceStatus close() override;
    void readProcess(SINT framesPerBuffer) override;
    void writeProcess(SINT framesPerBuffer) override;
    QString getError() const override;
    mixxx::audio::SampleRate getDefaultSampleRate() const override;

    const std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice>& usbDevice() const {
        return m_pDevice;
    }

  private:
    void render(uint8_t* pDestination, int frames);
    /// For a sound card that isn't the clock reference: reads the frames
    /// from the FIFO at a rate that follows the card's own clock, so that
    /// the two clocks drifting apart causes no clicks
    void resampleFromFifo(int frames);
    void processEngine();
    void updateAudioLatencyUsage(SINT framesPerBuffer);

    std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> m_pDevice;
    std::unique_ptr<FIFO<CSAMPLE>> m_pFifo;
    std::vector<CSAMPLE> m_engineBuffer;
    std::vector<CSAMPLE> m_renderBuffer;
    SINT m_framesPerBuffer;
    bool m_isClkRef;
    std::atomic<bool> m_open;
    QString m_lastError;

    PollingControlProxy m_audioLatencyUsage;
    PerformanceTimer m_clkRefTimer;
    mixxx::Duration m_timeInAudioCallback;
    SINT m_framesSinceAudioLatencyUsageUpdate;

    // Adaptive resampling (see resampleFromFifo())
    std::vector<CSAMPLE> m_resampleInput;
    double m_resamplePosition;
    double m_resampleRatio;
    // Engine frames per frame of the sound card: 1, or the conversion when
    // the card doesn't support the rate of the engine
    double m_nominalRatio;
    // The rate the sound card plays at
    int m_deviceRate;
    double m_driftIntegral;
    double m_smoothedFill;
    int m_targetFill;
    bool m_primed;
    // For the log: underflows of the resampling (USB thread) and the engine
    // frames since the last report (engine thread)
    std::atomic<int> m_resampleUnderflows;
    SINT m_framesSinceReport;
};
