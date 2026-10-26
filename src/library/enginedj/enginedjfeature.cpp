#include "library/enginedj/enginedjfeature.h"

#include <QDir>
#include <QHash>
#include <QSet>
#include <QSqlDatabase>
#include <QSqlError>
#include <QSqlQuery>
#include <QStorageInfo>
#include <QThread>
#include <QtConcurrentRun>
#include <djinterop/djinterop.hpp>
#include <functional>
#include <sstream>

#include "library/dao/playlistdao.h"
#include "library/dao/trackschema.h"
#include "library/library.h"
#include "library/queryutil.h"
#include "library/trackcollection.h"
#include "library/trackcollectionmanager.h"
#include "library/treeitem.h"
#include "moc_enginedjfeature.cpp"
#include "util/assert.h"
#include "util/db/dbconnectionpooled.h"
#include "util/db/dbconnectionpooler.h"
#include "util/removablestorage.h"

#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

namespace {

const QString kLibraryTable = QStringLiteral("enginedj_library");
const QString kPlaylistsTable = QStringLiteral("enginedj_playlists");
const QString kPlaylistTracksTable = QStringLiteral("enginedj_playlist_tracks");

/// Engine DJ keeps its database in this folder at the root of the drive
const QString kEngineLibraryFolder = QStringLiteral("Engine Library");

// The data of the sidebar items: {path or playlist name, kind}
const QString kDeviceItem = QStringLiteral("::enginedj_device::");
const QString kPlaylistItem = QStringLiteral("::enginedj_playlist::");

bool execQuery(QSqlQuery& query) {
    if (!query.exec()) {
        LOG_FAILED_QUERY(query);
        return false;
    }
    return true;
}

void dropTables(const QSqlDatabase& database) {
    for (const auto& table : {kPlaylistTracksTable, kPlaylistsTable, kLibraryTable}) {
        QSqlQuery query(database);
        query.prepare(QStringLiteral("DROP TABLE IF EXISTS ") + table);
        execQuery(query);
    }
}

void createTables(const QSqlDatabase& database) {
    QSqlQuery library(database);
    library.prepare(QStringLiteral(
            "CREATE TABLE IF NOT EXISTS %1 ("
            "    id INTEGER PRIMARY KEY AUTOINCREMENT,"
            "    artist TEXT,"
            "    title TEXT,"
            "    album TEXT,"
            "    year INTEGER,"
            "    genre TEXT,"
            "    tracknumber TEXT,"
            "    location TEXT,"
            "    comment TEXT,"
            "    duration INTEGER,"
            "    bitrate TEXT,"
            "    bpm FLOAT,"
            "    key TEXT,"
            "    rating INTEGER,"
            "    device TEXT,"
            "    color INTEGER"
            ")")
                    .arg(kLibraryTable));
    execQuery(library);

    QSqlQuery playlists(database);
    playlists.prepare(QStringLiteral(
            "CREATE TABLE IF NOT EXISTS %1 ("
            "    id INTEGER PRIMARY KEY,"
            "    name TEXT UNIQUE,"
            "    device TEXT"
            ")")
                    .arg(kPlaylistsTable));
    execQuery(playlists);

    QSqlQuery playlistTracks(database);
    playlistTracks.prepare(QStringLiteral(
            "CREATE TABLE IF NOT EXISTS %1 ("
            "    id INTEGER PRIMARY KEY AUTOINCREMENT,"
            "    playlist_id INTEGER REFERENCES %2(id),"
            "    track_id INTEGER REFERENCES %3(id),"
            "    position INTEGER"
            ")")
                    .arg(kPlaylistTracksTable, kPlaylistsTable, kLibraryTable));
    execQuery(playlistTracks);
}

/// Removes what was read from a device before
void clearDevice(const QSqlDatabase& database, const QString& devicePath) {
    QSqlQuery playlistTracks(database);
    playlistTracks.prepare(QStringLiteral(
            "DELETE FROM %1 WHERE playlist_id IN (SELECT id FROM %2 WHERE device=:device)")
                    .arg(kPlaylistTracksTable, kPlaylistsTable));
    playlistTracks.bindValue(QStringLiteral(":device"), devicePath);
    execQuery(playlistTracks);
    for (const auto& table : {kPlaylistsTable, kLibraryTable}) {
        QSqlQuery query(database);
        query.prepare(QStringLiteral("DELETE FROM %1 WHERE device=:device").arg(table));
        query.bindValue(QStringLiteral(":device"), devicePath);
        execQuery(query);
    }
}

QString toQString(const std::optional<std::string>& value) {
    return value ? QString::fromStdString(*value) : QString();
}

QList<EngineDJFeature::Device> findDevices() {
    QList<EngineDJFeature::Device> devices;
    const QFileInfoList roots = mixxx::removableStorageRoots();
    for (const QFileInfo& root : roots) {
#ifdef __ANDROID_USB_DRIVE__
        // Copies the database of drives that Mixxx reads itself
        mixxx::usbdrive::prepareFiles(root.filePath(),
                {kEngineLibraryFolder + QStringLiteral("/Database2/*.db"),
                        kEngineLibraryFolder + QStringLiteral("/*.db")});
#endif
        const QString libraryDir = root.filePath() + QChar('/') + kEngineLibraryFolder;
        if (!QDir(libraryDir).exists()) {
            continue;
        }
        // Engine DJ 2.x and later, read with SQL
        bool exists = QFileInfo::exists(libraryDir + QStringLiteral("/Database2/m.db"));
        if (!exists) {
            try {
                exists = djinterop::engine::database_exists(libraryDir.toStdString());
            } catch (const std::exception& e) {
                qWarning() << "Unable to check the Engine Library at" << libraryDir << e.what();
            }
        }
        if (!exists) {
            continue;
        }
        QString label = QStorageInfo(root.filePath()).displayName();
        if (label.isEmpty() || label.startsWith(QChar('/'))) {
            label = root.fileName();
        }
        if (root.filePath() == QLatin1String("/storage/emulated/0")) {
            label = QObject::tr("Internal storage");
        }
        devices.append({root.filePath(), label});
    }
    return devices;
}

/// A track of an Engine Library, read with libdjinterop or SQL
struct TrackInfo {
    int64_t id = 0;
    QString relativePath;
    QString title;
    QString artist;
    QString album;
    QString genre;
    QString comment;
    QString key;
    QVariant year;
    QVariant trackNumber;
    QVariant bitrate;
    QVariant durationSeconds;
    QVariant bpm;
    /// 0 to 100
    QVariant rating;
};

QString keyName(int engineKey) {
    // Engine DJ numbers the keys like libdjinterop's musical_key
    if (engineKey < 0 || engineKey > 23) {
        return {};
    }
    std::ostringstream stream;
    stream << static_cast<djinterop::musical_key>(engineKey);
    return QString::fromStdString(stream.str());
}

TrackInfo trackInfo(const djinterop::track& track) {
    TrackInfo info;
    info.id = track.id();
    info.relativePath = QString::fromStdString(track.relative_path());
    info.title = toQString(track.title());
    info.artist = toQString(track.artist());
    info.album = toQString(track.album());
    info.genre = toQString(track.genre());
    info.comment = toQString(track.comment());
    if (const auto key = track.key()) {
        info.key = keyName(static_cast<int>(*key));
    }
    if (const auto year = track.year()) {
        info.year = *year;
    }
    if (const auto trackNumber = track.track_number()) {
        info.trackNumber = *trackNumber;
    }
    if (const auto bitrate = track.bitrate()) {
        info.bitrate = *bitrate;
    }
    if (const auto duration = track.duration()) {
        info.durationSeconds = static_cast<int>(duration->count() / 1000);
    }
    if (const auto bpm = track.bpm()) {
        info.bpm = *bpm;
    }
    if (const auto rating = track.rating()) {
        info.rating = *rating;
    }
    return info;
}

class DeviceParser {
  public:
    DeviceParser(const QSqlDatabase& database, const QString& devicePath)
            : m_database(database),
              m_devicePath(devicePath),
              m_libraryDir(devicePath + QChar('/') + kEngineLibraryFolder),
              m_insertTrack(database),
              m_insertPlaylistTrack(database) {
        m_insertTrack.prepare(QStringLiteral(
                "INSERT INTO %1 (artist, title, album, year, genre, tracknumber,"
                " location, comment, duration, bitrate, bpm, key, rating, device)"
                " VALUES (:artist, :title, :album, :year, :genre, :tracknumber,"
                " :location, :comment, :duration, :bitrate, :bpm, :key, :rating, :device)")
                        .arg(kLibraryTable));
        m_insertPlaylistTrack.prepare(QStringLiteral(
                "INSERT INTO %1 (playlist_id, track_id, position)"
                " VALUES (:playlist_id, :track_id, :position)")
                        .arg(kPlaylistTracksTable));
    }

