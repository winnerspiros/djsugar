#include "qml/qmlspotifyproxy.h"

#include <QCoreApplication>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QPointer>
#include <QRegularExpression>
#include <QSettings>
#include <QStandardPaths>
#include <QThreadPool>
#include <thread>

#include "library/library.h"
#include "library/trackcollectionmanager.h"
#include "library/youtube/youtubeaccount.h"
#include "library/youtube/youtubefeature.h"
#include "library/youtube/youtubeservice.h"
#include "mixer/playermanager.h"
#include "moc_qmlspotifyproxy.cpp"
#include "track/track.h"
#include "util/logger.h"

#ifdef HAVE_SPOTIFY_BRIDGE
#include "spotifybridge.h"
#endif

#ifdef Q_OS_ANDROID
#include <jni.h>

#include <QJniObject>
#endif
namespace {
const mixxx::Logger kLogger("QmlSpotifyProxy");

const QString kUserNameKey = QStringLiteral("Spotify/userName");
/// Tracks per page of a list
constexpr int kPage = 50;

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

} // namespace

namespace mixxx {
namespace qml {

QmlSpotifyProxy::QmlSpotifyProxy(QObject* pParent)
        : QObject(pParent) {
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

QString QmlSpotifyProxy::accountDir() {
    const QString dir = QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation))
                                .filePath(QStringLiteral("spotify"));
    QDir().mkpath(dir);
    return dir;
}

void QmlSpotifyProxy::login() {
#ifdef HAVE_SPOTIFY_BRIDGE
    if (m_loggingIn) {
        cancelLogin(); // "open the login page again"
    }
    m_loggingIn = true;
    emit loginChanged();
    const QByteArray dir = QFile::encodeName(accountDir());
    const quint64 attempt = ++m_loginAttempt;
    QPointer<QmlSpotifyProxy> pThis(this);
    std::thread([pThis, dir, attempt]() {
        // Android: inside the app (SpotifyLoginActivity), so it stays in
        // front and keeps its network; elsewhere the browser
        auto openUrl = [](void*, const char* pUrl) {
            const QString url = QString::fromUtf8(pUrl);
            QMetaObject::invokeMethod(
                    qApp,
                    [url]() {
#ifdef Q_OS_ANDROID
                        QJniObject::callStaticMethod<void>(
                                "org/mixxx/spotify/SpotifyLoginActivity",
                                "start",
                                "(Landroid/content/Context;Ljava/lang/String;)V",
                                QNativeInterface::QAndroidApplication::context(),
                                QJniObject::fromString(url).object<jstring>());
#else
                        QDesktopServices::openUrl(QUrl(url));
#endif
                    },
                    Qt::QueuedConnection);
        };
        char* pUser = nullptr;
        const QString error = take(spotify_login_web(dir.constData(), openUrl, nullptr, &pUser));
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
        emit loginChanged();
    }
}

void QmlSpotifyProxy::onLoginResult(const QString& user, const QString& error) {
    m_loggingIn = false;
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
    emit loginChanged();
    fetchProfile();
}

void QmlSpotifyProxy::logout() {
    cancelLogin();
    ++m_generation;
    ++m_accountEpoch;
    m_loggedIn = false;
    m_userName.clear();
    m_listRequest.clear();
    m_nextOffset = -1;
#ifdef HAVE_SPOTIFY_BRIDGE
    spotify_logout(QFile::encodeName(accountDir()).constData());
#endif
    QSettings().remove(kUserNameKey);
    emit loginChanged();
}

void QmlSpotifyProxy::fetchProfile() {
    browse(QStringLiteral("profile"), 0, [this](const QJsonObject& profile) {
        const QString name = profile.value(QStringLiteral("name")).toString();
        if (!name.isEmpty()) {
            m_userName = name;
        }
        QSettings().setValue(kUserNameKey, m_userName);
        emit loginChanged();
    });
}

void QmlSpotifyProxy::browse(const QString& request,
        int offset,
        std::function<void(const QJsonObject&)> onSuccess,
        std::function<void()> onFailure) {
    if (!m_loggedIn) {
        emit failed(tr("Log in to Spotify first"));
        return;
    }
#ifdef HAVE_SPOTIFY_BRIDGE
    const QByteArray dir = QFile::encodeName(accountDir());
    const QByteArray requestUtf8 = request.toUtf8();
    QPointer<QmlSpotifyProxy> pThis(this);
    const quint64 epoch = m_accountEpoch;
    QThreadPool::globalInstance()->start(
            [pThis, dir, requestUtf8, offset, onSuccess, onFailure, epoch]() {
                char* pJson = nullptr;
                const QString error = take(spotify_browse(
                        dir.constData(), requestUtf8.constData(), offset, kPage, &pJson));
                const QByteArray json = take(pJson).toUtf8();
                if (!pThis) {
                    return;
                }
                QMetaObject::invokeMethod(
                        pThis.data(),
                        [pThis, error, json, onSuccess, onFailure, epoch]() {
                            if (!pThis || epoch != pThis->m_accountEpoch) {
                                return; // gone, or the answer of a logged-out account
                            }
                            if (!error.isEmpty()) {
                                kLogger.warning() << "Spotify browsing:" << error;
                                if (error.contains(QStringLiteral("INVALID_CREDENTIALS"))) {
                                    // The stored login is refused: log in again (once
                                    // for all the lists that were loading)
                                    if (pThis->m_loggedIn) {
                                        pThis->logout();
                                        emit pThis->loginFailed(tr("Spotify wants a new login"));
                                    }
                                    return;
                                }
                                if (onFailure) {
                                    onFailure();
                                }
                                emit pThis->failed(error);
                                return;
                            }
                            onSuccess(QJsonDocument::fromJson(json).object());
                        },
                        Qt::QueuedConnection);
            });
#else
    Q_UNUSED(offset);
    Q_UNUSED(onSuccess);
    Q_UNUSED(onFailure);
    emit failed(tr("Spotify is not part of this build"));
#endif
}

void QmlSpotifyProxy::fetchList(const QString& request, ListKind kind, bool append) {
    const quint64 generation = append ? m_generation : ++m_generation;
    const int offset = append ? m_nextOffset : 0;
    m_listRequest = request;
    m_nextOffset = -1;
    browse(
            request,
            offset,
            [this, kind, append, generation](const QJsonObject& page) {
                if (generation != m_generation) {
                    return; // the user opened another list meanwhile
                }
                m_nextOffset = page.value(QStringLiteral("next")).toInt(-1);
                const QVariantList items = page.value(QStringLiteral("items"))
                                                   .toArray()
                                                   .toVariantList();
                if (kind == ListKind::Playlists) {
                    emit playlistsLoaded(items);
                } else {
                    emit tracksLoaded(items, append);
                }
            },
            [this, append, generation, offset]() {
                // The page can be asked for again
                if (append && generation == m_generation) {
                    m_nextOffset = offset;
                }
            });
}

void QmlSpotifyProxy::search(const QString& query) {
    if (query.trimmed().isEmpty()) {
        return;
    }
    fetchList(QStringLiteral("search:") + query.trimmed(), ListKind::Tracks, false);
}

void QmlSpotifyProxy::likedSongs() {
    fetchList(QStringLiteral("liked"), ListKind::Tracks, false);
}

void QmlSpotifyProxy::playlists() {
    fetchList(QStringLiteral("playlists"), ListKind::Playlists, false);
}

void QmlSpotifyProxy::playlistTracks(const QString& playlistId) {
    static const QRegularExpression kId(QStringLiteral("^[0-9A-Za-z]{22}$"));
    if (!kId.match(playlistId).hasMatch()) {
        return;
    }
    fetchList(QStringLiteral("playlist:") + playlistId, ListKind::Tracks, false);
}

void QmlSpotifyProxy::loadMore() {
    if (m_nextOffset < 0) {
        return;
    }
    fetchList(m_listRequest, ListKind::Tracks, true);
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
        if (m_spotifyRefused) {
            loadFromYouTube(spotifyId, m_pendingLoads.take(spotifyId));
        } else {
            downloadTrack(spotifyId);
        }
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
        kLogger.warning() << "Spotify download of" << spotifyId << "failed:" << error
                          << "- loading it from YouTube Music";
        m_spotifyRefused = true;
        loadFromYouTube(spotifyId, pending);
        return;
    }
    for (const PendingLoad& load : pending) {
        loadFile(load.track, path, load.deckGroup);
    }
}

void QmlSpotifyProxy::loadFromYouTube(
        const QString& spotifyId, const QList<PendingLoad>& pending) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pending.isEmpty()) {
        return;
    }
    if (!pFeature) {
        emit downloadFailed(spotifyId, tr("This song could not be loaded"));
        return;
    }
    if (!m_youTubeConnected) {
        m_youTubeConnected = true;
        // Its download shows as the Spotify row's
        mixxx::YouTubeService* pService = pFeature->service();
        connect(pService,
                &mixxx::YouTubeService::downloadProgress,
                this,
                [this](const QString& videoId, double progress) {
                    if (const QString id = m_fallbacks.value(videoId); !id.isEmpty()) {
                        m_progress.insert(id, progress);
                        emit downloadProgressChanged(id, progress);
                    }
                });
        auto finish = [this](const QString& videoId, const QString& path = QString()) {
            if (const QString id = m_fallbacks.take(videoId); !id.isEmpty()) {
                m_progress.remove(id);
                emit downloadProgressChanged(id, -1.0);
                if (!path.isEmpty()) {
                    emit fileReady(id, path);
                }
            }
        };
        connect(pService, &mixxx::YouTubeService::downloadFinished, this, finish);
        connect(pService,
                &mixxx::YouTubeService::downloadFailed,
                this,
                [this, finish](const QString& videoId, const QString&) {
                    if (const QString id = m_fallbacks.value(videoId); !id.isEmpty()) {
                        finish(videoId);
                        emit downloadFailed(id, tr("This song could not be loaded"));
                    }
                });
    }
    const QVariantMap& track = pending.first().track;
    const QString title = track.value(QStringLiteral("title")).toString();
    const QString artist = track.value(QStringLiteral("artist")).toString();
    m_progress.insert(spotifyId, 0.02);
    emit downloadProgressChanged(spotifyId, 0.02);
    mixxx::YouTubeAccount::instance().findSong(artist + QLatin1Char(' ') + title,
            track.value(QStringLiteral("durationSec")).toInt(),
            this,
            [this, spotifyId, pending, title, artist](const QString& videoId) {
                YouTubeFeature* pFeature =
                        s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
                if (videoId.isEmpty() || !pFeature) {
                    m_progress.remove(spotifyId);
                    emit downloadProgressChanged(spotifyId, -1.0);
                    emit downloadFailed(spotifyId, tr("This song could not be found"));
                    return;
                }
                // The library row gets Spotify's title and artist
                mixxx::YouTubeVideoInfo info;
                info.id = videoId;
                info.title = title;
                info.uploader = artist;
                pFeature->rememberExternalResults({info});
                m_fallbacks.insert(videoId, spotifyId);
                for (const PendingLoad& load : pending) {
                    if (load.deckGroup.isEmpty()) {
                        pFeature->requestDownload(videoId);
                    } else {
                        pFeature->requestDownloadToPlayer(videoId, load.deckGroup, false);
                    }
                }
            });
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
    emit fileReady(track.value(QStringLiteral("id")).toString(), path);
    if (deckGroup.isEmpty()) {
        return;
    }
#ifdef __STEM__
    s_pPlayerManager->slotLoadTrackToPlayer(
            pTrack, deckGroup, mixxx::StemChannelSelection(), false);
#else
    s_pPlayerManager->slotLoadTrackToPlayer(pTrack, deckGroup, false);
#endif
}

} // namespace qml
} // namespace mixxx

#if defined(Q_OS_ANDROID) && defined(HAVE_SPOTIFY_BRIDGE)
/// SpotifyLoginActivity: the in-app login page reached the redirect URL, or
/// was closed ("closed:<last page>")
extern "C" JNIEXPORT void JNICALL
Java_org_mixxx_spotify_SpotifyLoginActivity_nativeRedirect(JNIEnv* pEnv, jclass, jstring url) {
    const char* pUrl = pEnv->GetStringUTFChars(url, nullptr);
    if (qstrncmp(pUrl, "closed:", 7) == 0) {
        kLogger.info() << "Spotify login page closed at" << (pUrl + 7);
        spotify_cancel_login();
    } else {
        spotify_login_redirect(pUrl);
    }
    pEnv->ReleaseStringUTFChars(url, pUrl);
}
#endif
