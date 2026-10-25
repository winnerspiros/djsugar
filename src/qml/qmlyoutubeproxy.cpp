#include "qml/qmlyoutubeproxy.h"

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
    mixxx::YouTubeService* pService = pFeature->service();
    connect(pService,
            &mixxx::YouTubeService::searchResultsReady,
            this,
            [this](const QString&, const QList<mixxx::YouTubeVideoInfo>& results) {
                emit searchFinished(toVariantList(results));
            });
    connect(pService,
            &mixxx::YouTubeService::searchMoreReady,
            this,
            [this](const QString&, const QList<mixxx::YouTubeVideoInfo>& results) {
                emit searchFinished(toVariantList(results));
            });
    connect(pService,
            &mixxx::YouTubeService::searchFailed,
            this,
            [this](const QString&, const QString& error) {
                emit searchFailed(error);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadFinished,
            this,
            [this](const QString& videoId, const QString& localPath) {
                if (!m_pendingDeck.isEmpty() && s_pPlayerManager) {
                    const QString deck = m_pendingDeck;
                    m_pendingDeck.clear();
                    QMetaObject::invokeMethod(
                            s_pPlayerManager.get(),
                            [deck, localPath]() {
                                s_pPlayerManager->slotLoadLocationToPlayer(
                                        localPath, deck, false);
                            },
                            Qt::QueuedConnection);
                }
                emit downloadFinished(videoId, localPath);
            });
    connect(pService,
            &mixxx::YouTubeService::downloadFailed,
            this,
            [this](const QString& videoId, const QString& error) {
                m_pendingDeck.clear();
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
        pFeature->service()->fetchMoreSearchResults(QString());
    }
}

bool QmlYouTubeProxy::hasMore() const {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    return pFeature && pFeature->service()->hasMoreSearchResults();
}

void QmlYouTubeProxy::downloadToDeck(const QString& videoId, const QString& deckGroup) {
    YouTubeFeature* pFeature = s_pLibrary ? s_pLibrary->youtubeFeature() : nullptr;
    if (!pFeature) {
        emit downloadFailed(videoId, tr("YouTube is not available"));
        return;
    }
    m_pendingDeck = deckGroup;
    pFeature->requestDownloadToPlayer(videoId, deckGroup, false);
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
        out.append(m);
    }
    return out;
}

} // namespace qml
} // namespace mixxx
