#pragma once

#include <QList>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <functional>

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

    /// The channels of the Google account (a main one and brand accounts):
    /// accountsReady([{name, handle, pageId, selected}]); pageId "" is the
    /// main channel
    void fetchAccounts();
    void selectAccount(const QString& pageId, const QString& name);

    /// A song on YouTube Music by its name and length (no sign-in needed):
    /// done(videoId, "" when none is close)
    void findSong(const QString& query,
            int durationSec,
            QObject* pContext,
            const std::function<void(const QString&)>& done);

  signals:
    void accountChanged();
    void tracksReady(const QList<mixxx::YouTubeVideoInfo>& tracks, bool append);
    /// [{id, name, subtitle, artwork}]
    void playlistsReady(const QVariantList& playlists);
    void failed(const QString& error);
    void accountsReady(const QVariantList& accounts);

  private:
    YouTubeAccount();
    void fetchName();
    void fetchClientVersion();
    void post(const QString& endpoint,
            const QJsonObject& body,
            const std::function<void(const QJsonObject&)>& done,
            bool signedRequest = true,
            QObject* pContext = nullptr);
    void save();
    void collectPlaylists(const QJsonObject& page, QVariantList playlists, int pages);

    QNetworkAccessManager* m_pNetwork;
    QString m_cookies;
    QString m_clientVersion;
    QString m_visitorData;
    QString m_name;
    QString m_pageId; // a brand channel, "" for the main one
    QString m_continuation;
    quint64 m_generation = 0;     // drops the replies of a signed-out account
    quint64 m_listGeneration = 0; // drops the pages of a superseded list
    // Each streaming view asks when it opens: one request runs per kind
    QString m_listInFlight;
    bool m_playlistsInFlight = false;
    bool m_accountsInFlight = false;
    void clearRequestsInFlight();
};

} // namespace mixxx