    void addTrack(const TrackInfo& track) {
        if (track.relativePath.isEmpty()) {
            // e.g. streaming tracks
            return;
        }
        // The path is relative to the Engine Library folder
        const QString location = QDir::cleanPath(m_libraryDir + QChar('/') + track.relativePath);
        const QString title = track.title.isEmpty()
                ? QFileInfo(location).completeBaseName()
                : track.title;
        m_insertTrack.bindValue(QStringLiteral(":artist"), track.artist);
        m_insertTrack.bindValue(QStringLiteral(":title"), title);
        m_insertTrack.bindValue(QStringLiteral(":album"), track.album);
        m_insertTrack.bindValue(QStringLiteral(":year"), track.year);
        m_insertTrack.bindValue(QStringLiteral(":genre"), track.genre);
        m_insertTrack.bindValue(QStringLiteral(":tracknumber"),
                track.trackNumber.isValid() ? track.trackNumber.toString() : QString());
        m_insertTrack.bindValue(QStringLiteral(":location"), location);
        m_insertTrack.bindValue(QStringLiteral(":comment"), track.comment);
        m_insertTrack.bindValue(QStringLiteral(":duration"), track.durationSeconds);
        m_insertTrack.bindValue(QStringLiteral(":bitrate"),
                track.bitrate.isValid() ? track.bitrate.toString() : QString());
        m_insertTrack.bindValue(QStringLiteral(":bpm"), track.bpm);
        m_insertTrack.bindValue(QStringLiteral(":key"), track.key);
        // Engine DJ rates from 0 to 100, Mixxx from 0 to 5
        m_insertTrack.bindValue(QStringLiteral(":rating"),
                track.rating.isValid() ? QVariant(track.rating.toInt() / 20) : QVariant());
        m_insertTrack.bindValue(QStringLiteral(":device"), m_devicePath);
        if (execQuery(m_insertTrack)) {
            m_trackRows.insert(track.id, m_insertTrack.lastInsertId().toInt());
        }
    }

