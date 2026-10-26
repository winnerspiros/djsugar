#include "qml/qmlmobilelibraryproxy.h"

#include <QCoreApplication>
#include <QDir>
#include <QFutureWatcher>
#include <QSqlQuery>
#include <QStandardPaths>
#include <QTimer>
#include <QtConcurrentRun>

#include "analyzer/analyzerscheduledtrack.h"
#include "library/dao/directorydao.h"
#include "library/dao/playlistdao.h"
#include "library/library.h"
#include "library/rekordbox/rekordboxandroid.h"
#include "library/rekordbox/rekordboxdrivebackup.h"
#include "library/rekordbox/rekordboxplaylistcopy.h"
#include "library/trackcollection.h"
#include "library/trackcollectionmanager.h"
#include "moc_qmlmobilelibraryproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "qml/qmllibraryproxy.h"
#include "qml/qmllibrarytracklistmodel.h"
#include "track/track.h"
#include "util/androidbackup.h"
#include "util/assert.h"
#include "util/cmdlineargs.h"
#include "util/fileinfo.h"
#include "util/sandbox.h"
#ifdef __ANDROID__
#include "util/androidpermissions.h"
#endif
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

namespace mixxx {
namespace qml {

namespace {

const ConfigKey kSidebarVisibleKey(
        QStringLiteral("[Library]"), QStringLiteral("AndroidShowSidebar"));

TrackCollectionManager* trackCollectionManager() {
    Library* pLibrary = QmlLibraryProxy::get();
    return pLibrary ? pLibrary->trackCollectionManager() : nullptr;
}

QString cleanLocalPath(const QString& rawPath) {
    QString path = rawPath.trimmed();
    if (path.startsWith(QStringLiteral("file://"))) {
        path = QUrl(path).toLocalFile();
    }
    return path;
}

} // namespace

QmlMobileLibraryProxy::QmlMobileLibraryProxy(QObject* pParent)
        : QObject(pParent) {
}

// static
QmlMobileLibraryProxy* QmlMobileLibraryProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileLibraryProxy(pQmlEngine);
}

namespace {

QVariantMap result(bool ok, const QString& message) {
    return QVariantMap{
            {QStringLiteral("ok"), ok},
            {QStringLiteral("message"), message},
    };
}

/// Backups are kept in public storage, which needs "All files access"
bool checkStorageAccess() {
#ifdef __ANDROID__
    if (!mixxx::android::hasFullExternalStorageAccess()) {
        mixxx::android::requestFullExternalStorageAccess();
        return false;
    }
#endif
    return true;
}

} // namespace

QString QmlMobileLibraryProxy::backupDirectory() const {
    return mixxx::android::backupRootDirectory();
}

QStringList QmlMobileLibraryProxy::backups() const {
    return mixxx::android::listBackups();
}

QVariantMap QmlMobileLibraryProxy::createBackup() {
    if (!checkStorageAccess()) {
        return result(false, tr("Backups need \"All files access\". Allow it, then try again."));
    }
    TrackCollectionManager* pTrackCollectionManager = trackCollectionManager();
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pTrackCollectionManager || !pConfig) {
        return result(false, tr("The library is not available."));
    }
    // Writes the current settings to mixxx.cfg first
    pConfig->save();
    QString error;
    const QString backupDir = mixxx::android::createBackup(
            CmdlineArgs::Instance().getSettingsPath(),
            pTrackCollectionManager->internalCollection()->database(),
            &error);
    if (backupDir.isEmpty()) {
        return result(false, error);
    }
    return result(true, tr("Backup saved to %1").arg(backupDir));
}

QVariantMap QmlMobileLibraryProxy::restoreBackup(const QString& name) {
    if (!checkStorageAccess()) {
        return result(false, tr("Restoring needs \"All files access\". Allow it, then try again."));
    }
    QString error;
    if (!mixxx::android::stageRestore(CmdlineArgs::Instance().getSettingsPath(), name, &error)) {
        return result(false, error);
    }
    // Gives the UI time to show the message
    QTimer::singleShot(3000, QCoreApplication::instance(), &QCoreApplication::quit);
    return result(true, tr("Mixxx closes now. Open it again to finish restoring %1.").arg(name));
}

bool QmlMobileLibraryProxy::sidebarVisible() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig ? pConfig->getValue(kSidebarVisibleKey, true) : true;
}

