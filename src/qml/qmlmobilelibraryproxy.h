#pragma once

#include <QAbstractItemModel>
#include <QObject>
#include <QPointer>
#include <QQmlEngine>
#include <QString>
#include <QStringList>
#include <QUrl>
#include <QVariantList>

#include "analyzer/analyzertrack.h"
#include "qml/qmltrackproxy.h"

namespace mixxx {
namespace android {
class RekordboxDriveBackup;
class RekordboxPlaylistCopy;
} // namespace android

namespace qml {

/// Library functions used by the Android UI, exposed as the
/// `Mixxx.MobileLibrary` singleton. Kept apart from QmlLibraryProxy so that
/// merging upstream Mixxx stays conflict-free.
class QmlMobileLibraryProxy : public QObject {
    Q_OBJECT
    /// Whether the sidebar (playlists, crates...) of the library is shown.
    /// Remembered across restarts.
    Q_PROPERTY(bool sidebarVisible READ sidebarVisible WRITE setSidebarVisible NOTIFY
                    sidebarVisibleChanged)
    /// Where library backups are stored, see util/androidbackup.h
    Q_PROPERTY(QString backupDirectory READ backupDirectory CONSTANT)
    /// Progress of copyRekordboxPlaylist()
    Q_PROPERTY(bool playlistCopyRunning READ playlistCopyRunning NOTIFY playlistCopyProgressChanged)
    Q_PROPERTY(int playlistCopyDone READ playlistCopyDone NOTIFY playlistCopyProgressChanged)
    Q_PROPERTY(int playlistCopyTotal READ playlistCopyTotal NOTIFY playlistCopyProgressChanged)
    /// The Rekordbox sticks found by refreshRekordboxDrives(), as a list of
    /// {path, name}
    Q_PROPERTY(QVariantList rekordboxDrives READ rekordboxDrives NOTIFY rekordboxDrivesChanged)
    Q_PROPERTY(bool rekordboxDrivesLoading READ rekordboxDrivesLoading NOTIFY
                    rekordboxDrivesChanged)
    /// Progress of backUpRekordboxDrive()
    Q_PROPERTY(bool driveBackupRunning READ driveBackupRunning NOTIFY driveBackupProgressChanged)
    Q_PROPERTY(int driveBackupFilesDone READ driveBackupFilesDone NOTIFY
                    driveBackupProgressChanged)
    Q_PROPERTY(int driveBackupFilesTotal READ driveBackupFilesTotal NOTIFY
                    driveBackupProgressChanged)
    /// 0 to 1
    Q_PROPERTY(double driveBackupProgress READ driveBackupProgress NOTIFY
                    driveBackupProgressChanged)
    QML_NAMED_ELEMENT(MobileLibrary)
    QML_SINGLETON

  public:
    explicit QmlMobileLibraryProxy(QObject* pParent = nullptr);

    static QmlMobileLibraryProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    QString backupDirectory() const;

    /// The available backups, newest first
    Q_INVOKABLE QStringList backups() const;
    /// Backs up the library database and the settings. Returns
    /// {ok, message}.
    Q_INVOKABLE QVariantMap createBackup();
    /// Prepares restoring the backup and closes Mixxx; the backup replaces
    /// the library and the settings when Mixxx starts again. Returns
    /// {ok, message}.
    Q_INVOKABLE QVariantMap restoreBackup(const QString& name);

    bool sidebarVisible() const;
    void setSidebarVisible(bool visible);

    /// The music directories as a list of {path, trackCount, totalSecond}
    Q_INVOKABLE QVariantList getSourcesList() const;
    Q_INVOKABLE bool addSourcePath(const QString& rawPath);
    Q_INVOKABLE bool removeSourcePath(const QString& rawPath);
    Q_INVOKABLE QStringList getSubDirectories(const QString& rawPath) const;
    Q_INVOKABLE QString defaultMusicDirectory() const;

    /// Analyzes all tracks again. With redoBeats, also their beatgrids,
    /// except locked ones and those imported from Rekordbox: that replaces
    /// the beatgrids corrected by hand, so the UI asks first.
    Q_INVOKABLE void analyzeAllTracks(bool redoBeats) const;
    Q_INVOKABLE void analyzeUnanalyzedTracks() const;
    /// Throws the beatgrid of the track away and analyzes it again. Returns
    /// false for a locked grid.
    Q_INVOKABLE bool reanalyzeBeats(mixxx::qml::QmlTrackProxy* pTrack) const;

