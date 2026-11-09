#include "qml/qmlspotifyproxy.h"

#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QPointer>
#include <QRegularExpression>
#include <QSettings>
#include <QStandardPaths>
#include <QThreadPool>
#include <QUrlQuery>
#include <cmath>

#include "library/library.h"
#include "library/trackcollectionmanager.h"
#include "mixer/playermanager.h"
#include "moc_qmlspotifyproxy.cpp"
#include "track/track.h"
#include "util/logger.h"

#ifdef HAVE_SPOTIFY_BRIDGE
#include "spotifybridge.h"
#endif

#ifdef Q_OS_ANDROID
#include <jni.h>

#include <QCoreApplication>
#include <QJniObject>
#endif

namespace {
const mixxx::Logger kLogger("QmlSpotifyProxy");

#ifndef DJSUGAR_SPOTIFY_CLIENT_ID
#define DJSUGAR_SPOTIFY_CLIENT_ID ""
#endif
const QString kRedirectUri = QStringLiteral("djsugar://spotify-callback");
const QString kApi = QStringLiteral("https://api.spotify.com/v1");
const QString kTokenUrl = QStringLiteral("https://accounts.spotify.com/api/token");
const QString kClientIdKey = QStringLiteral("Spotify/clientId");
const QString kRefreshTokenKey = QStringLiteral("Spotify/refreshToken");
const QString kUserNameKey = QStringLiteral("Spotify/userName");
// Development-mode Spotify apps get at most 10 search results per page
constexpr int kSearchPage = 10;
constexpr int kListPage = 50;

QNetworkRequest jsonRequest(const QUrl& url) {
    QNetworkRequest request(url);
    request.setTransferTimeout(15000);
    return request;
}
} // namespace

namespace mixxx {
namespace qml {

QmlSpotifyProxy::QmlSpotifyProxy(QObject* pParent)
        : QObject(pParent),
          m_pNetwork(new QNetworkAccessManager(this)) {
    s_pInstance = this;
    QSettings settings;
    // A client id in the settings overrides the built-in one (own Spotify
    // developer app)
    m_clientId = settings.value(kClientIdKey).toString().trimmed();
    if (m_clientId.isEmpty()) {
        m_clientId = QStringLiteral(DJSUGAR_SPOTIFY_CLIENT_ID);
    }
    m_refreshToken = settings.value(kRefreshTokenKey).toString();
    m_userName = settings.value(kUserNameKey).toString();
}

QmlSpotifyProxy::~QmlSpotifyProxy() {
    if (s_pInstance == this) {
        s_pInstance = nullptr;
    }
}

// static
QmlSpotifyProxy* QmlSpotifyProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlSpotifyProxy(pQmlEngine);
}

// static
void QmlSpotifyProxy::registerLibrary(Library* pLibrary) {
    s_pLibrary = pLibrary;
}

// static
void QmlSpotifyProxy::registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager) {
    s_pPlayerManager = std::move(pPlayerManager);
}

bool QmlSpotifyProxy::available() const {
#if defined(Q_OS_ANDROID) && defined(HAVE_SPOTIFY_BRIDGE)
    return !m_clientId.isEmpty();
#else
    return false; // the login comes from the Spotify Android app
#endif
}

bool QmlSpotifyProxy::appInstalled() const {
#ifdef Q_OS_ANDROID
    return QJniObject::callStaticMethod<jboolean>("org/mixxx/spotify/SpotifyAuthActivity",
            "isSpotifyInstalled",
            "(Landroid/content/Context;)Z",
            QNativeInterface::QAndroidApplication::context().object());
#else
    return false;
#endif
}

