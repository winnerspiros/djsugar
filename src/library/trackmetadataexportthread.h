#pragma once

#include <QHash>
#include <deque>
#include <mutex>
#include <optional>

#include "track/track_decl.h"
#include "util/db/dbconnectionpool.h"
#include "util/workerthread.h"

/// Serialises track metadata export on a dedicated worker thread (avoids
/// blocking the GUI on slow/network storage). Each job clones metadata
/// from the evicted track, writes tags, updates DB timestamp, then
/// returns the temp track via trackExported() signal.
class TrackMetadataExportThread : public WorkerThread {
    Q_OBJECT

  public:
    explicit TrackMetadataExportThread(
            mixxx::DbConnectionPoolPtr pDbConnectionPool);
    ~TrackMetadataExportThread() override = default;

    /// Enqueue a snapshot of the track's metadata for deferred export. Thread-safe.
    void enqueueExport(
            const Track& track,
            const SyncTrackMetadataParams& syncParams);

  signals:
    /// Emitted after a job is processed; temp track destroyed on the receiving thread.
    void trackExported(
            TrackPointer pTrack,
            ExportTrackMetadataResult result);

  private:
    struct Job {
        TrackPointer pTrack;
        SyncTrackMetadataParams syncParams;
    };

    void doRun() override;
    TryFetchWorkItemsResult tryFetchWorkItems() override;
    std::optional<Job> tryPopJob();
    void processJob(Job&& job);
    int pendingJobs();
    void drainQueueInParallel();

    const mixxx::DbConnectionPoolPtr m_pDbConnectionPool;

    // std::deque element pointers stay valid when modified only at ends.
    std::deque<Job> m_queue;
    QHash<QString, Job*> m_pendingByLocation;
    std::mutex m_queueMutex;
    std::optional<Job> m_currentJob;
};