    /// Adds a playlist with the given tracks and returns its id
    int addPlaylist(const QString& name, const QList<int64_t>& trackIds) {
        QSqlQuery insertPlaylist(m_database);
        insertPlaylist.prepare(
                QStringLiteral("INSERT INTO %1 (name, device) VALUES (:name, :device)")
                        .arg(kPlaylistsTable));
        insertPlaylist.bindValue(QStringLiteral(":name"), name);
        insertPlaylist.bindValue(QStringLiteral(":device"), m_devicePath);
        if (!execQuery(insertPlaylist)) {
            return kInvalidPlaylistId;
        }
        const int playlistId = insertPlaylist.lastInsertId().toInt();
        int position = 0;
        for (const int64_t trackId : trackIds) {
            const auto row = m_trackRows.constFind(trackId);
            if (row == m_trackRows.constEnd()) {
                continue;
            }
            m_insertPlaylistTrack.bindValue(QStringLiteral(":playlist_id"), playlistId);
            m_insertPlaylistTrack.bindValue(QStringLiteral(":track_id"), row.value());
            m_insertPlaylistTrack.bindValue(QStringLiteral(":position"), ++position);
            execQuery(m_insertPlaylistTrack);
        }
        return playlistId;
    }

    /// Playlists or crates read with libdjinterop
    template<typename T>
    QList<EngineDJFeature::PlaylistNode> addTree(
            const std::vector<T>& items, const QString& parentName) {
        QList<EngineDJFeature::PlaylistNode> nodes;
        QList<int64_t> trackIds;
        for (const auto& item : items) {
            EngineDJFeature::PlaylistNode node;
            node.label = QString::fromStdString(item.name());
            node.name = parentName + QChar('/') + node.label;
            trackIds.clear();
            for (const auto& track : item.tracks()) {
                trackIds.append(track.id());
            }
            addPlaylist(node.name, trackIds);
            node.children = addTree(item.children(), node.name);
            nodes.append(node);
        }
        return nodes;
    }

    int trackCount() const {
        return m_trackRows.size();
    }

  private:
    QSqlDatabase m_database;
    const QString m_devicePath;
    const QString m_libraryDir;
    QSqlQuery m_insertTrack;
    QSqlQuery m_insertPlaylistTrack;
    QHash<int64_t, int> m_trackRows;
};

/// Reads an Engine DJ 2.x or later library (Database2/m.db) with SQL.
/// libdjinterop only knows the schemas that existed when it was released,
/// while the tables used here haven't changed since Engine DJ 2.0.
class SqlEngineLibrary {
  public:
    explicit SqlEngineLibrary(const QString& databasePath)
            : m_connectionName(QStringLiteral("enginedj-%1")
                              .arg(reinterpret_cast<quintptr>(
                                      QThread::currentThread()))) {
        QSqlDatabase database = QSqlDatabase::addDatabase(
                QStringLiteral("QSQLITE"), m_connectionName);
        database.setDatabaseName(databasePath);
        database.setConnectOptions(QStringLiteral("QSQLITE_OPEN_READONLY"));
        if (!database.open()) {
            m_error = database.lastError().text();
        }
    }
    ~SqlEngineLibrary() {
        QSqlDatabase::database(m_connectionName, false).close();
        QSqlDatabase::removeDatabase(m_connectionName);
    }

