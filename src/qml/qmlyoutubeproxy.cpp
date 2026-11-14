#include "qml/qmlyoutubeproxy.h"

#include <QCoreApplication>
#include <QDebug>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QRegularExpression>
#include <QSettings>
#include <QTimer>
#include <QUrlQuery>
#include <algorithm>
#include <utility>

#ifdef Q_OS_ANDROID
#include <jni.h>

#include <QJniObject>
#endif

#include "library/library.h"
#include "library/youtube/soundcloudaccount.h"
#include "library/youtube/youtubeaccount.h"
#include "library/youtube/youtubefeature.h"
#include "library/youtube/youtubeservice.h"
#include "mixer/playermanager.h"
#include "moc_qmlyoutubeproxy.cpp"

namespace mixxx {
namespace qml {

using mixxx::YouTubeAccount;

QmlYouTubeProxy::QmlYouTubeProxy(QObject* pParent)
        : QObject(pParent) {
    s_pInstance = this;
    if (!soundcloud::token().isEmpty()) {
        QTimer::singleShot(0, this, [this]() { fetchSoundCloudMe({}); });
    }
    YouTubeAccount& account = YouTubeAccount::instance();
    connect(&account, &YouTubeAccount::accountChanged, this, &QmlYouTubeProxy::accountsChanged);
    connect(&account, &YouTubeAccount::failed, this, &QmlYouTubeProxy::accountFailed);
    connect(&account,
            &YouTubeAccount::playlistsReady,
            this,
            &QmlYouTubeProxy::youTubePlaylistsLoaded);
    connect(&account,
            &YouTubeAccount::accountsReady,
            this,
            &QmlYouTubeProxy::youTubeAccountsLoaded);
    connect(&account,
            &YouTubeAccount::tracksReady,
            this,
            [this](const QList<mixxx::YouTubeVideoInfo>& tracks, bool append) {
                if (!m_accountList) {
                    return; // a search replaced the list
                }
                for (const auto& v : tracks) {
                    m_videoTitles.insert(v.id, v.title);
                }
                if (s_pLibrary) {
                    if (YouTubeFeature* pFeature = s_pLibrary->youtubeFeature()) {
                        pFeature->rememberExternalResults(tracks);
                    }
                }
                prefetchCovers(tracks);
                if (append) {
                    emit searchMoreFinished(toVariantList(tracks),
                            YouTubeAccount::instance().hasMore());
                } else {
                    emit searchFinished(toVariantList(tracks));
                }
                queueGenreLookups(tracks);
            });
    YouTubeFeature* pFeature = s_pLibrary
            ? s_pLibrary->youtubeFeature()
            : nullptr;
    if (!pFeature) {
        return;
    }
    connect(pFeature,
            &YouTubeFeature::downloadToPlayerRequested,
            this,
            [this](const QString& videoId, const QString& group, const QString& title) {
                if (!group.isEmpty()) {
                    m_videoToDeck.insert(videoId, group);
                }
                if (!title.isEmpty()) {
                    m_videoTitles.insert(videoId, title);
                }
                updateVideoProgress(
                        videoId,
                        m_downloadProgressByVideo.contains(videoId)
                                ? m_downloadProgressByVideo.value(videoId).toDouble()
                                : 0.04);
            });

    connect(pFeature, &YouTubeFeature::fileReady, this, &QmlYouTubeProxy::fileReady);
    connect(pFeature,
            &YouTubeFeature::playerLoadsSettled,
            this,
            [this](const QString& videoId) {
                // A cached track loads without any download event, so the
                // progress shown for its deck button is cleared here.
                updateVideoProgress(videoId, 1.0);
                QTimer::singleShot(350, this, [this, videoId]() {
                    clearVideoProgress(videoId);
                });
            });

    mixxx::YouTubeService* pService = pFeature->service();
    connect(pService,
            &mixxx::YouTubeService::searchResultsReady,
            this,
            [this](const QString&, const QList<mixxx::YouTubeVideoInfo>& results) {
                if (m_accountList) {
                    return; // the account's list is open, not a search
                }
                for (const auto& v : results) {
                    if (!v.id.isEmpty() && !v.title.isEmpty()) {
                        m_videoTitles.insert(v.id, v.title);
                    }
                }
                prefetchCovers(results);
                m_genreQueue.clear();
                m_genreQueued.clear();
                emit searchFinished(toVariantList(results));
                queueGenreLookups(results);
            });
    connect(pService,
            &mixxx::YouTubeService::searchMoreReady,
            this,
            [this, pFeature, pService](const QString& query,
                    const QList<mixxx::YouTubeVideoInfo>& results) {
                if (query != pFeature->lastQuery()) {
                    return; // superseded by a newer search
                }
                for (const auto& v : results) {
                    if (!v.id.isEmpty() && !v.title.isEmpty()) {
                        m_videoTitles.insert(v.id, v.title);
                    }
                }
                prefetchCovers(results);
                emit searchMoreFinished(toVariantList(results), pService->hasMoreSearchResults());
                queueGenreLookups(results);
            });
    connect(pService,
            &mixxx::YouTubeService::searchFailed,
            this,
            [this](const QString&, const QString& error) {
                emit searchFailed(error);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadStarted,
            this,
            [this](const QString& videoId) {
                updateVideoProgress(videoId, 0.04);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadProgress,
            this,
            [this](const QString& videoId, double progress) {
                updateVideoProgress(videoId, progress);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadFinished,
            this,
            [this](const QString& videoId, const QString& localPath) {
                updateVideoProgress(videoId, 1.0);
                QTimer::singleShot(350, this, [this, videoId]() {
                    clearVideoProgress(videoId);
                });
                // The deck load itself is done by YouTubeFeature, which holds
                // the pending load per deck. Loading here as well would hit
                // the same deck twice within the double-tap window, and the
                // second load then clones the other deck instead.
                emit downloadFinished(videoId, localPath);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadFailed,
            this,
            [this](const QString& videoId, const QString& error) {
                clearVideoProgress(videoId);
                emit downloadFailed(videoId, error);
            });
}

// static
QmlYouTubeProxy* QmlYouTubeProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlYouTubeProxy(pQmlEngine);
}

// static
void QmlYouTubeProxy::registerLibrary(Library* pLibrary) {
    s_pLibrary = pLibrary;
}

// static
void QmlYouTubeProxy::registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager) {
    s_pPlayerManager = std::move(pPlayerManager);
}

// static
QString QmlYouTubeProxy::extractVideoId(const QString& videoIdOrUrl) {
    if (videoIdOrUrl.isEmpty()) {
        return {};
    }
    if (videoIdOrUrl.startsWith(QStringLiteral("youtube://"))) {
        QString id = videoIdOrUrl.mid(10);
        while (id.endsWith(QLatin1Char('/'))) {
            id.chop(1);
        }
        return id;
    }
    if (videoIdOrUrl.contains(QLatin1Char('/'))) {
        return QFileInfo(videoIdOrUrl).completeBaseName();
    }
    return videoIdOrUrl;
}

void QmlYouTubeProxy::updateVideoProgress(const QString& videoId, double progress) {
    if (videoId.isEmpty()) {
        return;
    }
    const double clamped = std::max(0.02, std::min(1.0, progress));
    m_downloadProgressByVideo.insert(videoId, clamped);

    const QString deck = m_videoToDeck.value(videoId);
    if (!deck.isEmpty()) {
        QVariantMap deckInfo;
        deckInfo[QStringLiteral("videoId")] = videoId;
        deckInfo[QStringLiteral("title")] = m_videoTitles.value(videoId, videoId);
        deckInfo[QStringLiteral("progress")] = clamped;
        m_deckDownloadProgress.insert(deck, deckInfo);
    }
    emit downloadProgressChanged(videoId, clamped);
}

void QmlYouTubeProxy::clearVideoProgress(const QString& videoId) {
    if (videoId.isEmpty()) {
        return;
    }
    m_downloadProgressByVideo.remove(videoId);
    const QString deck = m_videoToDeck.take(videoId);
    if (!deck.isEmpty()) {
        m_deckDownloadProgress.remove(deck);
    }
    emit downloadProgressChanged(videoId, -1.0);
}

double QmlYouTubeProxy::videoProgress(const QString& videoIdOrUrl) const {
    const QString id = extractVideoId(videoIdOrUrl);
    if (id.isEmpty() || !m_downloadProgressByVideo.contains(id)) {
        return -1.0;
    }
    return m_downloadProgressByVideo.value(id).toDouble();
}

double QmlYouTubeProxy::deckProgress(const QString& deckGroup) const {
    if (deckGroup.isEmpty() || !m_deckDownloadProgress.contains(deckGroup)) {
        return -1.0;
    }
    return m_deckDownloadProgress.value(deckGroup)
            .toMap()
            .value(QStringLiteral("progress"), -1.0)
            .toDouble();
}

QString QmlYouTubeProxy::deckDownloadingTitle(const QString& deckGroup) const {
    if (deckGroup.isEmpty() || !m_deckDownloadProgress.contains(deckGroup)) {
        return {};
    }
    return m_deckDownloadProgress.value(deckGroup)
            .toMap()
            .value(QStringLiteral("title"))
            .toString();
}

void QmlYouTubeProxy::warmUp() {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        pFeature->service()->warmUpConnection();
    }
}

void QmlYouTubeProxy::search(const QString& query) {
    m_accountList = false;
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        pFeature->searchAndActivate(query);
    } else {
        emit searchFailed(tr("YouTube is not available"));
    }
}

void QmlYouTubeProxy::loadMore() {
    if (m_accountList) {
        YouTubeAccount::instance().more();
        return;
    }
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        pFeature->loadMoreResults();
    } else {
        emit searchMoreFinished({}, false);
    }
}

bool QmlYouTubeProxy::hasMore() const {
    if (m_accountList) {
        return YouTubeAccount::instance().hasMore();
    }
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    return pFeature && pFeature->service()->hasMoreSearchResults();
}

void QmlYouTubeProxy::download(const QString& videoId) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (!pFeature) {
        emit downloadFailed(videoId, tr("YouTube is not available"));
        return;
    }
    // Download only: the file lands in the YouTube cache and library without
    // touching any deck. Progress comes from the service's downloadStarted.
    pFeature->requestDownload(videoId);
}