void QmlSpotifyProxy::login(bool browser) {
    // Not blocked while one runs: a web login left in the browser never answers
    if (!available()) {
        return;
    }
    m_loggingIn = true;
    emit loginChanged();
#ifdef Q_OS_ANDROID
    QJniObject::callStaticMethod<void>("org/mixxx/spotify/SpotifyAuthActivity",
            "login",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Z)V",
            QNativeInterface::QAndroidApplication::context().object(),
            QJniObject::fromString(m_clientId).object<jstring>(),
            QJniObject::fromString(kRedirectUri).object<jstring>(),
            static_cast<jboolean>(browser));
#else
    Q_UNUSED(browser);
#endif
}

void QmlSpotifyProxy::logout() {
    ++m_generation;
    m_accessToken.clear();
    m_refreshToken.clear();
    m_userName.clear();
    m_nextUrl.clear();
    QSettings settings;
    settings.remove(kRefreshTokenKey);
    settings.remove(kUserNameKey);
    emit loginChanged();
}

// static
void QmlSpotifyProxy::authResult(const QString& accessToken,
        const QString& refreshToken,
        int expiresIn,
        const QString& error) {
    if (!s_pInstance) {
        return;
    }
    QMetaObject::invokeMethod(
            s_pInstance,
            [=]() {
                if (s_pInstance) {
                    s_pInstance->onAuthResult(accessToken, refreshToken, expiresIn, error);
                }
            },
            Qt::QueuedConnection);
}

void QmlSpotifyProxy::onAuthResult(const QString& accessToken,
        const QString& refreshToken,
        int expiresIn,
        const QString& error) {
    m_loggingIn = false;
    if (accessToken.isEmpty()) {
        kLogger.warning() << "Spotify login failed:" << error;
        emit loginChanged();
        if (error != QStringLiteral("cancelled")) {
            emit loginFailed(error);
        }
        return;
    }
    setTokens(accessToken, refreshToken, expiresIn);
    emit loginChanged();
    fetchProfile();
}

void QmlSpotifyProxy::setTokens(
        const QString& accessToken, const QString& refreshToken, int expiresIn) {
    m_accessToken = accessToken;
    // A minute early: a request never starts with a token about to expire
    m_accessExpiry = QDateTime::currentDateTimeUtc().addSecs(std::max(60, expiresIn) - 60);
    if (!refreshToken.isEmpty()) { // the refresh token may be rotated
        m_refreshToken = refreshToken;
        QSettings().setValue(kRefreshTokenKey, m_refreshToken);
    }
}

void QmlSpotifyProxy::refreshAccessToken(std::function<void(bool)> done) {
    if (m_refreshToken.isEmpty()) {
        done(false);
        return;
    }
    m_refreshWaiters.append(std::move(done));
    if (m_refreshWaiters.size() > 1) {
        return; // a refresh is already running
    }
    QNetworkRequest request = jsonRequest(QUrl(kTokenUrl));
    request.setHeader(QNetworkRequest::ContentTypeHeader,
            QStringLiteral("application/x-www-form-urlencoded"));
    QUrlQuery form;
    form.addQueryItem(QStringLiteral("grant_type"), QStringLiteral("refresh_token"));
    form.addQueryItem(QStringLiteral("refresh_token"), m_refreshToken);
    form.addQueryItem(QStringLiteral("client_id"), m_clientId);
    QNetworkReply* pReply = m_pNetwork->post(request, form.toString(QUrl::FullyEncoded).toUtf8());
    connect(pReply, &QNetworkReply::finished, this, [this, pReply]() {
        pReply->deleteLater();
        const int status = pReply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        const QJsonObject root = QJsonDocument::fromJson(pReply->readAll()).object();
        const QString token = root.value(QStringLiteral("access_token")).toString();
        bool ok = !token.isEmpty();
        if (ok) {
            setTokens(token,
                    root.value(QStringLiteral("refresh_token")).toString(),
                    root.value(QStringLiteral("expires_in")).toInt(3600));
        } else if (status == 400) {
            // invalid_grant: access revoked in the Spotify account settings
            kLogger.warning() << "Spotify refresh token refused:"
                              << root.value(QStringLiteral("error_description")).toString();
            logout();
        }
        const auto waiters = std::exchange(m_refreshWaiters, {});
        for (const auto& waiter : waiters) {
            waiter(ok);
        }
    });
}

