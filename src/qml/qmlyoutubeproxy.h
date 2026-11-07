#pragma once

#include <QHash>
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>
#include <QVariantMap>

#include "library/youtube/youtubeservice.h"

class Library;
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
    Q_INVOKABLE void downloadToDeck(const QString& videoId, const QString& deckGroup);
    void prefetchCovers(const QList<mixxx::YouTubeVideoInfo>& results);

    bool sponsorBlockAutoSkip() const;
    void setSponsorBlockAutoSkip(bool enabled);

  signals:
    void searchFinished(const QVariantList& results);
    void searchFailed(const QString& error);
    void downloadProgressChanged(const QString& videoId, double progress);
    void downloadFinished(const QString& videoId, const QString& localPath);
    void downloadFailed(const QString& videoId, const QString& error);
    void sponsorBlockAutoSkipChanged();

  private:
    static QString extractVideoId(const QString& videoIdOrUrl);
    static QVariantList toVariantList(const QList<mixxx::YouTubeVideoInfo>& videos);
    void updateVideoProgress(const QString& videoId, double progress);
    void clearVideoProgress(const QString& videoId);

    static inline Library* s_pLibrary = nullptr;
    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;
    QVariantMap m_downloadProgressByVideo;
    QVariantMap m_deckDownloadProgress;
    QHash<QString, QString> m_videoToDeck;
    QHash<QString, QString> m_videoTitles;
};

} // namespace qml
} // namespace mixxx
