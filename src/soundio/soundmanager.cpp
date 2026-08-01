#include "soundio/soundmanager.h"

#include <QLibrary>
#include <QThread>
#include <QtGlobal>
#include <cstring> // for memcpy and strcmp
#include <memory>

#include "control/controlobject.h"
#include "engine/audiolatencycalibrator.h"
#include "engine/enginemixer.h"
#include "moc_soundmanager.cpp"
#include "preferences/configobject.h"
#include "soundio/portaudioenumerator.h"
#include "soundio/sounddevice.h"
#include "soundio/sounddeviceenumerator.h"
#include "soundio/sounddevicenetwork.h"
#include "soundio/sounddevicenotfound.h"
#include "soundio/sounddeviceportaudio.h"
#include "soundio/soundmanagerconfig.h"
#include "soundio/soundmanagerutil.h"
#include "util/cmdlineargs.h"
#include "util/compatibility/qatomic.h"
#include "util/defs.h"
#include "util/sample.h"
#include "vinylcontrol/defs_vinylcontrol.h"

#ifdef __PIPEWIRE__
#include "soundio/pipewireenumerator.h"
#endif

namespace {

const QString kAppGroup = QStringLiteral("[App]");
#ifdef __PIPEWIRE__
const ConfigKey kPipeWire = ConfigKey(kAppGroup, QStringLiteral("pipewire"));
#endif

#define CPU_OVERLOAD_DURATION 500 // in ms

struct DeviceMode {
    SoundDevicePointer pDevice;
    bool isInput;
    bool isOutput;
};

#ifdef __LINUX__
constexpr unsigned int kSleepSecondsAfterClosingDevice = 5;
#endif
} // anonymous namespace

SoundManager::SoundManager(
        UserSettingsPointer pConfig, EngineMixer* pEngineMixer)
        : m_pEngineMixer(pEngineMixer),
          m_pConfig(pConfig),
          m_config(this),
          m_pErrorDevice(nullptr),
          m_underflowHappened(0),
          m_underflowUpdateCount(0),
          m_audioLatencyOverloadCount(
                  kAppGroup, QStringLiteral("audio_latency_overload_count")),
          m_audioLatencyOverload(
                  kAppGroup, QStringLiteral("audio_latency_overload")),
          m_pNetworkStream(QSharedPointer<EngineNetworkStream>::create(2, 0)),
          m_pNetworkDevice(QSharedPointer<SoundDeviceNetwork>::create(
                  pConfig, this, m_pNetworkStream)) {
    // TODO(xxx) some of these ControlObject are not needed by soundmanager, or are unused here.
    // It is possible to take them out?
    m_pControlObjectSoundStatusCO = new ControlObject(
            ConfigKey("[SoundManager]", "status"));
    m_pControlObjectSoundStatusCO->set(SOUNDMANAGER_DISCONNECTED);

    m_pControlObjectVinylControlGainCO = new ControlObject(
            ConfigKey(VINYL_PREF_KEY, "gain"));

#ifdef __PIPEWIRE__
    if (isPipewireSelected()) {
        m_pEnumerator = std::make_unique<PipewireEnumerator>(m_pConfig, this);
    } else
#endif
    {
        m_pEnumerator = std::make_unique<PortAudioEnumerator>(m_pConfig, this);
    }

    queryDevices();

    if (!m_config.readFromDisk()) {
        m_config.loadDefaults(this, SoundManagerConfig::ALL);
    }
    checkConfig();
    // Don't write config to disk, yet -- it may be reset to defaults in case
    // previously configured devices were not found.
    // Write new config after MixxxMainWindow::noOutputDlg where the user has
    // a chance to keep the previous sound config (exit).
}

SoundManager::~SoundManager() {
    // Clean up devices.
    const bool sleepAfterClosing = false;
    clearDeviceList(sleepAfterClosing);

    // vinyl control proxies and input buffers are freed in closeDevices, called
    // by clearDeviceList -- bkgood

    delete m_pControlObjectSoundStatusCO;
    delete m_pControlObjectVinylControlGainCO;
}

