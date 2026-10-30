#include "qml/qmlstreamingproxy.h"

#include <QDebug>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QUrlQuery>

#include "moc_qmlstreamingproxy.cpp"
#include "sources/metadatasourcetaglib.h"
#include "track/trackmetadata.h"
#include "util/fileinfo.h"
#include "util/sandbox.h"

namespace mixxx {
namespace qml {

namespace {
const QString kStreamingGroup = QStringLiteral("[Streaming]");
const QString kSubsonicUrlKey = QStringLiteral("SubsonicUrl");
const QString kSubsonicUserKey = QStringLiteral("SubsonicUsername");
const QString kSubsonicPassKey = QStringLiteral("SubsonicPassword");
const QString kDefaultUserAgent = QStringLiteral(
        "Mozilla/5.0 (Linux; Android 14; Mobile; Mixxx-DJ) AppleWebKit/537.36");
} // namespace

QmlStreamingProxy::QmlStreamingProxy(QObject* parent)
        : QObject(parent),
          m_pNetworkManager(new QNetworkAccessManager(this)) {
    // Determine the best cache directory for streaming tracks
#if defined(Q_OS_ANDROID)
    QString baseDir = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (baseDir.isEmpty()) {
        baseDir = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    }
    m_cacheDir = baseDir + QStringLiteral("/streaming_cache");
    QDir(m_cacheDir).mkpath(QStringLiteral("."));
#else
    m_cacheDir =
            QStandardPaths::writableLocation(QStandardPaths::CacheLocation) +
            QStringLiteral("/streaming");
    QDir(m_cacheDir).mkpath(QStringLiteral("."));
#endif

    // Load persisted settings if config is available
    if (s_pConfig) {
        m_subsonicUrl = s_pConfig->getValue(ConfigKey(kStreamingGroup, kSubsonicUrlKey), QString());
        m_subsonicUsername = s_pConfig->getValue(
                ConfigKey(kStreamingGroup, kSubsonicUserKey), QString());
        m_subsonicPassword = s_pConfig->getValue(
                ConfigKey(kStreamingGroup, kSubsonicPassKey), QString());
    }
}

QmlStreamingProxy::~QmlStreamingProxy() {
    for (auto it = m_activeDownloads.begin(); it != m_activeDownloads.end(); ++it) {
        if (it.value()->pReply) {
            it.value()->pReply->abort();
        }
        if (it.value()->pFile && it.value()->pFile->isOpen()) {
            it.value()->pFile->close();
        }
        QFile::remove(it.value()->partPath);
    }
    m_activeDownloads.clear();
}

// static
QmlStreamingProxy* QmlStreamingProxy::create(
        QQmlEngine* pQmlEngine, [[maybe_unused]] QJSEngine* pJsEngine) {
    return new QmlStreamingProxy(pQmlEngine);
}

QString QmlStreamingProxy::subsonicUrl() const {
    return m_subsonicUrl;
}

void QmlStreamingProxy::setSubsonicUrl(const QString& url) {
    if (m_subsonicUrl != url) {
        m_subsonicUrl = url;
        if (s_pConfig) {
            s_pConfig->setValue(ConfigKey(kStreamingGroup, kSubsonicUrlKey), url);
        }
        emit subsonicUrlChanged();
    }
}

QString QmlStreamingProxy::subsonicUsername() const {
    return m_subsonicUsername;
}

void QmlStreamingProxy::setSubsonicUsername(const QString& username) {
    if (m_subsonicUsername != username) {
        m_subsonicUsername = username;
        if (s_pConfig) {
            s_pConfig->setValue(ConfigKey(kStreamingGroup, kSubsonicUserKey), username);
        }
        emit subsonicUsernameChanged();
    }
}

QString QmlStreamingProxy::subsonicPassword() const {
    return m_subsonicPassword;
}

void QmlStreamingProxy::setSubsonicPassword(const QString& password) {
    if (m_subsonicPassword != password) {
        m_subsonicPassword = password;
        if (s_pConfig) {
            s_pConfig->setValue(ConfigKey(kStreamingGroup, kSubsonicPassKey), password);
        }
        emit subsonicPasswordChanged();
    }
}

QString QmlStreamingProxy::sanitizeFileName(const QString& input) const {
    QString sanitized = input;
    sanitized.replace(QRegularExpression(QStringLiteral("[\\\\/:*?\"<>|]")), QStringLiteral("_"));
    sanitized.replace(QRegularExpression(QStringLiteral("[\\x00-\\x1f]")), QStringLiteral(""));
    return sanitized.trimmed();
}

QString QmlStreamingProxy::buildTrackFileName(
        const QString& service,
        const QString& trackId,
        const QString& title,
        const QString& artist,
        const QString& ext) const {
    QString displayName;
    if (!artist.isEmpty() && !title.isEmpty()) {
        displayName = QStringLiteral("%1 - %2").arg(artist.trimmed(), title.trimmed());
    } else if (!title.isEmpty()) {
        displayName = title.trimmed();
    } else {
        displayName = QStringLiteral("%1_%2").arg(service, trackId);
    }
    const QString safeName = sanitizeFileName(displayName);
    const QString safeId = sanitizeFileName(trackId);
    return QStringLiteral("%1 [%2_%3]%4").arg(safeName, service, safeId, ext);
}

QString QmlStreamingProxy::buildSubsonicUrl(
        const QString& endpoint, const QString& extraParams) const {
    if (m_subsonicUrl.trimmed().isEmpty()) {
        return QString();
    }
    QString base = m_subsonicUrl.trimmed();
    while (base.endsWith(QLatin1Char('/'))) {
        base.chop(1);
    }
    if (!base.startsWith(QStringLiteral("http://")) &&
            !base.startsWith(QStringLiteral("https://"))) {
        base.prepend(QStringLiteral("http://"));
    }

    const QString salt = QString::number(QRandomGenerator::global()->generate(), 16);
    const QString token = QString::fromLatin1(QCryptographicHash::hash(
            (m_subsonicPassword + salt).toUtf8(), QCryptographicHash::Md5)
                    .toHex());

    QString fullUrl = QStringLiteral(
            "%1/rest/%2.view?u=%3&t=%4&s=%5&v=1.16.1&c=MixxxAndroid&f=json")
                              .arg(base,
                                      endpoint,
                                      QString::fromUtf8(QUrl::toPercentEncoding(
                                              m_subsonicUsername)),
                                      token,
                                      salt);

    if (!extraParams.isEmpty()) {
        fullUrl += QLatin1Char('&') + extraParams;
    }
    return fullUrl;
}

QString QmlStreamingProxy::getSubsonicStreamUrl(const QString& songId) const {
    return buildSubsonicUrl(QStringLiteral("stream"), QStringLiteral("id=") + songId);
}

QString QmlStreamingProxy::getSubsonicCoverArtUrl(const QString& coverArtId, int size) const {
    if (coverArtId.isEmpty()) {
        return QString();
    }
    return buildSubsonicUrl(QStringLiteral("getCoverArt"),
            QStringLiteral("id=%1&size=%2").arg(coverArtId).arg(size));
}

void QmlStreamingProxy::testSubsonicConnection() {
    const QString urlStr = buildSubsonicUrl(QStringLiteral("ping"));
    if (urlStr.isEmpty()) {
        emit subsonicConnectionTested(false, tr("The server URL is empty"), QString());
        return;
    }

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            m_isSubsonicConnected = false;
            emit subsonicConnectionChanged();
            emit subsonicConnectionTested(false, reply->errorString(), QString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonObject root = doc.object();
        QJsonObject resp = root.value(QStringLiteral("subsonic-response")).toObject();
        QString status = resp.value(QStringLiteral("status")).toString();

        if (status == QStringLiteral("ok")) {
            m_isSubsonicConnected = true;
            QString version =
                    resp.value(QStringLiteral("serverVersion")).toString();
            if (version.isEmpty()) {
                version = resp.value(QStringLiteral("version")).toString();
            }
            m_subsonicServerVersion = version;
            emit subsonicConnectionChanged();
            emit subsonicConnectionTested(
                    true, tr("Connection established successfully!"), version);
        } else {
            m_isSubsonicConnected = false;
            emit subsonicConnectionChanged();
            QJsonObject err = resp.value(QStringLiteral("error")).toObject();
            QString msg =
                    err.value(QStringLiteral("message"))
                            .toString(tr("Authentication or server error"));
            emit subsonicConnectionTested(false, msg, QString());
        }
    });
}

void QmlStreamingProxy::searchSubsonic(const QString& query) {
    if (query.trimmed().isEmpty()) {
        emit subsonicSearchFinished(true, QVariantList(), QString());
        return;
    }

    const QString extra = QStringLiteral("query=") +
            QString::fromUtf8(QUrl::toPercentEncoding(query.trimmed()));
    const QString urlStr = buildSubsonicUrl(QStringLiteral("search3"), extra);
    if (urlStr.isEmpty()) {
        emit subsonicSearchFinished(false, QVariantList(), tr("Set up the Navidrome/Subsonic URL"));
        return;
    }

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            emit subsonicSearchFinished(false, QVariantList(), reply->errorString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonObject resp = doc.object().value(QStringLiteral("subsonic-response")).toObject();
        QJsonObject searchResult = resp.value(QStringLiteral("searchResult3")).toObject();
        QJsonArray songs = searchResult.value(QStringLiteral("song")).toArray();

        QVariantList resultList;
        for (const auto& songVal : songs) {
            QJsonObject song = songVal.toObject();
            QVariantMap item;
            QString id = song.value(QStringLiteral("id")).toString();
            item[QStringLiteral("id")] = id;
            item[QStringLiteral("title")] = song.value(QStringLiteral("title"))
                                                    .toString(tr("Untitled"));
            item[QStringLiteral("artist")] =
                    song.value(QStringLiteral("artist"))
                            .toString(tr("Unknown artist"));
            item[QStringLiteral("album")] =
                    song.value(QStringLiteral("album")).toString();
            item[QStringLiteral("duration")] = song.value(QStringLiteral("duration")).toInt();
            item[QStringLiteral("suffix")] =
                    song.value(QStringLiteral("suffix"))
                            .toString(QStringLiteral("mp3"))
                            .toUpper();
            item[QStringLiteral("bitRate")] =
                    song.value(QStringLiteral("bitRate")).toInt();
            item[QStringLiteral("size")] =
                    song.value(QStringLiteral("size")).toVariant().toLongLong();
            item[QStringLiteral("coverArt")] =
                    song.value(QStringLiteral("coverArt")).toString();
            item[QStringLiteral("coverArtUrl")] = getSubsonicCoverArtUrl(
                    song.value(QStringLiteral("coverArt")).toString(), 160);
            item[QStringLiteral("streamUrl")] = getSubsonicStreamUrl(id);
            item[QStringLiteral("service")] = QStringLiteral("subsonic");
            item[QStringLiteral("isCached")] =
                    isTrackCached(QStringLiteral("subsonic"), id);
            resultList.append(item);
        }

        emit subsonicSearchFinished(true, resultList, QString());
    });
}

