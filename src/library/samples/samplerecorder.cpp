#include "library/samples/samplerecorder.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QtEndian>
#include <algorithm>
#include <cmath>

#include "library/samples/bundledsamples.h"
#include "moc_samplerecorder.cpp"

#ifdef Q_OS_ANDROID
#include "engine/androidmiccapture.h"
#include "util/androidpermissions.h"
#else
#include <QAudioFormat>
#include <QAudioSource>
#include <QMediaDevices>
#endif

namespace mixxx {
namespace samples {

namespace {

constexpr int kPollMs = 30;

/// 16-bit mono WAV
bool writeWav(const QString& path, const QVector<CSAMPLE>& mono, int sampleRate) {
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly)) {
        return false;
    }
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
    const auto dataBytes = static_cast<quint32>(mono.size() * 2);
    QByteArray out;
    out.reserve(static_cast<int>(44 + dataBytes));
    out += "RIFF" + le32(36 + dataBytes) + "WAVEfmt " + le32(16) + le16(1) + le16(1) +
            le32(static_cast<quint32>(sampleRate)) +
            le32(static_cast<quint32>(sampleRate * 2)) + le16(2) + le16(16) + "data" +
            le32(dataBytes);
    for (const CSAMPLE s : mono) {
        out += le16(static_cast<quint16>(static_cast<qint16>(
                std::lround(std::clamp(s, -1.f, 1.f) * 32767.f))));
    }
    return file.write(out) == out.size();
}

/// Cut silence below -45 dB of the peak at both ends (keeping 5 ms of lead
/// in), normalize to -1 dBFS, fade the last 10 ms.
void tighten(QVector<CSAMPLE>* pTake, int sampleRate) {
    auto& t = *pTake;
    CSAMPLE peak = 0;
    for (const CSAMPLE s : std::as_const(t)) {
        peak = std::max(peak, std::fabs(s));
    }
    if (peak < 1e-4f) {
        t.clear();
        return;
    }
    const CSAMPLE gate = peak * 0.0056f;
    const auto loud = [gate](CSAMPLE s) { return std::fabs(s) > gate; };
    const auto first = std::find_if(t.cbegin(), t.cend(), loud) - t.cbegin();
    const auto last = t.crend() - std::find_if(t.crbegin(), t.crend(), loud);
    const auto start = std::max<qsizetype>(0, first - sampleRate / 200);
    t = t.mid(start, last - start);
    const CSAMPLE gain = 0.891f / peak;
    const qsizetype fade = std::min<qsizetype>(t.size(), sampleRate / 100);
    for (qsizetype i = 0; i < t.size(); ++i) {
        const qsizetype left = t.size() - i;
        t[i] *= gain * (left < fade ? static_cast<CSAMPLE>(left) / fade : 1.f);
    }
}

} // namespace

SampleRecorder::SampleRecorder(QObject* parent)
        : QObject(parent),
          m_sampleRate(48000),
          m_recording(false)
#ifndef Q_OS_ANDROID
          ,
          m_pDevice(nullptr)
#endif
{
    m_poll.setInterval(kPollMs);
    connect(&m_poll, &QTimer::timeout, this, &SampleRecorder::pull);
}

SampleRecorder::~SampleRecorder() {
    if (m_recording) {
        stop();
    }
}

bool SampleRecorder::start() {
    if (m_recording) {
        return true;
    }
    m_take.clear();
#ifdef Q_OS_ANDROID
    if (!android::requestMicrophonePermission()) {
        emit failed(tr("Microphone permission denied"));
        return false;
    }
    m_pMic = std::make_unique<AndroidMicCapture>();
    if (!m_pMic->startCapture(m_sampleRate)) {
        m_pMic.reset();
        emit failed(tr("Could not open the microphone"));
        return false;
    }
#else
    const QAudioDevice input = QMediaDevices::defaultAudioInput();
    if (input.isNull()) {
        emit failed(tr("No microphone found"));
        return false;
    }
    QAudioFormat format = input.preferredFormat();
    format.setSampleFormat(QAudioFormat::Float);
    if (!input.isFormatSupported(format)) {
        format = input.preferredFormat();
    }
    m_sampleRate = format.sampleRate();
    m_pSource = std::make_unique<QAudioSource>(input, format);
    m_pDevice = m_pSource->start();
    if (!m_pDevice) {
        m_pSource.reset();
        emit failed(tr("Could not open the microphone"));
        return false;
    }
#endif
    m_recording = true;
    m_poll.start();
    emit recordingChanged();
    emit secondsChanged();
    return true;
}

void SampleRecorder::pull() {
#ifdef Q_OS_ANDROID
    CSAMPLE buffer[4096];
    int frames;
    while ((frames = m_pMic->readFrames(buffer, 4096)) > 0) {
        m_take.append(QVector<CSAMPLE>(buffer, buffer + frames));
    }
#else
    const QByteArray bytes = m_pDevice->readAll();
    const QAudioFormat format = m_pSource->format();
    const int frameBytes = format.bytesPerFrame();
    const int channels = format.channelCount();
    const int sampleBytes = format.bytesPerSample();
    for (int i = 0; i + frameBytes <= bytes.size(); i += frameBytes) {
        float sum = 0;
        for (int c = 0; c < channels; ++c) {
            sum += format.normalizedSampleValue(bytes.constData() + i + c * sampleBytes);
        }
        m_take.append(sum / static_cast<float>(channels));
    }
#endif
    emit secondsChanged();
}

QString SampleRecorder::stop() {
    if (!m_recording) {
        return QString();
    }
    pull();
    m_poll.stop();
#ifdef Q_OS_ANDROID
    m_pMic->stopCapture();
    m_pMic.reset();
#else
    m_pSource->stop();
    m_pSource.reset();
    m_pDevice = nullptr;
#endif
    m_recording = false;
    emit recordingChanged();

    tighten(&m_take, m_sampleRate);
    if (m_take.isEmpty()) {
        emit failed(tr("Nothing was recorded"));
        return QString();
    }
    const QString dir = userSamplesDir();
    QDir().mkpath(dir);
    const QString path = dir +
            QStringLiteral("rec-%1.wav").arg(QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss")));
    if (!writeWav(path, m_take, m_sampleRate)) {
        emit failed(tr("Could not save the recording"));
        return QString();
    }
    m_take.clear();
    emit secondsChanged();
    return path;
}

} // namespace samples
} // namespace mixxx
