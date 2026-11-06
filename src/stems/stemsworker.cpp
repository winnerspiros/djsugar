#include "stems/stemsworker.h"

#include <sys/resource.h>
#include <unistd.h>

#include <QDir>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QThread>
#include <cmath>
#include <exception>
#include <random>

#include "moc_stemsworker.cpp"
#include "sources/soundsourceproxy.h"
#include "stems/stemcompanion.h"
#include "stems/stemfilewriter.h"
#include "stems/stemmodels.h"
#include "track/track.h"
#include "util/logger.h"
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

extern "C" {
#include <libavutil/channel_layout.h>
#include <libswresample/swresample.h>
}

namespace mixxx {
namespace stems {

namespace {

const Logger kLogger("StemsWorker");

// Separating takes the memory of the whole decoded track
constexpr double kMaxTrackMinutes = 20.0;
constexpr SINT kDecodeChunkFrames = 65536;
// Below the UI, far below the audio
constexpr int kWorkerNiceness = 10;

/// Resamples interleaved stereo float audio
class Resampler {
  public:
    Resampler(int fromRate, int toRate) {
        AVChannelLayout stereo = AV_CHANNEL_LAYOUT_STEREO;
        swr_alloc_set_opts2(&m_pContext,
                &stereo,
                AV_SAMPLE_FMT_FLT,
                toRate,
                &stereo,
                AV_SAMPLE_FMT_FLT,
                fromRate,
                0,
                nullptr);
        if (m_pContext && swr_init(m_pContext) < 0) {
            swr_free(&m_pContext);
        }
    }
    ~Resampler() {
        swr_free(&m_pContext);
    }
    bool isValid() const {
        return m_pContext != nullptr;
    }
    /// Appends the resampled frames to pOutput. Without input, flushes.
    void process(const float* pInput, SINT frames, std::vector<float>* pOutput) {
        const int capacity = swr_get_out_samples(m_pContext, static_cast<int>(frames));
        if (capacity <= 0) {
            return;
        }
        const size_t at = pOutput->size();
        pOutput->resize(at + 2 * static_cast<size_t>(capacity));
        auto* pOut = reinterpret_cast<uint8_t*>(pOutput->data() + at);
        const auto* pIn = reinterpret_cast<const uint8_t*>(pInput);
        const int done = swr_convert(m_pContext,
                &pOut,
                capacity,
                pInput ? &pIn : nullptr,
                pInput ? static_cast<int>(frames) : 0);
        pOutput->resize(at + 2 * static_cast<size_t>(std::max(0, done)));
    }

