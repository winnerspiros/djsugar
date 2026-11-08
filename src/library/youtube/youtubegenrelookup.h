#pragma once

#include <QByteArray>
#include <QObject>
#include <QString>
#include <functional>

class QNetworkAccessManager;

namespace mixxx {

/// Finds the genre of a song downloaded from YouTube. YouTube itself only
/// knows the category ("Music"), so the artist and title are looked up in the
/// iTunes Search API (US, then the Greek store) and, failing that, in Deezer's
/// catalogue. Both are free and need no account. A hit only counts when artist,
/// title and (if known) duration agree, so a namesake song from another artist
/// is not taken.
class YouTubeGenreLookup : public QObject {
    Q_OBJECT
  public:
    using Callback = std::function<void(const QString& genre)>;

    explicit YouTubeGenreLookup(QObject* parent = nullptr);

    /// `done` is called once on the object's thread, with an empty genre
    /// when no source knows the song.
    void lookup(const QString& artist,
            const QString& title,
            int durationSec,
            const Callback& done);

    /// Lower-case, accents, punctuation and "feat." credits removed.
    static QString normalize(const QString& text);
    /// True if a catalogue entry is the same song as the YouTube video.
    static bool isSameSong(const QString& artist,
            const QString& title,
            int durationSec,
            const QString& candidateArtist,
            const QString& candidateTitle,
            int candidateDurationSec);
    /// Genre of the first matching song in an iTunes Search API reply.
    static QString genreFromITunes(const QByteArray& json,
            const QString& artist,
            const QString& title,
            int durationSec);
    /// Album id of the first matching song in a Deezer search reply, or 0.
    static qint64 albumFromDeezer(const QByteArray& json,
            const QString& artist,
            const QString& title,
            int durationSec);
    /// Genres of a Deezer album reply, joined with "/".
    static QString genreFromDeezerAlbum(const QByteArray& json);

  private:
    void lookupITunes(const QString& artist,
            const QString& title,
            int durationSec,
            const QString& country,
            const Callback& done);
    void lookupDeezer(const QString& artist,
            const QString& title,
            int durationSec,
            const Callback& done);

    QNetworkAccessManager* m_pNam;
};

} // namespace mixxx