void QmlMobileLibraryProxy::setSidebarVisible(bool visible) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || visible == sidebarVisible()) {
        return;
    }
    pConfig->setValue(kSidebarVisibleKey, visible);
    emit sidebarVisibleChanged();
}

QVariantList QmlMobileLibraryProxy::getSourcesList() const {
    QVariantList list;
    if (!trackCollectionManager()) {
        return list;
    }
    const auto roots = trackCollectionManager()
                               ->internalCollection()
                               ->getRootDirectories();
    for (const auto& r : roots) {
        QVariantMap item;
        item[QStringLiteral("path")] = r.path;
        item[QStringLiteral("trackCount")] = r.trackCount;
        item[QStringLiteral("totalSecond")] = r.totalSecond;
        list.append(item);
    }
    return list;
}

bool QmlMobileLibraryProxy::addSourcePath(const QString& rawPath) {
    if (!trackCollectionManager() || rawPath.trimmed().isEmpty()) {
        return false;
    }
    const QString cleanPath = cleanLocalPath(rawPath);
    QDir dir(cleanPath);
    if (!dir.exists()) {
        return false;
    }
    Sandbox::createSecurityTokenForDir(dir);
    auto res = trackCollectionManager()->addDirectory(mixxx::FileInfo(cleanPath));
    emit sourcesChanged();
    return (res == DirectoryDAO::AddResult::Ok || res == DirectoryDAO::AddResult::AlreadyWatching);
}

bool QmlMobileLibraryProxy::removeSourcePath(const QString& rawPath) {
    if (!trackCollectionManager() || rawPath.trimmed().isEmpty()) {
        return false;
    }
    const QString cleanPath = cleanLocalPath(rawPath);
    auto res = trackCollectionManager()->removeDirectory(mixxx::FileInfo(cleanPath));
    emit sourcesChanged();
    return (res == DirectoryDAO::RemoveResult::Ok);
}

QStringList QmlMobileLibraryProxy::getSubDirectories(const QString& rawPath) const {
    QString path = cleanLocalPath(rawPath);
    if (path.isEmpty()) {
#if defined(Q_OS_ANDROID)
        path = QStringLiteral("/storage/emulated/0");
#else
        path = QDir::homePath();
#endif
    }
    QDir dir(path);
    if (!dir.exists() || !dir.isReadable()) {
        return {};
    }
    return dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::Readable, QDir::Name);
}

QString QmlMobileLibraryProxy::defaultMusicDirectory() const {
#if defined(Q_OS_ANDROID)
    return QStringLiteral("/storage/emulated/0/Music");
#else
    QStringList paths = QStandardPaths::standardLocations(QStandardPaths::MusicLocation);
    if (!paths.isEmpty()) {
        return paths.first();
    }
    return QDir::homePath();
#endif
}

void QmlMobileLibraryProxy::analyzeTracksMatching(
        const QString& sqlQuery, AnalyzerTrack::Options options) const {
    if (!trackCollectionManager()) {
        return;
    }
    const auto pTrackCollection = trackCollectionManager()->internalCollection();
    if (!pTrackCollection) {
        return;
    }
    QSqlQuery query(pTrackCollection->database());
    query.prepare(sqlQuery);
    if (!query.exec()) {
        return;
    }
    QList<AnalyzerScheduledTrack> tracks;
    while (query.next()) {
        TrackId id(query.value(0));
        if (id.isValid()) {
            tracks.append(AnalyzerScheduledTrack(id, options));
        }
    }
    if (!tracks.isEmpty()) {
        emit QmlLibraryProxy::get() -> analyzeTracks(tracks);
    }
}

void QmlMobileLibraryProxy::analyzeAllTracks(bool redoBeats) const {
    AnalyzerTrack::Options options;
    options.redoBeats = redoBeats;
    analyzeTracksMatching(
            QStringLiteral("SELECT id FROM library WHERE mixxx_deleted = 0"),
            options);
}

bool QmlMobileLibraryProxy::reanalyzeBeats(QmlTrackProxy* pTrackProxy) const {
    const TrackPointer pTrack = pTrackProxy ? pTrackProxy->internal() : TrackPointer();
    if (!pTrack || pTrack->isBpmLocked()) {
        return false;
    }
    // Like "Reanalyze" in the track menu of the desktop UI
    pTrack->trySetBeats(mixxx::BeatsPointer());
    emit QmlLibraryProxy::get() -> analyzeTracks({AnalyzerScheduledTrack(pTrack->getId())});
    return true;
}

