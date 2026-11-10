#include "library/youtube/youtubeaccount.h"

#include <QCryptographicHash>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QSet>
#include <QStandardPaths>

#include "moc_youtubeaccount.cpp"
#include "util/logger.h"

namespace mixxx {

namespace {

const Logger kLogger("YouTubeAccount");

const QString kOrigin = QStringLiteral("https://music.youtube.com");
// The YouTube Music web player, whose requests these are
const QString kClientVersion = QStringLiteral("1.20251103.01.00");
const QByteArray kUserAgent =
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/142.0.0.0 Safari/537.36";
constexpr int kTimeoutMs = 20000;
// Liked playlists come a page at a time; a library rarely has more
constexpr int kMaxPlaylistPages = 10;

QString cookieFile() {
    const QString dir =
            QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation))
                    .filePath(QStringLiteral("youtube"));
    QDir().mkpath(dir);
    return QDir(dir).filePath(QStringLiteral("account"));
}

QString cookieValue(const QString& cookies, const QString& name) {
    const QStringList parts = cookies.split(QLatin1Char(';'));
    for (const QString& part : parts) {
        const QString pair = part.trimmed();
        if (pair.startsWith(name + QLatin1Char('='))) {
            return pair.mid(name.size() + 1);
        }
    }
    return QString();
}

/// The SAPISID cookie, which the account's requests are signed with
QString sapisid(const QString& cookies) {
    const QString value = cookieValue(cookies, QStringLiteral("SAPISID"));
    return value.isEmpty() ? cookieValue(cookies, QStringLiteral("__Secure-3PAPISID")) : value;
}

/// Text of {"runs": [{"text"}...]} or {"simpleText"}
QString text(const QJsonValue& value) {
    const QJsonObject object = value.toObject();
    if (object.contains(QStringLiteral("simpleText"))) {
        return object.value(QStringLiteral("simpleText")).toString();
    }
    QString out;
    const QJsonArray runs = object.value(QStringLiteral("runs")).toArray();
    for (const QJsonValue& run : runs) {
        out += run.toObject().value(QStringLiteral("text")).toString();
    }
    return out;
}

/// "3:45" or "1:02:03" in seconds
int seconds(const QString& duration) {
    int total = 0;
    const QStringList parts = duration.split(QLatin1Char(':'));
    for (const QString& part : parts) {
        bool ok = false;
        const int n = part.toInt(&ok);
        if (!ok) {
            return 0;
        }
        total = total * 60 + n;
    }
    return total;
}

/// Calls `visit` on every object inside `root`
void walk(const QJsonValue& root, const std::function<bool(const QJsonObject&)>& visit) {
    QList<QJsonValue> stack{root};
    while (!stack.isEmpty()) {
        const QJsonValue value = stack.takeLast();
        if (value.isObject()) {
            const QJsonObject object = value.toObject();
            if (visit(object)) {
                continue; // matched: not looked into
            }
            for (auto it = object.end(); it != object.begin();) {
                --it;
                stack.append(it.value());
            }
        } else if (value.isArray()) {
            const QJsonArray array = value.toArray();
            for (auto it = array.end(); it != array.begin();) {
                --it;
                stack.append(*it);
            }
        }
    }
}

QString flexColumn(const QJsonObject& item, int column) {
    return text(item.value(QStringLiteral("flexColumns"))
                    .toArray()
                    .at(column)
                    .toObject()
                    .value(QStringLiteral("musicResponsiveListItemFlexColumnRenderer"))
                    .toObject()
                    .value(QStringLiteral("text")));
}

QString browseId(const QJsonObject& renderer) {
    return renderer.value(QStringLiteral("navigationEndpoint"))
            .toObject()
            .value(QStringLiteral("browseEndpoint"))
            .toObject()
            .value(QStringLiteral("browseId"))
            .toString();
}

QString lastThumbnail(const QJsonValue& value) {
    QString url;
    walk(value, [&url](const QJsonObject& object) {
        const QJsonArray thumbnails = object.value(QStringLiteral("thumbnails")).toArray();
        if (thumbnails.isEmpty()) {
            return false;
        }
        url = thumbnails.last().toObject().value(QStringLiteral("url")).toString();
        return true;
    });
    return url;
}

