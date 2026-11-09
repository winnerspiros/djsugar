#pragma once

#include <QDateTime>
#include <QHash>
#include <QJsonObject>
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <functional>
#include <memory>

class Library;
class PlayerManager;
class QNetworkAccessManager;
class QNetworkReply;

namespace mixxx {
namespace qml {

/// Spotify for the QML skin, exposed as `Mixxx.Spotify`.
///
/// Everything goes through lib/spotify-bridge (librespot), no Spotify
/// developer app: login from the Spotify app (DJ Sugar is offered as a
/// Spotify Connect device; picking it hands the account over) or on Spotify's
/// web login. The account is stored, so this happens once. Browsing: search,
/// Liked Songs, the user's playlists and top tracks through the Spotify Web
/// API with tokens from the session.
///
/// Audio: Spotify's own files, downloaded and decrypted (Ogg Vorbis
/// 320 kbit/s) into the cache; they load to the deck like local files, so
/// beatgrid, waveform, scratching and loops work. Needs Spotify Premium.
class QmlSpotifyProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool available READ available CONSTANT)
    Q_PROPERTY(bool loggedIn READ loggedIn NOTIFY loginChanged)
    Q_PROPERTY(bool loggingIn READ loggingIn NOTIFY loginChanged)
    /// "app" or "web" while a login waits, "" otherwise
    Q_PROPERTY(QString loginStep READ loginStep NOTIFY loginChanged)
    Q_PROPERTY(QString userName READ userName NOTIFY loginChanged)
    Q_PROPERTY(bool appInstalled READ appInstalled CONSTANT)
    QML_NAMED_ELEMENT(Spotify)
    QML_SINGLETON

  public:
    explicit QmlSpotifyProxy(QObject* pParent = nullptr);
    ~QmlSpotifyProxy() override;

    static QmlSpotifyProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerLibrary(Library* pLibrary);
    static void registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager);

    /// The librespot bridge is compiled in
    bool available() const;
    bool loggedIn() const {
        return m_loggedIn;
    }
    bool loggingIn() const {
        return m_loggingIn;
    }
    QString loginStep() const {
        return m_loginStep;
    }
    QString userName() const {
        return m_userName;
    }
    bool appInstalled() const;

    /// From the Spotify app (Spotify Connect; opens the app) or Spotify's
    /// web login in the browser. Waits until done or cancelLogin().
    Q_INVOKABLE void login(bool browser = false);
    Q_INVOKABLE void cancelLogin();
    Q_INVOKABLE void openSpotifyApp();
    Q_INVOKABLE void logout();

    Q_INVOKABLE void search(const QString& query);
    Q_INVOKABLE void likedSongs();
    Q_INVOKABLE void topTracks();
    Q_INVOKABLE void playlists();
    Q_INVOKABLE void playlistTracks(const QString& playlistId);
    /// Next page of the current track list
    Q_INVOKABLE void loadMore();
    Q_INVOKABLE bool hasMore() const {
        return !m_nextUrl.isEmpty();
    }

    /// Downloads the track (once, then from the cache) and loads it to the deck
    Q_INVOKABLE void loadToDeck(const QVariantMap& track, const QString& deckGroup);
    /// 0..1 while the track downloads, -1 otherwise
    Q_INVOKABLE double downloadProgress(const QString& spotifyId) const {
        return m_progress.value(spotifyId, -1.0);
    }

  signals:
    void loginChanged();
    void loginFailed(const QString& error);
    void tracksLoaded(const QVariantList& tracks, bool append);
    void playlistsLoaded(const QVariantList& playlists);
    void failed(const QString& error);
    /// `progress` 0..1, -1 when the download ended (done or failed)
    void downloadProgressChanged(const QString& spotifyId, double progress);
    void downloadFailed(const QString& spotifyId, const QString& error);

  private:
    enum class ListKind {
        Tracks,
        Playlists,
    };

    static QString accountDir();
    void onLoginResult(const QString& user, const QString& error);
    void refreshAccessToken(std::function<void(bool)> done);
    void fetchProfile();
    void apiGet(const QUrl& url,
            std::function<void(const QJsonObject&)> onSuccess,
            bool retried = false);
    void fetchList(const QUrl& url, ListKind kind, bool append);
    QString cacheDir() const;
    void downloadTrack(const QString& spotifyId);
    void onDownloaded(const QString& spotifyId, const QString& path, const QString& error);
    void loadFile(const QVariantMap& track, const QString& path, const QString& deckGroup);

    static QVariantMap trackToVariant(const QJsonObject& track);

    static inline Library* s_pLibrary = nullptr;
    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;

    QNetworkAccessManager* m_pNetwork;
    bool m_loggedIn = false;
    QString m_accessToken;
    QDateTime m_accessExpiry;
    QString m_userName;
    bool m_loggingIn = false;
    QString m_loginStep;
    quint64 m_loginAttempt = 0; // drops the results of cancelled logins
    QString m_nextUrl;
    quint64 m_generation = 0; // drops the replies of superseded lists
    QList<std::function<void(bool)>> m_refreshWaiters;
    struct PendingLoad {
        QVariantMap track;
        QString deckGroup;
    };
    QHash<QString, QList<PendingLoad>> m_pendingLoads; // spotify id -> decks
    QHash<QString, double> m_progress;
};

} // namespace qml
} // namespace mixxx
