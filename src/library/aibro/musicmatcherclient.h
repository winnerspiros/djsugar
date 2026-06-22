#pragma once

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QList>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QObject>
#include <QUrl>

#include "track/track_decl.h"

namespace mixxx {

/// A single track suggestion from Music Matcher.
struct MusicMatcherSuggestion {
    QString title;
    QString artist;
    QString spotifyId;
    QString spotifyUrl;
    QString imageUrl;
    QString previewUrl;
    double similarityScore = 0.0;
};

/// Music Matcher client — finds similar songs using Deezer's recommendation
/// engine (free, no API key required). Replaces the non-functional
/// musicmatcher.app HTTP API with a working alternative that provides
/// AI-powered similar song recommendations.
class MusicMatcherClient : public QObject {
    Q_OBJECT
  public:
    explicit MusicMatcherClient(QObject* parent = nullptr);
    ~MusicMatcherClient() override = default;

    /// Find similar songs for a query (artist name, song title, or both).
    /// Emits suggestionsReady() on success or suggestionsFailed() on error.
    void findSimilar(const QString& query, int limit = 12);

    /// Cancel any in-flight request.
    void cancel();

  signals:
    void suggestionsReady(const QList<mixxx::MusicMatcherSuggestion>& suggestions);
    void suggestionsFailed(const QString& error);

  private slots:
    void onReplyFinished(QNetworkReply* reply);

  private:
    void fetchArtistRadio(int artistId, int limit);
    void searchGeneral(const QString& query, int limit);

    enum class SearchType {
        FindArtist,
        ArtistRadio,
        GeneralSearch,
    };

    QNetworkAccessManager* m_pNam;
    SearchType m_searchType = SearchType::FindArtist;
    int m_pendingLimit = 12;
    QString m_pendingQuery;
};

} // namespace mixxx