void QmlSpotifyProxy::fetchProfile() {
    apiGet(QUrl(kApi + QStringLiteral("/me")), [this](const QJsonObject& me) {
        m_userName = me.value(QStringLiteral("display_name")).toString();
        if (m_userName.isEmpty()) {
            m_userName = me.value(QStringLiteral("id")).toString();
        }
        QSettings().setValue(kUserNameKey, m_userName);
        emit loginChanged();
    });
}

void QmlSpotifyProxy::apiGet(const QUrl& url,
        std::function<void(const QJsonObject&)> onSuccess,
        bool retried) {
    if (!loggedIn()) {
        emit failed(tr("Log in to Spotify first"));
        return;
    }
    if (m_accessToken.isEmpty() || QDateTime::currentDateTimeUtc() >= m_accessExpiry) {
        refreshAccessToken([this, url, onSuccess, retried](bool ok) {
            if (!ok) {
                emit failed(tr("Spotify login expired, log in again"));
                return;
            }
            if (!retried) {
                apiGet(url, onSuccess, true);
            }
        });
        return;
    }
    QNetworkRequest request = jsonRequest(url);
    request.setRawHeader("Authorization", "Bearer " + m_accessToken.toUtf8());
    QNetworkReply* pReply = m_pNetwork->get(request);
    connect(pReply, &QNetworkReply::finished, this, [this, pReply, url, onSuccess, retried]() {
        pReply->deleteLater();
        const int status = pReply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        if (status == 401 && !retried) {
            m_accessToken.clear(); // revoked or expired early
            apiGet(url, onSuccess, true);
            return;
        }
        const QJsonObject root = QJsonDocument::fromJson(pReply->readAll()).object();
        if (status == 429) {
            emit failed(tr("Spotify is busy, try again in a moment"));
            return;
        }
        if (status == 403) {
            // Development-mode apps only answer users added to the app
            emit failed(tr("Spotify refused the request: %1")
                            .arg(root.value(QStringLiteral("error"))
                                            .toObject()
                                            .value(QStringLiteral("message"))
                                            .toString()));
            return;
        }
        if (pReply->error() != QNetworkReply::NoError) {
            emit failed(pReply->errorString());
            return;
        }
        onSuccess(root);
    });
}

// static
QVariantMap QmlSpotifyProxy::trackToVariant(const QJsonObject& track) {
    QVariantMap map;
    const QString id = track.value(QStringLiteral("id")).toString();
    if (id.isEmpty() || track.value(QStringLiteral("is_local")).toBool()) {
        return map; // local files of the Spotify app have nothing to find
    }
    QStringList artists;
    for (const QJsonValue& artist : track.value(QStringLiteral("artists")).toArray()) {
        artists.append(artist.toObject().value(QStringLiteral("name")).toString());
    }
    const QJsonObject album = track.value(QStringLiteral("album")).toObject();
    // Images come largest first; the smallest one of at least 64 px suits a row
    QString artwork;
    for (const QJsonValue& image : album.value(QStringLiteral("images")).toArray()) {
        const QJsonObject img = image.toObject();
        if (artwork.isEmpty() || img.value(QStringLiteral("width")).toInt() >= 64) {
            artwork = img.value(QStringLiteral("url")).toString();
        }
    }
    map[QStringLiteral("id")] = id;
    map[QStringLiteral("title")] = track.value(QStringLiteral("name")).toString();
    map[QStringLiteral("artist")] = artists.join(QStringLiteral(", "));
    map[QStringLiteral("album")] = album.value(QStringLiteral("name")).toString();
    map[QStringLiteral("durationSec")] = static_cast<int>(std::lround(
            track.value(QStringLiteral("duration_ms")).toDouble() / 1000.0));
    map[QStringLiteral("artwork")] = artwork;
    map[QStringLiteral("explicit")] = track.value(QStringLiteral("explicit")).toBool();
    map[QStringLiteral("uri")] = track.value(QStringLiteral("uri")).toString();
    return map;
}

