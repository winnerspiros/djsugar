#include "soundio/sounddeviceandroidusb.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>

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
// Adaptive resampling of a sound card that isn't the clock reference: the
// rate follows the fill of its FIFO, smoothed, with a proportional and an
// integral term, within half a percent (clocks of sound cards differ by far
// less)
constexpr double kMaxRateDeviation = 0.005;
constexpr double kProportionalGain = 0.002;
constexpr double kIntegralGain = 0.000004;
constexpr double kFillSmoothing = 0.01;

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
        std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> pDevice,
        int instance)
        : SoundDevice(config, sm),
          m_pDevice(std::move(pDevice)),
          m_framesPerBuffer(0),
          m_isClkRef(false),
          m_open(false),
          m_audioLatencyUsage(kAppGroup, QStringLiteral("audio_latency_usage")),
          m_framesSinceAudioLatencyUsageUpdate(0),
          m_resamplePosition(1.0),
          m_resampleRatio(1.0),
          m_nominalRatio(1.0),
          m_deviceRate(0),
          m_driftIntegral(0.0),
          m_smoothedFill(0.0),
          m_targetFill(0),
          m_primed(false),
          m_resampleUnderflows(0),
          m_framesSinceReport(0) {
    // The ": USB" suffix lets the audio presets recognize USB sound cards. The
    // host API is the Android one, so the device is listed with the others.
    // Identical sound cards (e.g. two players of the same model) get a number
    m_strDisplayName = QStringLiteral("%1%2 directo: USB")
                               .arg(m_pDevice->productName(),
                                       instance > 1 ? QStringLiteral(" (%1)").arg(instance)
                                                    : QString());
    m_deviceId.name = m_strDisplayName;
    m_deviceId.deviceIndex = instance - 1;
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
    const bool supportsEngineRate = m_pDevice->outputFormat().supportsRate(sampleRate);
    // A card that follows the clock of another one resamples anyway: it
    // plays at a rate it supports, e.g. a player at 48 kHz while the engine
    // runs at 44.1 kHz, instead of failing
    m_deviceRate = sampleRate;
    if (!supportsEngineRate && !isClkRefDevice) {
        m_deviceRate = 0;
        for (int rate : m_pDevice->sampleRates()) {
            // The closest rate, the higher one on a tie
            if (m_pDevice->outputFormat().supportsRate(rate) &&
                    (m_deviceRate == 0 ||
                            std::abs(rate - sampleRate) < std::abs(m_deviceRate - sampleRate) ||
                            (std::abs(rate - sampleRate) == std::abs(m_deviceRate - sampleRate) &&
                                    rate > m_deviceRate))) {
                m_deviceRate = rate;
            }
        }
    }
    if (m_deviceRate <= 0 ||
            (isClkRefDevice && !supportsEngineRate &&
                    !m_pDevice->sampleRates().contains(sampleRate))) {
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
    // Two engine buffers queued for a card that follows its own clock
    m_targetFill = 2 * m_framesPerBuffer;
    m_resampleInput.clear();
    m_resampleInput.reserve((m_framesPerBuffer * 4 + 1024) * channels);
    m_resamplePosition = 1.0;
    m_nominalRatio = static_cast<double>(sampleRate) / m_deviceRate;
    m_resampleRatio = m_nominalRatio;
    m_driftIntegral = 0.0;
    m_smoothedFill = m_targetFill;
    m_primed = false;
    m_resampleUnderflows = 0;
    m_framesSinceReport = 0;

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
    if (!m_pDevice->startOutput(m_deviceRate,
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
    qInfo() << "Opened direct USB audio device" << m_strDisplayName << m_deviceRate
            << "Hz (engine" << sampleRate << "Hz), buffer" << m_framesPerBuffer << "frames, clock reference:" << m_isClkRef;
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
    // The resampling follows the drift of the clocks (see
    // resampleFromFifo()); this only catches a card that stopped reading
    if (m_pFifo->readAvailable() > 6 * m_framesPerBuffer * channels) {
        m_pSoundManager->underflowHappened(27);
        return;
    }
    if (m_pFifo->write(m_engineBuffer.data(), frames * channels) < frames * channels) {
        m_pSoundManager->underflowHappened(27);
    }
    // Every 10 s, how this card follows the clock of the other one
    m_framesSinceReport += frames;
    if (m_framesSinceReport >= 10 * m_sampleRate.value()) {
        m_framesSinceReport = 0;
        qInfo() << m_strDisplayName << "follows the clock: queued"
                << m_pFifo->readAvailable() / channels << "of" << m_targetFill
                << "frames, rate" << (m_resampleRatio / m_nominalRatio - 1.0) * 1e6
                << "ppm, underflows" << m_resampleUnderflows.exchange(0);
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
        int read = m_pFifo->read(m_renderBuffer.data(), samples);
        if (read < samples) {
            if (m_open) {
                m_pSoundManager->underflowHappened(28);
            }
            std::fill(m_renderBuffer.begin() + read, m_renderBuffer.begin() + samples, 0.0f);
        }
    } else {
        resampleFromFifo(frames);
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

void SoundDeviceAndroidUsb::resampleFromFifo(int frames) {
    const int channels = m_pDevice->outputFormat().channels;
    const int samples = frames * channels;
    const int stagedFrames = static_cast<int>(m_resampleInput.size()) / channels;
    const double fill = m_pFifo->readAvailable() / channels +
            std::max(0.0, stagedFrames - m_resamplePosition);
    if (!m_primed) {
        // Silence until the target is queued, then follow it
        if (fill < m_targetFill) {
            std::fill(m_renderBuffer.begin(), m_renderBuffer.begin() + samples, 0.0f);
            return;
        }
        m_primed = true;
        m_smoothedFill = fill;
    }

    // The engine writes whole buffers: the fill is smoothed before steering
    m_smoothedFill += (fill - m_smoothedFill) * kFillSmoothing;
    const double error = (m_smoothedFill - m_targetFill) / m_targetFill;
    m_driftIntegral = std::clamp(m_driftIntegral + error * kIntegralGain,
            -kMaxRateDeviation,
            kMaxRateDeviation);
    m_resampleRatio = m_nominalRatio *
            std::clamp(1.0 + error * kProportionalGain + m_driftIntegral,
                    1.0 - kMaxRateDeviation,
                    1.0 + kMaxRateDeviation);

    // Enough input for this transfer, and three frames for the interpolation
    const int needed = static_cast<int>(
                               std::ceil(m_resamplePosition + frames * m_resampleRatio)) +
            3;
    if (stagedFrames < needed) {
        const size_t at = m_resampleInput.size();
        m_resampleInput.resize(static_cast<size_t>(needed) * channels);
        const int wanted = (needed - stagedFrames) * channels;
        const int read = m_pFifo->read(m_resampleInput.data() + at, wanted);
        if (read < wanted) {
            // Underflow: silence, and queue the target again
            m_resampleInput.resize(at + read);
            m_pSoundManager->underflowHappened(28);
            ++m_resampleUnderflows;
            std::fill(m_renderBuffer.begin(), m_renderBuffer.begin() + samples, 0.0f);
            m_primed = false;
            return;
        }
    }

    // Cubic (Catmull-Rom) interpolation between the input frames
    const CSAMPLE* pIn = m_resampleInput.data();
    double position = m_resamplePosition;
    for (int frame = 0; frame < frames; ++frame) {
        const int index = static_cast<int>(position);
        const float t = static_cast<float>(position - index);
        for (int channel = 0; channel < channels; ++channel) {
            const float y0 = pIn[(index - 1) * channels + channel];
            const float y1 = pIn[index * channels + channel];
            const float y2 = pIn[(index + 1) * channels + channel];
            const float y3 = pIn[(index + 2) * channels + channel];
            const float a = -0.5f * y0 + 1.5f * y1 - 1.5f * y2 + 0.5f * y3;
            const float b = y0 - 2.5f * y1 + 2.f * y2 - 0.5f * y3;
            const float c = -0.5f * y0 + 0.5f * y2;
            m_renderBuffer[frame * channels + channel] = ((a * t + b) * t + c) * t + y1;
        }
        position += m_resampleRatio;
    }
    // Keep one frame before the position for the next interpolation
    const int consumed = std::max(0, static_cast<int>(position) - 1);
    m_resampleInput.erase(m_resampleInput.begin(),
            m_resampleInput.begin() + static_cast<ptrdiff_t>(consumed) * channels);
    m_resamplePosition = position - consumed;
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