void QmlStreamingProxy::getSubsonicPlaylists() {
    const QString urlStr = buildSubsonicUrl(QStringLiteral("getPlaylists"));
    if (urlStr.isEmpty()) {
        emit subsonicPlaylistsFinished(
                false, QVariantList(), tr("Set up the Navidrome/Subsonic URL"));
        return;
    }

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            emit subsonicPlaylistsFinished(false, QVariantList(), reply->errorString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonObject resp = doc.object().value(QStringLiteral("subsonic-response")).toObject();
        QJsonObject playlistsObj = resp.value(QStringLiteral("playlists")).toObject();
        QJsonArray playlists = playlistsObj.value(QStringLiteral("playlist")).toArray();

        QVariantList resultList;
        for (const auto& plVal : playlists) {
            QJsonObject pl = plVal.toObject();
            QVariantMap item;
            item[QStringLiteral("id")] = pl.value(QStringLiteral("id")).toString();
            item[QStringLiteral("name")] = pl.value(QStringLiteral("name")).toString();
            item[QStringLiteral("songCount")] = pl.value(QStringLiteral("songCount")).toInt();
            item[QStringLiteral("duration")] = pl.value(QStringLiteral("duration")).toInt();
            resultList.append(item);
        }

        emit subsonicPlaylistsFinished(true, resultList, QString());
    });
}

