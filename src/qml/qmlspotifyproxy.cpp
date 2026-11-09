#include "qml/qmlspotifyproxy.h"

#include <QDesktopServices>
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
#include <thread>

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
#include <QJniObject>
#endif

namespace {
const mixxx::Logger kLogger("QmlSpotifyProxy");

const QString kApi = QStringLiteral("https://api.spotify.com/v1");
const QString kUserNameKey = QStringLiteral("Spotify/userName");
const QString kSpotifyPackage = QStringLiteral("com.spotify.music");
// Shown in the Spotify app's device list for the login from the app
const QString kDeviceName = QStringLiteral("DJ Sugar");
constexpr int kSearchPage = 20;
constexpr int kListPage = 50;

QNetworkRequest jsonRequest(const QUrl& url) {
    QNetworkRequest request(url);
    request.setTransferTimeout(15000);
    return request;
}

#ifdef HAVE_SPOTIFY_BRIDGE
/// Takes a string handed out by the bridge
QString take(char* pString) {
    if (!pString) {
        return QString();
    }
    QString out = QString::fromUtf8(pString);
    spotify_free_string(pString);
    return out;
}
#endif

#ifdef Q_OS_ANDROID
QJniObject androidContext() {
    return QJniObject(QNativeInterface::QAndroidApplication::context());
}

QJniObject spotifyLaunchIntent() {
    QJniObject packageManager = androidContext().callObjectMethod(
            "getPackageManager", "()Landroid/content/pm/PackageManager;");
    if (!packageManager.isValid()) {
        return QJniObject();
    }
    return packageManager.callObjectMethod("getLaunchIntentForPackage",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            QJniObject::fromString(kSpotifyPackage).object<jstring>());
}

/// Android drops multicast (the Spotify Connect discovery) unless an app
/// holds this lock
class MulticastLock {
  public:
    MulticastLock() {
        QJniObject wifi = androidContext().callObjectMethod("getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                QJniObject::fromString(QStringLiteral("wifi")).object<jstring>());
        if (!wifi.isValid()) {
            return;
        }
        m_lock = wifi.callObjectMethod("createMulticastLock",
                "(Ljava/lang/String;)Landroid/net/wifi/WifiManager$MulticastLock;",
                QJniObject::fromString(QStringLiteral("djsugar-spotify")).object<jstring>());
        if (m_lock.isValid()) {
            m_lock.callMethod<void>("acquire");
        }
    }
    ~MulticastLock() {
        if (m_lock.isValid()) {
            m_lock.callMethod<void>("release");
        }
    }

  private:
    QJniObject m_lock;
};
#endif
} // namespace

namespace mixxx {
namespace qml {

QmlSpotifyProxy::QmlSpotifyProxy(QObject* pParent)
        : QObject(pParent),
          m_pNetwork(new QNetworkAccessManager(this)) {
#ifdef HAVE_SPOTIFY_BRIDGE
    m_loggedIn = spotify_logged_in(QFile::encodeName(accountDir()).constData());
#endif
    if (m_loggedIn) {
        m_userName = QSettings().value(kUserNameKey).toString();
    }
}

QmlSpotifyProxy::~QmlSpotifyProxy() {
    cancelLogin();
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
#ifdef HAVE_SPOTIFY_BRIDGE
    return true;
#else
    return false;
#endif
}

bool QmlSpotifyProxy::appInstalled() const {
#ifdef Q_OS_ANDROID
    return spotifyLaunchIntent().isValid();
#else
    return false;
#endif
}

QString QmlSpotifyProxy::accountDir() {
    const QString dir = QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation))
                                .filePath(QStringLiteral("spotify"));
    QDir().mkpath(dir);
    return dir;
}

void QmlSpotifyProxy::login(bool browser) {
#ifdef HAVE_SPOTIFY_BRIDGE
    if (m_loggingIn) {
        cancelLogin(); // another way picked while one waits
    }
    m_loggingIn = true;
    m_loginStep = browser ? QStringLiteral("web") : QStringLiteral("app");
    emit loginChanged();
    const QByteArray dir = QFile::encodeName(accountDir());
    const QByteArray name = kDeviceName.toUtf8();
    const quint64 attempt = ++m_loginAttempt;
    QPointer<QmlSpotifyProxy> pThis(this);
    std::thread([pThis, dir, name, browser, attempt]() {
        char* pUser = nullptr;
        char* pError;
        if (browser) {
            auto openUrl = [](void* pUser, const char* pUrl) {
                const QUrl url(QString::fromUtf8(pUrl));
                auto* pProxy = static_cast<QPointer<QmlSpotifyProxy>*>(pUser);
                if (*pProxy) {
                    QMetaObject::invokeMethod(
                            pProxy->data(),
                            [url]() { QDesktopServices::openUrl(url); },
                            Qt::QueuedConnection);
                }
            };
            QPointer<QmlSpotifyProxy> pProxy = pThis;
            pError = spotify_login_web(dir.constData(), openUrl, &pProxy, &pUser);
        } else {
#ifdef Q_OS_ANDROID
            MulticastLock multicast;
#endif
            pError = spotify_login_app(dir.constData(), name.constData(), &pUser);
        }
        const QString error = take(pError);
        const QString user = take(pUser);
        if (pThis) {
            QMetaObject::invokeMethod(
                    pThis.data(),
                    [pThis, user, error, attempt]() {
                        if (pThis && attempt == pThis->m_loginAttempt) {
                            pThis->onLoginResult(user, error);
                        }
                    },
                    Qt::QueuedConnection);
        }
    }).detach();
    if (!browser) {
        openSpotifyApp();
    }
#else
    Q_UNUSED(browser);
#endif
}

void QmlSpotifyProxy::openSpotifyApp() {
#ifdef Q_OS_ANDROID
    QJniObject intent = spotifyLaunchIntent();
    if (!intent.isValid()) {
        return;
    }
    intent.callObjectMethod("addFlags", "(I)Landroid/content/Intent;", jint(0x10000000));
    androidContext().callMethod<void>("startActivity", "(Landroid/content/Intent;)V", intent.object());
#endif
}

void QmlSpotifyProxy::cancelLogin() {
#ifdef HAVE_SPOTIFY_BRIDGE
    if (m_loggingIn) {
        spotify_cancel_login();
    }
#endif
    ++m_loginAttempt;
    if (m_loggingIn) {
        m_loggingIn = false;
        m_loginStep.clear();
        emit loginChanged();
    }
}

void QmlSpotifyProxy::onLoginResult(const QString& user, const QString& error) {
    m_loggingIn = false;
    m_loginStep.clear();
    if (!error.isEmpty()) {
        kLogger.warning() << "Spotify login failed:" << error;
        emit loginChanged();
        if (error != QStringLiteral("cancelled")) {
            emit loginFailed(error);
        }
        return;
    }
    m_loggedIn = true;
    m_userName = user;
    m_accessToken.clear();
    emit loginChanged();
    fetchProfile();
}

void QmlSpotifyProxy::logout() {
    cancelLogin();
    ++m_generation;
    m_loggedIn = false;
    m_accessToken.clear();
    m_userName.clear();
    m_nextUrl.clear();
#ifdef HAVE_SPOTIFY_BRIDGE
    spotify_logout(QFile::encodeName(accountDir()).constData());
#endif
    QSettings().remove(kUserNameKey);
    emit loginChanged();
}

void QmlSpotifyProxy::refreshAccessToken(std::function<void(bool)> done) {
    m_refreshWaiters.append(std::move(done));
    if (m_refreshWaiters.size() > 1) {
        return; // a refresh is already running
    }
    auto finish = [this](const QString& token, int expiresIn, const QString& error) {
        const bool ok = !token.isEmpty();
        if (ok) {
            m_accessToken = token;
            // A minute early: a request never starts with a token about to expire
            m_accessExpiry = QDateTime::currentDateTimeUtc().addSecs(std::max(60, expiresIn) - 60);
        } else {
            kLogger.warning() << "Spotify token:" << error;
        }
        const auto waiters = std::exchange(m_refreshWaiters, {});
        for (const auto& waiter : waiters) {
            waiter(ok);
        }
    };
#ifdef HAVE_SPOTIFY_BRIDGE
    const QByteArray dir = QFile::encodeName(accountDir());
    QPointer<QmlSpotifyProxy> pThis(this);
    QThreadPool::globalInstance()->start([pThis, dir, finish]() {
        char* pToken = nullptr;
        int32_t expiresIn = 0;
        const QString error = take(spotify_web_token(dir.constData(), &pToken, &expiresIn));
        const QString token = take(pToken);
        if (pThis) {
            QMetaObject::invokeMethod(
                    pThis.data(),
                    [pThis, finish, token, expiresIn, error]() {
                        if (pThis) {
                            finish(token, expiresIn, error);
                        }
                    },
                    Qt::QueuedConnection);
        }
    });
#else
    finish(QString(), 0, QStringLiteral("not part of this build"));
#endif
}

void QmlSpotifyProxy::fetchProfile() {
    apiGet(QUrl(kApi + QStringLiteral("/me")), [this](const QJsonObject& me) {
        const QString name = me.value(QStringLiteral("display_name")).toString();
        if (!name.isEmpty()) {
            m_userName = name;
        }
        QSettings().setValue(kUserNameKey, m_userName);
        emit loginChanged();
    });
}

void QmlSpotifyProxy::apiGet(const QUrl& url,
        std::function<void(const QJsonObject&)> onSuccess,
        bool retried) {
    if (!m_loggedIn) {
        emit failed(tr("Log in to Spotify first"));
        return;
    }
    if (m_accessToken.isEmpty() || QDateTime::currentDateTimeUtc() >= m_accessExpiry) {
        refreshAccessToken([this, url, onSuccess, retried](bool ok) {
            if (!ok) {
                emit failed(tr("Spotify did not answer, try again"));
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
    const QString path = QDir(cacheDir()).filePath(spotifyId + QStringLiteral(".ogg"));
    const QByteArray dir = QFile::encodeName(accountDir());
    QPointer<QmlSpotifyProxy> pThis(this);
    QThreadPool::globalInstance()->start([pThis, spotifyId, path, dir]() {
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
        const QString error = take(spotify_download(dir.constData(),
                spotifyId.toUtf8().constData(),
                QFile::encodeName(path).constData(),
                progress,
                &context));
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
