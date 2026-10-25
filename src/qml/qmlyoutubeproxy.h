#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>

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
    QML_NAMED_ELEMENT(YouTube)
    QML_SINGLETON

  public:
    explicit QmlYouTubeProxy(QObject* pParent = nullptr);

    static QmlYouTubeProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerLibrary(Library* pLibrary);
    static void registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager);

    Q_INVOKABLE void search(const QString& query);
    Q_INVOKABLE void loadMore();
    Q_INVOKABLE bool hasMore() const;
    Q_INVOKABLE void downloadToDeck(const QString& videoId, const QString& deckGroup);

  signals:
    void searchFinished(const QVariantList& results);
    void searchFailed(const QString& error);
    void downloadFinished(const QString& videoId, const QString& localPath);
    void downloadFailed(const QString& videoId, const QString& error);

  private:
    static QVariantList toVariantList(const QList<mixxx::YouTubeVideoInfo>& videos);

    static inline Library* s_pLibrary = nullptr;
    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;
    QString m_pendingDeck;
};

} // namespace qml
} // namespace mixxx