void QmlStreamingProxy::getSubsonicPlaylistSongs(const QString& playlistId) {
    const QString extra = QStringLiteral("id=") + playlistId;
    const QString urlStr = buildSubsonicUrl(QStringLiteral("getPlaylist"), extra);
    if (urlStr.isEmpty()) {
        emit subsonicPlaylistSongsFinished(
                false, QVariantList(), tr("Set up the Navidrome/Subsonic URL"));
        return;
    }

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            emit subsonicPlaylistSongsFinished(false, QVariantList(), reply->errorString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonObject resp = doc.object()
                                   .value(QStringLiteral("subsonic-response"))
                                   .toObject();
        QJsonObject playlistObj = resp.value(QStringLiteral("playlist")).toObject();
        QJsonArray entries = playlistObj.value(QStringLiteral("entry")).toArray();

        QVariantList resultList;
        for (const auto& songVal : entries) {
            QJsonObject song = songVal.toObject();
            QVariantMap item;
            QString id = song.value(QStringLiteral("id")).toString();
            item[QStringLiteral("id")] = id;
            item[QStringLiteral("title")] = song.value(QStringLiteral("title"))
                                                    .toString(tr("Untitled"));
            item[QStringLiteral("artist")] =
                    song.value(QStringLiteral("artist"))
                            .toString(tr("Unknown artist"));
            item[QStringLiteral("album")] =
                    song.value(QStringLiteral("album")).toString();
            item[QStringLiteral("duration")] = song.value(QStringLiteral("duration")).toInt();
            item[QStringLiteral("suffix")] =
                    song.value(QStringLiteral("suffix"))
                            .toString(QStringLiteral("mp3"))
                            .toUpper();
            item[QStringLiteral("bitRate")] =
                    song.value(QStringLiteral("bitRate")).toInt();
            item[QStringLiteral("coverArt")] = song.value(QStringLiteral("coverArt")).toString();
            item[QStringLiteral("coverArtUrl")] = getSubsonicCoverArtUrl(
                    song.value(QStringLiteral("coverArt")).toString(), 160);
            item[QStringLiteral("streamUrl")] = getSubsonicStreamUrl(id);
            item[QStringLiteral("service")] = QStringLiteral("subsonic");
            item[QStringLiteral("isCached")] = isTrackCached(QStringLiteral("subsonic"), id);
            resultList.append(item);
        }

        emit subsonicPlaylistSongsFinished(true, resultList, QString());
    });
}