    const QString& error() const {
        return m_error;
    }

    bool read(DeviceParser* pParser,
            const QString& devicePath,
            QList<EngineDJFeature::PlaylistNode>* pPlaylists) {
        QSqlDatabase database = QSqlDatabase::database(m_connectionName, false);
        const QStringList trackColumns = columns(database, QStringLiteral("Track"));
        if (!trackColumns.contains(QLatin1String("path"))) {
            m_error = QStringLiteral("No Track table");
            return false;
        }
        auto column = [&trackColumns](const char* name) {
            return trackColumns.contains(QLatin1String(name)) ? QString::fromLatin1(name)
                                                              : QStringLiteral("NULL");
        };
        QSqlQuery tracks(database);
        tracks.setForwardOnly(true);
        if (!tracks.exec(QStringLiteral(
                    "SELECT id, path, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12,"
                    " %13, %14 FROM Track")
                            .arg(column("title"),
                                    column("artist"),
                                    column("album"),
                                    column("genre"),
                                    column("comment"),
                                    column("key"),
                                    column("year"),
                                    column("bitrate"),
                                    column("length"),
                                    column("bpmAnalyzed"),
                                    column("bpm"),
                                    column("rating"),
                                    column("originDatabaseUuid"),
                                    column("originTrackId")))) {
            m_error = tracks.lastError().text();
            return false;
        }
        QList<int64_t> allTracks;
        while (tracks.next()) {
            TrackInfo info;
            info.id = tracks.value(0).toLongLong();
            info.relativePath = tracks.value(1).toString();
            info.title = tracks.value(2).toString();
            info.artist = tracks.value(3).toString();
            info.album = tracks.value(4).toString();
            info.genre = tracks.value(5).toString();
            info.comment = tracks.value(6).toString();
            if (!tracks.value(7).isNull()) {
                info.key = keyName(tracks.value(7).toInt());
            }
            if (tracks.value(8).toInt() > 0) {
                info.year = tracks.value(8).toInt();
            }
            if (tracks.value(9).toInt() > 0) {
                info.bitrate = tracks.value(9).toInt();
            }
            if (tracks.value(10).toInt() > 0) {
                info.durationSeconds = tracks.value(10).toInt();
            }
            const double bpm = tracks.value(11).toDouble() > 0 ? tracks.value(11).toDouble()
                                                               : tracks.value(12).toDouble();
            if (bpm > 0) {
                info.bpm = bpm;
            }
            if (!tracks.value(13).isNull()) {
                info.rating = tracks.value(13).toInt();
            }
            // Tracks exported from another library remember their origin
            const QString originDatabase = tracks.value(14).toString();
            if (!originDatabase.isEmpty()) {
                m_tracksByOrigin.insert(originKey(originDatabase, tracks.value(15).toLongLong()),
                        info.id);
            }
            pParser->addTrack(info);
            allTracks.append(info.id);
        }
        // All tracks of the device, shown when the device is opened
        pParser->addPlaylist(devicePath, allTracks);
        *pPlaylists = readPlaylists(database, pParser, devicePath);
        return true;
    }

  private:
    static QString originKey(const QString& databaseUuid, int64_t trackId) {
        return databaseUuid.toLower() + QChar(':') + QString::number(trackId);
    }

    static QStringList columns(const QSqlDatabase& database, const QString& table) {
        QStringList names;
        QSqlQuery query(database);
        if (query.exec(QStringLiteral("PRAGMA table_info(%1)").arg(table))) {
            while (query.next()) {
                names.append(query.value(1).toString());
            }
        }
        return names;
    }

    /// Orders items linked by their "next" ids; the first one is the item
    /// that no other item points to. Broken chains fall back to the id order.
    static QList<int64_t> linkedOrder(
            const QList<int64_t>& ids, const QHash<int64_t, int64_t>& next) {
        QSet<int64_t> pointedTo;
        for (const int64_t id : ids) {
            pointedTo.insert(next.value(id));
        }
        QList<int64_t> ordered;
        QSet<int64_t> visited;
        for (const int64_t id : ids) {
            if (pointedTo.contains(id)) {
                continue;
            }
            for (int64_t current = id; current != 0 && ids.contains(current) &&
                    !visited.contains(current);
                    current = next.value(current)) {
                visited.insert(current);
                ordered.append(current);
            }
        }
        for (const int64_t id : ids) {
            if (!visited.contains(id)) {
                ordered.append(id);
            }
        }
        return ordered;
    }