void QmlSpotifyProxy::fetchList(const QUrl& url, ListKind kind, bool append) {
    const quint64 generation = append ? m_generation : ++m_generation;
    if (!append) {
        m_nextUrl.clear();
    }
    apiGet(url, [this, kind, append, generation](const QJsonObject& response) {
        if (generation != m_generation) {
            return; // the user opened another list meanwhile
        }
        // Search wraps the page in "tracks"; the other lists are the page
        QJsonObject page = response;
        if (response.contains(QStringLiteral("tracks")) &&
                response.value(QStringLiteral("tracks")).isObject()) {
            page = response.value(QStringLiteral("tracks")).toObject();
        }
        m_nextUrl = page.value(QStringLiteral("next")).toString();
        const QJsonArray items = page.value(QStringLiteral("items")).toArray();
        QVariantList out;
        if (kind == ListKind::Playlists) {
            for (const QJsonValue& value : items) {
                const QJsonObject playlist = value.toObject();
                const QJsonArray images = playlist.value(QStringLiteral("images")).toArray();
                // Track count: "tracks" until 2026, "items" since
                QJsonObject count = playlist.value(QStringLiteral("items")).toObject();
                if (count.isEmpty()) {
                    count = playlist.value(QStringLiteral("tracks")).toObject();
                }
                QVariantMap map;
                map[QStringLiteral("id")] = playlist.value(QStringLiteral("id")).toString();
                map[QStringLiteral("name")] = playlist.value(QStringLiteral("name")).toString();
                map[QStringLiteral("count")] = count.value(QStringLiteral("total")).toInt();
                map[QStringLiteral("artwork")] = images.isEmpty()
                        ? QString()
                        : images.last().toObject().value(QStringLiteral("url")).toString();
                out.append(map);
            }
            emit playlistsLoaded(out);
            return;
        }
        for (const QJsonValue& value : items) {
            QJsonObject track = value.toObject();
            // Saved/playlist entries wrap the track ("item" in the playlist
            // items, "track" in Liked Songs); search and top tracks are bare
            for (const auto& key : {QStringLiteral("track"), QStringLiteral("item")}) {
                if (track.value(key).isObject()) {
                    track = track.value(key).toObject();
                    break;
                }
            }
            if (track.contains(QStringLiteral("type")) &&
                    track.value(QStringLiteral("type")).toString() != QStringLiteral("track")) {
                continue; // podcast episodes in playlists
            }
            const QVariantMap map = trackToVariant(track);
            if (!map.isEmpty()) {
                out.append(map);
            }
        }
        emit tracksLoaded(out, append);
    });
}

void QmlSpotifyProxy::search(const QString& query) {
    if (query.trimmed().isEmpty()) {
        return;
    }
    QUrl url(kApi + QStringLiteral("/search"));
    QUrlQuery urlQuery;
    urlQuery.addQueryItem(QStringLiteral("q"), query.trimmed());
    urlQuery.addQueryItem(QStringLiteral("type"), QStringLiteral("track"));
    urlQuery.addQueryItem(QStringLiteral("limit"), QString::number(kSearchPage));
    url.setQuery(urlQuery);
    fetchList(url, ListKind::Tracks, false);
}

void QmlSpotifyProxy::likedSongs() {
    fetchList(QUrl(kApi + QStringLiteral("/me/tracks?limit=%1").arg(kListPage)),
            ListKind::Tracks,
            false);
}

void QmlSpotifyProxy::topTracks() {
    fetchList(QUrl(kApi +
                      QStringLiteral("/me/top/tracks?limit=%1&time_range=short_term")
                              .arg(kListPage)),
            ListKind::Tracks,
            false);
}

