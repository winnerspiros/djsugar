#include "soundio/soundmanagerandroid.h"

#include <QCoreApplication>
#include <QJniObject>
#include <QTimer>

#include "control/controlobject.h"
#include "controllers/android.h"
#include "moc_soundmanagerandroid.cpp"
#include "soundio/portaudioenumerator.h"
#include "soundio/soundmanager.h"
#ifdef __ANDROID_USB_AUDIO__
#include "soundio/androidusbaudiodevice.h"
#include "soundio/sounddeviceandroidusb.h"
#endif

const ConfigKey SoundManagerAndroid::kUsbDirectModeKey =
        ConfigKey(QStringLiteral("[Soundcard]"), QStringLiteral("AndroidUsbDirectMode"));
const ConfigKey SoundManagerAndroid::kAutoRouteControllerKey =
        ConfigKey(QStringLiteral("[Soundcard]"), QStringLiteral("AndroidAutoRouteController"));

namespace {

bool isConfiguredOutput(const SoundManagerConfig& config, const SoundDevicePointer& pDevice) {
    return config.getOutputs().contains(pDevice->getDeviceId());
}

} // namespace

SoundManagerAndroid::SoundManagerAndroid(SoundManager* pSoundManager)
        : QObject(pSoundManager),
          m_pSoundManager(pSoundManager) {
    SoundManagerConfig& config = m_pSoundManager->m_config;
    if (config.getOutputs().isEmpty()) {
        config.loadDefaults(m_pSoundManager,
                SoundManagerConfig::API | SoundManagerConfig::DEVICES |
                        SoundManagerConfig::OTHER);
        config.writeToDisk();
    }

#ifdef __ANDROID_USB_AUDIO__
    // USB sound cards driven by Mixxx can be plugged in and out at any time
    m_pUsbTimer = new QTimer(this);
    m_pUsbTimer->setSingleShot(true);
    m_pUsbTimer->setInterval(1000);
    connect(m_pUsbTimer,
            &QTimer::timeout,
            this,
            &SoundManagerAndroid::slotUsbDevicesChanged);
    m_usbListenerId = mixxx::android::addDevicesChangedListener([this]() {
        QMetaObject::invokeMethod(
                this, [this]() { m_pUsbTimer->start(); }, Qt::QueuedConnection);
    });
#endif
}

SoundManagerAndroid::~SoundManagerAndroid() {
#ifdef __ANDROID_USB_AUDIO__
    mixxx::android::removeDevicesChangedListener(m_usbListenerId);
#endif
}

void SoundManagerAndroid::slotUsbDevicesChanged() {
#ifdef __ANDROID_USB_AUDIO__
    auto* pEnumerator = dynamic_cast<PortAudioEnumerator*>(m_pSoundManager->m_pEnumerator.get());
    if (!pEnumerator) {
        return;
    }
    std::vector<SoundDevicePointer> removed;
    if (pEnumerator->refreshAndroidUsbDevices(&removed, false)) {
        qInfo() << "USB sound cards changed, reconfiguring the audio devices";
    } else if (pEnumerator->androidUsbOutputFailed()) {
        qWarning() << "The output of a USB sound card stopped, setting the audio devices up again";
    } else {
        return;
    }
    for (const auto& pDevice : removed) {
        if (pDevice->isOpen()) {
            pDevice->close();
        }
    }
    const bool sleepAfterClosing = false;
    m_pSoundManager->closeDevices(sleepAfterClosing);
    m_pSoundManager->setupDevices();
    emit m_pSoundManager->devicesUpdated();
#endif
}

void SoundManagerAndroid::autoRouteToController() {
    if (!m_pSoundManager->m_pConfig->getValue(kAutoRouteControllerKey, true)) {
        return;
    }
    SoundManagerConfig& config = m_pSoundManager->m_config;
    // The Android enumerator suffixes USB devices with ": USB"
    SoundDevicePointer pController;
    for (const auto& pDevice : m_pSoundManager->m_pEnumerator->queryDevices()) {
        if (pDevice->getNumOutputChannels() >= 4 &&
                pDevice->getDisplayName().endsWith(QStringLiteral(": USB")) &&
                (!pController ||
                        pDevice->getNumOutputChannels() > pController->getNumOutputChannels())) {
            pController = pDevice;
        }
    }
    if (!pController || isConfiguredOutput(config, pController)) {
        return;
    }

    qInfo() << "DJ controller sound card detected, routing Main to channels 1-2 "
               "and Headphones to channels 3-4 of"
            << pController->getDisplayName();
    config.clearOutputs();
    config.addOutput(pController->getDeviceId(),
            AudioOutput(AudioPathType::Main, 0, mixxx::audio::ChannelCount::stereo(), 0));
    config.addOutput(pController->getDeviceId(),
            AudioOutput(AudioPathType::Headphones, 2, mixxx::audio::ChannelCount::stereo(), 0));
    // Play at the controller's own rate to avoid resampling
    const auto sampleRate = pController->getDefaultSampleRate();
    if (sampleRate.isValid()) {
        config.setSampleRate(sampleRate);
    }
    // The cable splitter preset is not used with a controller
    ControlObject::set(ConfigKey(QStringLiteral("[Master]"), QStringLiteral("headSplit")), 0.0);
    config.writeToDisk();
}

