#include "engine/fxstation/fxstationinstruments.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>
#include <cmath>
#include <random>

#include "library/samples/bundledsamples.h"
#include "moc_fxstationinstruments.cpp"
#include "sources/soundsourceproxy.h"
#include "track/track.h"
#include "util/samplebuffer.h"

namespace mixxx {
namespace fxstation {

namespace {

constexpr float kSynthesisRate = 48000.f;
constexpr float kPi = 3.14159265358979f;
// User sounds are cut after this long
constexpr double kMaxUserSeconds = 4.0;

const char* const kNamePrefixes[kInstruments] = {"kick", "snare", "clap", "hat"};

/// A mono sound into stereo, peaking at peak
Instrument stereo(const std::vector<float>& mono, float peak) {
    float maximum = 1e-6f;
    for (float sample : mono) {
        maximum = std::max(maximum, std::fabs(sample));
    }
    Instrument instrument;
    instrument.sampleRate = kSynthesisRate;
    instrument.samples.resize(2 * mono.size());
    for (size_t i = 0; i < mono.size(); ++i) {
        const float sample = mono[i] * peak / maximum;
        instrument.samples[2 * i] = sample;
        instrument.samples[2 * i + 1] = sample;
    }
    return instrument;
}

std::vector<float> silence(float seconds) {
    return std::vector<float>(static_cast<size_t>(seconds * kSynthesisRate), 0.f);
}

// A one pole high pass, enough to shape noise
struct HighPass {
    float previousIn = 0.f;
    float previousOut = 0.f;
    float coefficient;
    explicit HighPass(float frequency)
            : coefficient(1.f / (1.f + 2.f * kPi * frequency / kSynthesisRate)) {
    }
    float process(float x) {
        previousOut = coefficient * (previousOut + x - previousIn);
        previousIn = x;
        return previousOut;
    }
};

Instrument synthesizeKick() {
    std::vector<float> sound = silence(0.45f);
    std::mt19937 random(1);
    std::uniform_real_distribution<float> noise(-1.f, 1.f);
    float phase = 0.f;
    for (size_t i = 0; i < sound.size(); ++i) {
        const float t = static_cast<float>(i) / kSynthesisRate;
        // The pitch drops fast from a click to the body
        const float frequency = 48.f + 140.f * std::exp(-t * 38.f);
        phase += 2.f * kPi * frequency / kSynthesisRate;
        const float body = std::sin(phase) * std::exp(-t * 7.f);
        const float click = t < 0.003f ? noise(random) * (1.f - t / 0.003f) * 0.4f : 0.f;
        sound[i] = std::tanh(1.6f * body) + click;
    }
    return stereo(sound, 0.9f);
}

Instrument synthesizeSnare() {
    std::vector<float> sound = silence(0.3f);
    std::mt19937 random(2);
    std::uniform_real_distribution<float> noise(-1.f, 1.f);
    HighPass highPass(1500.f);
    float phase = 0.f;
    for (size_t i = 0; i < sound.size(); ++i) {
        const float t = static_cast<float>(i) / kSynthesisRate;
        phase += 2.f * kPi * (185.f + 60.f * std::exp(-t * 60.f)) / kSynthesisRate;
        const float tone = std::sin(phase) * std::exp(-t * 28.f) * 0.55f;
        const float rattle = highPass.process(noise(random)) * std::exp(-t * 16.f);
        sound[i] = tone + rattle;
    }
    return stereo(sound, 0.75f);
}

Instrument synthesizeClap() {
    std::vector<float> sound = silence(0.32f);
    std::mt19937 random(3);
    std::uniform_real_distribution<float> noise(-1.f, 1.f);
    HighPass highPass(900.f);
    // Three short bursts, then the room
    const float bursts[] = {0.f, 0.011f, 0.022f};
    for (size_t i = 0; i < sound.size(); ++i) {
        const float t = static_cast<float>(i) / kSynthesisRate;
        float envelope = 0.f;
        for (float start : bursts) {
            if (t >= start && t < start + 0.01f) {
                envelope = std::max(envelope, std::exp(-(t - start) * 220.f));
            }
        }
        if (t >= 0.033f) {
            envelope = std::max(envelope, 0.7f * std::exp(-(t - 0.033f) * 17.f));
        }
        sound[i] = highPass.process(noise(random)) * envelope;
    }
    return stereo(sound, 0.7f);
}

Instrument synthesizeHat() {
    std::vector<float> sound = silence(0.12f);
    std::mt19937 random(4);
    std::uniform_real_distribution<float> noise(-1.f, 1.f);
    HighPass highPass1(6500.f);
    HighPass highPass2(6500.f);
    for (size_t i = 0; i < sound.size(); ++i) {
        const float t = static_cast<float>(i) / kSynthesisRate;
        sound[i] = highPass2.process(highPass1.process(noise(random))) * std::exp(-t * 48.f);
    }
    return stereo(sound, 0.5f);
}

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
    auto pFactory = std::make_unique<Bank>();
    pFactory->instruments[0] = synthesizeKick();
    pFactory->instruments[1] = synthesizeSnare();
    pFactory->instruments[2] = synthesizeClap();
    pFactory->instruments[3] = synthesizeHat();
    publish(0, std::move(pFactory));
    m_threadPool.setMaxThreadCount(1);
    reloadUserBanks();
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
    if (index == 1 && dir.isEmpty()) {
        const QString bundled = samples::bundledSamplesDir(m_pConfig);
        if (!bundled.isEmpty()) {
            dir.setPath(bundled + QStringLiteral("djsugar"));
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

void Instruments::reloadUserBanks() {
    for (int index = 1; index < kBanks; ++index) {
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
