#include "soundio/portaudioenumerator.h"

#include <portaudio.h>

#include <QLibrary>
#include <QSharedPointer>
#ifdef Q_OS_ANDROID
#include <QDir>
#include <QDomDocument>
#include <QFile>
#include <algorithm>
#include <bit>

#include "soundio/soundmanagerconfig.h"
#include "util/cmdlineargs.h"
#endif

#include "audio/types.h"
#include "soundio/sounddevice.h"
#include "soundio/sounddeviceportaudio.h"
#include "util/versionstore.h"

#ifdef Q_OS_IOS
#include "soundio/soundmanagerios.h"
#elif defined(Q_OS_ANDROID)
#include <QtCore/private/qandroidextras_p.h>
#include <android/api-level.h>
#include <android/log.h>
#include <jni.h>
#include <pa_oboe.h>
#include <pthread.h>
#include <sys/syscall.h>

#include <QJniObject>
#include <cstdlib>

/// Called by PortAudio instead of malloc() (see MixxxAndroid.cmake): its
/// Oboe host API allocates strlen(name) bytes for a copy of each device name
/// and strcpy() then writes the terminating null byte past the end.
// (from JaviLabarumDJ/mixxx-android)
extern "C" void* mixxx_portaudio_malloc(size_t size) {
    return std::malloc(size + 1);
}
#endif

typedef PaError (*SetJackClientName)(const char* name);

#ifdef Q_OS_ANDROID
namespace {
/// Number of Oboe bursts that hold two engine buffers of the configured size.
unsigned int oboeBufferCount(unsigned int framesPerBurst) {
    constexpr unsigned int kMinBuffers = 2;
    if (framesPerBurst == 0) {
        return kMinBuffers;
    }
    unsigned int framesPerBuffer = 1024;
    QFile file(QDir(CmdlineArgs::Instance().getSettingsPath())
                    .filePath(SOUNDMANAGERCONFIG_FILENAME));
    QDomDocument doc;
    if (file.open(QIODevice::ReadOnly) && doc.setContent(&file)) {
        const QDomElement root = doc.documentElement();
        const unsigned int index = root.attribute(QStringLiteral("latency"), "0").toUInt();
        const unsigned int rateKhz =
                root.attribute(QStringLiteral("samplerate"), "48000").toUInt() / 1000;
        if (index > 0 && index <= SoundManagerConfig::kMaxAudioBufferSizeIndex && rateKhz > 0) {
            framesPerBuffer = std::bit_ceil(rateKhz) << (index - 1);
        }
    }
    const unsigned int buffers = (2 * framesPerBuffer + framesPerBurst - 1) / framesPerBurst;
    qInfo() << "Oboe:" << buffers << "bursts of" << framesPerBurst << "frames for"
            << framesPerBuffer << "frames per engine buffer";
    return std::max(kMinBuffers, buffers);
}
} // namespace
#endif

PortAudioEnumerator::PortAudioEnumerator(UserSettingsPointer config, SoundManager* sm)
        : m_pConfig(config),
          m_pSoundManager(sm) {
    initialize();
}

PortAudioEnumerator::~PortAudioEnumerator() {
    deinitialize();
}

