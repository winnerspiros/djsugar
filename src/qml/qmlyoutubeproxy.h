#pragma once

#include <QHash>
#include <QObject>
#include <QQmlEngine>
#include <QQueue>
#include <QSet>
#include <QString>
#include <QVariantList>
#include <QVariantMap>
#include <functional>

#include "library/youtube/youtubegenrelookup.h"
#include "library/youtube/youtubeservice.h"

class Library;
class QNetworkAccessManager;
class PlayerManager;
class YouTubeFeature;

namespace mixxx {
namespace qml {

/// YouTube search/download for the Booth Android skin, exposed as
/// `Mixxx.YouTube`. Wraps the YouTube library feature (search + cached
/// download) so QML can search and load results straight into a deck.
class QmlYouTubeProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantMap downloadProgressByVideo
                    READ downloadProgressByVideo
                            NOTIFY downloadProgressChanged)
    Q_PROPERTY(QVariantMap deckDownloadProgress
                    READ deckDownloadProgress
                            NOTIFY downloadProgressChanged)
    Q_PROPERTY(bool sponsorBlockAutoSkip
                    READ sponsorBlockAutoSkip
                            WRITE setSponsorBlockAutoSkip
                                    NOTIFY sponsorBlockAutoSkipChanged)
    QML_NAMED_ELEMENT(YouTube)
    QML_SINGLETON

  public:
    explicit QmlYouTubeProxy(QObject* pParent = nullptr);

    static QmlYouTubeProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerLibrary(Library* pLibrary);
    static void registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager);

    QVariantMap downloadProgressByVideo() const {
        return m_downloadProgressByVideo;
    }
    QVariantMap deckDownloadProgress() const {
        return m_deckDownloadProgress;
    }

    Q_INVOKABLE double videoProgress(const QString& videoIdOrUrl) const;
    Q_INVOKABLE double deckProgress(const QString& deckGroup) const;
    Q_INVOKABLE QString deckDownloadingTitle(const QString& deckGroup) const;

    Q_INVOKABLE void search(const QString& query);
    Q_INVOKABLE void loadMore();
    Q_INVOKABLE bool hasMore() const;
    Q_INVOKABLE void download(const QString& videoId);
    Q_INVOKABLE void downloadToDeck(const QString& videoId, const QString& deckGroup);
    /// Extracts the stream link of a result the user is about to load (the
    /// controller highlight is on it), so LOAD starts the download at once
    Q_INVOKABLE void prefetchLink(const QString& videoId);

    /// SoundCloud search (api-v2, with the web player's public client id).
    /// Results use "sc-<track id>" ids and download/load like YouTube ones.
    Q_INVOKABLE void soundCloudSearch(const QString& query);
    Q_INVOKABLE void soundCloudMore();
    Q_INVOKABLE bool soundCloudHasMore() const {
        return !m_scNextHref.isEmpty();
    }
    void prefetchCovers(const QList<mixxx::YouTubeVideoInfo>& results);

    bool sponsorBlockAutoSkip() const;
    void setSponsorBlockAutoSkip(bool enabled);

  signals:
    void searchFinished(const QVariantList& results);
    /// Next batch of the current search; empty when the fetch ended without
    /// results. `hasMore` is false once YouTube has no further pages.
    void searchMoreFinished(const QVariantList& results, bool hasMore);
    void searchFailed(const QString& error);
    void soundCloudResults(const QVariantList& results, bool append);
    void soundCloudFailed(const QString& error);
    void downloadProgressChanged(const QString& videoId, double progress);
    void downloadFinished(const QString& videoId, const QString& localPath);
    void downloadFailed(const QString& videoId, const QString& error);
    void sponsorBlockAutoSkipChanged();
    /// Genre of a search result, found in the iTunes/Deezer catalogues.
    void genreResolved(const QString& videoId, const QString& genre);

  private:
    static QString extractVideoId(const QString& videoIdOrUrl);
    static QVariantList toVariantList(const QList<mixxx::YouTubeVideoInfo>& videos);
    void updateVideoProgress(const QString& videoId, double progress);
    void clearVideoProgress(const QString& videoId);
    void queueGenreLookups(const QList<mixxx::YouTubeVideoInfo>& results);
    void pumpGenreLookups();

    static inline Library* s_pLibrary = nullptr;
    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;
    QVariantMap m_downloadProgressByVideo;
    QVariantMap m_deckDownloadProgress;
    QHash<QString, QString> m_videoToDeck;
    QHash<QString, QString> m_videoTitles;
    mixxx::YouTubeGenreLookup* m_pGenreLookup = nullptr;
    QQueue<mixxx::YouTubeVideoInfo> m_genreQueue;
    QHash<QString, QString> m_genreCache; // videoId -> genre ("" = unknown)
    QSet<QString> m_genreQueued;
    int m_genreLookupsRunning = 0;

    void soundCloudRequest(const QUrl& url, bool append, bool clientIdRetried);
    void fetchSoundCloudClientId(std::function<void(bool)> done);
    QNetworkAccessManager* m_pScNetwork = nullptr;
    QString m_scClientId;
    QString m_scNextHref;
    quint64 m_scGeneration = 0; // drops the replies of superseded searches
};

} // namespace qml
} // namespace mixxx
