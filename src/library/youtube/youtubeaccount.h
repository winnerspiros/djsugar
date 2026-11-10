#pragma once

#include <QList>
#include <QObject>
#include <QString>
#include <QVariantList>

#include "library/youtube/youtubeservice.h"

class QNetworkAccessManager;
class QJsonObject;

namespace mixxx {

/// The user's own YouTube (Music) account: signed in through Google's page
/// in the app (YouTubeLoginActivity), whose cookies are kept here and sign
/// the YouTube Music requests for the liked songs and the playlists, as the
/// YouTube Music web player does. Downloads stay anonymous.
class YouTubeAccount : public QObject {
    Q_OBJECT
  public:
    static YouTubeAccount& instance();

    bool signedIn() const {
        return !m_cookies.isEmpty();
    }
    QString name() const {
        return m_name;
    }

    /// The cookies of a finished sign-in ("" when the page was closed)
    void setCookies(const QString& cookies);
    void signOut();

    /// Track lists: "liked" or a playlist's browse id ("VL..."). The first
    /// page comes as tracksReady(.., false), later ones (more()) as
    /// tracksReady(.., true)
    void openList(const QString& list);
    void more();
    bool hasMore() const {
        return !m_continuation.isEmpty();
    }
    void fetchPlaylists();

  signals:
    void accountChanged();
    void tracksReady(const QList<mixxx::YouTubeVideoInfo>& tracks, bool append);
    /// [{id, name, subtitle, artwork}]
    void playlistsReady(const QVariantList& playlists);
    void failed(const QString& error);

  private:
    YouTubeAccount();
    void fetchName();
    void post(const QString& endpoint,
            const QJsonObject& body,
            const std::function<void(const QJsonObject&)>& done);
    void collectPlaylists(const QJsonObject& page, QVariantList playlists, int pages);

    QNetworkAccessManager* m_pNetwork;
    QString m_cookies;
    QString m_name;
    QString m_continuation;
    quint64 m_generation = 0; // drops the replies of a superseded list
};

} // namespace mixxx