/// The page's playable tracks (unavailable ones have no video id)
QList<YouTubeVideoInfo> tracks(const QJsonObject& page) {
    QList<YouTubeVideoInfo> out;
    QSet<QString> seen;
    walk(page, [&](const QJsonObject& object) {
        const QJsonObject item =
                object.value(QStringLiteral("musicResponsiveListItemRenderer")).toObject();
        if (item.isEmpty()) {
            return false;
        }
        QString id = item.value(QStringLiteral("playlistItemData"))
                             .toObject()
                             .value(QStringLiteral("videoId"))
                             .toString();
        if (id.isEmpty()) {
            walk(item.value(QStringLiteral("overlay")), [&id](const QJsonObject& o) {
                id = o.value(QStringLiteral("watchEndpoint"))
                             .toObject()
                             .value(QStringLiteral("videoId"))
                             .toString();
                return !id.isEmpty();
            });
        }
        if (id.isEmpty() || seen.contains(id)) {
            return true;
        }
        seen.insert(id);
        YouTubeVideoInfo video;
        video.id = id;
        video.title = flexColumn(item, 0);
        video.uploader = flexColumn(item, 1);
        video.durationSec = seconds(
                text(item.value(QStringLiteral("fixedColumns"))
                                .toArray()
                                .at(0)
                                .toObject()
                                .value(QStringLiteral(
                                        "musicResponsiveListItemFixedColumnRenderer"))
                                .toObject()
                                .value(QStringLiteral("text"))));
        video.genre = YouTubeVideoInfo::inferGenre(video.title, video.uploader);
        out.append(video);
        return true;
    });
    return out;
}

/// The token of the page after this one ("" at the end)
QString continuation(const QJsonObject& page) {
    QString token;
    walk(page, [&token](const QJsonObject& object) {
        token = object.value(QStringLiteral("continuationCommand"))
                        .toObject()
                        .value(QStringLiteral("token"))
                        .toString();
        if (token.isEmpty()) {
            token = object.value(QStringLiteral("nextContinuationData"))
                            .toObject()
                            .value(QStringLiteral("continuation"))
                            .toString();
        }
        return !token.isEmpty();
    });
    return token;
}

} // namespace

YouTubeAccount& YouTubeAccount::instance() {
    static YouTubeAccount* s_pInstance = new YouTubeAccount();
    return *s_pInstance;
}

YouTubeAccount::YouTubeAccount()
        : m_pNetwork(new QNetworkAccessManager(this)) {
    QFile file(cookieFile());
    if (file.open(QIODevice::ReadOnly)) {
        m_cookies = QString::fromUtf8(file.readAll()).trimmed();
    }
    if (signedIn()) {
        fetchName();
    }
}

void YouTubeAccount::setCookies(const QString& cookies) {
    if (sapisid(cookies).isEmpty()) {
        kLogger.info() << "Sign-in page closed without a sign-in";
        emit failed(tr("Not signed in"));
        return;
    }
    QFile file(cookieFile());
    if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        file.write(cookies.toUtf8());
    }
    m_cookies = cookies;
    m_name.clear();
    emit accountChanged();
    fetchName();
}

void YouTubeAccount::signOut() {
    QFile::remove(cookieFile());
    m_cookies.clear();
    m_name.clear();
    m_continuation.clear();
    ++m_generation;
    emit accountChanged();
}

void YouTubeAccount::post(const QString& endpoint,
        const QJsonObject& body,
        const std::function<void(const QJsonObject&)>& done) {
    QNetworkRequest request(QUrl(kOrigin + QStringLiteral("/youtubei/v1/") + endpoint +
            QStringLiteral("?prettyPrint=false")));
    request.setHeader(QNetworkRequest::ContentTypeHeader, QStringLiteral("application/json"));
    request.setRawHeader("User-Agent", kUserAgent);
    request.setRawHeader("Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Goog-AuthUser", "0");
    request.setRawHeader("X-YouTube-Client-Name", "67");
    request.setRawHeader("X-YouTube-Client-Version", kClientVersion.toUtf8());
    request.setRawHeader("Cookie", m_cookies.toUtf8());
    // Signed as the web player signs: SHA-1 of "<time> <SAPISID> <origin>"
    const QString now = QString::number(QDateTime::currentSecsSinceEpoch());
    const QByteArray hash = QCryptographicHash::hash(
            (now + QLatin1Char(' ') + sapisid(m_cookies) + QLatin1Char(' ') + kOrigin)
                    .toUtf8(),
            QCryptographicHash::Sha1)
                                    .toHex();
    request.setRawHeader("Authorization", "SAPISIDHASH " + now.toUtf8() + '_' + hash);
    request.setTransferTimeout(kTimeoutMs);

    QJsonObject full = body;
    full.insert(QStringLiteral("context"),
            QJsonObject{{QStringLiteral("client"),
                    QJsonObject{{QStringLiteral("clientName"), QStringLiteral("WEB_REMIX")},
                            {QStringLiteral("clientVersion"), kClientVersion},
                            {QStringLiteral("hl"), QStringLiteral("en")}}}});
    QNetworkReply* pReply =
            m_pNetwork->post(request, QJsonDocument(full).toJson(QJsonDocument::Compact));
    const quint64 generation = m_generation;
    connect(pReply, &QNetworkReply::finished, this, [this, pReply, generation, done, endpoint]() {
        pReply->deleteLater();
        if (generation != m_generation) {
            return;
        }
        const int status = pReply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        if (status == 401 || status == 403) {
            kLogger.warning() << endpoint << "refused the sign-in:" << status;
            signOut();
            emit failed(tr("YouTube wants a new sign-in"));
            return;
        }
        if (pReply->error() != QNetworkReply::NoError) {
            kLogger.warning() << endpoint << "failed:" << pReply->errorString();
            emit failed(pReply->errorString());
            return;
        }
        done(QJsonDocument::fromJson(pReply->readAll()).object());
    });
}