QList<SoundDevicePointer> SoundManager::getDeviceList(
        const QString& filterAPI, bool bOutputDevices, bool bInputDevices) const {
    // qDebug() << "SoundManager::getDeviceList";

    if (filterAPI == SoundManagerConfig::kAPINone) {
        return QList<SoundDevicePointer>();
    }

    // Create a list of sound devices filtered to match given API and
    // input/output.
    QList<SoundDevicePointer> filteredDeviceList;

    for (const auto& pDevice : m_pEnumerator->queryDevices()) {
        // Skip devices that don't match the API, don't have input channels when
        // we

        ...[OUTPUT TRUNCATED - 14297 chars omitted out of 24297 total]...

                ng deviceName(tr("a device"));
        QString detailedError(tr("An unknown error occurred"));
        SoundDevicePointer pDevice = getErrorDevice();
        if (pDevice) {
            deviceName = pDevice->getDisplayName();
            detailedError = pDevice->getError();
        }
        switch (status) {
        case SoundDeviceStatus::ErrorDuplicateOutputChannel:
            error = tr("Two outputs cannot share channels on \"%1\"").arg(deviceName);
            break;
        default:
            error = tr("Error opening \"%1\"").arg(deviceName) + "\n" + detailedError;
            break;
        }
        return error;
    }

SoundManagerConfig SoundManager::getConfig() const {
    return m_config;
}

void SoundManager::closeActiveConfig(bool async) {
    // Close open devices. After this call we will not get any more
    // onDeviceOutputCallback() or pushBuffer() calls because all the
    // SoundDevices are closed. closeDevices() blocks and can take a while.
    const bool sleepAfterClosing = true;
    closeDevices(sleepAfterClosing, async);
}

SoundDeviceStatus SoundManager::setConfig(const SoundManagerConfig& config) {
    SoundDeviceStatus status = SoundDeviceStatus::Ok;
    m_config = config;
    checkConfig();

    closeActiveConfig();

    status = setupDevices();
    if (status == SoundDeviceStatus::Ok) {
        m_config.writeToDisk();
    }
    return status;
}

void SoundManager::checkConfig() {
    if (!m_config.checkAPI()) {
        m_config.setAPI(SoundManagerConfig::kAPINone);
        m_config.loadDefaults(this, SoundManagerConfig::API | SoundManagerConfig::DEVICES);
    }
    if (!m_config.checkSampleRate(*this)) {
        m_config.setSampleRate(SoundManagerConfig::kFallbackSampleRate);
        m_config.loadDefaults(this, SoundManagerConfig::OTHER);
    }

    // Even if we have a two-deck skin, if someone has configured a deck > 2
    // then the configuration needs to know about that extra deck.
    m_config.setCorrectDeckCount(getConfiguredDeckCount());
    // latency checks itself for validity on SMConfig::setLatency()
}

void SoundManager::onDeviceOutputCallback(const SINT iFramesPerBuffer) {
    // Produce a block of samples for output. EngineMixer expects stereo
    // samples so multiply iFramesPerBuffer by 2.
    m_pEngineMixer->process(iFramesPerBuffer * 2);
}

void SoundManager::pushInputBuffers(const QList<AudioInputBuffer>& inputs,
        const SINT iFramesPerBuffer) {
    for (QList<AudioInputBuffer>::ConstIterator i = inputs.begin(),
                                                e = inputs.end();
            i != e;
            ++i) {
        const AudioInputBuffer& in = *i;
        CSAMPLE* pInputBuffer = in.getBuffer();
        for (auto it = m_registeredDestinations.constFind(in);
                it != m_registeredDestinations.constEnd() && it.key() == in;
                ++it) {
            it.value()->receiveBuffer(in, pInputBuffer, iFramesPerBuffer);
        }
    }
}

void SoundManager::writeProcess(SINT framesPerBuffer) const {
    for (const auto& pDevice : m_pEnumerator->queryDevices()) {
        if (pDevice) {
            pDevice->writeProcess(framesPerBuffer);
        }
    }
    m_pNetworkDevice->writeProcess(framesPerBuffer);
}

void SoundManager::readProcess(SINT framesPerBuffer) const {
    for (const auto& pDevice : m_pEnumerator->queryDevices()) {
        if (pDevice) {
            pDevice->readProcess(framesPerBuffer);
        }
    }
    m_pNetworkDevice->readProcess(framesPerBuffer);
}

void SoundManager::registerOutput(const AudioOutput& output, AudioSource* src) {
    VERIFY_OR_DEBUG_ASSERT(!m_registeredSources.contains(output)) {
        return;
    }
    m_registeredSources.insert(output, src);
    emit outputRegistered(output, src);
}

void SoundManager::registerInput(const AudioInput& input, AudioDestination* dest) {
    // Vinyl control inputs are registered twice, once for timecode and once for
    // passthrough, each with different outputs. So unlike outputs, do not assert
    // that the input has not been registered yet.
    m_registeredDestinations.insert(input, dest);

    emit inputRegistered(input, dest);
}

QList<AudioOutput> SoundManager::registeredOutputs() const {
    return m_registeredSources.keys();
}

QList<AudioInput> SoundManager::registeredInputs() const {
    return m_registeredDestinations.keys();
}

void SoundManager::setConfiguredDeckCount(int count) {
    if (getConfiguredDeckCount() == count) {
        // Unchanged
        return;
    }
    m_config.setDeckCount(count);
    checkConfig();
    m_config.writeToDisk();
}

int SoundManager::getConfiguredDeckCount() const {
    return m_config.getDeckCount();
}

void SoundManager::processUnderflowHappened(SINT framesPerBuffer) {
    if (m_underflowUpdateCount == 0) {
        if (atomicLoadRelaxed(m_underflowHappened)) {
            m_audioLatencyOverload.set(1.0);
            m_audioLatencyOverloadCount.set(
                    m_audioLatencyOverloadCount.get() + 1);
            m_underflowUpdateCount = CPU_OVERLOAD_DURATION *
                    m_config.getSampleRate() / framesPerBuffer / 1000;

            m_underflowHappened = 0; // resetting here is not thread safe,
                                     // but that is OK, because we count only
                                     // 1 underflow each 500 ms
        } else {
            m_audioLatencyOverload.set(0.0);
        }
    } else {
        --m_underflowUpdateCount;
    }
}

void SoundManager::addDevice(SoundDevicePointer pDevice) {
    qDebug() << "SoundManager::addDevice" << pDevice->getDisplayName();
    emit deviceAdded(pDevice);
}

void SoundManager::removeDevice(SoundDevicePointer pDevice) {
    qDebug() << "SoundManager::removeDevice" << pDevice->getDisplayName();
    emit deviceRemoved(pDevice);
}

void SoundManager::updateDeviceChannels(SoundDevicePointer pDevice) {
    emit deviceChannelsUpdated(pDevice);
}

#ifdef __PIPEWIRE__
bool SoundManager::isPipewireSelected() {
    return CmdlineArgs::Instance().getDeveloper() && m_pConfig->getValue(kPipeWire, false);
}
#endif

void SoundManager::queryDevicesPortaudio() {
    m_paEnumerator.initialize();
}

void SoundManager::queryDevicesMixxx() {
    // Use the Network enumerator for our fork's network/Android device query
    for (auto& device : m_networkEnumerator.queryDevices()) {
        m_devices.push_back(SoundDevicePointer(device));
    }
}

void SoundManager::registerMainOutput(const AudioOutput& output) {
    registerOutput(output, m_pEngineMixer);
}

void SoundManager::unregisterOutput(const AudioOutput& output) {
    auto it = m_registeredSources.find(output);
    if (it != m_registeredSources.end()) {
        m_registeredSources.erase(it);
    }
}

void SoundManager::startCalibration(AudioLatencyCalibrator * calibrator) {
    m_pCalibrator = calibrator;
}

void SoundManager::stopCalibration() {
    m_pCalibrator = nullptr;
    m_calibFrameCache.clear();
}
