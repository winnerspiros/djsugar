#pragma once

#include <QFuture>
#include <QHash>
#include <QList>
#include <QObject>
#include <QString>
#include <atomic>
#include <memory>

#include "library/rekordbox/rekordboxandroid.h"
#include "track/trackid.h"

class TrackCollectionManager;

namespace mixxx {
namespace android {

/// Copies the files of a Rekordbox playlist into a local folder and makes a
/// Mixxx playlist of the copies, in the order of the Rekordbox playlist.
///
/// The files are copied on a worker thread. Each copy is then added to the
/// library on the main thread, in the order of the playlist, and gets the
/// beatgrid, cues, key, color and "date added" Rekordbox exported for it.
///
/// Copying a playlist again updates the folder and the playlist: files that
/// are already in the folder with the same size are not copied again, and
/// the Mixxx playlist is refilled. Tracks that already were in the library
/// keep their data.
class RekordboxPlaylistCopy : public QObject {
    Q_OBJECT
  public:
    RekordboxPlaylistCopy(TrackCollectionManager* pTrackCollectionManager,
            QObject* pParent = nullptr);
    ~RekordboxPlaylistCopy() override;

    /// /storage/emulated/0/Music/Mixxx/<name>, with the characters that are
    /// not allowed in a file name replaced
    static QString targetDirectory(const QString& playlistName);

    bool isRunning() const {
        return m_running;
    }
    int done() const {
        return m_done;
    }
    int total() const {
        return static_cast<int>(m_sources.size());
    }

    /// Returns an error message, or an empty string if the copy started
    QString start(const QList<RekordboxTrack>& tracks, const QString& playlistName);
    void cancel();

  signals:
    void progressChanged();
    /// The copy ended. `message` describes the result.
    void finished(const QString& message, bool error);

  private:
    enum class FileResult {
        Copied,
        Existing,
        Failed,
    };
    /// A file of the drive, copied once even if the playlist repeats it
    struct Source {
        RekordboxTrack track;
        QString target;
        TrackId trackId;
    };

    /// True if `directory` was made by a copy of the playlist `playlistId`
    static bool isCopyOfPlaylist(const QString& directory, int playlistId);
    static void markCopyOfPlaylist(const QString& directory, int playlistId);

    void copyFiles(const QList<Source>& sources);
    void onFileDone(int sourceIndex, FileResult result);
    void finish();

    TrackCollectionManager* const m_pTrackCollectionManager;
    QString m_playlistName;
    QString m_directory;
    QList<Source> m_sources;
    /// The index in m_sources of every track of the playlist, in order
    QList<int> m_playlistOrder;
    int m_done = 0;
    int m_copied = 0;
    int m_existing = 0;
    int m_failed = 0;
    bool m_running = false;
    std::atomic<bool> m_canceled = false;
    std::atomic<bool> m_outOfSpace = false;
    QFuture<void> m_future;
};

} // namespace android
} // namespace mixxx
