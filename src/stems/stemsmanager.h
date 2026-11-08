#pragma once

#include <QElapsedTimer>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QThread>
#include <QTimer>

#include "preferences/usersettings.h"
#include "stems/stemmodels.h"
#include "stems/stemsworker.h"

namespace mixxx {
namespace stems {

/// The queue of tracks to separate into stems. The tracks are separated one
/// at a time in a background thread, and the queue is kept across restarts.
/// Lives in the main thread.
class StemsManager : public QObject {
    Q_OBJECT
  public:
    enum class JobState {
        Queued,
        Running,
        Done,
        Failed,
    };

    struct Job {
        QString location;
        Model model = Model::Spleeter;
        JobState state = JobState::Queued;
        /// An error code of StemsWorker, e.g. "write"
        QString error;
    };

    /// Why the queue waits
    enum class Hold {
        None,
        /// Paused by the user
        Paused,
        /// A deck plays and the user chose to wait
        Playing,
        /// Not charging and the user chose to separate only while charging
        NotCharging,
    };

    static StemsManager* instance(UserSettingsPointer pConfig);

    /// Adds tracks to the queue. Skips missing files, files already in the
    /// queue and, unless replace is set, tracks that have stems. Returns the
    /// number of tracks added.
    int enqueue(const QStringList& locations, Model model, bool replace);
    /// Removes a track from the queue, stopping it if it is being separated
    void cancel(const QString& location);
    void cancelAll();
    void removeFinished();
    void setPaused(bool paused);

    const QList<Job>& jobs() const {
        return m_jobs;
    }
    Hold hold() const {
        return m_hold;
    }
    StemsWorker::Stage stage() const {
        return m_stage;
    }
    double progress() const {
        return m_progress;
    }
    /// Seconds left of the current job, -1 if unknown
    int secondsLeft() const;

    /// Deletes the stem files of tracks. Returns how many were deleted.
    int deleteStems(const QStringList& locations);
    /// Deletes the stem files of all tracks the app separated that are in
    /// its own storage. The ones next to the tracks are deleted per track.
    int deleteStoredStems();
    /// Size in bytes of the stem files in the app's storage
    qint64 storedStemsSize() const;

    StemModels* models() {
        return &m_models;
    }

    /// Measures how fast the device separates with a model, see
    /// benchmarkFinished()
    void benchmark(Model model);
    bool isBenchmarking() const {
        return m_benchmarking;
    }
    /// The measured speed: seconds of audio per second, 0 if not measured
    double measuredSpeed(Model model) const;

    UserSettingsPointer config() const {
        return m_pConfig;
    }

  signals:
    void jobsChanged();
    void progressChanged();
    void holdChanged();
    /// A track was separated (stemFile set) or failed (error set)
    void jobFinished(const QString& location, const QString& stemFile, const QString& error);
    void benchmarkFinished(mixxx::stems::Model model, double speed, const QString& error);

  private:
    explicit StemsManager(UserSettingsPointer pConfig, QObject* pParent);
    ~StemsManager() override;

    void startNextJob();
    void slotProgress(const QString& location, StemsWorker::Stage stage, double fraction);
    void slotFinished(const QString& location, const QString& stemFile, const QString& error);
    void updateHold();
    bool isAnyDeckPlaying() const;
    int threadsForJob() const;
    void saveQueue();
    void loadQueue();
    void updateService();

    UserSettingsPointer m_pConfig;
    StemModels m_models;
    QThread m_thread;
    StemsWorker* m_pWorker;
    QList<Job> m_jobs;
    QString m_runningLocation;
    Hold m_hold;
    bool m_userPaused;
    StemsWorker::Stage m_stage;
    double m_progress;
    QElapsedTimer m_jobTimer;
    QTimer m_holdTimer;
    bool m_benchmarking;
    QString m_serviceText;
};

} // namespace stems
} // namespace mixxx