void QmlYouTubeProxy::downloadToDeck(const QString& videoId, const QString& deckGroup) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (!pFeature) {
        emit downloadFailed(videoId, tr("YouTube is not available"));
        return;
    }
    m_videoToDeck.insert(videoId, deckGroup);
    updateVideoProgress(videoId, 0.04);
    pFeature->requestDownloadToPlayer(videoId, deckGroup, false);
}

void QmlYouTubeProxy::prefetchCovers(const QList<mixxx::YouTubeVideoInfo>& results) {
    // Cover art for the deck: a track played straight from the search list
    // needs its thumbnail on disk (the backfill then sets it on the deck).
    if (s_pLibrary) {
        if (YouTubeFeature* pFeature = s_pLibrary->youtubeFeature()) {
            pFeature->prefetchThumbnails(results);
            // Extract the stream links of the first results, so a tap skips
            // yt-dlp and starts the 16-way download straight away. The top 3
            // used to be downloaded too, but that lagged the app while
            // browsing; links (and SponsorBlock) are small and quick.
            constexpr int kPrefetchLinks = 10;
            QStringList linkIds;
            for (const auto& v : results) {
                if (v.id.isEmpty() || v.durationSec >= 15 * 60) {
                    continue;
                }
                if (linkIds.size() >= kPrefetchLinks) {
                    break;
                }
                linkIds.append(v.id);
            }
            pFeature->service()->prefetchStreamLinks(linkIds);
        }
    }
}

