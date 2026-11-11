#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QVector>
#include <memory>

#include "util/types.h"

class QAudioSource;
class QIODevice;
class AndroidMicCapture;

namespace mixxx {
namespace samples {

/// Records the microphone into a pad sample. stop() trims the silence
/// before and after the take, normalizes it and saves it as WAV in
/// userSamplesDir().
class SampleRecorder : public QObject {
    Q_OBJECT
  public:
    explicit SampleRecorder(QObject* parent = nullptr);
    ~SampleRecorder() override;

    bool start();
    /// Returns the saved file, or an empty string if nothing was recorded.
    QString stop();

    bool isRecording() const {
        return m_recording;
    }
    double seconds() const {
        return m_sampleRate > 0 ? static_cast<double>(m_take.size()) / m_sampleRate : 0.0;
    }

  signals:
    void recordingChanged();
    void secondsChanged();
    void failed(const QString& reason);

  private:
    void pull();

    QVector<CSAMPLE> m_take; // mono
    int m_sampleRate;
    bool m_recording;
    QTimer m_poll;
#ifdef Q_OS_ANDROID
    std::unique_ptr<AndroidMicCapture> m_pMic;
#else
    std::unique_ptr<QAudioSource> m_pSource;
    QIODevice* m_pDevice;
#endif
};

} // namespace samples
} // namespace mixxx