void QmlStreamingProxy::searchAudius(const QString& query) {
    if (query.trimmed().isEmpty()) {
        getAudiusTrending();
        return;
    }

    const QString urlStr =
            QStringLiteral(
                    "https://discoveryprovider.audius.co/v1/tracks/"
                    "search?query=%1&app_name=MixxxAndroid")
                    .arg(QString::fromUtf8(
                            QUrl::toPercentEncoding(query.trimmed())));

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            emit audiusSearchFinished(
                    false, QVariantList(), reply->errorString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonArray tracks = doc.object().value(QStringLiteral("data")).toArray();

        QVariantList resultList;
        for (const auto& tVal : tracks) {
            QJsonObject t = tVal.toObject();
            QJsonObject user = t.value(QStringLiteral("user")).toObject();
            QString id = t.value(QStringLiteral("id")).toString();
            if (id.isEmpty() && t.contains(QStringLiteral("track_id"))) {
                id = QString::number(t.value(QStringLiteral("track_id")).toInteger());
            }
            QVariantMap item;
            item[QStringLiteral("id")] = id;
            item[QStringLiteral("title")] =
                    t.value(QStringLiteral("title"))
                            .toString(tr("Audius track"));
            item[QStringLiteral("artist")] =
                    user.value(QStringLiteral("name"))
                            .toString(tr("Audius artist"));
            item[QStringLiteral("duration")] =
                    t.value(QStringLiteral("duration")).toInt();
            item[QStringLiteral("genre")] =
                    t.value(QStringLiteral("genre"))
                            .toString(QStringLiteral("Electronic"));
            item[QStringLiteral("bpm")] =
                    t.value(QStringLiteral("bpm")).toDouble(126.0);
            item[QStringLiteral("key")] =
                    t.value(QStringLiteral("musical_key")).toString();
            QJsonObject artwork = t.value(QStringLiteral("artwork")).toObject();
            QString artUrl = artwork.value(QStringLiteral("150x150")).toString();
            if (artUrl.isEmpty()) {
                artUrl = artwork.value(QStringLiteral("480x480")).toString();
            }
            item[QStringLiteral("coverArtUrl")] = artUrl;
            item[QStringLiteral("suffix")] = QStringLiteral("MP3");
            item[QStringLiteral("streamUrl")] = QStringLiteral(
                    "https://discoveryprovider.audius.co/v1/tracks/%1/"
                    "stream?app_name=MixxxAndroid")
                                                        .arg(id);
            item[QStringLiteral("service")] = QStringLiteral("audius");
            item[QStringLiteral("isCached")] = isTrackCached(QStringLiteral("audius"), id);
            resultList.append(item);
        }

        emit audiusSearchFinished(true, resultList, QString());
    });
}

