#include "library/aibro/musicmatcherclient.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkRequest>
#include <QUrl>
#include <QUrlQuery>

#include "util/logger.h"

namespace mixxx {

const QString kDeezerBaseUrl = QStringLiteral("https://api.deezer.com");

MusicMatcherClient::MusicMatcherClient(QObject* parent)
        : QObject(parent),
          m_pNam(new QNetworkAccessManager(this)) {
    connect(m_pNam,
            &QNetworkAccessManager::finished,
            this,
            &MusicMatcherClient::onReplyFinished);
}

void MusicMatcherClient::findSimilar(const QString& query, int limit) {
    m_pendingLimit = limit;
    m_pendingQuery = query;

    // Step 1: Search Deezer for the artist from the query
    // Query can be "Artist - Title" or just "Artist" or just "Title"
    QString artist = query;
    QString title;

    static const QRegularExpression dashRe(
            QStringLiteral("\\s*[-–—]\\s*"),
            QRegularExpression::CaseInsensitiveOption);
    QStringList parts = query.split(dashRe, Qt::SkipEmptyParts);
    if (parts.size() >= 2) {
        artist = parts.first().trimmed();
        title = parts.last().trimmed();
    }

    // Search for artist on Deezer
    QUrl searchUrl(kDeezerBaseUrl + QStringLiteral("/search/artist"));
    QUrlQuery q;
    q.addQueryItem(QStringLiteral("q"), artist);
    q.addQueryItem(QStringLiteral("limit"), QStringLiteral("1"));
    searchUrl.setQuery(q);

    QNetworkRequest req(searchUrl);
    req.setTransferTimeout(10000);
    m_searchType = SearchType::FindArtist;
    m_pNam->get(req);

    qDebug() << "MusicMatcherClient: searching Deezer artist:" << artist;
}

void MusicMatcherClient::cancel() {
    const auto replies = m_pNam->findChildren<QNetworkReply*>();
    for (QNetworkReply* reply : replies) {
        reply->abort();
    }
}

void MusicMatcherClient::onReplyFinished(QNetworkReply* reply) {
    reply->deleteLater();

    if (reply->error() != QNetworkReply::NoError) {
        const QString err = reply->errorString();
        qWarning() << "MusicMatcherClient: request failed:" << err;
        emit suggestionsFailed(err);
        return;
    }

    const QByteArray data = reply->readAll();
    QJsonDocument doc = QJsonDocument::fromJson(data);
    if (doc.isNull() || !doc.isObject()) {
        emit suggestionsFailed(QStringLiteral("Invalid JSON response"));
        return;
    }

    const QJsonObject root = doc.object();

    if (m_searchType == SearchType::FindArtist) {
        // Parse artist search results
        const QJsonArray results = root.value(QStringLiteral("data")).toArray();
        if (results.isEmpty()) {
            // No artist found — try a general search with the full query
            searchGeneral(m_pendingQuery, m_pendingLimit);
            return;
        }

        const QJsonObject artistObj = results.first().toObject();
        const int artistId = artistObj.value(QStringLiteral("id")).toInt();
        const QString artistName = artistObj.value(QStringLiteral("name")).toString();

        qDebug() << "MusicMatcherClient: found artist:" << artistName
                 << "id:" << artistId;

        // Step 2: Get artist radio (similar/recommended tracks)
        fetchArtistRadio(artistId, m_pendingLimit);
    } else if (m_searchType == SearchType::ArtistRadio) {
        // Parse artist radio results
        const QJsonArray results = root.value(QStringLiteral("data")).toArray();
        if (results.isEmpty()) {
            emit suggestionsFailed(QStringLiteral("No similar tracks found"));
            return;
        }

        QList<MusicMatcherSuggestion> suggestions;
        suggestions.reserve(qMin(results.size(), m_pendingLimit));

        for (const QJsonValue& val : results) {
            const QJsonObject trackObj = val.toObject();
            MusicMatcherSuggestion s;
            s.title = trackObj.value(QStringLiteral("title")).toString();

            const QJsonObject artistObj =
                    trackObj.value(QStringLiteral("artist")).toObject();
            s.artist = artistObj.value(QStringLiteral("name")).toString();

            s.previewUrl =
                    trackObj.value(QStringLiteral("preview")).toString();

            // Build a search-friendly query for YouTube
            s.title = s.title;
            s.artist = s.artist;

            if (!s.title.isEmpty() && !s.artist.isEmpty()) {
                // Use position as similarity score (Deezer returns in
                // relevance order)
                double idx = suggestions.size();
                s.similarityScore =
                        qMax(0.5, 1.0 - (idx / m_pendingLimit) * 0.5);
                suggestions.append(s);
            }
        }

        if (suggestions.isEmpty()) {
            emit suggestionsFailed(
                    QStringLiteral("No usable similar tracks found"));
            return;
        }

        qDebug() << "MusicMatcherClient:" << suggestions.size()
                 << "similar tracks for" << m_pendingQuery;
        emit suggestionsReady(suggestions);
    } else if (m_searchType == SearchType::GeneralSearch) {
        // Parse general search results
        const QJsonArray results = root.value(QStringLiteral("data")).toArray();
        if (results.isEmpty()) {
            emit suggestionsFailed(
                    QStringLiteral("No results found for query"));
            return;
        }

        QList<MusicMatcherSuggestion> suggestions;
        suggestions.reserve(qMin(results.size(), m_pendingLimit));

        for (const QJsonValue& val : results) {
            const QJsonObject trackObj = val.toObject();
            MusicMatcherSuggestion s;
            s.title = trackObj.value(QStringLiteral("title")).toString();

            const QJsonObject artistObj =
                    trackObj.value(QStringLiteral("artist")).toObject();
            s.artist = artistObj.value(QStringLiteral("name")).toString();

            if (!s.title.isEmpty() && !s.artist.isEmpty()) {
                double idx = suggestions.size();
                s.similarityScore =
                        qMax(0.4, 0.9 - (idx / m_pendingLimit) * 0.5);
                suggestions.append(s);
            }
        }

        if (suggestions.isEmpty()) {
            emit suggestionsFailed(QStringLiteral("No usable tracks found"));
            return;
        }

        qDebug() << "MusicMatcherClient:" << suggestions.size()
                 << "tracks from general search for" << m_pendingQuery;
        emit suggestionsReady(suggestions);
    }
}

void MusicMatcherClient::fetchArtistRadio(int artistId, int limit) {
    QUrl url(kDeezerBaseUrl +
            QStringLiteral("/artist/%1/radio").arg(artistId));
    QUrlQuery q;
    q.addQueryItem(QStringLiteral("limit"), QString::number(limit));
    url.setQuery(q);

    QNetworkRequest req(url);
    req.setTransferTimeout(10000);
    m_searchType = SearchType::ArtistRadio;
    m_pNam->get(req);
}

void MusicMatcherClient::searchGeneral(const QString& query, int limit) {
    QUrl url(kDeezerBaseUrl + QStringLiteral("/search"));
    QUrlQuery q;
    q.addQueryItem(QStringLiteral("q"), query);
    q.addQueryItem(QStringLiteral("limit"), QString::number(limit));
    url.setQuery(q);

    QNetworkRequest req(url);
    req.setTransferTimeout(10000);
    m_searchType = SearchType::GeneralSearch;
    m_pNam->get(req);
}

} // namespace mixxx

#include "moc_musicmatcherclient.cpp"
