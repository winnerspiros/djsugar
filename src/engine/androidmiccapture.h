#pragma once

#include <QAtomicInt>
#include <QObject>
#include <QThread>
#include <QVector>

#include "util/types.h"

#ifdef Q_OS_ANDROID
#include <QJniObject>
#endif

/// Captures mic audio via Android AudioRecord (avoids BT SCO routing issues).
/// Uses VOICE_RECOGNITION source, 48 kHz PCM 16-bit mono, delivers CSAMPLE.
class AndroidMicCapture : public QThread {
    Q_OBJECT
  public:
    explicit AndroidMicCapture(QObject* parent = nullptr);
    ~AndroidMicCapture() override;

    /// Start capturing. Returns true if AudioRecord was created.
    bool startCapture(int sampleRate = 48000);

    /// Stop capturing and release AudioRecord.
    void stopCapture();

    /// Returns the number of frames captured so far.
    int framesCaptured() const {
        return m_framesCaptured.loadAcquire();
    }

    /// Read captured frames into buffer. Returns number of frames read.
    int readFrames(CSAMPLE* buffer, int maxFrames);

  signals:
    /// Emitted when AudioRecord creation fails.
    void captureFailed(const QString& reason);

  protected:
    void run() override;

  private:
    void openAudioRecord(int sampleRate);
    void closeAudioRecord();

#ifdef Q_OS_ANDROID
    QJniObject m_audioRecord;
#endif
    QVector<CSAMPLE> m_circularBuffer;
    QAtomicInt m_framesCaptured;
    QAtomicInt m_framesConsumed;
    QAtomicInt m_running;
    int m_sampleRate;
    static constexpr int kBufferSize = 48000 * 10; // 10 seconds of mono @ 48kHz
};