void QmlMobileLibraryProxy::analyzeUnanalyzedTracks() const {
    analyzeTracksMatching(QStringLiteral(
            "SELECT id FROM library WHERE mixxx_deleted = 0 AND "
            "(bpm = 0 OR bpm IS NULL OR key IS NULL OR key = '')"));
}

QVariantList QmlMobileLibraryProxy::playlists() const {
    QVariantList result;
    VERIFY_OR_DEBUG_ASSERT(trackCollectionManager()) {
        return result;
    }
    const PlaylistDAO& playlistDao =
            trackCollectionManager()->internalCollection()->getPlaylistDAO();
    const auto playlists = playlistDao.getPlaylists(PlaylistDAO::PLHT_NOT_HIDDEN);
    for (const auto& [id, name] : playlists) {
        result.append(QVariantMap{
                {QStringLiteral("id"), id},
                {QStringLiteral("name"), name},
                {QStringLiteral("locked"), playlistDao.isPlaylistLocked(id)},
        });
    }
    return result;
}

int QmlMobileLibraryProxy::createNamedPlaylist(const QString& name) {
    VERIFY_OR_DEBUG_ASSERT(trackCollectionManager()) {
        return -1;
    }
    const QString trimmedName = name.trimmed();
    if (trimmedName.isEmpty()) {
        return -1;
    }
    PlaylistDAO& playlistDao =
            trackCollectionManager()->internalCollection()->getPlaylistDAO();
    if (playlistDao.getPlaylistIdFromName(trimmedName) != -1) {
        return -1;
    }
    return playlistDao.createPlaylist(trimmedName);
}

bool QmlMobileLibraryProxy::addTrackToPlaylist(int playlistId, const QUrl& trackUrl) {
    VERIFY_OR_DEBUG_ASSERT(trackCollectionManager()) {
        return false;
    }
    TrackCollectionManager* pTrackCollectionManager = trackCollectionManager();
    PlaylistDAO& playlistDao = pTrackCollectionManager->internalCollection()->getPlaylistDAO();
    if (!playlistDao.playlistExists(playlistId) || playlistDao.isPlaylistLocked(playlistId)) {
        return false;
    }
    bool alreadyInLibrary = false;
    const TrackPointer pTrack = pTrackCollectionManager->getOrAddTrack(
            TrackRef::fromUrl(trackUrl), &alreadyInLibrary);
    if (!pTrack || !pTrack->getId().isValid()) {
        qWarning() << "Unable to add" << trackUrl << "to playlist" << playlistId;
        return false;
    }
#ifdef Q_OS_ANDROID
    if (!alreadyInLibrary) {
        // A track of a Rekordbox drive keeps its date of the collection
        const QSqlDatabase database = pTrackCollectionManager->internalCollection()->database();
        android::setTrackDateAdded(database,
                pTrack->getId(),
                android::rekordboxDateAdded(database, pTrack->getLocation()));
    }
#endif
    return playlistDao.appendTrackToPlaylist(pTrack->getId(), playlistId);
}