void YouTubeAccount::fetchName() {
    post(QStringLiteral("account/account_menu"), {}, [this](const QJsonObject& page) {
        QString name;
        walk(page, [&name](const QJsonObject& object) {
            if (!object.contains(QStringLiteral("accountName"))) {
                return false;
            }
            name = text(object.value(QStringLiteral("accountName")));
            return true;
        });
        if (name.isEmpty()) {
            // Signed-out answer: the cookies no longer sign anyone in
            kLogger.warning() << "YouTube knows no account for the stored sign-in";
            signOut();
            emit failed(tr("YouTube wants a new sign-in"));
            return;
        }
        m_name = name;
        emit accountChanged();
    });
}

void YouTubeAccount::openList(const QString& list) {
    ++m_generation;
    m_continuation.clear();
    const QString id = list == QStringLiteral("liked") ? QStringLiteral("VLLM") : list;
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("browseId"), id}},
            [this](const QJsonObject& page) {
                m_continuation = continuation(page);
                emit tracksReady(tracks(page), false);
            });
}

void YouTubeAccount::more() {
    if (m_continuation.isEmpty()) {
        emit tracksReady({}, true);
        return;
    }
    const QString token = m_continuation;
    m_continuation.clear();
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("continuation"), token}},
            [this](const QJsonObject& page) {
                m_continuation = continuation(page);
                emit tracksReady(tracks(page), true);
            });
}

void YouTubeAccount::fetchPlaylists() {
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("browseId"), QStringLiteral("FEmusic_liked_playlists")}},
            [this](const QJsonObject& page) {
                collectPlaylists(page, {}, 1);
            });
}

void YouTubeAccount::collectPlaylists(const QJsonObject& page, QVariantList playlists, int pages) {
    walk(page, [&playlists](const QJsonObject& object) {
        for (const QString& kind : {QStringLiteral("musicTwoRowItemRenderer"),
                     QStringLiteral("musicResponsiveListItemRenderer")}) {
            const QJsonObject item = object.value(kind).toObject();
            if (item.isEmpty()) {
                continue;
            }
            const QString id = browseId(item);
            // Playlists only ("VL<playlist id>"), not "New playlist" and
            // the like
            if (!id.startsWith(QStringLiteral("VL"))) {
                return true;
            }
            const bool twoRow = kind == QStringLiteral("musicTwoRowItemRenderer");
            playlists.append(QVariantMap{
                    {QStringLiteral("id"), id},
                    {QStringLiteral("name"),
                            twoRow ? text(item.value(QStringLiteral("title")))
                                   : flexColumn(item, 0)},
                    {QStringLiteral("subtitle"),
                            twoRow ? text(item.value(QStringLiteral("subtitle")))
                                   : flexColumn(item, 1)},
                    {QStringLiteral("artwork"),
                            lastThumbnail(item.value(twoRow
                                            ? QStringLiteral("thumbnailRenderer")
                                            : QStringLiteral("thumbnail")))},
            });
            return true;
        }
        return false;
    });
    const QString token = continuation(page);
    if (token.isEmpty() || pages >= kMaxPlaylistPages) {
        emit playlistsReady(playlists);
        return;
    }
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("continuation"), token}},
            [this, playlists, pages](const QJsonObject& next) {
                collectPlaylists(next, playlists, pages + 1);
            });
}

} // namespace mixxx