    QList<EngineDJFeature::PlaylistNode> readPlaylists(
            const QSqlDatabase& database, DeviceParser* pParser, const QString& devicePath) {
        const QStringList playlistColumns = columns(database, QStringLiteral("Playlist"));
        if (!playlistColumns.contains(QLatin1String("parentListId"))) {
            return {};
        }
        QSqlQuery playlists(database);
        playlists.exec(QStringLiteral("SELECT id, title, parentListId, %1, %2 FROM Playlist")
                        .arg(playlistColumns.contains(QLatin1String("nextListId"))
                                        ? QStringLiteral("nextListId")
                                        : QStringLiteral("0"),
                                playlistColumns.contains(QLatin1String("isPersisted"))
                                        ? QStringLiteral("isPersisted")
                                        : QStringLiteral("1")));
        QHash<int64_t, QString> titles;
        QHash<int64_t, QList<int64_t>> children;
        QHash<int64_t, int64_t> nextList;
        QList<int64_t> allIds;
        while (playlists.next()) {
            if (playlists.value(4).toInt() == 0) {
                // Not saved by the user
                continue;
            }
            const int64_t id = playlists.value(0).toLongLong();
            titles.insert(id, playlists.value(1).toString());
            children[playlists.value(2).toLongLong()].append(id);
            nextList.insert(id, playlists.value(3).toLongLong());
            allIds.append(id);
        }

        // The tracks of each playlist, in order
        QString uuid;
        QSqlQuery information(database);
        if (information.exec(QStringLiteral("SELECT uuid FROM Information")) &&
                information.next()) {
            uuid = information.value(0).toString();
        }
        const QStringList entityColumns = columns(database, QStringLiteral("PlaylistEntity"));
        QHash<int64_t, QList<int64_t>> entitiesOfList;
        QHash<int64_t, int64_t> nextEntity;
        QHash<int64_t, int64_t> trackOfEntity;
        QSqlQuery entities(database);
        entities.exec(QStringLiteral("SELECT id, listId, trackId, %1, %2 FROM PlaylistEntity")
                        .arg(entityColumns.contains(QLatin1String("nextEntityId"))
                                        ? QStringLiteral("nextEntityId")
                                        : QStringLiteral("0"),
                                entityColumns.contains(QLatin1String("databaseUuid"))
                                        ? QStringLiteral("databaseUuid")
                                        : QStringLiteral("NULL")));
        int resolved = 0;
        int unresolved = 0;
        while (entities.next()) {
            const int64_t id = entities.value(0).toLongLong();
            const int64_t listId = entities.value(1).toLongLong();
            int64_t trackId = entities.value(2).toLongLong();
            const QString databaseUuid = entities.value(4).toString();
            // Entries refer to tracks by their id in the library they were
            // added from. Drives exported by Engine DJ map those to their own
            // copies of the tracks.
            if (!databaseUuid.isEmpty() &&
                    databaseUuid.compare(uuid, Qt::CaseInsensitive) != 0) {
                const auto local = m_tracksByOrigin.constFind(originKey(databaseUuid, trackId));
                if (local == m_tracksByOrigin.constEnd()) {
                    ++unresolved;
                    continue;
                }
                trackId = local.value();
            }
            ++resolved;
            entitiesOfList[listId].append(id);
            trackOfEntity.insert(id, trackId);
            nextEntity.insert(id, entities.value(3).toLongLong());
        }
        qInfo() << "Engine Library:" << resolved << "playlist entries," << unresolved
                << "of tracks that are not on the drive";

        std::function<QList<EngineDJFeature::PlaylistNode>(int64_t, const QString&)> build =
                [&](int64_t parentId, const QString& parentName) {
                    QList<EngineDJFeature::PlaylistNode> nodes;
                    const QList<int64_t> ordered = linkedOrder(children.value(parentId), nextList);
                    QList<int64_t> trackIds;
                    for (const int64_t id : ordered) {
                        EngineDJFeature::PlaylistNode node;
                        node.label = titles.value(id);
                        node.name = parentName + QChar('/') + node.label;
                        trackIds.clear();
                        const QList<int64_t> orderedEntities =
                                linkedOrder(entitiesOfList.value(id), nextEntity);
                        for (const int64_t entity : orderedEntities) {
                            trackIds.append(trackOfEntity.value(entity));
                        }
                        pParser->addPlaylist(node.name, trackIds);
                        node.children = build(id, node.name);
                        nodes.append(node);
                    }
                    return nodes;
                };
        return build(0, devicePath);
    }