int QmlMobileLibraryProxy::addAllTracksToPlaylist(int playlistId, QAbstractItemModel* pTrackList) {
    auto* pModel = qobject_cast<QmlLibraryTrackListModel*>(pTrackList);
    TrackCollectionManager* pTrackCollectionManager = trackCollectionManager();
    VERIFY_OR_DEBUG_ASSERT(pModel && pTrackCollectionManager) {
        return -1;
    }
    PlaylistDAO& playlistDao = pTrackCollectionManager->internalCollection()->getPlaylistDAO();
    if (!playlistDao.playlistExists(playlistId) || playlistDao.isPlaylistLocked(playlistId)) {
        return -1;
    }
    // The track lists load their rows on demand
    while (pModel->canFetchMore(QModelIndex())) {
        pModel->fetchMore(QModelIndex());
    }
    QList<QUrl> urls;
    urls.reserve(pModel->rowCount());
    for (int row = 0; row < pModel->rowCount(); ++row) {
        const QUrl url = pModel->getUrl(row);
        if (url.isValid()) {
            urls.append(url);
        }
    }
#ifdef Q_OS_ANDROID
    // The tracks of a Rekordbox drive that are not in the library yet keep
    // their date of the Rekordbox collection
    const QSqlDatabase database = pTrackCollectionManager->internalCollection()->database();
    QList<std::pair<QString, QDateTime>> rekordboxDates;
    for (const QUrl& url : std::as_const(urls)) {
        const QString location = url.toLocalFile();
        const QDateTime dateAdded = android::rekordboxDateAdded(database, location);
        if (dateAdded.isValid() && !android::libraryTrackId(database, location).isValid()) {
            rekordboxDates.append({location, dateAdded});
        }
    }
#endif
    // Keeps the order of the list
    const QList<TrackId> trackIds =
            pTrackCollectionManager->resolveTrackIdsFromUrls(urls, true);
#ifdef Q_OS_ANDROID
    for (const auto& [location, dateAdded] : std::as_const(rekordboxDates)) {
        android::setTrackDateAdded(database,
                android::libraryTrackId(database, location),
                dateAdded);
    }
#endif
    if (trackIds.isEmpty()) {
        return 0;
    }
    return playlistDao.appendTracksToPlaylist(trackIds, playlistId)
            ? static_cast<int>(trackIds.size())
            : -1;
}

#ifdef Q_OS_ANDROID
bool QmlMobileLibraryProxy::isRekordboxPlaylist(QAbstractItemModel* pTrackList) const {
    return android::isRekordboxPlaylist(pTrackList);
}
#endif
#ifndef Q_OS_ANDROID
bool QmlMobileLibraryProxy::isRekordboxPlaylist(QAbstractItemModel*) const {
    return false;
}
#endif
#ifdef Q_OS_ANDROID
QString
QmlMobileLibraryProxy::rekordboxPlaylistCopyDirectory(const QString& name) const {
    return android::RekordboxPlaylistCopy::targetDirectory(name);
}

#endif
#ifndef Q_OS_ANDROID
QString QmlMobileLibraryProxy::rekordboxPlaylistCopyDirectory(const QString&) const {
    return QString();
}
#endif
#ifdef Q_OS_ANDROID
QString
QmlMobileLibraryProxy::copyRekordboxPlaylist(
        QAbstractItemModel* pTrackList, const QString& name) {
    const auto tracks = android::rekordboxPlaylistTracks(pTrackList);
    if (!tracks) {
        return tr("This list is not a Rekordbox playlist");
    }
    if (!m_pPlaylistCopy) {
        TrackCollectionManager* pTrackCollectionManager = trackCollectionManager();
        VERIFY_OR_DEBUG_ASSERT(pTrackCollectionManager) {
            return tr("The library is not available");
        }
        m_pPlaylistCopy = new android::RekordboxPlaylistCopy(pTrackCollectionManager, this);
        connect(m_pPlaylistCopy,
                &android::RekordboxPlaylistCopy::progressChanged,
                this,
                &QmlMobileLibraryProxy::playlistCopyProgressChanged);
        connect(m_pPlaylistCopy,
                &android::RekordboxPlaylistCopy::finished,
                this,
                &QmlMobileLibraryProxy::playlistCopyFinished);
    }
    return m_pPlaylistCopy->start(*tracks, name);
}

void QmlMobileLibraryProxy::cancelPlaylistCopy() {
    if (m_pPlaylistCopy) {
        m_pPlaylistCopy->cancel();
    }
}

bool QmlMobileLibraryProxy::playlistCopyRunning() const {
    return m_pPlaylistCopy && m_pPlaylistCopy->isRunning();
}

int QmlMobileLibraryProxy::playlistCopyDone() const {
    return m_pPlaylistCopy ? m_pPlaylistCopy->done() : 0;
}

int QmlMobileLibraryProxy::playlistCopyTotal() const {
    return m_pPlaylistCopy ? m_pPlaylistCopy->total() : 0;
}

#endif
#ifndef Q_OS_ANDROID
QString QmlMobileLibraryProxy::copyRekordboxPlaylist(QAbstractItemModel*, const QString&) {
return tr("Rekordbox copy needs Android");
}

void QmlMobileLibraryProxy::cancelPlaylistCopy() {
}