void QmlYouTubeProxy::prefetchLink(const QString& videoId) {
    if (YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr) {
        pFeature->service()->prefetchStreamLinks({videoId});
    }
}

void QmlYouTubeProxy::queueGenreLookups(const QList<mixxx::YouTubeVideoInfo>& results) {
    for (const auto& v : results) {
        if (v.id.isEmpty() || m_genreQueued.contains(v.id)) {
            continue;
        }
        m_genreQueued.insert(v.id);
        const auto cached = m_genreCache.constFind(v.id);
        if (cached != m_genreCache.constEnd()) {
            if (!cached->isEmpty()) {
                emit genreResolved(v.id, *cached);
            }
            continue;
        }
        m_genreQueue.enqueue(v);
    }
    pumpGenreLookups();
}

void QmlYouTubeProxy::pumpGenreLookups() {
    // A few at a time: the catalogue APIs rate-limit bursts.
    constexpr int kMaxRunning = 3;
    if (!m_pGenreLookup) {
        m_pGenreLookup = new mixxx::YouTubeGenreLookup(this);
    }
    while (m_genreLookupsRunning < kMaxRunning && !m_genreQueue.isEmpty()) {
        const mixxx::YouTubeVideoInfo v = m_genreQueue.dequeue();
        // "Artist - Title (Official Video)" or the channel as the artist.
        QString artist = v.uploader;
        QString title = v.title;
        for (const QString& sep : {QStringLiteral(" - "),
                     QStringLiteral(" – "),
                     QStringLiteral(" — ")}) {
            const int pos = v.title.indexOf(sep);
            if (pos > 0) {
                artist = v.title.left(pos);
                title = v.title.mid(pos + sep.size());
                break;
            }
        }
        artist.remove(QRegularExpression(QStringLiteral(R"(\s*(-\s*Topic|VEVO|Official)\s*$)"),
                QRegularExpression::CaseInsensitiveOption));
        ++m_genreLookupsRunning;
        const QString videoId = v.id;
        const QString fallback = v.genre;
        m_pGenreLookup->lookup(artist,
                title,
                v.durationSec,
                [this, videoId, fallback](const QString& found) {
                    --m_genreLookupsRunning;
                    const QString genre = found.isEmpty() ? fallback : found;
                    m_genreCache.insert(videoId, genre);
                    if (!found.isEmpty()) {
                        emit genreResolved(videoId, found);
                    }
                    pumpGenreLookups();
                });
    }
}

