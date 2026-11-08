#include "library/youtube/youtubegenrelookup.h"

#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QRegularExpression>
#include <QStringList>
#include <QUrl>
#include <QUrlQuery>
#include <cstdlib>

namespace mixxx {

namespace {

constexpr int kTimeoutMs = 8000;
const QString kGreekArtistPass = QStringLiteral("GR-artist");
// Catalogue versions of a song (radio edit, album version) differ a little
// in length from the video; more than this is another recording or a video
// with a long skit.
constexpr int kMaxDurationDiffSec = 40;

// Genres too vague to help a DJ sort a crate.
bool isUselessGenre(const QString& genre) {
    const QString g = genre.trimmed().toLower();
    return g.isEmpty() || g == QStringLiteral("music") || g == QStringLiteral("all") ||
            g == QStringLiteral("other") || g == QStringLiteral("unknown");
}

QNetworkRequest makeRequest(const QUrl& url) {
    QNetworkRequest request(url);
    request.setRawHeader("User-Agent", "DJSugar/GenreLookup");
    request.setTransferTimeout(kTimeoutMs);
    return request;
}

// Greek to Greeklish, the way the iTunes and Deezer catalogues spell Greek
// artists ("Παντελής Παντελίδης" is "Pantelis Pantelidis"). Input is
// lower-case without accents.
QString greekToLatin(const QString& text) {
    static const QHash<QString, QString> kPairs = {
            {QStringLiteral("ου"), QStringLiteral("ou")},
            {QStringLiteral("ει"), QStringLiteral("i")},
            {QStringLiteral("οι"), QStringLiteral("i")},
            {QStringLiteral("αι"), QStringLiteral("e")},
            {QStringLiteral("μπ"), QStringLiteral("mp")},
            {QStringLiteral("ντ"), QStringLiteral("nt")},
            {QStringLiteral("γκ"), QStringLiteral("gk")},
    };
    static const QHash<QChar, QString> kLetters = {
            {u'α', QStringLiteral("a")},
            {u'β', QStringLiteral("v")},
            {u'γ', QStringLiteral("g")},
            {u'δ', QStringLiteral("d")},
            {u'ε', QStringLiteral("e")},
            {u'ζ', QStringLiteral("z")},
            {u'η', QStringLiteral("i")},
            {u'θ', QStringLiteral("th")},
            {u'ι', QStringLiteral("i")},
            {u'κ', QStringLiteral("k")},
            {u'λ', QStringLiteral("l")},
            {u'μ', QStringLiteral("m")},
            {u'ν', QStringLiteral("n")},
            {u'ξ', QStringLiteral("x")},
            {u'ο', QStringLiteral("o")},
            {u'π', QStringLiteral("p")},
            {u'ρ', QStringLiteral("r")},
            {u'σ', QStringLiteral("s")},
            {u'ς', QStringLiteral("s")},
            {u'τ', QStringLiteral("t")},
            {u'υ', QStringLiteral("i")},
            {u'φ', QStringLiteral("f")},
            {u'χ', QStringLiteral("ch")},
            {u'ψ', QStringLiteral("ps")},
            {u'ω', QStringLiteral("o")},
    };
    QString out;
    out.reserve(text.size());
    for (qsizetype i = 0; i < text.size(); ++i) {
        const auto pair = kPairs.constFind(text.mid(i, 2));
        if (pair != kPairs.constEnd()) {
            out += *pair;
            ++i;
            continue;
        }
        const auto letter = kLetters.constFind(text.at(i));
        if (letter != kLetters.constEnd()) {
            out += *letter;
        } else {
            out += text.at(i);
        }
    }
    return out;
}

// The single artists of a credit such as "Daft Punk, Pharrell & Nile
// Rodgers" (already normalized, so "&" reads "and").
QStringList artistCredits(const QString& normalizedArtist) {
    static const QRegularExpression kSeparators(QStringLiteral(R"(\s+(?:and|x|vs)\s+)"));
    QStringList credits = normalizedArtist.split(kSeparators, Qt::SkipEmptyParts);
    credits.append(normalizedArtist);
    return credits;
}

} // namespace

YouTubeGenreLookup::YouTubeGenreLookup(QObject* parent)
        : QObject(parent),
          m_pNam(new QNetworkAccessManager(this)) {
}

// static
QString YouTubeGenreLookup::normalize(const QString& text) {
    static const QRegularExpression kCredits(
            QStringLiteral(
                    R"(\s*[\[(]?\b(feat|ft|featuring|prod|with)\b\.?.*$)"),
            QRegularExpression::CaseInsensitiveOption);
    static const QRegularExpression kBrackets(QStringLiteral(R"(\s*[\[(][^\])]*[\])])"));
    static const QRegularExpression kNonWord(QStringLiteral(R"([^\p{L}\p{N}]+)"));
    QString s = text;
    s.remove(kBrackets);
    s.remove(kCredits);
    s = s.normalized(QString::NormalizationForm_KD);
    QString out;
    out.reserve(s.size());
    for (const QChar c : std::as_const(s)) {
        if (c.category() != QChar::Mark_NonSpacing) {
            out.append(c.toLower());
        }
    }
    out = greekToLatin(out);
    out.replace(QStringLiteral("&"), QStringLiteral(" and "));
    out.replace(kNonWord, QStringLiteral(" "));
    return out.simplified();
}

// static
bool YouTubeGenreLookup::isSameSong(const QString& artist,
        const QString& title,
        int durationSec,
        const QString& candidateArtist,
        const QString& candidateTitle,
        int candidateDurationSec) {
    const QString a = normalize(artist);
    const QString t = normalize(title);
    const QString ca = normalize(candidateArtist);
    const QString ct = normalize(candidateTitle);
    if (a.isEmpty() || t.isEmpty() || ca.isEmpty() || ct.isEmpty()) {
        return false;
    }
    // "Daft Punk" vs "Daft Punk & Pharrell": the credits share an artist.
    // Whole names only, "Rack" is not "Roof Rack".
    const QStringList credits = artistCredits(a);
    const QStringList candidateCredits = artistCredits(ca);
    bool artistOk = false;
    for (const QString& credit : credits) {
        artistOk = artistOk || candidateCredits.contains(credit);
    }
    // "One More Time" vs "One More Time - Radio Edit".
    const bool titleOk = t == ct || ct.startsWith(t + QLatin1Char(' ')) ||
            t.startsWith(ct + QLatin1Char(' '));
    if (!artistOk || !titleOk) {
        return false;
    }
    if (durationSec > 0 && candidateDurationSec > 0 &&
            std::abs(durationSec - candidateDurationSec) > kMaxDurationDiffSec) {
        return false;
    }
    return true;
}

// static
QString YouTubeGenreLookup::genreFromITunes(const QByteArray& json,
        const QString& artist,
        const QString& title,
        int durationSec) {
    const QJsonArray results =
            QJsonDocument::fromJson(json).object().value(QStringLiteral("results")).toArray();
    for (const QJsonValue& value : results) {
        const QJsonObject song = value.toObject();
        const int songSec = song.value(QStringLiteral("trackTimeMillis")).toInt() / 1000;
        if (!isSameSong(artist,
                    title,
                    durationSec,
                    song.value(QStringLiteral("artistName")).toString(),
                    song.value(QStringLiteral("trackName")).toString(),
                    songSec)) {
            continue;
        }
        const QString genre = song.value(QStringLiteral("primaryGenreName")).toString();
        if (!isUselessGenre(genre)) {
            return genre.trimmed();
        }
    }
    // The song itself is not listed (or spelled another way, common with
    // Greek titles): the genre the artist's own songs share.
    const QString wanted = normalize(artist);
    QHash<QString, int> votes;
    for (const QJsonValue& value : results) {
        const QJsonObject song = value.toObject();
        const QString genre = song.value(QStringLiteral("primaryGenreName")).toString().trimmed();
        if (!isUselessGenre(genre) &&
                normalize(song.value(QStringLiteral("artistName")).toString()) == wanted) {
            ++votes[genre];
        }
    }
    QString best;
    int bestVotes = 1; // one song is not a pattern
    for (auto it = votes.constBegin(); it != votes.constEnd(); ++it) {
        if (it.value() > bestVotes) {
            best = it.key();
            bestVotes = it.value();
        }
    }
    return best;
}

// static
qint64 YouTubeGenreLookup::albumFromDeezer(const QByteArray& json,
        const QString& artist,
        const QString& title,
        int durationSec) {
    const QJsonArray data =
            QJsonDocument::fromJson(json).object().value(QStringLiteral("data")).toArray();
    for (const QJsonValue& value : data) {
        const QJsonObject song = value.toObject();
        if (!isSameSong(artist,
                    title,
                    durationSec,
                    song.value(QStringLiteral("artist"))
                            .toObject()
                            .value(QStringLiteral("name"))
                            .toString(),
                    song.value(QStringLiteral("title")).toString(),
                    song.value(QStringLiteral("duration")).toInt())) {
            continue;
        }
        const qint64 albumId = song.value(QStringLiteral("album"))
                                       .toObject()
                                       .value(QStringLiteral("id"))
                                       .toInteger();
        if (albumId > 0) {
            return albumId;
        }
    }
    return 0;
}

// static
QString YouTubeGenreLookup::genreFromDeezerAlbum(const QByteArray& json) {
    const QJsonArray genres = QJsonDocument::fromJson(json)
                                      .object()
                                      .value(QStringLiteral("genres"))
                                      .toObject()
                                      .value(QStringLiteral("data"))
                                      .toArray();
    QStringList names;
    for (const QJsonValue& value : genres) {
        const QString name = value.toObject().value(QStringLiteral("name")).toString().trimmed();
        if (!isUselessGenre(name) && !names.contains(name)) {
            names.append(name);
        }
        if (names.size() == 2) {
            break;
        }
    }
    return names.join(QLatin1Char('/'));
}

void YouTubeGenreLookup::lookup(const QString& artist,
        const QString& title,
        int durationSec,
        const Callback& done) {
    if (normalize(artist).isEmpty() || normalize(title).isEmpty()) {
        done(QString());
        return;
    }
    lookupITunes(artist, title, durationSec, QStringLiteral("US"), done);
}

void YouTubeGenreLookup::lookupITunes(const QString& artist,
        const QString& title,
        int durationSec,
        const QString& country,
        const Callback& done) {
    QUrl url(QStringLiteral("https://itunes.apple.com/search"));
    QUrlQuery query;
    // Greeklish finds Greek artists; Latin text is unchanged by normalize's
    // transliteration beyond lower-casing, which the API ignores anyway.
    // The last pass searches the artist alone, for the artist-genre fallback
    // when the song title is spelled differently in the catalogue.
    const bool artistOnly = country == kGreekArtistPass;
    query.addQueryItem(QStringLiteral("term"),
            artistOnly ? normalize(artist)
                       : normalize(artist) + QLatin1Char(' ') + normalize(title));
    query.addQueryItem(QStringLiteral("entity"), QStringLiteral("song"));
    query.addQueryItem(QStringLiteral("limit"), QStringLiteral("15"));
    query.addQueryItem(QStringLiteral("country"), artistOnly ? QStringLiteral("GR") : country);
    url.setQuery(query);
    QNetworkReply* reply = m_pNam->get(makeRequest(url));
    connect(reply,
            &QNetworkReply::finished,
            this,
            [this, reply, artist, title, durationSec, country, done]() {
                reply->deleteLater();
                const QString genre = reply->error() == QNetworkReply::NoError
                        ? genreFromITunes(reply->readAll(), artist, title, durationSec)
                        : QString();
                if (!genre.isEmpty()) {
                    done(genre);
                    return;
                }
                // Greek artists are often only in the Greek store.
                if (country == QStringLiteral("US")) {
                    lookupITunes(artist, title, durationSec, QStringLiteral("GR"), done);
                    return;
                }
                if (country == QStringLiteral("GR")) {
                    lookupITunes(artist, title, durationSec, kGreekArtistPass, done);
                    return;
                }
                lookupDeezer(artist, title, durationSec, done);
            });
}

void YouTubeGenreLookup::lookupDeezer(const QString& artist,
        const QString& title,
        int durationSec,
        const Callback& done) {
    QUrl url(QStringLiteral("https://api.deezer.com/search"));
    QUrlQuery query;
    query.addQueryItem(QStringLiteral("q"),
            QStringLiteral("artist:\"%1\" track:\"%2\"").arg(artist, title));
    query.addQueryItem(QStringLiteral("limit"), QStringLiteral("10"));
    url.setQuery(query);
    QNetworkReply* reply = m_pNam->get(makeRequest(url));
    connect(reply,
            &QNetworkReply::finished,
            this,
            [this, reply, artist, title, durationSec, done]() {
                reply->deleteLater();
                const qint64 albumId = reply->error() == QNetworkReply::NoError
                        ? albumFromDeezer(
                                  reply->readAll(), artist, title, durationSec)
                        : 0;
                if (albumId <= 0) {
                    done(QString());
                    return;
                }
                QNetworkReply* albumReply = m_pNam->get(makeRequest(
                        QUrl(QStringLiteral("https://api.deezer.com/album/%1")
                                        .arg(albumId))));
                connect(albumReply,
                        &QNetworkReply::finished,
                        this,
                        [albumReply, done]() {
                            albumReply->deleteLater();
                            done(albumReply->error() == QNetworkReply::NoError
                                            ? genreFromDeezerAlbum(
                                                      albumReply->readAll())
                                            : QString());
                        });
            });
}

} // namespace mixxx

#include "moc_youtubegenrelookup.cpp"