void QmlStreamingProxy::getAudiusTrending(const QString& genre) {
    const QString urlStr =
            QStringLiteral(
                    "https://discoveryprovider.audius.co/v1/tracks/"
                    "trending?genre=%1&app_name=MixxxAndroid")
                    .arg(QString::fromUtf8(QUrl::toPercentEncoding(genre)));

    QNetworkRequest request{QUrl(urlStr)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (reply->error() != QNetworkReply::NoError) {
            emit audiusTrendingFinished(false, QVariantList(), reply->errorString());
            return;
        }

        const QByteArray data = reply->readAll();
        QJsonDocument doc = QJsonDocument::fromJson(data);
        QJsonArray tracks = doc.object().value(QStringLiteral("data")).toArray();

        QVariantList resultList;
        for (const auto& tVal : tracks) {
            QJsonObject t = tVal.toObject();
            QJsonObject user = t.value(QStringLiteral("user")).toObject();
            QString id = t.value(QStringLiteral("id")).toString();
            if (id.isEmpty() && t.contains(QStringLiteral("track_id"))) {
                id = QString::number(t.value(QStringLiteral("track_id")).toInteger());
            }
            QVariantMap item;
            item[QStringLiteral("id")] = id;
            item[QStringLiteral("title")] =
                    t.value(QStringLiteral("title"))
                            .toString(tr("Audius track"));
            item[QStringLiteral("artist")] =
                    user.value(QStringLiteral("name"))
                            .toString(tr("Audius artist"));
            item[QStringLiteral("duration")] =
                    t.value(QStringLiteral("duration")).toInt();
            item[QStringLiteral("genre")] =
                    t.value(QStringLiteral("genre"))
                            .toString(QStringLiteral("Electronic"));
            item[QStringLiteral("bpm")] =
                    t.value(QStringLiteral("bpm")).toDouble(126.0);
            item[QStringLiteral("key")] = t.value(QStringLiteral("musical_key")).toString();
            QJsonObject artwork = t.value(QStringLiteral("artwork")).toObject();
            QString artUrl = artwork.value(QStringLiteral("150x150")).toString();
            if (artUrl.isEmpty()) {
                artUrl = artwork.value(QStringLiteral("480x480")).toString();
            }
            item[QStringLiteral("coverArtUrl")] = artUrl;
            item[QStringLiteral("suffix")] = QStringLiteral("MP3");
            item[QStringLiteral("streamUrl")] = QStringLiteral(
                    "https://discoveryprovider.audius.co/v1/tracks/%1/"
                    "stream?app_name=MixxxAndroid")
                                                        .arg(id);
            item[QStringLiteral("service")] = QStringLiteral("audius");
            item[QStringLiteral("isCached")] = isTrackCached(QStringLiteral("audius"), id);
            resultList.append(item);
        }

        emit audiusTrendingFinished(true, resultList, QString());
    });
}

bool QmlStreamingProxy::isTrackCached(const QString& service, const QString& trackId) const {
    return !getCachedTrackPath(service, trackId).isEmpty();
}

QString QmlStreamingProxy::getCachedTrackPath(
        const QString& service, const QString& trackId) const {
    const QString safeId = sanitizeFileName(trackId);
    const QString marker = QStringLiteral("[%1_%2]").arg(service, safeId);
    const QString legacyPrefix = service + QLatin1Char('_') + safeId;

    QDir dir(m_cacheDir);
    const QStringList files = dir.entryList(QDir::Files);
    for (const QString& file : files) {
        if (!file.endsWith(QStringLiteral(".part"))) {
            if (file.contains(marker) || file.startsWith(legacyPrefix)) {
                const QString fullPath = dir.absoluteFilePath(file);
                QFileInfo fi(fullPath);
                if (fi.size() > 1024) {
                    return fullPath;
                }
            }
        }
    }
    return QString();
}