// static
QVariantList QmlYouTubeProxy::toVariantList(const QList<mixxx::YouTubeVideoInfo>& videos) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    const QHash<QString, QVariantMap> libraryInfo =
            pFeature ? pFeature->libraryInfoForResults(videos) : QHash<QString, QVariantMap>();
    QVariantList out;
    out.reserve(videos.size());
    for (const auto& v : videos) {
        QVariantMap m = libraryInfo.value(v.id);
        m[QStringLiteral("id")] = v.id;
        m[QStringLiteral("title")] = v.title;
        m[QStringLiteral("uploader")] = v.uploader;
        m[QStringLiteral("durationSec")] = v.durationSec;
        m[QStringLiteral("isLive")] = v.isLive;
        m[QStringLiteral("genre")] = v.genre;
        out.append(m);
    }
    return out;
}

namespace {
const QString kScClientIdKey = QStringLiteral("SoundCloud/clientId");
const QString kScUserAgent = QStringLiteral(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/130.0 Safari/537.36");
constexpr int kScPageSize = 30;

QNetworkRequest scRequest(const QUrl& url) {
    QNetworkRequest request(url);
    request.setHeader(QNetworkRequest::UserAgentHeader, kScUserAgent);
    // Signed in: the account's view (its likes; Go+ tracks in full)
    if (const QString token = mixxx::soundcloud::token();
            !token.isEmpty() && url.host() == QStringLiteral("api-v2.soundcloud.com")) {
        request.setRawHeader("Authorization", "OAuth " + token.toUtf8());
    }
    request.setTransferTimeout(15000);
    return request;
}
} // namespace

// SoundCloud's api-v2 needs the client id its web player uses. It sits in
// one of the player's JS bundles (the last ones, usually); found once and
// kept in the settings, looked up again only when the API refuses it.
void QmlYouTubeProxy::fetchSoundCloudClientId(std::function<void(bool)> done) {
    QNetworkReply* pPage = m_pScNetwork->get(
            scRequest(QUrl(QStringLiteral("https://soundcloud.com/"))));
    connect(pPage, &QNetworkReply::finished, this, [this, pPage, done]() {
        pPage->deleteLater();
        static const QRegularExpression kScript(
                QStringLiteral(R"(https://a-v2\.sndcdn\.com/assets/[^"]+\.js)"));
        QStringList scripts;
        auto it = kScript.globalMatch(QString::fromUtf8(pPage->readAll()));
        while (it.hasNext()) {
            scripts.prepend(it.next().captured(0)); // newest bundles last
        }
        auto tryNext = std::make_shared<std::function<void(int)>>();
        *tryNext = [this, scripts, done, tryNext](int index) {
            if (index >= std::min<int>(scripts.size(), 8)) {
                done(false);
                return;
            }
            QNetworkReply* pJs = m_pScNetwork->get(scRequest(QUrl(scripts.at(index))));
            connect(pJs, &QNetworkReply::finished, this, [this, pJs, index, done, tryNext]() {
                pJs->deleteLater();
                static const QRegularExpression kClientId(
                        QStringLiteral(R"re(client_id\s*:\s*"([0-9A-Za-z]{32})")re"));
                const auto match = kClientId.match(QString::fromUtf8(pJs->readAll()));
                if (match.hasMatch()) {
                    m_scClientId = match.captured(1);
                    QSettings().setValue(kScClientIdKey, m_scClientId);
                    done(true);
                    return;
                }
                (*tryNext)(index + 1);
            });
        };
        (*tryNext)(0);
    });
}

void QmlYouTubeProxy::ensureSoundCloudNetwork() {
    if (!m_pScNetwork) {
        m_pScNetwork = new QNetworkAccessManager(this);
        m_scClientId = QSettings().value(kScClientIdKey).toString();
    }
}

void QmlYouTubeProxy::fetchSoundCloudMe(std::function<void()> done) {
    if (done) {
        m_scMeWaiters.append(std::move(done));
    }
    if (m_scMeInFlight) {
        return;
    }
    m_scMeInFlight = true;
    requestSoundCloudMe(false);
}

void QmlYouTubeProxy::finishSoundCloudMe(bool ok) {
    m_scMeInFlight = false;
    const QList<std::function<void()>> waiters = std::exchange(m_scMeWaiters, {});
    if (!ok) {
        return;
    }
    for (const auto& waiter : waiters) {
        waiter();
    }
}

void QmlYouTubeProxy::requestSoundCloudMe(bool clientIdRetried) {
    ensureSoundCloudNetwork();
    if (mixxx::soundcloud::token().isEmpty()) {
        finishSoundCloudMe(false);
        return;
    }
    if (m_scClientId.isEmpty()) {
        fetchSoundCloudClientId([this](bool ok) {
            if (!ok) {
                finishSoundCloudMe(false);
                emit soundCloudFailed(tr("SoundCloud is not reachable"));
                return;
            }
            requestSoundCloudMe(true);
        });
        return;
    }
    QUrl url(QStringLiteral("https://api-v2.soundcloud.com/me"));
    url.setQuery(QStringLiteral("client_id=") + m_scClientId);
    QNetworkReply* pReply = m_pScNetwork->get(scRequest(url));
    connect(pReply, &QNetworkReply::finished, this, [this, pReply, clientIdRetried]() {
        pReply->deleteLater();
        const int status = pReply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        const QJsonObject me = QJsonDocument::fromJson(pReply->readAll()).object();
        const qint64 id = static_cast<qint64>(me.value(QStringLiteral("id")).toDouble());
        if (pReply->error() == QNetworkReply::NoError && id > 0) {
            m_scUserId = QString::number(id);
            m_scName = me.value(QStringLiteral("username")).toString();
            m_scAvatar = me.value(QStringLiteral("avatar_url")).toString();
            qInfo() << "SoundCloud: signed in as" << m_scName;
            emit accountsChanged();
            finishSoundCloudMe(true);
            return;
        }
        qWarning() << "SoundCloud /me failed:" << status << pReply->errorString();
        if ((status == 401 || status == 403) && !clientIdRetried) {
            m_scClientId.clear(); // maybe the rotated client id
            requestSoundCloudMe(true);
            return;
        }
        const bool parsedWithoutUser = pReply->error() == QNetworkReply::NoError &&
                !me.isEmpty() && !me.contains(QStringLiteral("id"));
        finishSoundCloudMe(false);
        if (status == 401 || status == 403 || parsedWithoutUser) {
            // The token is no account's
            mixxx::soundcloud::setToken(QString());
            m_scUserId.clear();
            m_scName.clear();
            m_scAvatar.clear();
            emit accountsChanged();
            emit accountFailed(tr("SoundCloud did not accept the sign-in. Sign in again"));
            return;
        }
        emit soundCloudFailed(pReply->errorString());
    });
}

void QmlYouTubeProxy::soundCloudSearch(const QString& query) {
    ensureSoundCloudNetwork();
    m_scNextHref.clear();
    ++m_scGeneration;
    QUrl url(QStringLiteral("https://api-v2.soundcloud.com/search/tracks"));
    QUrlQuery urlQuery;
    urlQuery.addQueryItem(QStringLiteral("q"), query.trimmed());
    urlQuery.addQueryItem(QStringLiteral("limit"), QString::number(kScPageSize));
    url.setQuery(urlQuery);
    soundCloudRequest(url, false, false);
}

void QmlYouTubeProxy::soundCloudLikes() {
    ensureSoundCloudNetwork();
    if (m_scUserId.isEmpty()) {
        // The likes are the user's: who that is first
        fetchSoundCloudMe([this]() { soundCloudLikes(); });
        return;
    }
    m_scNextHref.clear();
    ++m_scGeneration;
    QUrl url(QStringLiteral("https://api-v2.soundcloud.com/users/") + m_scUserId +
            QStringLiteral("/track_likes"));
    url.setQuery(QStringLiteral("limit=") + QString::number(kScPageSize));
    soundCloudRequest(url, false, false);
}

void QmlYouTubeProxy::soundCloudMore() {
    if (m_scNextHref.isEmpty() || !m_pScNetwork) {
        return;
    }
    const QUrl next(m_scNextHref);
    m_scNextHref.clear();
    soundCloudRequest(next, true, false);
}

void QmlYouTubeProxy::soundCloudRequest(const QUrl& baseUrl, bool append, bool clientIdRetried) {
    if (m_scClientId.isEmpty()) {
        if (clientIdRetried) {
            emit soundCloudFailed(tr("SoundCloud is not reachable"));
            return;
        }
        fetchSoundCloudClientId([this, baseUrl, append](bool ok) {
            if (!ok) {
                emit soundCloudFailed(tr("SoundCloud is not reachable"));
                return;
            }
            soundCloudRequest(baseUrl, append, true);
        });
        return;
    }
    QUrl url(baseUrl);
    QUrlQuery urlQuery(url);
    urlQuery.removeAllQueryItems(QStringLiteral("client_id"));
    urlQuery.addQueryItem(QStringLiteral("client_id"), m_scClientId);
    url.setQuery(urlQuery);
    const quint64 generation = m_scGeneration;
    QNetworkReply* pReply = m_pScNetwork->get(scRequest(url));
    connect(pReply,
            &QNetworkReply::finished,
            this,
            [this, pReply, baseUrl, append, clientIdRetried, generation]() {
                pReply->deleteLater();
                if (generation != m_scGeneration) {
                    return; // a newer search started
                }
                const int status =
                        pReply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
                if ((status == 401 || status == 403) && !clientIdRetried) {
                    // The web player rotated its client id
                    m_scClientId.clear();
                    soundCloudRequest(baseUrl, append, false);
                    return;
                }
                if (pReply->error() != QNetworkReply::NoError) {
                    qWarning() << "SoundCloud" << baseUrl.path() << "failed:" << status
                               << pReply->errorString();
                    if ((status == 401 || status == 403) &&
                            !mixxx::soundcloud::token().isEmpty()) {
                        // A fresh client id did not help: the sign-in is
                        // what SoundCloud refuses
                        mixxx::soundcloud::setToken(QString());
                        m_scUserId.clear();
                        m_scName.clear();
                        m_scAvatar.clear();
                        emit accountsChanged();
                        emit accountFailed(tr("SoundCloud wants a new sign-in"));
                    }
                    emit soundCloudFailed(pReply->errorString());
                    return;
                }
                const QJsonObject root = QJsonDocument::fromJson(pReply->readAll()).object();
                m_scNextHref = root.value(QStringLiteral("next_href")).toString();
                qInfo() << "SoundCloud" << baseUrl.path() << "answered"
                        << root.value(QStringLiteral("collection")).toArray().size()
                        << "items";
                QList<mixxx::YouTubeVideoInfo> videos;
                QHash<QString, QVariantMap> extra;
                const QJsonArray tracks = root.value(QStringLiteral("collection")).toArray();
                for (const QJsonValue& value : tracks) {
                    // Likes come as {"track": {...}}
                    const QJsonObject track = value.toObject().contains(QStringLiteral("track"))
                            ? value.toObject().value(QStringLiteral("track")).toObject()
                            : value.toObject();
                    const qint64 id =
                            static_cast<qint64>(track.value(QStringLiteral("id")).toDouble());
                    const QString title = track.value(QStringLiteral("title")).toString();
                    if (id <= 0 || title.isEmpty()) {
                        continue;
                    }
                    const QJsonObject user = track.value(QStringLiteral("user")).toObject();
                    mixxx::YouTubeVideoInfo info;
                    info.id = QStringLiteral("sc-") + QString::number(id);
                    info.title = title;
                    info.uploader = user.value(QStringLiteral("username")).toString();
                    info.durationSec = static_cast<int>(
                            track.value(QStringLiteral("duration")).toDouble() / 1000);
                    // The uploader's genre; else the first tag, which is
                    // often the genre on SoundCloud ("techno", "rebetiko")
                    info.genre = track.value(QStringLiteral("genre")).toString().trimmed();
                    if (info.genre.isEmpty()) {
                        static const QRegularExpression kTag(
                                QStringLiteral(R"re(^\s*(?:"([^"]+)"|(\S+)))re"));
                        const auto tag = kTag.match(
                                track.value(QStringLiteral("tag_list")).toString());
                        info.genre = tag.captured(1).isEmpty() ? tag.captured(2)
                                                               : tag.captured(1);
                    }
                    QString artwork = track.value(QStringLiteral("artwork_url")).toString();
                    if (artwork.isEmpty()) {
                        artwork = user.value(QStringLiteral("avatar_url")).toString();
                    }
                    QVariantMap more;
                    more[QStringLiteral("artwork")] = artwork; // "-large" = 100 px
                    more[QStringLiteral("plays")] =
                            track.value(QStringLiteral("playback_count")).toDouble();
                    // SNIP = Go+ only: a 30 s preview without the subscription
                    // (signed in with Go+, SoundCloud says ALLOW for them)
                    more[QStringLiteral("preview")] =
                            track.value(QStringLiteral("policy")).toString() ==
                            QStringLiteral("SNIP");
                    extra.insert(info.id, more);
                    videos.append(info);
                    m_videoTitles.insert(info.id, info.title);
                }
                if (s_pLibrary) {
                    if (YouTubeFeature* pFeature = s_pLibrary->youtubeFeature()) {
                        pFeature->rememberExternalResults(videos);
                    }
                }
                QVariantList out = toVariantList(videos);
                for (QVariant& entry : out) {
                    QVariantMap map = entry.toMap();
                    const QVariantMap more = extra.value(
                            map.value(QStringLiteral("id")).toString());
                    for (auto it = more.cbegin(); it != more.cend(); ++it) {
                        map.insert(it.key(), it.value());
                    }
                    entry = map;
                }
                emit soundCloudResults(out, append);
            });
}

bool QmlYouTubeProxy::sponsorBlockAutoSkip() const {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        return pFeature->sponsorBlockAutoSkip();
    }
    return true; // default: ON
}

void QmlYouTubeProxy::setSponsorBlockAutoSkip(bool enabled) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        // The feature's change signal reaches SponsorBlockController (playback
        // skipping) and the service (cutting downloads).
        pFeature->setSponsorBlockAutoSkip(enabled);
        emit sponsorBlockAutoSkipChanged();
    }
}