    const QString m_connectionName;
    QString m_error;
    /// Local track ids by "<origin database uuid>:<origin track id>"
    QHash<QString, int64_t> m_tracksByOrigin;
};

EngineDJFeature::ParseResult parseDevice(
        mixxx::DbConnectionPoolPtr pDbConnectionPool, const QString& devicePath) {
    EngineDJFeature::ParseResult result;
    result.devicePath = devicePath;

    // The pooler limits the lifetime of the thread-local connection
    const mixxx::DbConnectionPooler dbConnectionPooler(pDbConnectionPool);
    QSqlDatabase database = mixxx::DbConnectionPooled(pDbConnectionPool);
    VERIFY_OR_DEBUG_ASSERT(database.isOpen()) {
        result.error = QStringLiteral("Unable to open the Mixxx database");
        return result;
    }
    QThread::currentThread()->setPriority(QThread::LowPriority);

    const QString libraryDir = devicePath + QChar('/') + kEngineLibraryFolder;
    ScopedTransaction transaction(database);
    clearDevice(database, devicePath);
    DeviceParser parser(database, devicePath);

    const QString sqlDatabasePath = libraryDir + QStringLiteral("/Database2/m.db");
    if (QFileInfo::exists(sqlDatabasePath)) {
        SqlEngineLibrary library(sqlDatabasePath);
        if (library.error().isEmpty() && library.read(&parser, devicePath, &result.playlists)) {
            transaction.commit();
            qInfo() << "Read" << parser.trackCount() << "tracks and" << result.playlists.size()
                    << "playlists from the Engine Library of" << devicePath;
            return result;
        }
        result.error = library.error();
        qWarning() << "Unable to read the Engine Library of" << devicePath << result.error;
        return result;
    }

    // Engine Prime 1.x libraries
    try {
        const djinterop::database engineDb =
                djinterop::engine::load_database(libraryDir.toStdString());
        const std::vector<djinterop::track> tracks = engineDb.tracks();
        QList<int64_t> allTracks;
        for (const auto& track : tracks) {
            parser.addTrack(trackInfo(track));
            allTracks.append(track.id());
        }
        // All tracks of the device, shown when the device is opened
        parser.addPlaylist(devicePath, allTracks);
        result.playlists = parser.addTree(engineDb.root_playlists(), devicePath);
        // Old Engine Prime libraries organize the tracks in crates
        try {
            result.playlists += parser.addTree(
                    engineDb.root_crates(), devicePath + QStringLiteral("/crates"));
        } catch (const std::exception& e) {
            qDebug() << "No crates in the Engine Library of" << devicePath << e.what();
        }
        transaction.commit();
        qInfo() << "Read" << parser.trackCount() << "tracks from the Engine Library of"
                << devicePath;
    } catch (const std::exception& e) {
        result.error = QString::fromUtf8(e.what());
        qWarning() << "Unable to read the Engine Library of" << devicePath << result.error;
    }
    return result;
}

std::vector<std::unique_ptr<TreeItem>> createPlaylistItems(
        const QList<EngineDJFeature::PlaylistNode>& nodes) {
    std::vector<std::unique_ptr<TreeItem>> items;
    for (const auto& node : nodes) {
        auto pItem = std::make_unique<TreeItem>(
                node.label, QVariant(QVariantList{node.name, kPlaylistItem}));
        pItem->insertChildren(0, createPlaylistItems(node.children));
        items.push_back(std::move(pItem));
    }
    return items;
}

} // anonymous namespace

EngineDJPlaylistModel::EngineDJPlaylistModel(QObject* parent,
        TrackCollectionManager* pTrackCollectionManager,
        QSharedPointer<BaseTrackCache> trackSource)
        : BaseExternalPlaylistModel(parent,
                  pTrackCollectionManager,
                  "mixxx.db.model.enginedj.playlistmodel",
                  kPlaylistsTable,
                  kPlaylistTracksTable,
                  trackSource) {
}