void QmlStreamingProxy::downloadAndLoad(
        const QString& streamUrl,
        const QString& service,
        const QString& trackId,
        const QString& title,
        const QString& artist,
        const QString& targetDeck,
        bool autoPlay) {
    if (streamUrl.isEmpty() || trackId.isEmpty()) {
        emit downloadFailed(trackId, tr("Invalid URL or track ID"));
        return;
    }

    // 1. Check if already cached on disk
    const QString cachedPath = getCachedTrackPath(service, trackId);
    if (!cachedPath.isEmpty()) {
        qInfo() << "QmlStreamingProxy: Pista en caché encontrada:" << cachedPath;
        emit downloadProgress(trackId, 1.0, 100, 100);
        if (!targetDeck.isEmpty() && s_pPlayerManager) {
            mixxx::FileInfo fileInfo(cachedPath);
            Sandbox::createSecurityToken(&fileInfo);
            QMetaObject::invokeMethod(s_pPlayerManager.get(), [cachedPath, targetDeck, autoPlay]() {
                if (s_pPlayerManager) {
                    s_pPlayerManager->slotLoadLocationToPlayer(cachedPath, targetDeck, autoPlay);
                } }, Qt::QueuedConnection);
        }
        emit downloadFinished(trackId, cachedPath, targetDeck, autoPlay);
        return;
    }

    // 2. Check if currently being downloaded
    if (m_activeDownloads.contains(trackId)) {
        qInfo() << "QmlStreamingProxy: La pista ya se está descargando:" << trackId;
        return;
    }

    // Determine extension
    QString ext = QStringLiteral(".mp3");
    if (streamUrl.contains(QStringLiteral(".flac"), Qt::CaseInsensitive)) {
        ext = QStringLiteral(".flac");
    } else if (streamUrl.contains(
                       QStringLiteral(".m4a"), Qt::CaseInsensitive) ||
            streamUrl.contains(QStringLiteral(".aac"), Qt::CaseInsensitive)) {
        ext = QStringLiteral(".m4a");
    } else if (streamUrl.contains(
                       QStringLiteral(".ogg"), Qt::CaseInsensitive)) {
        ext = QStringLiteral(".ogg");
    } else if (streamUrl.contains(
                       QStringLiteral(".wav"), Qt::CaseInsensitive)) {
        ext = QStringLiteral(".wav");
    }

    const QString finalFileName = buildTrackFileName(service, trackId, title, artist, ext);
    const QString finalPath = m_cacheDir + QLatin1Char('/') + finalFileName;
    const QString partPath = finalPath + QStringLiteral(".part");

    auto pFile = std::make_shared<QFile>(partPath);
    if (!pFile->open(QIODevice::WriteOnly)) {
        emit downloadFailed(
                trackId, tr("Cannot create the temporary cache file"));
        return;
    }

    QNetworkRequest request{QUrl(streamUrl)};
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
            QNetworkRequest::NoLessSafeRedirectPolicy);
    request.setHeader(QNetworkRequest::UserAgentHeader, kDefaultUserAgent);

    QNetworkReply* reply = m_pNetworkManager->get(request);
    connect(reply, &QNetworkReply::sslErrors, reply, [reply](const QList<QSslError>& errors) {
        Q_UNUSED(errors);
        reply->ignoreSslErrors();
    });
    reply->ignoreSslErrors();

    auto job = std::make_shared<StreamingDownloadJob>();
    job->trackId = trackId;
    job->service = service;
    job->title = title;
    job->artist = artist;
    job->targetDeck = targetDeck;
    job->autoPlay = autoPlay;
    job->ext = ext;
    job->finalPath = finalPath;
    job->partPath = partPath;
    job->pFile = pFile;
    job->pReply = reply;

    m_activeDownloads.insert(trackId, job);
    emit activeDownloadsChanged();

    connect(reply,
            &QNetworkReply::downloadProgress,
            this,
            [this, trackId](qint64 received, qint64 total) {
                qreal progress = total > 0 ? static_cast<qreal>(received) /
                                static_cast<qreal>(total)
                                           : 0.0;
                emit downloadProgress(trackId, progress, received, total);
            });

    connect(reply, &QIODevice::readyRead, this, [job]() {
        if (job->pFile && job->pFile->isOpen()) {
            job->pFile->write(job->pReply->readAll());
        }
    });

    connect(reply, &QNetworkReply::finished, this, [this, job]() {
        job->pReply->deleteLater();
        if (job->pFile && job->pFile->isOpen()) {
            job->pFile->write(job->pReply->readAll());
            job->pFile->flush();
            job->pFile->close();
        }

        const bool hasError = (job->pReply->error() != QNetworkReply::NoError);
        const QString errorMsg = job->pReply->errorString();
        const QString trackId = job->trackId;
        const QString targetDeck = job->targetDeck;
        const QString finalPath = job->finalPath;
        const QString partPath = job->partPath;
        const bool autoPlay = job->autoPlay;

        m_activeDownloads.remove(trackId);
        emit activeDownloadsChanged();

        if (hasError) {
            qWarning() << "QmlStreamingProxy: Error al descargar:" << errorMsg;
            QFile::remove(partPath);
            emit downloadFailed(trackId, errorMsg);
            return;
        }

        // Rename part file to final file
        QFile::remove(finalPath);
        if (!QFile::rename(partPath, finalPath)) {
            emit downloadFailed(trackId, tr("Could not save the downloaded track"));
            return;
        }

        // Write ID3 metadata tags (Title, Artist) before loading so Mixxx will
        // never display file fallback names
        mixxx::TrackMetadata trackMetadata;
        if (!job->title.isEmpty()) {
            trackMetadata.refTrackInfo().setTitle(job->title);
        }
        if (!job->artist.isEmpty()) {
            trackMetadata.refTrackInfo().setArtist(job->artist);
        }
        QString extFormat = job->ext.startsWith(QLatin1Char('.')) ? job->ext.mid(1) : job->ext;
        mixxx::MetadataSourceTagLib tagSource(finalPath, extFormat);
        tagSource.exportTrackMetadata(trackMetadata);

        qInfo() << "QmlStreamingProxy: Descarga completada:" << finalPath << "para" << targetDeck;
        emit refreshCacheSize();

        // Automatically load into Deck if targetDeck is requested (safely queued to main thread)
        if (!targetDeck.isEmpty() && s_pPlayerManager) {
            mixxx::FileInfo fileInfo(finalPath);
            Sandbox::createSecurityToken(&fileInfo);
            QMetaObject::invokeMethod(s_pPlayerManager.get(), [finalPath, targetDeck, autoPlay]() {
                if (s_pPlayerManager) {
                    s_pPlayerManager->slotLoadLocationToPlayer(finalPath, targetDeck, autoPlay);
                } }, Qt::QueuedConnection);
        }

        emit downloadFinished(trackId, finalPath, targetDeck, autoPlay);
    });
}

void QmlStreamingProxy::cancelDownload(const QString& trackId) {
    if (m_activeDownloads.contains(trackId)) {
        auto job = m_activeDownloads.take(trackId);
        emit activeDownloadsChanged();
        if (job->pReply) {
            job->pReply->abort();
        }
        if (job->pFile && job->pFile->isOpen()) {
            job->pFile->close();
        }
        QFile::remove(job->partPath);
    }
}

qint64 QmlStreamingProxy::cacheSizeBytes() const {
    qint64 total = 0;
    QDir dir(m_cacheDir);
    const QFileInfoList list = dir.entryInfoList(QDir::Files | QDir::NoDotAndDotDot);
    for (const QFileInfo& fi : list) {
        total += fi.size();
    }
    return total;
}

void QmlStreamingProxy::refreshCacheSize() {
    emit cacheSizeChanged();
}

void QmlStreamingProxy::clearCache() {
    QDir dir(m_cacheDir);
    const QFileInfoList list = dir.entryInfoList(QDir::Files | QDir::NoDotAndDotDot);
    for (const QFileInfo& fi : list) {
        QFile::remove(fi.absoluteFilePath());
    }
    emit cacheSizeChanged();
}

} // namespace qml
} // namespace mixxx