void QmlYouTubeProxy::signIn(const QString& service) {
#ifdef Q_OS_ANDROID
    const bool youTube = service == QStringLiteral("youtube");
    // The sign-in ends back on the service, with its sign-in cookie set
    const QString url = youTube
            ? QStringLiteral(
                      "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube"
                      "&passive=true&continue=https%3A%2F%2Fmusic.youtube.com%2F")
            : QStringLiteral("https://soundcloud.com/signin");
    QJniObject::callStaticMethod<void>("org/mixxx/AccountLoginActivity",
            "start",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/"
            "String;Ljava/lang/String;)V",
            QNativeInterface::QAndroidApplication::context(),
            QJniObject::fromString(service).object<jstring>(),
            QJniObject::fromString(url).object<jstring>(),
            QJniObject::fromString(youTube ? QStringLiteral("music.youtube.com")
                                           : QStringLiteral("soundcloud.com"))
                    .object<jstring>(),
            QJniObject::fromString(youTube ? QStringLiteral("SAPISID")
                                           : QStringLiteral("oauth_token"))
                    .object<jstring>());
#else
    Q_UNUSED(service);
    emit accountFailed(tr("Signing in is in the Android app"));
#endif
}

void QmlYouTubeProxy::onSignedIn(const QString& service, const QString& cookies) {
    if (service == QStringLiteral("youtube")) {
        YouTubeAccount::instance().setCookies(cookies);
        return;
    }
    // SoundCloud: the oauth_token cookie is the account's token
    QString token;
    const QStringList parts = cookies.split(QLatin1Char(';'));
    for (const QString& part : parts) {
        if (part.trimmed().startsWith(QStringLiteral("oauth_token="))) {
            token = part.trimmed().mid(12);
        }
    }
    if (token.isEmpty()) {
        qInfo() << "SoundCloud: sign-in page closed without a sign-in";
        if (s_pInstance) {
            emit s_pInstance->accountFailed(tr("Not signed in"));
        }
        return;
    }
    qInfo() << "SoundCloud: signed in";
    soundcloud::setToken(token);
    if (s_pInstance) {
        s_pInstance->m_scUserId.clear();
        emit s_pInstance->accountsChanged();
        s_pInstance->fetchSoundCloudMe({});
    }
}