    /// The visible playlists as a list of {id, name, locked}
    Q_INVOKABLE QVariantList playlists() const;
    /// Creates a playlist without asking for the name. Returns its id, or -1
    /// if the name is empty or already used.
    Q_INVOKABLE int createNamedPlaylist(const QString& name);
    /// Appends the track, adding it to the library if needed
    Q_INVOKABLE bool addTrackToPlaylist(int playlistId, const QUrl& trackUrl);
    /// Appends all tracks of a track list, in its current order, adding them
    /// to the library if needed. Returns how many were added, or -1.
    Q_INVOKABLE int addAllTracksToPlaylist(int playlistId, QAbstractItemModel* pTrackList);

    /// Whether the track list shows a Rekordbox playlist of a USB drive
    Q_INVOKABLE bool isRekordboxPlaylist(QAbstractItemModel* pTrackList) const;
    /// The folder copyRekordboxPlaylist() copies the files to
    Q_INVOKABLE QString rekordboxPlaylistCopyDirectory(const QString& name) const;
    /// Starts copying the files of the Rekordbox playlist shown by the track
    /// list into rekordboxPlaylistCopyDirectory(name), and makes a playlist
    /// `name` of the copies, see library/rekordbox/rekordboxplaylistcopy.h.
    /// Returns an error message, or an empty string if the copy started;
    /// playlistCopyFinished() is emitted at the end.
    Q_INVOKABLE QString copyRekordboxPlaylist(QAbstractItemModel* pTrackList, const QString& name);
    Q_INVOKABLE void cancelPlaylistCopy();
    bool playlistCopyRunning() const;
    int playlistCopyDone() const;
    int playlistCopyTotal() const;

    /// Looks for connected Rekordbox sticks in the background, see
    /// rekordboxDrives
    Q_INVOKABLE void refreshRekordboxDrives();
    QVariantList rekordboxDrives() const {
        return m_rekordboxDrives;
    }
    bool rekordboxDrivesLoading() const {
        return m_rekordboxDrivesLoading;
    }
    /// The folder backUpRekordboxDrive() copies a stick named `name` to
    Q_INVOKABLE QString rekordboxDriveBackupDirectory(const QString& name) const;
    /// Starts copying the Rekordbox export of the stick at `path`, see
    /// library/rekordbox/rekordboxdrivebackup.h. Returns an error message,
    /// or an empty string if the backup started; driveBackupFinished() is
    /// emitted at the end.
    Q_INVOKABLE QString backUpRekordboxDrive(const QString& path, const QString& name);
    Q_INVOKABLE void cancelDriveBackup();
    bool driveBackupRunning() const;
    int driveBackupFilesDone() const;
    int driveBackupFilesTotal() const;
    double driveBackupProgress() const;

    /// Whether the databases on USB drives (Rekordbox, Engine DJ) can be
    /// read. On Android this needs "All files access".
    Q_INVOKABLE bool hasFullStorageAccess() const;
    /// Lets the next search for USB drives ask again for the permission to
    /// read the drives that Android didn't mount, see
    /// library/usbdrive/androidusbdrive.h
    Q_INVOKABLE void allowUsbPermissionRequests() const;
    /// Opens the system settings to grant "All files access"
    Q_INVOKABLE void requestFullStorageAccess() const;

  signals:
    void sourcesChanged();
    void sidebarVisibleChanged();
    void playlistCopyProgressChanged();
    void playlistCopyFinished(const QString& message, bool error);
    void rekordboxDrivesChanged();
    void driveBackupProgressChanged();
    void driveBackupFinished(const QString& message, bool error);

  private:
    void analyzeTracksMatching(const QString& sqlQuery,
            AnalyzerTrack::Options options = AnalyzerTrack::Options()) const;

    QPointer<android::RekordboxPlaylistCopy> m_pPlaylistCopy;
    QPointer<android::RekordboxDriveBackup> m_pDriveBackup;
    QVariantList m_rekordboxDrives;
    bool m_rekordboxDrivesLoading = false;
};

} // namespace qml
} // namespace mixxx
