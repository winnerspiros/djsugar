#include "soundio/sounddeviceandroidusb.h"

#include <algorithm>
#include <cmath>

#include "control/controlobject.h"
#include "soundio/androidusbaudiodevice.h"
#include "soundio/soundmanager.h"
#include "util/math.h"
#include "waveform/visualplayposition.h"

namespace {

const QString kAppGroup = QStringLiteral("[App]");
constexpr int kUpdatesPerSecond = 30;
// Each USB transfer carries this much audio
constexpr int kTransferMillis = 2;
// Lower bound of the audio queued at the USB controller
constexpr int kMinQueuedMillis = 8;

int nextPowerOfTwo(int value) {
    int result = 1;
    while (result < value) {
        result <<= 1;
    }
    return result;
}

} // namespace

SoundDeviceAndroidUsb::SoundDeviceAndroidUsb(UserSettingsPointer config,
        SoundManager* sm,
        std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> pDevice)
        : SoundDevice(config, sm),
          m_pDevice(std::move(pDevice)),
          m_framesPerBuffer(0),
          m_isClkRef(false),
          m_open(false),
          m_audioLatencyUsage(kAppGroup, QStringLiteral("audio_latency_usage")),
          m_framesSinceAudioLatencyUsageUpdate(0) {
    // The ": USB" suffix lets the audio presets recognize USB sound cards. The
    // host API is the Android one, so the device is listed with the others.
    m_strDisplayName = QStringLiteral("%1 directo: USB").arg(m_pDevice->productName());
    m_deviceId.name = m_strDisplayName;
    m_deviceId.deviceIndex = 0;
    m_hostAPI = QStringLiteral("Android Oboe");
    m_numOutputChannels = mixxx::audio::ChannelCount(m_pDevice->outputFormat().channels);
    m_numInputChannels = mixxx::audio::ChannelCount(0);
    m_sampleRate = getDefaultSampleRate();
}

SoundDeviceAndroidUsb::~SoundDeviceAndroidUsb() {
    if (m_open) {
        close();
    }
}

mixxx::audio::SampleRate SoundDeviceAndroidUsb::getDefaultSampleRate() const {
    const QList<int> rates = m_pDevice->sampleRates();
    if (rates.contains(44100)) {
        return mixxx::audio::SampleRate(44100);
    }
    if (rates.contains(48000)) {
        return mixxx::audio::SampleRate(48000);
    }
    return mixxx::audio::SampleRate(rates.isEmpty() ? 44100 : rates.first());
}

SoundDeviceStatus SoundDeviceAndroidUsb::open(bool isClkRefDevice, int syncBuffers) {
    Q_UNUSED(syncBuffers);
    if (m_open) {
        return SoundDeviceStatus::Ok;
    }
    const int sampleRate = static_cast<int>(m_sampleRate.value());
    if (!m_pDevice->outputFormat().supportsRate(sampleRate) &&
            !m_pDevice->sampleRates().contains(sampleRate)) {
        m_lastError = QObject::tr("%1 doesn't support a sample rate of %2 Hz")
                              .arg(m_pDevice->productName(), QString::number(sampleRate));
        qWarning() << m_lastError;
        return SoundDeviceStatus::Error;
    }
    m_framesPerBuffer = m_configFramesPerBuffer > 0 ? m_configFramesPerBuffer : 512;
    const int channels = m_pDevice->outputFormat().channels;
    m_engineBuffer.assign(m_framesPerBuffer * channels, 0);
    // Enough for one transfer at up to 192 kHz
    m_renderBuffer.assign((192 * kTransferMillis + 64) * channels, 0);
    m_pFifo = std::make_unique<FIFO<CSAMPLE>>(
            nextPowerOfTwo((m_framesPerBuffer * 4 + 1024) * channels));
    m_isClkRef = isClkRefDevice;

    // Queue roughly one engine buffer at the USB controller
    const int bufferMillis = static_cast<int>(
            std::ceil(m_framesPerBuffer * 1000.0 / sampleRate));
    const int queuedMillis = std::max(kMinQueuedMillis, bufferMillis);
    const int numTransfers = std::max(3, (queuedMillis + kTransferMillis - 1) / kTransferMillis);

    if (m_isClkRef) {
        // The engine renders at this rate. Without it, the engine kept its
        // default of 44.1 kHz and a device opened at 48 kHz played 8.8% too
        // fast and too high. Like SoundDevicePortAudio, also the latency for
        // the waveforms.
        ControlObject::set(ConfigKey(kAppGroup, QStringLiteral("samplerate")), sampleRate);
        ControlObject::set(ConfigKey(kAppGroup, QStringLiteral("output_latency_ms")),
                queuedMillis);
    }

    m_open = true;
    if (!m_pDevice->startOutput(sampleRate,
                kTransferMillis,
                numTransfers,
                [this](uint8_t* pDestination, int frames) {
                    render(pDestination, frames);
                })) {
        m_open = false;
        m_lastError = QObject::tr("Unable to start the USB audio stream of %1")
                              .arg(m_pDevice->productName());
        qWarning() << m_lastError;
        return SoundDeviceStatus::Error;
    }
    qInfo() << "Opened direct USB audio device" << m_strDisplayName << sampleRate
            << "Hz, buffer" << m_framesPerBuffer << "frames, clock reference:" << m_isClkRef;
    return SoundDeviceStatus::Ok;
}

bool SoundDeviceAndroidUsb::isOpen() const {
    return m_open;
}