void QmlYouTubeProxy::signOut(const QString& service) {
    if (service == QStringLiteral("youtube")) {
        m_accountList = false;
        YouTubeAccount::instance().signOut();
    } else {
        soundcloud::setToken(QString());
        m_scUserId.clear();
        m_scName.clear();
        m_scAvatar.clear();
        emit accountsChanged();
    }
}

bool QmlYouTubeProxy::youTubeSignedIn() const {
    return YouTubeAccount::instance().signedIn();
}

QString QmlYouTubeProxy::youTubeAccount() const {
    return YouTubeAccount::instance().name();
}

bool QmlYouTubeProxy::soundCloudSignedIn() const {
    return !soundcloud::token().isEmpty();
}

void QmlYouTubeProxy::youTubeLiked() {
    m_accountList = true;
    YouTubeAccount::instance().openList(QStringLiteral("liked"));
}

void QmlYouTubeProxy::youTubeAccounts() {
    YouTubeAccount::instance().fetchAccounts();
}

void QmlYouTubeProxy::selectYouTubeAccount(const QString& pageId, const QString& name) {
    YouTubeAccount::instance().selectAccount(pageId, name);
}

void QmlYouTubeProxy::youTubePlaylists() {
    YouTubeAccount::instance().fetchPlaylists();
}

void QmlYouTubeProxy::youTubePlaylist(const QString& playlistId) {
    m_accountList = true;
    YouTubeAccount::instance().openList(playlistId);
}

} // namespace qml
} // namespace mixxx

#ifdef Q_OS_ANDROID
/// AccountLoginActivity: the service's cookies after a sign-in ("" when the
/// page was closed)
extern "C" JNIEXPORT void JNICALL Java_org_mixxx_AccountLoginActivity_nativeSignedIn(
        JNIEnv* pEnv, jclass, jstring service, jstring cookies) {
    auto string = [pEnv](jstring value) {
        const char* pChars = pEnv->GetStringUTFChars(value, nullptr);
        const QString out = QString::fromUtf8(pChars);
        pEnv->ReleaseStringUTFChars(value, pChars);
        return out;
    };
    const QString serviceName = string(service);
    const QString cookieText = string(cookies);
    QMetaObject::invokeMethod(
            qApp,
            [serviceName, cookieText]() {
                mixxx::qml::QmlYouTubeProxy::onSignedIn(serviceName, cookieText);
            },
            Qt::QueuedConnection);
}
#endif