void SoundManagerAndroid::prepareSetup() {
    SoundManagerConfig& config = m_pSoundManager->m_config;
    if (config.getOutputs().isEmpty()) {
        qWarning() << "SoundManager on Android: No outputs configured in "
                      "SoundManagerConfig. Loading defaults...";
        config.loadDefaults(m_pSoundManager,
                SoundManagerConfig::API | SoundManagerConfig::DEVICES |
                        SoundManagerConfig::OTHER);
        config.writeToDisk();
    }
    autoRouteToController();
#ifdef __ANDROID_USB_AUDIO__
    // A USB sound card driven directly by Mixxx only plays at the rates it
    // supports (e.g. 44.1 kHz for the DDJ-400), there is no resampling
    for (const auto& pDevice : m_pSoundManager->m_pEnumerator->queryDevices()) {
        const auto* pUsbDevice = dynamic_cast<const SoundDeviceAndroidUsb*>(pDevice.data());
        if (!pUsbDevice || !isConfiguredOutput(config, pDevice)) {
            continue;
        }
        const int sampleRate = static_cast<int>(config.getSampleRate().value());
        if (!pUsbDevice->usbDevice()->sampleRates().contains(sampleRate)) {
            qInfo() << pDevice->getDisplayName() << "doesn't support" << sampleRate
                    << "Hz, using" << pDevice->getDefaultSampleRate();
            config.setSampleRate(pDevice->getDefaultSampleRate());
        }
    }
#endif
}

bool SoundManagerAndroid::skipFailedDevice(
        const SoundDevicePointer& pDevice, SoundDevicePointer* pMainClockRef) {
    // Never end up without any sound: skip the device and let
    // openFallbackOutput() open another one
    qWarning() << "Unable to open" << pDevice->getDisplayName() << ":" << pDevice->getError();
    if (*pMainClockRef == pDevice) {
        pMainClockRef->clear();
    }
    return true;
}

void SoundManagerAndroid::finishSetup(const std::vector<SoundDevicePointer>& devices,
        SoundDevicePointer* pMainClockRef,
        int* pOutputDevicesOpened,
        QSet<SoundDeviceId>* pDevicesNotFound) {
    openFallbackOutput(devices, pMainClockRef, pOutputDevicesOpened, pDevicesNotFound);

    // Android attenuates USB sound cards with the media volume. DJ
    // controllers have their own level knobs, so play at full scale.
    const SoundManagerConfig& config = m_pSoundManager->m_config;
    for (const auto& pDevice : devices) {
        if (isConfiguredOutput(config, pDevice) &&
                pDevice->getDisplayName().endsWith(QStringLiteral(": USB"))) {
            QNativeInterface::QAndroidApplication::runOnAndroidMainThread([]() {
                QJniObject context = QNativeInterface::QAndroidApplication::context();
                QJniObject::callStaticMethod<jboolean>("org/mixxx/AudioHelper",
                        "maximizeMediaVolumeForUsb",
                        "(Landroid/content/Context;)Z",
                        context.object<jobject>());
            });
            break;
        }
    }
}

void SoundManagerAndroid::openFallbackOutput(const std::vector<SoundDevicePointer>& devices,
        SoundDevicePointer* pMainClockRef,
        int* pOutputDevicesOpened,
        QSet<SoundDeviceId>* pDevicesNotFound) {
    const SoundManagerConfig& config = m_pSoundManager->m_config;
    for (const auto& pDevice : devices) {
        if (pDevice && pDevice->isOpen() &&
                pDevice->getDeviceId().name != kNetworkDeviceInternalName &&
                isConfiguredOutput(config, pDevice)) {
            // A physical output is playing
            return;
        }
    }

    qWarning() << "SoundManager on Android: No physical output device opened! "
                  "Forcing first available output device...";
    SoundDevicePointer targetDevice;
    for (const auto& pDevice : devices) {
        if (pDevice && pDevice->getNumOutputChannels() > 0 &&
                pDevice->getDeviceId().name != kNetworkDeviceInternalName) {
            if (pDevice->getDisplayName().contains(
                        QStringLiteral("earpiece"), Qt::CaseInsensitive)) {
                continue;
            }
            if (pDevice->getHostAPI().contains(QStringLiteral("Oboe"), Qt::CaseInsensitive)) {
                // Prefer the built-in speaker, which is always present
                if (pDevice->getDisplayName().contains(QStringLiteral("Speaker"))) {
                    targetDevice = pDevice;
                    break;
                }
                if (!targetDevice ||
                        !targetDevice->getHostAPI().contains(
                                QStringLiteral("Oboe"), Qt::CaseInsensitive)) {
                    targetDevice = pDevice;
                }
                continue;
            }
            if (!targetDevice) {
                targetDevice = pDevice;
            }
        }
    }
    if (!targetDevice) {
        return;
    }

    const auto channelCount = targetDevice->getNumOutputChannels() >= 2
            ? mixxx::audio::ChannelCount::stereo()
            : mixxx::audio::ChannelCount::mono();
    AudioOutput forcedOut(AudioPathType::Main, 0, channelCount, 0);
    auto pSource = m_pSoundManager->m_registeredSources.value(forcedOut);
    const CSAMPLE* pBuffer = pSource ? pSource->buffer(forcedOut).data() : nullptr;
    if (!pBuffer) {
        return;
    }
    targetDevice->clearOutputs();
    AudioOutputBuffer aob(forcedOut, pBuffer);
    targetDevice->addOutput(aob);
    targetDevice->setSampleRate(config.getSampleRate());
    targetDevice->setConfigFramesPerBuffer(config.getFramesPerBuffer());
    if (targetDevice->open(true, config.getSyncBuffers()) == SoundDeviceStatus::Ok) {
        *pMainClockRef = targetDevice;
        ++*pOutputDevicesOpened;
        pDevicesNotFound->remove(targetDevice->getDeviceId());
        // Not saved to the configuration on purpose: the configured device
        // (e.g. a DJ controller that isn't plugged in yet) must be used
        // again next time.
        qInfo() << "Successfully opened fallback Android audio device:"
                << targetDevice->getDisplayName();
    }
}