bool QmlMobileLibraryProxy::playlistCopyRunning() const {
    return false;
}

int QmlMobileLibraryProxy::playlistCopyDone() const {
    return 0;
}

int QmlMobileLibraryProxy::playlistCopyTotal() const {
    return 0;
}
#endif
#ifdef Q_OS_ANDROID
void QmlMobileLibraryProxy::refreshRekordboxDrives() {
    if (m_rekordboxDrivesLoading) {
        return;
    }
    m_rekordboxDrivesLoading = true;
    emit rekordboxDrivesChanged();
    auto* pWatcher = new QFutureWatcher<QVariantList>(this);
    connect(pWatcher, &QFutureWatcher<QVariantList>::finished, this, [this, pWatcher]() {
        m_rekordboxDrives = pWatcher->result();
        m_rekordboxDrivesLoading = false;
        emit rekordboxDrivesChanged();
        pWatcher->deleteLater();
    });
    pWatcher->setFuture(QtConcurrent::run(&android::RekordboxDriveBackup::findDrives));
}

QString QmlMobileLibraryProxy::rekordboxDriveBackupDirectory(const QString& name) const {
    return android::RekordboxDriveBackup::targetDirectory(name);
}

QString QmlMobileLibraryProxy::backUpRekordboxDrive(const QString& path, const QString& name) {
    if (!m_pDriveBackup) {
        m_pDriveBackup = new android::RekordboxDriveBackup(this);
        connect(m_pDriveBackup,
                &android::RekordboxDriveBackup::progressChanged,
                this,
                &QmlMobileLibraryProxy::driveBackupProgressChanged);
        connect(m_pDriveBackup,
                &android::RekordboxDriveBackup::finished,
                this,
                &QmlMobileLibraryProxy::driveBackupFinished);
    }
    return m_pDriveBackup->start(path, name);
}

void QmlMobileLibraryProxy::cancelDriveBackup() {
    if (m_pDriveBackup) {
        m_pDriveBackup->cancel();
    }
}

bool QmlMobileLibraryProxy::driveBackupRunning() const {
    return m_pDriveBackup && m_pDriveBackup->isRunning();
}

int QmlMobileLibraryProxy::driveBackupFilesDone() const {
    return m_pDriveBackup ? m_pDriveBackup->filesDone() : 0;
}

int QmlMobileLibraryProxy::driveBackupFilesTotal() const {
    return m_pDriveBackup ? m_pDriveBackup->filesTotal() : 0;
}

double QmlMobileLibraryProxy::driveBackupProgress() const {
    if (!m_pDriveBackup || m_pDriveBackup->bytesTotal() <= 0) {
        return 0;
    }
    return static_cast<double>(m_pDriveBackup->bytesDone()) /
            static_cast<double>(m_pDriveBackup->bytesTotal());
}
#endif
#ifndef Q_OS_ANDROID
void QmlMobileLibraryProxy::refreshRekordboxDrives() {
}

QString QmlMobileLibraryProxy::rekordboxDriveBackupDirectory(const QString&) const {
    return QString();
}

QString QmlMobileLibraryProxy::backUpRekordboxDrive(const QString&, const QString&) {
    return tr("Rekordbox backup needs Android");
}

void QmlMobileLibraryProxy::cancelDriveBackup() {
}

bool QmlMobileLibraryProxy::driveBackupRunning() const {
    return false;
}

int QmlMobileLibraryProxy::driveBackupFilesDone() const {
    return 0;
}

int QmlMobileLibraryProxy::driveBackupFilesTotal() const {
    return 0;
}

double QmlMobileLibraryProxy::driveBackupProgress() const {
    return 0;
}
#endif
bool QmlMobileLibraryProxy::hasFullStorageAccess() const {
#ifdef __ANDROID__
    return mixxx::android::hasFullExternalStorageAccess();
#else
    return true;
#endif
}

void QmlMobileLibraryProxy::allowUsbPermissionRequests() const {
#ifdef __ANDROID_USB_DRIVE__
    mixxx::usbdrive::allowPermissionRequests();
#endif
}

void QmlMobileLibraryProxy::requestFullStorageAccess() const {
#ifdef __ANDROID__
    mixxx::android::requestFullExternalStorageAccess();
#endif
}

} // namespace qml
} // namespace mixxx