void EngineDJPlaylistModel::initSortColumnMapping() {
    for (int i = 0; i < static_cast<int>(TrackModel::SortColumnId::IdMax); ++i) {
        m_columnIndexBySortColumnId[i] = -1;
    }
    const QList<std::pair<TrackModel::SortColumnId, ColumnCache::Column>> mapping = {
            {TrackModel::SortColumnId::Artist, ColumnCache::COLUMN_LIBRARYTABLE_ARTIST},
            {TrackModel::SortColumnId::Title, ColumnCache::COLUMN_LIBRARYTABLE_TITLE},
            {TrackModel::SortColumnId::Album, ColumnCache::COLUMN_LIBRARYTABLE_ALBUM},
            {TrackModel::SortColumnId::Year, ColumnCache::COLUMN_LIBRARYTABLE_YEAR},
            {TrackModel::SortColumnId::Genre, ColumnCache::COLUMN_LIBRARYTABLE_GENRE},
            {TrackModel::SortColumnId::TrackNumber,
                    ColumnCache::COLUMN_LIBRARYTABLE_TRACKNUMBER},
            {TrackModel::SortColumnId::NativeLocation,
                    ColumnCache::COLUMN_TRACKLOCATIONSTABLE_LOCATION},
            {TrackModel::SortColumnId::Comment, ColumnCache::COLUMN_LIBRARYTABLE_COMMENT},
            {TrackModel::SortColumnId::Duration, ColumnCache::COLUMN_LIBRARYTABLE_DURATION},
            {TrackModel::SortColumnId::BitRate, ColumnCache::COLUMN_LIBRARYTABLE_BITRATE},
            {TrackModel::SortColumnId::Bpm, ColumnCache::COLUMN_LIBRARYTABLE_BPM},
            {TrackModel::SortColumnId::Rating, ColumnCache::COLUMN_LIBRARYTABLE_RATING},
            {TrackModel::SortColumnId::Key, ColumnCache::COLUMN_LIBRARYTABLE_KEY},
            {TrackModel::SortColumnId::Color, ColumnCache::COLUMN_LIBRARYTABLE_COLOR},
            {TrackModel::SortColumnId::Position,
                    ColumnCache::COLUMN_PLAYLISTTRACKSTABLE_POSITION},
    };
    for (const auto& [sortColumn, column] : mapping) {
        m_columnIndexBySortColumnId[static_cast<int>(sortColumn)] = fieldIndex(column);
    }
    m_sortColumnIdByColumnIndex.clear();
    for (int i = static_cast<int>(TrackModel::SortColumnId::IdMin);
            i < static_cast<int>(TrackModel::SortColumnId::IdMax);
            ++i) {
        TrackModel::SortColumnId sortColumn = static_cast<TrackModel::SortColumnId>(i);
        m_sortColumnIdByColumnIndex.insert(
                m_columnIndexBySortColumnId[static_cast<int>(sortColumn)], sortColumn);
    }
}

EngineDJFeature::EngineDJFeature(Library* pLibrary, UserSettingsPointer pConfig)
        : BaseExternalLibraryFeature(pLibrary, pConfig, QStringLiteral("enginedj")),
          m_pSidebarModel(make_parented<TreeItemModel>(this)),
          m_title(tr("Engine DJ")) {
    QStringList columns = {
            LIBRARYTABLE_ID,
            LIBRARYTABLE_ARTIST,
            LIBRARYTABLE_TITLE,
            LIBRARYTABLE_ALBUM,
            LIBRARYTABLE_YEAR,
            LIBRARYTABLE_GENRE,
            LIBRARYTABLE_TRACKNUMBER,
            TRACKLOCATIONSTABLE_LOCATION,
            LIBRARYTABLE_COMMENT,
            LIBRARYTABLE_RATING,
            LIBRARYTABLE_DURATION,
            LIBRARYTABLE_BITRATE,
            LIBRARYTABLE_BPM,
            LIBRARYTABLE_KEY,
            LIBRARYTABLE_COLOR};
    QStringList searchColumns = {
            LIBRARYTABLE_ARTIST,
            LIBRARYTABLE_TITLE,
            LIBRARYTABLE_ALBUM,
            LIBRARYTABLE_GENRE,
            TRACKLOCATIONSTABLE_LOCATION,
            LIBRARYTABLE_COMMENT};
    m_trackSource = QSharedPointer<BaseTrackCache>::create(m_pTrackCollection,
            kLibraryTable,
            LIBRARYTABLE_ID,
            std::move(columns),
            std::move(searchColumns),
            false);
    m_pPlaylistModel = make_parented<EngineDJPlaylistModel>(
            this, pLibrary->trackCollectionManager(), m_trackSource);

    // The tables only hold what was read from the drives in this session
    QSqlDatabase database = m_pTrackCollection->database();
    ScopedTransaction transaction(database);
    dropTables(database);
    createTables(database);
    transaction.commit();

    connect(&m_devicesFutureWatcher,
            &QFutureWatcher<QList<Device>>::finished,
            this,
            &EngineDJFeature::onDevicesFound);
    connect(&m_parseFutureWatcher,
            &QFutureWatcher<ParseResult>::finished,
            this,
            &EngineDJFeature::onDeviceParsed);
    m_pSidebarModel->setRootItem(TreeItem::newRoot(this));
}