void PortAudioEnumerator::initialize() {
    PaError err;
    if (m_initialized) {
        return;
    }

    m_jackSampleRate = mixxx::audio::SampleRate();

#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
    setJACKName();
#endif
#ifdef Q_OS_IOS
    mixxx::initializeAVAudioSession();
#elif defined(Q_OS_ANDROID)
    unsigned int nativeFramesPerBurst = 0;
    QNativeInterface::QAndroidApplication::runOnAndroidMainThread([&nativeFramesPerBurst]() {
        QJniObject context = QNativeInterface::QAndroidApplication::context();
        QJniObject AUDIO_SERVICE =
                QJniObject::getStaticObjectField(
                        "android/content/Context",
                        "AUDIO_SERVICE",
                        "Ljava/lang/String;");
        auto audioManager = context.callObjectMethod("getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                AUDIO_SERVICE.object());
        if (!audioManager.isValid()) {
            qDebug() << "audioManager invalid";
            return;
        }
        qDebug() << "audioManager valid:" << audioManager.toString();

        jint GET_DEVICES_INPUTS =
                QJniObject::getStaticField<jint>(
                        "android/media/AudioManager",
                        "GET_DEVICES_INPUTS");
        jint GET_DEVICES_OUTPUTS =
                QJniObject::getStaticField<jint>(
                        "android/media/AudioManager",
                        "GET_DEVICES_OUTPUTS");

        auto const supportedFriendlyName = [](int type) -> std::optional<QString> {
            switch (type) {
            case 1: // AudioDeviceInfo.TYPE_BUILTIN_EARPIECE
                // An earpiece is exclusively for phone calls and cannot be opened
                // for low-latency media playback on Android. Ignore it.
                return std::nullopt;
            case 2: // AudioDeviceInfo.TYPE_BUILTIN_SPEAKER
                return tr("Speaker");
            case 4: // AudioDeviceInfo.TYPE_WIRED_HEADPHONES
                return tr("Wired headphones");
            case 3: // AudioDeviceInfo.TYPE_WIRED_HEADSET
                return tr("Wired headset");
            case 9: // AudioDeviceInfo.TYPE_HDMI
                return tr("HDMI");
            case 10: // AudioDeviceInfo.TYPE_HDMI_ARC
                return tr("HDMI audio return channel");
            case 15: // AudioDeviceInfo.TYPE_BUILTIN_MIC
                return tr("Microphone");
            case 25: // AudioDeviceInfo.TYPE_REMOTE_SUBMIX:
                return tr("Mixed");
            // The suffixes below let the QML presets tell devices apart
            case 11: // AudioDeviceInfo.TYPE_USB_DEVICE
            case 22: // AudioDeviceInfo.TYPE_USB_HEADSET
            case 12: // AudioDeviceInfo.TYPE_USB_ACCESSORY
                return QStringLiteral("USB");
            case 8:  // AudioDeviceInfo.TYPE_BLUETOOTH_A2DP
            case 26: // AudioDeviceInfo.TYPE_BLE_HEADSET
            case 27: // AudioDeviceInfo.TYPE_BLE_SPEAKER
                return QStringLiteral("Bluetooth");
            case 13: // AudioDeviceInfo.TYPE_DOCK
            case 23: // AudioDeviceInfo.TYPE_HEARING_AID
                // supported, but no friendly name
                return QStringLiteral();
            default:
                // unsupported
                break;
            }
            return std::nullopt;
        };

        QJniObject PROPERTY_OUTPUT_SAMPLE_RATE =
                QJniObject::getStaticField<jstring>(
                        "android/media/AudioManager",
                        "PROPERTY_OUTPUT_SAMPLE_RATE");
        int defaultNativeSampleRate =
                audioManager
                        .callMethod<jstring>("getProperty",
                                "(Ljava/lang/String;)Ljava/lang/String;",
                                PROPERTY_OUTPUT_SAMPLE_RATE)
                        .toString()
                        .toInt();
        if (defaultNativeSampleRate <= 0) {
            defaultNativeSampleRate = 48000;
        }
        qDebug() << "audioManager defaultNativeSampleRate:" << defaultNativeSampleRate;

        auto const parse = [supportedFriendlyName, defaultNativeSampleRate](
                                   PaOboe_Direction direction,
                                   QJniArray<QJniObject>& devices) {
            for (const auto& device : devices) {
                jint type = device->callMethod<jint>("getType");
                auto maybeName = supportedFriendlyName(type);
                if (!maybeName.has_value()) {
                    continue;
                }
                QString name = device->callObjectMethod("getProductName",
                                             "()Ljava/lang/CharSequence;")
                                       .toString();
                if (!maybeName.value().isEmpty()) {
                    name.append(QStringLiteral(": %1").arg(maybeName.value()));
                }
                int32_t id = device->callMethod<jint>("getId");
                auto channelCounts = device->callMethod<QJniArray<jint>>("getChannelCounts");
                // AudioDeviceInfo.getChannelCounts() returns an empty array
                // when the device accepts arbitrary channel counts (common for
                // the built-in speaker and Bluetooth A2DP). Default to stereo
                // for outputs and mono for inputs; previously std::max_element
                // dereferenced end() and passed garbage (or 0) to PortAudio.
                int channelCount = 0;
                if (!channelCounts.isEmpty()) {
                    channelCount = *std::max_element(
                            channelCounts.begin(), channelCounts.end());
                }
                if (channelCount <= 0) {
                    channelCount = (direction == PaOboe_Direction::Output) ? 2 : 1;
                }
                auto sampleRates = device->callMethod<QJniArray<jint>>("getSampleRates");
                // Likewise, getSampleRates() is empty when the device supports
                // arbitrary rates (e.g. A2DP Bluetooth or the system mixer);
                // fall back to AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE instead
                // of silently dropping the device.
                int sampleRate = defaultNativeSampleRate;
                if (!sampleRates.isEmpty()) {
                    if (std::find(sampleRates.cbegin(),
                                sampleRates.cend(),
                                defaultNativeSampleRate) != sampleRates.cend()) {
                        sampleRate = defaultNativeSampleRate;
                    } else {
                        sampleRate = *std::max_element(
                                sampleRates.cbegin(), sampleRates.cend());
                    }
                }
                qDebug() << "audioManager - Type:" << type
                         << "- Name:" << name
                         << "- Id:" << id
                         << "- Direction:" << static_cast<int>(direction)
                         << "- ChannelCount:" << channelCount
                         << "(" << channelCounts.size() << "reported)"
                         << "- SampleRate:" << sampleRate
                         << "(" << sampleRates.size() << "reported)";
                auto result = PaOboe_RegisterDevice(name.toStdString().c_str(),
                        id,
                        direction,
                        channelCount,
                        sampleRate);
                if (result != paNoError) {
                    qWarning()
                            << "Error registering device to PortAudio:"
                            << Pa_GetErrorText(result);
                }
            }
        };

        auto inputDevices =
                audioManager.callMethod<QJniArray<QJniObject>>("getDevices",
                        "(I)[Landroid/media/AudioDeviceInfo;",
                        GET_DEVICES_INPUTS);
        qDebug() << "audioManager inputDevices:" << inputDevices.size();
        parse(PaOboe_Direction::Input, inputDevices);

        auto outputDevices =
                audioManager.callMethod<QJniArray<QJniObject>>("getDevices",
                        "(I)[Landroid/media/AudioDeviceInfo;",
                        GET_DEVICES_OUTPUTS);
        qDebug() << "audioManager outputDevices:" << outputDevices.size();
        parse(PaOboe_Direction::Output, outputDevices);

        QJniObject PROPERTY_OUTPUT_FRAMES_PER_BUFFER =
                QJniObject::getStaticField<jstring>(
                        "android/media/AudioManager",
                        "PROPERTY_OUTPUT_FRAMES_PER_BUFFER");
        auto outputFramePerBuffer =
                audioManager
                        .callMethod<jstring>("getProperty",
                                "(Ljava/lang/String;)Ljava/lang/String;",
                                PROPERTY_OUTPUT_FRAMES_PER_BUFFER)
                        .toString()
                        .toUInt();
        qDebug() << "audioManager outputFramePerBuffer:" << outputFramePerBuffer;
        PaOboe_SetNativeBufferSize(outputFramePerBuffer);
        nativeFramesPerBurst = outputFramePerBuffer;
    }).waitForFinished();
    // PortAudio's Oboe stream asks for one engine buffer (framesPerBuffer,
    // 1024 frames by default) per callback, but sizes the AAudio buffer to
    // burst * numberOfBuffers. With 2 buffers of a 192 frame burst the
    // device buffer (384 frames) cannot even hold one callback, so AAudio
    // underruns on every cycle: the "popcorn" crackle on the phone output.
    // Keep room for two engine buffers. The count is fixed before
    // Pa_Initialize(), so it is taken from the saved buffer size.
    PaOboe_SetNumberOfBuffers(oboeBufferCount(nativeFramesPerBurst));

    // NOTE: Do NOT call sched_setaffinity on the calling thread here.
    // PortAudioEnumerator::initialize() runs on the Qt main thread (or the
    // CoreServices init thread), and child threads spawned afterwards inherit
    // the affinity mask — pinning the whole app to a single core (#4), which
    // may not even exist on 4-core devices. Oboe's callback thread is created
    // internally by AAudio/OpenSL ES and scheduled by Android's audio server.
#endif
    err = Pa_Initialize();
    m_initialized = true;

    if (err != paNoError) {
        qDebug() << "Error:" << Pa_GetErrorText(err);
        m_initialized = false;
        return;
    }

    int iNumDevices = Pa_GetDeviceCount();
    if (iNumDevices < 0) {
        qDebug() << "ERROR: Pa_CountDevices returned" << Pa_GetErrorText(iNumDevices);
        return;
    } else if (iNumDevices == 0) {
        qWarning() << "Pa_CountDevices returned no devices!";
    } else {
        qDebug() << "Pa_CountDevices found" << iNumDevices << "devices";
    }
    qDebug() << "Pa_GetHostApiCount returns" << Pa_GetHostApiCount();

    // PaDeviceInfo structs have a PaHostApiIndex member, but PortAudio
    // unfortunately provides no good way to associate this with a persistent,
    // unique identifier for the API. So, build a QHash to do that and pass
    // it to the SoundDevicePortAudio constructor.
    QHash<PaHostApiIndex, PaHostApiTypeId> paApiIndexToTypeId;
    for (PaHostApiIndex i = 0; i < Pa_GetHostApiCount(); i++) {
        const PaHostApiInfo* api = Pa_GetHostApiInfo(i);
        if (api && QString(api->name) != "skeleton implementation") {
            paApiIndexToTypeId.insert(i, api->type);
        }
    }

    const PaDeviceInfo* deviceInfo;
    for (int i = 0; i < iNumDevices; i++) {
        deviceInfo = Pa_GetDeviceInfo(i);
        qDebug() << "Pa_GetDeviceInfo on" << i << deviceInfo;
        if (!deviceInfo) {
            continue;
        }
        /* deviceInfo fields for quick reference:
            int     structVersion
            const char *    name
            PaHostApiIndex  hostApi
            int     maxInputChannels
            int     maxOutputChannels
            PaTime  defaultLowInputLatency
            PaTime  defaultLowOutputLatency
            PaTime  defaultHighInputLatency
            PaTime  defaultHighOutputLatency
            double  defaultSampleRate
         */
        const auto deviceTypeId = paApiIndexToTypeId.value(deviceInfo->hostApi);
        auto currentDevice = QSharedPointer<SoundDevicePortAudio>::create(
                m_pConfig, m_pSoundManager, deviceInfo, deviceTypeId, i);
        m_devices.push_back(currentDevice);
        if (Pa_GetHostApiInfo(deviceInfo->hostApi)->name == SoundManagerConfig::kAPIJack) {
            m_jackSampleRate = static_cast<mixxx::audio::SampleRate::value_t>(
                    deviceInfo->defaultSampleRate);
        }
    }

#ifdef __ANDROID_USB_AUDIO__
    m_androidUsbDevices.clear();
    // At startup the audio can only be configured once the permission for
    // the sound card is known
    refreshAndroidUsbDevices(nullptr, true);
#endif

    for (PaHostApiIndex i = 0; i < Pa_GetHostApiCount(); i++) {
        const PaHostApiInfo* api = Pa_GetHostApiInfo(i);
        if (api && std::strcmp(api->name, "skeleton implementation")) {
#ifdef Q_OS_ANDROID
            if (std::strcmp(api->name, "ALSA") == 0) {
                continue;
            }
#endif
            m_apis.push_back(api->name);
        }
    }
}

std::vector<SoundDevicePointer> PortAudioEnumerator::queryDevices() const {
    if (!m_initialized) {
        return {};
    }

    std::vector<SoundDevicePointer> devices;
    for (const auto& device : m_devices) {
        devices.push_back(device);
    }
#ifdef __ANDROID_USB_AUDIO__
    for (const auto& device : m_androidUsbDevices.devices()) {
        devices.push_back(device);
    }
#endif

    return devices;
}

void PortAudioEnumerator::setJACKName() const {
#ifdef Q_OS_LINUX
    typedef PaError (*SetJackClientName)(const char* name);
    QLibrary portaudio("libportaudio.so.2");
    if (portaudio.load()) {
        SetJackClientName func(
                reinterpret_cast<SetJackClientName>(
                        portaudio.resolve("PaJack_SetClientName")));
        if (func) {
            // PortAudio does not make a copy of the string we provide it so we
            // need to make sure it will last forever so we intentionally leak
            // this string.
            char* jackNameCopy = strdup(VersionStore::applicationName().toLocal8Bit().constData());
            if (!func(jackNameCopy)) {
                qDebug() << "JACK client name set";
            }
        } else {
            qWarning() << "failed to resolve JACK name method";
        }
    } else {
        qWarning() << "failed to load portaudio for JACK rename";
    }
#endif
}

void PortAudioEnumerator::deinitialize() {
    if (!m_initialized) {
        return;
    }

    m_devices.clear();
#ifdef __ANDROID_USB_AUDIO__
    m_androidUsbDevices.clear();
#endif

    PaError err = Pa_Terminate();
    if (err == paNoError) {
        m_initialized = false;
    }
}

QList<mixxx::audio::SampleRate> PortAudioEnumerator::getSampleRates(bool jackSampleRate) const {
    if (jackSampleRate and m_jackSampleRate.isValid()) {
        return {m_jackSampleRate};
    }

    return {};
}

#ifdef __ANDROID_USB_AUDIO__
bool PortAudioEnumerator::refreshAndroidUsbDevices(
        std::vector<SoundDevicePointer>* pRemoved, bool waitForPermission) {
    return m_androidUsbDevices.refresh(
            m_pConfig, m_pSoundManager, &m_devices, pRemoved, waitForPermission);
}
#endif
