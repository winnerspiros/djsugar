#include "library/samples/sampleeditor.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QPointer>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QThreadPool>
#include <QtEndian>
#include <algorithm>
#include <cmath>

#include "library/samples/bundledsamples.h"
#include "moc_sampleeditor.cpp"
#include "sources/soundsourceproxy.h"
#include "track/track.h"
#include "util/samplebuffer.h"

namespace mixxx {
namespace samples {

namespace {

constexpr int kOverviewPoints = 600;
constexpr SINT kChunkFrames = 16384;
constexpr double kFadeSeconds = 0.004;

AudioSourcePointer openSource(const QString& path) {
    TrackPointer pTrack = Track::newTemporary(path);
    SoundSourceProxy proxy(pTrack);
    return proxy.openAudioSource(
            AudioSource::OpenParams(audio::ChannelCount::stereo(), audio::SampleRate()));
}

/// The file name a sound is known by: content:// URLs may carry an opaque
/// id ("msf:1234") as file name, the display name is the better guess
QString displayName(const QUrl& url, const QFile& file) {
    const QString fromFile = QFileInfo(file.fileName()).fileName();
    const QString fromUrl = QFileInfo(url.fileName()).fileName();
    if (!isSupportedSample(fromFile) && isSupportedSample(fromUrl)) {
        return fromUrl;
    }
    return fromFile.isEmpty() ? fromUrl : fromFile;
}

QByteArray wavHeader(quint32 frames, int sampleRate) {
    const auto le32 = [](quint32 v) {
        QByteArray b(4, 0);
        qToLittleEndian(v, b.data());
        return b;
    };
    const auto le16 = [](quint16 v) {
        QByteArray b(2, 0);
        qToLittleEndian(v, b.data());
        return b;
    };
    const quint32 dataBytes = frames * 4;
    return "RIFF" + le32(36 + dataBytes) + "WAVEfmt " + le32(16) + le16(1) + le16(2) +
            le32(static_cast<quint32>(sampleRate)) +
            le32(static_cast<quint32>(sampleRate * 4)) + le16(4) + le16(16) + "data" +
            le32(dataBytes);
}

} // namespace

SampleEditor::SampleEditor(QObject* pParent)
        : QObject(pParent),
          m_generation(0) {
}

// static
QString SampleEditor::workDir() {
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation) +
            QStringLiteral("/sample-edit/");
}

void SampleEditor::open(const QUrl& url) {
    const QString source = url.isLocalFile() ? url.toLocalFile() : url.toString();
    QFile in(source);
    if (!in.open(QIODevice::ReadOnly)) {
        qWarning() << "Cannot open sample" << source;
        emit failed(tr("This file can't be opened"));
        return;
    }
    const QString name = displayName(url, in);
    // Without a known extension, a container suffix sends the file to
    // FFmpeg, which detects the real format from the content
    QString suffix = QFileInfo(name).suffix();
    if (!isSupportedSample(name)) {
        for (const auto& fallback : {"mka", "webm", "m4a"}) {
            suffix = QString::fromLatin1(fallback);
            if (isSupportedSample(QStringLiteral("x.") + suffix)) {
                break;
            }
        }
    }
    QDir().mkpath(workDir());
    const int generation = ++m_generation;
    const QString work = workDir() + QStringLiteral("edit-%1.%2").arg(generation).arg(suffix);
    QFile::remove(work);
    QFile out(work);
    if (!out.open(QIODevice::WriteOnly)) {
        emit failed(tr("This file can't be opened"));
        return;
    }
    while (!in.atEnd()) {
        if (out.write(in.read(1 << 20)) < 0) {
            out.remove();
            emit failed(tr("Not enough space to open this file"));
            return;
        }
    }
    out.close();
    // Drop the work files of earlier edits
    const auto stale = QDir(workDir()).entryInfoList(QDir::Files);
    for (const QFileInfo& fi : stale) {
        if (fi.absoluteFilePath() != QFileInfo(work).absoluteFilePath()) {
            QFile::remove(fi.absoluteFilePath());
        }
    }
    m_workFile = work;

    QPointer<SampleEditor> pThis(this);
    QThreadPool::globalInstance()->start([pThis, work, name, generation]() {
        const auto pSource = openSource(work);
        QVariantMap sound;
        if (pSource) {
            const IndexRange range = pSource->frameIndexRange();
            const double sampleRate = pSource->getSignalInfo().getSampleRate();
            const SINT frames = range.length();
            QVariantList peaks;
            peaks.reserve(kOverviewPoints);
            std::vector<float> pointPeaks(kOverviewPoints, 0.f);
            SampleBuffer buffer(2 * kChunkFrames);
            for (SINT start = range.start(); start < range.end(); start += kChunkFrames) {
                const auto chunk = IndexRange::forward(
                        start, std::min(kChunkFrames, range.end() - start));
                const auto readable = pSource->readSampleFrames(
                        WritableSampleFrames(chunk, SampleBuffer::WritableSlice(buffer)));
                const CSAMPLE* pData = readable.readableData();
                const SINT first = readable.frameIndexRange().start() - range.start();
                for (SINT i = 0; i < readable.readableLength() / 2; ++i) {
                    const auto point = static_cast<int>(
                            (first + i) * kOverviewPoints / std::max<SINT>(1, frames));
                    const float level = std::max(
                            std::fabs(pData[2 * i]), std::fabs(pData[2 * i + 1]));
                    float& peak = pointPeaks[std::clamp(point, 0, kOverviewPoints - 1)];
                    peak = std::max(peak, level);
                }
            }
            const float loudest = std::max(1e-4f,
                    *std::max_element(pointPeaks.cbegin(), pointPeaks.cend()));
            for (const float peak : pointPeaks) {
                peaks.append(peak / loudest);
            }
            sound = QVariantMap{
                    {QStringLiteral("path"), work},
                    {QStringLiteral("name"), QFileInfo(name).completeBaseName()},
                    {QStringLiteral("duration"), frames / std::max(1.0, sampleRate)},
                    {QStringLiteral("peaks"), peaks},
            };
        }
        QMetaObject::invokeMethod(
                pThis,
                [pThis, sound, generation]() {
                    if (!pThis || generation != pThis->m_generation) {
                        return; // a newer sound was opened meanwhile
                    }
                    if (sound.isEmpty()) {
                        emit pThis->failed(tr("This file can't be used as a sample"));
                    } else {
                        emit pThis->opened(sound);
                    }
                },
                Qt::QueuedConnection);
    });
}

QString SampleEditor::save(double startSeconds, double endSeconds, const QString& name) const {
    const auto pSource = openSource(m_workFile);
    if (!pSource) {
        return QString();
    }
    const IndexRange range = pSource->frameIndexRange();
    const int sampleRate = static_cast<int>(pSource->getSignalInfo().getSampleRate());
    const SINT first = std::clamp(range.start() + static_cast<SINT>(startSeconds * sampleRate),
            range.start(),
            range.end());
    const SINT last = std::clamp(range.start() + static_cast<SINT>(endSeconds * sampleRate),
            first,
            range.end());
    const SINT frames = last - first;
    if (frames <= 0) {
        return QString();
    }

    QString base = name.trimmed();
    static const QRegularExpression kUnsafe(QStringLiteral("[\\\\/:*?\"<>|]"));
    base.replace(kUnsafe, QStringLiteral("_"));
    if (base.isEmpty()) {
        base = QStringLiteral("sample");
    }
    const QString dir = userSamplesDir();
    QDir().mkpath(dir);
    QString dest = dir + base + QStringLiteral(".wav");
    for (int n = 2; QFile::exists(dest); ++n) {
        dest = dir + QStringLiteral("%1-%2.wav").arg(base).arg(n);
    }
    QFile file(dest);
    if (!file.open(QIODevice::WriteOnly)) {
        return QString();
    }
    file.write(wavHeader(static_cast<quint32>(frames), sampleRate));

    // Short fades at both ends, so a cut in the middle of a sound never
    // clicks
    const SINT fadeFrames = std::max<SINT>(1,
            std::min<SINT>(frames / 4, static_cast<SINT>(kFadeSeconds * sampleRate)));
    SampleBuffer buffer(2 * kChunkFrames);
    QByteArray pcm;
    for (SINT start = first; start < last; start += kChunkFrames) {
        const auto chunk = IndexRange::forward(start, std::min(kChunkFrames, last - start));
        const auto readable = pSource->readSampleFrames(
                WritableSampleFrames(chunk, SampleBuffer::WritableSlice(buffer)));
        const CSAMPLE* pData = readable.readableData();
        const SINT offset = readable.frameIndexRange().start() - first;
        const SINT count = readable.readableLength() / 2;
        pcm.resize(static_cast<qsizetype>(count * 4));
        for (SINT i = 0; i < count; ++i) {
            const SINT position = offset + i;
            const float gain = std::min(1.f,
                    static_cast<float>(std::min(position, frames - 1 - position)) /
                            static_cast<float>(fadeFrames));
            for (int channel = 0; channel < 2; ++channel) {
                const float value = std::clamp(pData[2 * i + channel] * gain, -1.f, 1.f);
                qToLittleEndian(static_cast<qint16>(std::lround(value * 32767.f)),
                        pcm.data() + 4 * i + 2 * channel);
            }
        }
        if (file.write(pcm) != pcm.size()) {
            file.remove();
            return QString();
        }
    }
    return dest;
}

} // namespace samples
} // namespace mixxx