  private:
    SwrContext* m_pContext = nullptr;
};

void lowerThreadPriority() {
    setpriority(PRIO_PROCESS, static_cast<id_t>(gettid()), kWorkerNiceness);
}

} // namespace

bool StemsWorker::waitWhileHeld() {
    while (m_hold && !m_canceled) {
        QThread::msleep(250);
    }
    return !m_canceled;
}

void StemsWorker::run(const QString& location, int model, int bitrateKbps, int threads) {
    lowerThreadPriority();
    m_canceled = false;
    Job job;
    job.location = location;
    job.model = static_cast<Model>(model);
    job.bitrateKbps = bitrateKbps;
    job.threads = threads;
    QString stemFile;
    QString error;
    try {
        error = separate(job, &stemFile);
    } catch (const std::exception& e) {
        kLogger.warning() << "Separation failed" << e.what();
        error = QStringLiteral("memory");
    }
    if (!error.isEmpty()) {
        stemFile.clear();
    }
    emit finished(location, stemFile, error);
}

QString StemsWorker::separate(const Job& job, QString* pStemFile) {
    const QString initError = initOnnxRuntime();
    if (!initError.isEmpty()) {
        kLogger.warning() << initError;
        return QStringLiteral("runtime");
    }
    if (!StemModels::isInstalled(job.model)) {
        return QStringLiteral("model");
    }
#ifdef __ANDROID_USB_DRIVE__
    // Tracks of USB drives that Mixxx reads itself are copied on demand
    const bool onReadOnlyDrive = mixxx::usbdrive::isOnDrive(job.location);
    if (onReadOnlyDrive) {
        mixxx::usbdrive::fetchFile(job.location);
    }
#else
    const bool onReadOnlyDrive = false;
#endif
    if (!QFileInfo::exists(job.location)) {
        return QStringLiteral("not-found");
    }

    // Decode the whole track
    // Where the time goes, logged at the end
    QElapsedTimer totalTimer;
    totalTimer.start();
    QElapsedTimer stageTimer;
    stageTimer.start();
    qint64 writeNanos = 0;

    emit progress(job.location, Stage::Decoding, 0.0);
    TrackPointer pTrack = Track::newTemporary(job.location);
    SoundSourceProxy proxy(pTrack);
    const auto pAudioSource = proxy.openTrackFileAudioSource(mixxx::AudioSource::OpenParams(
            mixxx::audio::ChannelCount::stereo(), mixxx::audio::SampleRate()));
    if (!pAudioSource) {
        return QStringLiteral("decode");
    }
    const int sampleRate = static_cast<int>(pAudioSource->getSignalInfo().getSampleRate());
    const IndexRange range = pAudioSource->frameIndexRange();
    if (range.length() > kMaxTrackMinutes * 60 * sampleRate) {
        return QStringLiteral("too-long");
    }
    if (range.length() < sampleRate) {
        return QStringLiteral("decode");
    }
    std::vector<float> track;
    track.reserve(2 * static_cast<size_t>(range.length()));
    {
        mixxx::SampleBuffer buffer(2 * kDecodeChunkFrames);
        IndexRange remaining = range;
        while (!remaining.empty()) {
            if (m_canceled) {
                return QStringLiteral("canceled");
            }
            const IndexRange chunk = remaining.splitAndShrinkFront(
                    std::min(kDecodeChunkFrames, remaining.length()));
            const auto readable = pAudioSource->readSampleFrames(mixxx::WritableSampleFrames(
                    chunk, mixxx::SampleBuffer::WritableSlice(buffer)));
            const CSAMPLE* pData = readable.readableData();
            track.insert(track.end(), pData, pData + readable.readableLength());
            emit progress(job.location,
                    Stage::Decoding,
                    static_cast<double>(range.length() - remaining.length()) / range.length());
        }
    }
    pAudioSource->close();
    const SINT trackFrames = static_cast<SINT>(track.size() / 2);
    const qint64 decodeMs = stageTimer.restart();

    // The models separate audio at 44.1 kHz, the stem file keeps the rate of
    // the track so that its cues and beats stay in place
    std::vector<float> resampledTrack;
    const float* pModelInput = track.data();
    SINT modelFrames = trackFrames;
    std::array<std::unique_ptr<Resampler>, kStemCount> stemResamplers;
    if (sampleRate != kModelSampleRate) {
        Resampler toModel(sampleRate, kModelSampleRate);
        if (!toModel.isValid()) {
            return QStringLiteral("decode");
        }
        toModel.process(track.data(), trackFrames, &resampledTrack);
        toModel.process(nullptr, 0, &resampledTrack);
        pModelInput = resampledTrack.data();
        modelFrames = static_cast<SINT>(resampledTrack.size() / 2);
        for (auto& pResampler : stemResamplers) {
            pResampler = std::make_unique<Resampler>(kModelSampleRate, sampleRate);
            if (!pResampler->isValid()) {
                return QStringLiteral("decode");
            }
        }
    }

    // Load the model
    const qint64 resampleMs = stageTimer.restart();

    emit progress(job.location, Stage::Loading, 0.0);
    std::unique_ptr<StemSeparator> pSeparator = StemSeparator::create(job.model);
    StemSeparator::Options options;
    options.threads = job.threads;
    if (!pSeparator->load(StemModels::modelDir(job.model), options)) {
        kLogger.warning() << "Unable to load the model" << pSeparator->error();
        return QStringLiteral("model");
    }

    // Next to the track if its folder can be written, otherwise in the
    // app's storage. The drives Mixxx reads itself can't be written, their
    // tracks are only copies.
    QString stemFile = onReadOnlyDrive ? fallbackPath(job.location) : companionPath(job.location);
    QDir().mkpath(QFileInfo(stemFile).absolutePath());
    StemFileWriter writer;
    const QMap<QString, QString> tags = {
            {QStringLiteral("comment"),
                    job.model == Model::Spleeter
                            ? QStringLiteral("Stems by Mixxx Android (Spleeter)")
                            : QStringLiteral("Stems by Mixxx Android (HTDemucs)")},
    };
    if (!writer.open(stemFile + QStringLiteral(".part"), sampleRate, job.bitrateKbps, tags)) {
        kLogger.info() << "Unable to write next to the track:" << writer.error();
        writer.abort();
        stemFile = fallbackPath(job.location);
        QDir().mkpath(QFileInfo(stemFile).absolutePath());
        if (!writer.open(stemFile + QStringLiteral(".part"),
                    sampleRate,
                    job.bitrateKbps,
                    tags)) {
            kLogger.warning() << "Unable to write the stem file" << writer.error();
            writer.abort();
            return QStringLiteral("write");
        }
    }

    // Separate, writing the stems as they come with the matching part of
    // the track as the mix
    SINT mixFrames = 0;
    bool writeFailed = false;
    std::array<std::vector<float>, kStemCount> resampled;
    const auto writeStems = [&](const std::array<const float*, kStemCount>& stems,
                                    SINT frameCount) {
        // The resamplers may lag a few frames behind: only what all have
        const SINT count = std::min(frameCount, trackFrames - mixFrames);
        QElapsedTimer writeTimer;
        writeTimer.start();
        const bool written = count <= 0 || writer.write(track.data() + 2 * mixFrames, stems, count);
        writeNanos += writeTimer.nsecsElapsed();
        if (!written) {
            writeFailed = true;
            return false;
        }
        mixFrames += std::max<SINT>(0, count);
        return true;
    };
    const StemSink sink = [&](const std::array<const float*, kStemCount>& stems,
                                  SINT frameCount) {
        if (!stemResamplers[0]) {
            return writeStems(stems, frameCount);
        }
        for (int stem = 0; stem < kStemCount; ++stem) {
            stemResamplers[stem]->process(stems[stem], frameCount, &resampled[stem]);
        }
        SINT available = static_cast<SINT>(resampled[0].size() / 2);
        for (const auto& buffer : resampled) {
            available = std::min(available, static_cast<SINT>(buffer.size() / 2));
        }
        if (!writeStems({resampled[0].data(),
                                resampled[1].data(),
                                resampled[2].data(),
                                resampled[3].data()},
                    available)) {
            return false;
        }
        for (auto& buffer : resampled) {
            buffer.erase(buffer.begin(), buffer.begin() + 2 * available);
        }
        return true;
    };
    const ProgressCallback progressCallback = [&](double fraction) {
        emit progress(job.location, Stage::Separating, fraction);
        return waitWhileHeld();
    };
    const qint64 loadMs = stageTimer.restart();
    emit progress(job.location, Stage::Separating, 0.0);
    if (!pSeparator->separate(pModelInput, modelFrames, sink, progressCallback)) {
        writer.abort();
        if (m_canceled) {
            return QStringLiteral("canceled");
        }
        if (writeFailed) {
            kLogger.warning() << "Unable to write the stem file" << writer.error();
            return QStringLiteral("write");
        }
        kLogger.warning() << "Unable to separate" << job.location << pSeparator->error();
        return pSeparator->error() == QStringLiteral("Low memory") ? QStringLiteral("memory")
                                                                   : QStringLiteral("model");
    }
    pSeparator.reset();
    const qint64 separateMs = stageTimer.restart();

    emit progress(job.location, Stage::Saving, 0.0);
    if (stemResamplers[0]) {
        for (int stem = 0; stem < kStemCount; ++stem) {
            stemResamplers[stem]->process(nullptr, 0, &resampled[stem]);
        }
        SINT available = static_cast<SINT>(resampled[0].size() / 2);
        for (const auto& buffer : resampled) {
            available = std::min(available, static_cast<SINT>(buffer.size() / 2));
        }
        writeStems({resampled[0].data(),
                           resampled[1].data(),
                           resampled[2].data(),
                           resampled[3].data()},
                available);
    }
    // Rounding of the resampling: silence until the end of the mix
    if (mixFrames < trackFrames && !writeFailed) {
        const std::vector<float> silence(2 * static_cast<size_t>(trackFrames - mixFrames), 0.0f);
        writeStems({silence.data(), silence.data(), silence.data(), silence.data()},
                trackFrames - mixFrames);
    }
    if (writeFailed || !writer.finish()) {
        kLogger.warning() << "Unable to write the stem file" << writer.error();
        writer.abort();
        return QStringLiteral("write");
    }
    kLogger.info() << "Separated" << trackFrames / sampleRate << "s of audio in"
                   << totalTimer.elapsed() << "ms: decoding" << decodeMs << "resampling"
                   << resampleMs << "loading" << loadMs << "separating" << separateMs
                   << "(writing" << writeNanos / 1000000 << ") finishing"
                   << stageTimer.elapsed();
    QFile::remove(stemFile);
    if (!QFile::rename(stemFile + QStringLiteral(".part"), stemFile)) {
        QFile::remove(stemFile + QStringLiteral(".part"));
        return QStringLiteral("write");
    }
    // A stem file of another place is outdated now
    const QString otherPlace = stemFile == companionPath(job.location)
            ? fallbackPath(job.location)
            : companionPath(job.location);
    QFile::remove(otherPlace);
    *pStemFile = stemFile;
    return QString();
}

void StemsWorker::benchmark(int model, int threads) {
    lowerThreadPriority();
    m_canceled = false;
    const QString initError = initOnnxRuntime();
    if (!initError.isEmpty()) {
        emit benchmarkFinished(model, 0.0, QStringLiteral("runtime"));
        return;
    }
    if (!StemModels::isInstalled(static_cast<Model>(model))) {
        emit benchmarkFinished(model, 0.0, QStringLiteral("model"));
        return;
    }
    try {
        std::unique_ptr<StemSeparator> pSeparator = StemSeparator::create(static_cast<Model>(model));
        StemSeparator::Options options;
        options.threads = threads;
        if (!pSeparator->load(StemModels::modelDir(static_cast<Model>(model)), options)) {
            emit benchmarkFinished(model, 0.0, QStringLiteral("model"));
            return;
        }
        // Noise with a beat: the speed doesn't depend on the music
        constexpr SINT kFrames = 20 * kModelSampleRate;
        std::vector<float> audio(2 * kFrames);
        std::mt19937 random(42);
        std::normal_distribution<float> noise(0.0f, 0.05f);
        for (SINT i = 0; i < kFrames; ++i) {
            const float kick = (i % (kModelSampleRate / 2)) < 4000
                    ? 0.5f * std::sin(2.0f * static_cast<float>(M_PI) * 55.0f * i / kModelSampleRate)
                    : 0.0f;
            audio[2 * i] = kick + noise(random);
            audio[2 * i + 1] = kick + noise(random);
        }
        QElapsedTimer timer;
        timer.start();
        const bool done = pSeparator->separate(
                audio.data(),
                kFrames,
                [](const std::array<const float*, kStemCount>&, SINT) { return true; },
                [this](double) { return !m_canceled; });
        if (!done) {
            emit benchmarkFinished(model,
                    0.0,
                    m_canceled ? QStringLiteral("canceled") : QStringLiteral("model"));
            return;
        }
        const double seconds = std::max(0.001, timer.elapsed() / 1000.0);
        emit benchmarkFinished(model, static_cast<double>(kFrames) / kModelSampleRate / seconds, QString());
    } catch (const std::exception& e) {
        kLogger.warning() << "Benchmark failed" << e.what();
        emit benchmarkFinished(model, 0.0, QStringLiteral("memory"));
    }
}

} // namespace stems
} // namespace mixxx