SoundDeviceStatus SoundDeviceAndroidUsb::close() {
    if (!m_open) {
        return SoundDeviceStatus::Ok;
    }
    m_open = false;
    m_pDevice->stopOutput();
    return SoundDeviceStatus::Ok;
}

void SoundDeviceAndroidUsb::readProcess(SINT framesPerBuffer) {
    // No inputs
    Q_UNUSED(framesPerBuffer);
}

void SoundDeviceAndroidUsb::writeProcess(SINT framesPerBuffer) {
    // Only used when another device drives the engine (e.g. the speaker of
    // the tablet plays the main mix and this device the headphones)
    if (!m_open || m_isClkRef || !m_pFifo) {
        return;
    }
    const int channels = m_pDevice->outputFormat().channels;
    const SINT frames = std::min<SINT>(framesPerBuffer, m_framesPerBuffer);
    composeOutputBuffer(m_engineBuffer.data(), frames, 0, channels);
    // The two clocks drift apart: drop a buffer when too much is queued
    if (m_pFifo->readAvailable() > 3 * m_framesPerBuffer * channels) {
        m_pSoundManager->underflowHappened(27);
        return;
    }
    if (m_pFifo->write(m_engineBuffer.data(), frames * channels) < frames * channels) {
        m_pSoundManager->underflowHappened(27);
    }
}

QString SoundDeviceAndroidUsb::getError() const {
    return m_lastError;
}

void SoundDeviceAndroidUsb::processEngine() {
    const int channels = m_pDevice->outputFormat().channels;
    const double sampleRate = m_sampleRate.toDouble();

    // Time until the first frame of this buffer reaches the DAC
    m_clkRefTimer.start();
    const int queuedFrames = m_pFifo->readAvailable() / channels + m_pDevice->framesInFlight();
    VisualPlayPosition::setCallbackEntryToDacSecs(
            math_max(queuedFrames / sampleRate, 0.0001), m_clkRefTimer);

    m_pSoundManager->readProcess(m_framesPerBuffer);
    m_pSoundManager->onDeviceOutputCallback(m_framesPerBuffer);
    composeOutputBuffer(m_engineBuffer.data(), m_framesPerBuffer, 0, channels);
    m_pSoundManager->writeProcess(m_framesPerBuffer);
    m_pSoundManager->processUnderflowHappened(m_framesPerBuffer);

    m_pFifo->write(m_engineBuffer.data(), m_framesPerBuffer * channels);
    updateAudioLatencyUsage(m_framesPerBuffer);
}

void SoundDeviceAndroidUsb::render(uint8_t* pDestination, int frames) {
    const int channels = m_pDevice->outputFormat().channels;
    const int subframeBytes = m_pDevice->outputFormat().subframeBytes;
    const int samples = frames * channels;
    if (static_cast<int>(m_renderBuffer.size()) < samples) {
        m_renderBuffer.resize(samples);
    }

    if (m_isClkRef) {
        while (m_pFifo->readAvailable() < samples) {
            processEngine();
        }
    }
    int read = m_pFifo->read(m_renderBuffer.data(), samples);
    if (read < samples) {
        if (m_open) {
            m_pSoundManager->underflowHappened(28);
        }
        std::fill(m_renderBuffer.begin() + read, m_renderBuffer.begin() + samples, 0.0f);
    }

    // Float to little endian signed integers
    uint8_t* pOut = pDestination;
    if (subframeBytes == 3) {
        for (int i = 0; i < samples; ++i) {
            const float sample = std::clamp(m_renderBuffer[i], -1.0f, 1.0f);
            const auto value = static_cast<int32_t>(std::lrint(sample * 8388607.0f));
            *pOut++ = static_cast<uint8_t>(value & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 8) & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 16) & 0xff);
        }
    } else if (subframeBytes == 4) {
        // 24 or 32 bit resolution in 32 bit slots, left aligned
        for (int i = 0; i < samples; ++i) {
            const double sample = std::clamp(m_renderBuffer[i], -1.0f, 1.0f);
            const auto value = static_cast<int32_t>(std::llrint(sample * 2147483647.0));
            *pOut++ = static_cast<uint8_t>(value & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 8) & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 16) & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 24) & 0xff);
        }
    } else {
        for (int i = 0; i < samples; ++i) {
            const float sample = std::clamp(m_renderBuffer[i], -1.0f, 1.0f);
            const auto value = static_cast<int16_t>(std::lrint(sample * 32767.0f));
            *pOut++ = static_cast<uint8_t>(value & 0xff);
            *pOut++ = static_cast<uint8_t>((value >> 8) & 0xff);
        }
    }
}

void SoundDeviceAndroidUsb::updateAudioLatencyUsage(SINT framesPerBuffer) {
    m_timeInAudioCallback += m_clkRefTimer.elapsed();
    m_framesSinceAudioLatencyUsageUpdate += framesPerBuffer;
    if (m_framesSinceAudioLatencyUsageUpdate > m_sampleRate.toDouble() / kUpdatesPerSecond) {
        const double secInAudioCb = m_timeInAudioCallback.toDoubleSeconds();
        m_audioLatencyUsage.set(secInAudioCb /
                (m_framesSinceAudioLatencyUsageUpdate / m_sampleRate.toDouble()));
        m_timeInAudioCallback = mixxx::Duration::empty();
        m_framesSinceAudioLatencyUsageUpdate = 0;
    }
}