void QmlSpotifyProxy::playlists() {
    fetchList(QUrl(kApi + QStringLiteral("/me/playlists?limit=%1").arg(kListPage)),
            ListKind::Playlists,
            false);
}

void QmlSpotifyProxy::playlistTracks(const QString& playlistId) {
    static const QRegularExpression kId(QStringLiteral("^[0-9A-Za-z]{1,64}$"));
    if (!kId.match(playlistId).hasMatch()) {
        return;
    }
    // Since 2026 only playlists the user owns or collaborates on list their
    // items; others answer with 403
    fetchList(QUrl(kApi +
                      QStringLiteral("/playlists/%1/items?limit=%2")
                              .arg(playlistId)
                              .arg(kListPage)),
            ListKind::Tracks,
            false);
}

void QmlSpotifyProxy::loadMore() {
    if (m_nextUrl.isEmpty()) {
        return;
    }
    const QUrl next(m_nextUrl);
    m_nextUrl.clear();
    fetchList(next, ListKind::Tracks, true);
}

QString QmlSpotifyProxy::cacheDir() const {
    const QString dir = QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation))
                                .filePath(QStringLiteral("spotify_cache"));
    QDir().mkpath(dir);
    return dir;
}

void QmlSpotifyProxy::loadToDeck(const QVariantMap& track, const QString& deckGroup) {
    static const QRegularExpression kId(QStringLiteral("^[0-9A-Za-z]{22}$"));
    const QString spotifyId = track.value(QStringLiteral("id")).toString();
    if (!kId.match(spotifyId).hasMatch()) {
        return;
    }
    const QString path = QDir(cacheDir()).filePath(spotifyId + QStringLiteral(".ogg"));
    if (QFileInfo::exists(path)) {
        loadFile(track, path, deckGroup);
        return;
    }
    QList<PendingLoad>& pending = m_pendingLoads[spotifyId];
    // One load per deck: a second tap while it downloads changes nothing
    for (const PendingLoad& load : std::as_const(pending)) {
        if (load.deckGroup == deckGroup) {
            return;
        }
    }
    pending.append(PendingLoad{track, deckGroup});
    if (pending.size() == 1) {
        downloadTrack(spotifyId);
    }
}

void QmlSpotifyProxy::downloadTrack(const QString& spotifyId) {
#ifdef HAVE_SPOTIFY_BRIDGE
    m_progress.insert(spotifyId, 0.02);
    emit downloadProgressChanged(spotifyId, 0.02);
    // The audio login uses the same token: refreshed first when it expires
    // during the next minutes (a download can take that long)
    if (m_accessToken.isEmpty() ||
            QDateTime::currentDateTimeUtc().addSecs(120) >= m_accessExpiry) {
        refreshAccessToken([this, spotifyId](bool ok) {
            if (!ok) {
                onDownloaded(spotifyId, QString(), tr("Spotify login expired, log in again"));
                return;
            }
            downloadTrack(spotifyId);
        });
        return;
    }
    const QString path = QDir(cacheDir()).filePath(spotifyId + QStringLiteral(".ogg"));
    const QByteArray token = m_accessToken.toUtf8();
    const QByteArray clientId = m_clientId.toUtf8();
    const QByteArray tmpDir = QFile::encodeName(QDir::tempPath());
    QPointer<QmlSpotifyProxy> pThis(this);
    QThreadPool::globalInstance()->start([pThis, spotifyId, path, token, clientId, tmpDir]() {
        struct Context {
            QPointer<QmlSpotifyProxy> pProxy;
            QString spotifyId;
            int lastPercent = -1;
        } context{pThis, spotifyId};
        auto progress = [](void* user, double fraction) {
            auto* pContext = static_cast<Context*>(user);
            const int percent = static_cast<int>(fraction * 100);
            if (percent == pContext->lastPercent) {
                return; // a few dozen updates per track, not one per block
            }
            pContext->lastPercent = percent;
            if (!pContext->pProxy) {
                return;
            }
            QMetaObject::invokeMethod(
                    pContext->pProxy.data(),
                    [pProxy = pContext->pProxy, id = pContext->spotifyId, fraction]() {
                        if (pProxy && pProxy->m_progress.contains(id)) {
                            pProxy->m_progress.insert(id, fraction);
                            emit pProxy->downloadProgressChanged(id, fraction);
                        }
                    },
                    Qt::QueuedConnection);
        };
        char* pError = spotify_download(token.constData(),
                clientId.constData(),
                spotifyId.toUtf8().constData(),
                QFile::encodeName(path).constData(),
                tmpDir.constData(),
                progress,
                &context);
        QString error;
        if (pError) {
            error = QString::fromUtf8(pError);
            spotify_free_string(pError);
        }
        if (!pThis) {
            return;
        }
        QMetaObject::invokeMethod(
                pThis.data(),
                [pThis, spotifyId, path, error]() {
                    if (pThis) {
                        pThis->onDownloaded(spotifyId, error.isEmpty() ? path : QString(), error);
                    }
                },
                Qt::QueuedConnection);
    });
#else
    onDownloaded(spotifyId, QString(), tr("Spotify audio is not part of this build"));
#endif
}