EngineDJFeature::~EngineDJFeature() {
    m_devicesFuture.waitForFinished();
    m_parseFuture.waitForFinished();
    QSqlDatabase database = m_pTrackCollection->database();
    ScopedTransaction transaction(database);
    dropTables(database);
    transaction.commit();
}

QVariant EngineDJFeature::title() {
    return m_title;
}

TreeItemModel* EngineDJFeature::sidebarModel() const {
    return m_pSidebarModel;
}

void EngineDJFeature::activate() {
    if (m_devicesFuture.isRunning()) {
        return;
    }
    m_devicesFuture = QtConcurrent::run(findDevices);
    m_devicesFutureWatcher.setFuture(m_devicesFuture);
    m_title = tr("(loading) Engine DJ");
    emit featureIsLoading(this, true);
    emit enableCoverArtDisplay(true);
}

void EngineDJFeature::onDevicesFound() {
    const QList<Device> devices = m_devicesFuture.result();
    TreeItem* pRoot = m_pSidebarModel->getRootItem();
    QSqlDatabase database = m_pTrackCollection->database();

    // Forget the devices that have been removed
    for (int row = pRoot->childRows() - 1; row >= 0; --row) {
        const QString path = pRoot->child(row)->getData().toList().value(0).toString();
        const bool connected = std::any_of(devices.cbegin(),
                devices.cend(),
                [&path](const Device& device) { return device.path == path; });
        if (!connected) {
            ScopedTransaction transaction(database);
            clearDevice(database, path);
            transaction.commit();
            m_pSidebarModel->removeRows(row, 1);
        }
    }
    std::vector<std::unique_ptr<TreeItem>> newItems;
    for (const auto& device : devices) {
        bool known = false;
        for (int row = 0; row < pRoot->childRows(); ++row) {
            if (pRoot->child(row)->getData().toList().value(0).toString() == device.path) {
                known = true;
                break;
            }
        }
        if (!known) {
            newItems.push_back(std::make_unique<TreeItem>(
                    device.label, QVariant(QVariantList{device.path, kDeviceItem})));
        }
    }
    if (!newItems.empty()) {
        m_pSidebarModel->insertTreeItemRows(std::move(newItems), pRoot->childRows());
    }

    m_title = tr("Engine DJ");
    emit featureLoadingFinished(this);
}

void EngineDJFeature::activateChild(const QModelIndex& index) {
    if (!index.isValid()) {
        return;
    }
    TreeItem* pItem = static_cast<TreeItem*>(index.internalPointer());
    if (!pItem) {
        return;
    }
    const QVariantList data = pItem->getData().toList();
    if (data.size() != 2) {
        return;
    }
    const QString name = data.at(0).toString();
    if (data.at(1).toString() == kPlaylistItem) {
        showPlaylist(name);
        return;
    }
    // A device: it's read the first time it is opened
    if (pItem->childRows() > 0) {
        showPlaylist(name);
        return;
    }
    if (m_parseFuture.isRunning()) {
        return;
    }
    m_parseFuture = QtConcurrent::run(parseDevice, m_pLibrary->dbConnectionPool(), name);
    m_parseFutureWatcher.setFuture(m_parseFuture);
    emit featureIsLoading(this, true);
}

void EngineDJFeature::onDeviceParsed() {
    const ParseResult result = m_parseFuture.result();
    emit featureLoadingFinished(this);
    if (!result.error.isEmpty()) {
        return;
    }
    TreeItem* pRoot = m_pSidebarModel->getRootItem();
    for (int row = 0; row < pRoot->childRows(); ++row) {
        TreeItem* pDevice = pRoot->child(row);
        if (pDevice->getData().toList().value(0).toString() != result.devicePath) {
            continue;
        }
        if (pDevice->childRows() == 0 && !result.playlists.isEmpty()) {
            m_pSidebarModel->insertTreeItemRows(createPlaylistItems(result.playlists),
                    0,
                    m_pSidebarModel->index(row, 0));
        }
        break;
    }
    showPlaylist(result.devicePath);
}

void EngineDJFeature::showPlaylist(const QString& name) {
    m_pPlaylistModel->setPlaylist(name);
    emit showTrackModel(m_pPlaylistModel);
}

std::unique_ptr<BaseSqlTableModel> EngineDJFeature::createPlaylistModelForPlaylist(
        const QVariant& data) {
    const QVariantList playlist = data.toList();
    VERIFY_OR_DEBUG_ASSERT(!playlist.isEmpty()) {
        return {};
    }
    auto pModel = std::make_unique<EngineDJPlaylistModel>(
            this, m_pLibrary->trackCollectionManager(), m_trackSource);
    pModel->setPlaylist(playlist.at(0).toString());
    return pModel;
}
