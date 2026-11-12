#include "engine/fxstation/fxstationinstruments.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>

#include "library/samples/bundledsamples.h"
#include "moc_fxstationinstruments.cpp"
#include "sources/soundsourceproxy.h"
#include "track/track.h"
#include "util/samplebuffer.h"

namespace mixxx {
namespace fxstation {

namespace {

// User sounds are cut after this long
constexpr double kMaxUserSeconds = 4.0;

const char* const kNamePrefixes[kInstruments] = {"kick", "snare", "clap", "hat"};
// Bank 1: the app's own drum samples, in res/samples. The kick has a
// click: kick.ogg is nearly all below 150 Hz, which phone speakers drop
const char* const kFactoryFiles[kInstruments] = {
        "hardstyle-kick.ogg", "snare.ogg", "clap.ogg", "hi-hat.ogg"};

/// Decodes a sound file into stereo, at most kMaxUserSeconds long
Instrument decode(const QString& path) {
    Instrument instrument;
    TrackPointer pTrack = Track::newTemporary(path);
    SoundSourceProxy proxy(pTrack);
    const auto pAudioSource = proxy.openAudioSource(mixxx::AudioSource::OpenParams(
            mixxx::audio::ChannelCount::stereo(), mixxx::audio::SampleRate()));
    if (!pAudioSource) {
        return instrument;
    }
    const auto sampleRate = pAudioSource->getSignalInfo().getSampleRate();
    IndexRange range = pAudioSource->frameIndexRange();
    const SINT maxFrames = static_cast<SINT>(kMaxUserSeconds * sampleRate);
    if (range.length() > maxFrames) {
        range = IndexRange::forward(range.start(), maxFrames);
    }
    mixxx::SampleBuffer buffer(2 * range.length());
    const auto readable = pAudioSource->readSampleFrames(mixxx::WritableSampleFrames(
            range, mixxx::SampleBuffer::WritableSlice(buffer)));
    instrument.sampleRate = static_cast<float>(sampleRate);
    instrument.samples.assign(readable.readableData(),
            readable.readableData() + readable.readableLength());
    return instrument;
}

} // namespace

Instruments::Instruments(UserSettingsPointer pConfig, QObject* pParent)
        : QObject(pParent),
          m_pConfig(std::move(pConfig)) {
    for (auto& bank : m_banks) {
        bank.store(nullptr);
    }
    m_threadPool.setMaxThreadCount(1);
    reloadBanks();
}

Instruments::~Instruments() {
    m_threadPool.waitForDone();
}

// static
QString Instruments::bankDirectory(int index) {
    return QStringLiteral("/storage/emulated/0/Mixxx/FxStation/Bank %1").arg(index + 1);
}

void Instruments::publish(int index, std::unique_ptr<Bank> pBank) {
    const Bank* pPublished = pBank.get();
    {
        std::lock_guard lock(m_ownedMutex);
        m_owned.push_back(std::move(pBank));
    }
    m_banks[index].store(pPublished, std::memory_order_release);
}

// static
ConfigKey Instruments::soundKey(int index, int instrument) {
    return ConfigKey(QStringLiteral("[FxStation]"),
            QStringLiteral("bank%1_sound%2").arg(index + 1).arg(instrument + 1));
}

std::unique_ptr<Bank> Instruments::loadBank(int index) const {
    std::array<QString, kInstruments> chosen;
    if (index == 0) {
        const QString bundled = samples::bundledSamplesDir(m_pConfig);
        for (int instrument = 0; instrument < kInstruments; ++instrument) {
            if (!bundled.isEmpty()) {
                chosen[instrument] = bundled + QLatin1String(kFactoryFiles[instrument]);
            }
        }
    } else {
        chosen = folderSounds(index);
    }
    bool any = false;
    for (int instrument = 0; instrument < kInstruments; ++instrument) {
        const QString picked = m_pConfig->getValueString(soundKey(index, instrument));
        if (!picked.isEmpty() && QFile::exists(picked)) {
            chosen[instrument] = picked;
        }
        any = any || !chosen[instrument].isEmpty();
    }
    if (!any) {
        return nullptr;
    }
    auto pBank = std::make_unique<Bank>();
    for (int instrument = 0; instrument < kInstruments; ++instrument) {
        if (!chosen[instrument].isEmpty()) {
            pBank->instruments[instrument] = decode(chosen[instrument]);
        }
    }
    return pBank;
}

std::array<QString, kInstruments> Instruments::folderSounds(int index) const {
    QDir dir(bankDirectory(index));
    // The sounds stay out of the music apps
    const QString noMedia = QFileInfo(dir.absolutePath()).path() + QStringLiteral("/.nomedia");
    if (!dir.exists()) {
        // Created empty, so that users find where their sounds go
        QDir().mkpath(dir.absolutePath());
    }
    if (!QFile::exists(noMedia)) {
        QFile file(noMedia);
        if (!file.open(QIODevice::WriteOnly)) {
            qWarning() << "FxStation: cannot create" << noMedia;
        }
    }
    QStringList files;
    const auto entries = dir.entryInfoList(QDir::Files, QDir::Name);
    for (const QFileInfo& entry : entries) {
        if (SoundSourceProxy::isFileNameSupported(entry.fileName())) {
            files.append(entry.absoluteFilePath());
        }
    }
    // By name, or else in order
    std::array<QString, kInstruments> chosen;
    for (int instrument = 0; instrument < kInstruments; ++instrument) {
        for (const QString& file : std::as_const(files)) {
            // "kick.wav" or "01_kick.wav"
            static const QRegularExpression kNumberPrefix(QStringLiteral("^\\d+[_ -]"));
            if (QFileInfo(file)
                            .fileName()
                            .remove(kNumberPrefix)
                            .startsWith(
                                    QLatin1String(kNamePrefixes[instrument]),
                                    Qt::CaseInsensitive)) {
                chosen[instrument] = file;
                break;
            }
        }
    }
    int next = 0;
    for (auto& path : chosen) {
        while (path.isEmpty() && next < files.size()) {
            if (!std::count(chosen.begin(), chosen.end(), files[next])) {
                path = files[next];
            }
            ++next;
        }
    }
    return chosen;
}

void Instruments::reloadBanks() {
    for (int index = 0; index < kBanks; ++index) {
        m_threadPool.start([this, index]() {
            auto pBank = loadBank(index);
            int sounds = 0;
            if (pBank) {
                for (const auto& instrument : pBank->instruments) {
                    sounds += instrument.frames() > 0 ? 1 : 0;
                }
            }
            if (sounds > 0) {
                publish(index, std::move(pBank));
            } else {
                m_banks[index].store(nullptr, std::memory_order_release);
            }
            emit bankLoaded(index, sounds);
        });
    }
}

} // namespace fxstation
} // namespace mixxx