void QmlSpotifyProxy::onDownloaded(
        const QString& spotifyId, const QString& path, const QString& error) {
    m_progress.remove(spotifyId);
    emit downloadProgressChanged(spotifyId, -1.0);
    const QList<PendingLoad> pending = m_pendingLoads.take(spotifyId);
    if (path.isEmpty()) {
        kLogger.warning() << "Spotify download of" << spotifyId << "failed:" << error;
        emit downloadFailed(spotifyId, error);
        return;
    }
    for (const PendingLoad& load : pending) {
        loadFile(load.track, path, load.deckGroup);
    }
}

void QmlSpotifyProxy::loadFile(
        const QVariantMap& track, const QString& path, const QString& deckGroup) {
    if (!s_pLibrary || !s_pPlayerManager) {
        return;
    }
    TrackPointer pTrack = s_pLibrary->trackCollectionManager()->getOrAddTrack(
            TrackRef::fromFilePath(path));
    if (!pTrack) {
        emit downloadFailed(track.value(QStringLiteral("id")).toString(),
                tr("Could not add the track to the library"));
        return;
    }
    // The file has no tags: the library row shows Spotify's
    pTrack->setTitle(track.value(QStringLiteral("title")).toString());
    pTrack->setArtist(track.value(QStringLiteral("artist")).toString());
    pTrack->setAlbum(track.value(QStringLiteral("album")).toString());
    pTrack->setComment(track.value(QStringLiteral("uri")).toString());
#ifdef __STEM__
    s_pPlayerManager->slotLoadTrackToPlayer(
            pTrack, deckGroup, mixxx::StemChannelSelection(), false);
#else
    s_pPlayerManager->slotLoadTrackToPlayer(pTrack, deckGroup, false);
#endif
}

} // namespace qml
} // namespace mixxx

#ifdef Q_OS_ANDROID
extern "C" JNIEXPORT void JNICALL
Java_org_mixxx_spotify_SpotifyAuthActivity_nativeAuthResult(JNIEnv* env,
        jclass,
        jstring accessToken,
        jstring refreshToken,
        jint expiresIn,
        jstring error) {
    auto toQString = [env](jstring value) {
        if (!value) {
            return QString();
        }
        const char* chars = env->GetStringUTFChars(value, nullptr);
        QString out = QString::fromUtf8(chars);
        env->ReleaseStringUTFChars(value, chars);
        return out;
    };
    mixxx::qml::QmlSpotifyProxy::authResult(toQString(accessToken),
            toQString(refreshToken),
            static_cast<int>(expiresIn),
            toQString(error));
}
#endif
