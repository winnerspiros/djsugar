#pragma once

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMap>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QObject>
#include <QQmlEngine>
#include <QRandomGenerator>
#include <QStandardPaths>
#include <QString>
#include <QUrl>
#include <QUrlQuery>
#include <QVariantList>
#include <QVariantMap>
#include <memory>

#include "mixer/playermanager.h"
#include "preferences/usersettings.h"

namespace mixxx {
namespace qml {

struct StreamingDownloadJob {
    QString trackId;
    QString service;
    QString title;
    QString artist;
    QString targetDeck;
    bool autoPlay;
    QString ext;
    QString finalPath;
    QString partPath;
    std::shared_ptr<QFile> pFile;
    QNetworkReply* pReply{nullptr};
};

class QmlStreamingProxy : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(StreamingManager)
    QML_SINGLETON

    Q_PROPERTY(QString subsonicUrl READ subsonicUrl WRITE setSubsonicUrl NOTIFY subsonicUrlChanged)
    Q_PROPERTY(QString subsonicUsername READ subsonicUsername WRITE setSubsonicUsername NOTIFY subsonicUsernameChanged)
    Q_PROPERTY(QString subsonicPassword READ subsonicPassword WRITE setSubsonicPassword NOTIFY subsonicPasswordChanged)
    Q_PROPERTY(bool isSubsonicConnected READ isSubsonicConnected NOTIFY subsonicConnectionChanged)
    Q_PROPERTY(QString subsonicServerVersion READ subsonicServerVersion NOTIFY subsonicConnectionChanged)
    Q_PROPERTY(QString cacheDirectory READ cacheDirectory CONSTANT)
    Q_PROPERTY(qint64 cacheSizeBytes READ cacheSizeBytes NOTIFY cacheSizeChanged)
    Q_PROPERTY(int activeDownloadCount READ activeDownloadCount NOTIFY activeDownloadsChanged)

  public:
    explicit QmlStreamingProxy(QObject* parent = nullptr);
    ~QmlStreamingProxy() override;

    static QmlStreamingProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager) {
        s_pPlayerManager = std::move(pPlayerManager);
    }
    static void registerUserSettings(UserSettingsPointer pConfig) {
        s_pConfig = std::move(pConfig);
    }

    QString subsonicUrl() const;
    void setSubsonicUrl(const QString& url);

    QString subsonicUsername() const;
    void setSubsonicUsername(const QString& username);

    QString subsonicPassword() const;
    void setSubsonicPassword(const QString& password);

    bool isSubsonicConnected() const {
        return m_isSubsonicConnected;
    }
    QString subsonicServerVersion() const {
        return m_subsonicServerVersion;
    }
    QString cacheDirectory() const {
        return m_cacheDir;
    }
    qint64 cacheSizeBytes() const;
    int activeDownloadCount() const {
        return m_activeDownloads.size();
    }

    // Subsonic / Navidrome API
    Q_INVOKABLE void testSubsonicConnection();
    Q_INVOKABLE void searchSubsonic(const QString& query);
    Q_INVOKABLE void getSubsonicPlaylists();
    Q_INVOKABLE void getSubsonicPlaylistSongs(const QString& playlistId);
    Q_INVOKABLE QString getSubsonicStreamUrl(const QString& songId) const;
    Q_INVOKABLE QString getSubsonicCoverArtUrl(const QString& coverArtId, int size = 160) const;

    // Audius / Online Commercial DJ Streaming API
    Q_INVOKABLE void searchAudius(const QString& query);
    Q_INVOKABLE void getAudiusTrending(const QString& genre = QStringLiteral("Electronic"));

    // Track Cache & Download Engine
    Q_INVOKABLE void downloadAndLoad(
            const QString& streamUrl,
            const QString& service,
            const QString& trackId,
            const QString& title,
            const QString& artist,
            const QString& targetDeck,
            bool autoPlay = false);

    Q_INVOKABLE void cancelDownload(const QString& trackId);
    Q_INVOKABLE bool isTrackCached(const QString& service, const QString& trackId) const;
    Q_INVOKABLE QString getCachedTrackPath(const QString& service, const QString& trackId) const;
    Q_INVOKABLE void clearCache();
    Q_INVOKABLE void refreshCacheSize();

  signals:
    void subsonicUrlChanged();
    void subsonicUsernameChanged();
    void subsonicPasswordChanged();
    void subsonicConnectionChanged();
    void cacheSizeChanged();
    void activeDownloadsChanged();

    void subsonicConnectionTested(bool success, const QString& message, const QString& serverVersion);
    void subsonicSearchFinished(bool success, const QVariantList& songs, const QString& errorString);
    void subsonicPlaylistsFinished(bool success, const QVariantList& playlists, const QString& errorString);
    void subsonicPlaylistSongsFinished(bool success, const QVariantList& songs, const QString& errorString);

    void audiusSearchFinished(bool success, const QVariantList& tracks, const QString& errorString);
    void audiusTrendingFinished(bool success, const QVariantList& tracks, const QString& errorString);

    void downloadProgress(const QString& trackId, qreal progress, qint64 bytesReceived, qint64 bytesTotal);
    void downloadFinished(const QString& trackId, const QString& localPath, const QString& targetDeck, bool autoPlay);
    void downloadFailed(const QString& trackId, const QString& errorMessage);

  private:
    QString buildSubsonicUrl(const QString& endpoint, const QString& extraParams = QString()) const;
    QString sanitizeFileName(const QString& input) const;
    QString buildTrackFileName(
            const QString& service,
            const QString& trackId,
            const QString& title,
            const QString& artist,
            const QString& ext) const;

    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;
    static inline UserSettingsPointer s_pConfig;

    QNetworkAccessManager* m_pNetworkManager;
    QString m_subsonicUrl;
    QString m_subsonicUsername;
    QString m_subsonicPassword;
    bool m_isSubsonicConnected{false};
    QString m_subsonicServerVersion;
    QString m_cacheDir;

    QMap<QString, std::shared_ptr<StreamingDownloadJob>> m_activeDownloads;
};

} // namespace qml
} // namespace mixxx
