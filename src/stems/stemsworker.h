#pragma once

#include <QObject>
#include <QString>
#include <atomic>

#include "stems/stemseparator.h"

namespace mixxx {
namespace stems {

/// Separates tracks into stem files, one at a time, in its own thread (see
/// StemsManager)
class StemsWorker : public QObject {
    Q_OBJECT
  public:
    enum class Stage {
        Loading,
        Decoding,
        Separating,
        Saving,
    };
    Q_ENUM(Stage)

    struct Job {
        QString location;
        Model model = Model::Spleeter;
        /// AAC bitrate of each stream in kbps
        int bitrateKbps = 128;
        int threads = 2;
    };

    /// Stops the current job as soon as possible. Thread-safe.
    void cancel() {
        m_canceled = true;
    }
    /// While set, the current job waits, e.g. while a deck plays.
    /// Thread-safe.
    void setHold(bool hold) {
        m_hold = hold;
    }

  public slots:
    void run(const QString& location, int model, int bitrateKbps, int threads);
    /// Separates 20 s of generated audio and reports how fast it was
    void benchmark(int model, int threads);

  signals:
    void progress(const QString& location, mixxx::stems::StemsWorker::Stage stage, double fraction);
    /// stemFile is empty on error
    void finished(const QString& location, const QString& stemFile, const QString& error);
    /// realtimeFactor: seconds of audio separated per second, 0 on error
    void benchmarkFinished(int model, double realtimeFactor, const QString& error);

  private:
    QString separate(const Job& job, QString* pStemFile);
    /// Waits while held. Returns false if canceled.
    bool waitWhileHeld();

    std::atomic<bool> m_canceled{false};
    std::atomic<bool> m_hold{false};
};

} // namespace stems
} // namespace mixxx
