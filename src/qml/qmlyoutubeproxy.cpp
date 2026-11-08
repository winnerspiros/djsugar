#include "qml/qmlyoutubeproxy.h"

#include <QFileInfo>
#include <QTimer>
#include <algorithm>

#include "library/library.h"
#include "library/youtube/youtubefeature.h"
#include "library/youtube/youtubeservice.h"
#include "mixer/playermanager.h"
#include "moc_qmlyoutubeproxy.cpp"

namespace mixxx {
namespace qml {

QmlYouTubeProxy::QmlYouTubeProxy(QObject* pParent)
        : QObject(pParent) {
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
                for (const auto& v : results) {
                    if (!v.id.isEmpty() && !v.title.isEmpty()) {
                        m_videoTitles.insert(v.id, v.title);
                    }
                }
                prefetchCovers(results);
                emit searchFinished(toVariantList(results));
            });
    connect(pService,
            &mixxx::YouTubeService::searchMoreReady,
            this,
            [this, pFeature, pService](const QString& query, const QList<mixxx::YouTubeVideoInfo>& results) {
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

void QmlYouTubeProxy::search(const QString& query) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        pFeature->searchAndActivate(query);
    } else {
        emit searchFailed(tr("YouTube is not available"));
    }
}

void QmlYouTubeProxy::loadMore() {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (pFeature) {
        pFeature->loadMoreResults();
    } else {
        emit searchMoreFinished({}, false);
    }
}

bool QmlYouTubeProxy::hasMore() const {
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
        }
    }
}

// static
QVariantList QmlYouTubeProxy::toVariantList(const QList<mixxx::YouTubeVideoInfo>& videos) {
    QVariantList out;
    out.reserve(videos.size());
    for (const auto& v : videos) {
        QVariantMap m;
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

} // namespace qml
} // namespace mixxx
