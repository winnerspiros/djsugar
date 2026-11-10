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
#include <QRegularExpression>
#include <QSet>
#include <QStandardPaths>
#include <cstdlib>

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
        if (video.durationSec == 0) {
            // Search rows: "Song • Artist • Album • 3:45"
            static const QRegularExpression kLength(QStringLiteral("(\\d+(?::\\d{2}){1,2})$"));
            const QRegularExpressionMatch length = kLength.match(video.uploader);
            if (length.hasMatch()) {
                video.durationSec = seconds(length.captured(1));
            }
        }
        video.genre = YouTubeVideoInfo::inferGenre(video.title, video.uploader);
        out.append(video);
        return true;
    });
    return out;
}

/// Whether YouTube answered as to a signed-in user
/// (responseContext.serviceTrackingParams: logged_in = 1)
bool signedInPage(const QJsonObject& page) {
    bool signedIn = false;
    walk(page.value(QStringLiteral("responseContext")), [&signedIn](const QJsonObject& object) {
        if (object.value(QStringLiteral("key")).toString() == QStringLiteral("logged_in")) {
            signedIn = object.value(QStringLiteral("value")).toString() == QStringLiteral("1");
            return true;
        }
        return false;
    });
    return signedIn;
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
        // Cookies, then the chosen channel's page id and name
        const QStringList lines = QString::fromUtf8(file.readAll()).split(QLatin1Char('\n'));
        m_cookies = lines.value(0).trimmed();
        m_pageId = lines.value(1).trimmed();
        m_name = lines.value(2).trimmed();
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
    m_cookies = cookies;
    m_pageId.clear();
    m_name.clear();
    save();
    emit accountChanged();
    fetchName();
}

void YouTubeAccount::save() {
    QFile file(cookieFile());
    if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        file.write((m_cookies + QLatin1Char('\n') + m_pageId + QLatin1Char('\n') + m_name)
                        .toUtf8());
    }
}

void YouTubeAccount::signOut() {
    QFile::remove(cookieFile());
    m_cookies.clear();
    m_pageId.clear();
    m_name.clear();
    m_continuation.clear();
    ++m_generation;
    emit accountChanged();
}

void YouTubeAccount::post(const QString& endpoint,
        const QJsonObject& body,
        const std::function<void(const QJsonObject&)>& done,
        bool signedRequest,
        QObject* pContext) {
    QNetworkRequest request(QUrl(kOrigin + QStringLiteral("/youtubei/v1/") + endpoint +
            QStringLiteral("?prettyPrint=false")));
    request.setHeader(QNetworkRequest::ContentTypeHeader, QStringLiteral("application/json"));
    request.setRawHeader("User-Agent", kUserAgent);
    request.setRawHeader("Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Goog-AuthUser", "0");
    request.setRawHeader("X-YouTube-Client-Name", "67");
    request.setRawHeader("X-YouTube-Client-Version", kClientVersion.toUtf8());
    request.setRawHeader("Referer", (kOrigin + QLatin1Char('/')).toUtf8());
    QJsonObject context{{QStringLiteral("client"),
            QJsonObject{{QStringLiteral("clientName"), QStringLiteral("WEB_REMIX")},
                    {QStringLiteral("clientVersion"), kClientVersion},
                    {QStringLiteral("hl"), QStringLiteral("en")}}}};
    if (signedRequest) {
        request.setRawHeader("Cookie", m_cookies.toUtf8());
        // Signed as the web player signs: SHA-1 of "<time> <SAPISID>
        // <origin>", once per cookie it has
        const QString now = QString::number(QDateTime::currentSecsSinceEpoch());
        auto sign = [&now](const QString& secret) -> QByteArray {
            return now.toUtf8() + '_' +
                    QCryptographicHash::hash(
                            (now + QLatin1Char(' ') + secret + QLatin1Char(' ') + kOrigin)
                                    .toUtf8(),
                            QCryptographicHash::Sha1)
                            .toHex();
        };
        QByteArray authorization = "SAPISIDHASH " + sign(sapisid(m_cookies));
        for (const auto& [cookie, scheme] :
                {std::pair{QStringLiteral("__Secure-1PAPISID"), QByteArray("SAPISID1PHASH ")},
                        std::pair{QStringLiteral("__Secure-3PAPISID"),
                                QByteArray("SAPISID3PHASH ")}}) {
            const QString secret = cookieValue(m_cookies, cookie);
            if (!secret.isEmpty()) {
                authorization += ' ' + scheme + sign(secret);
            }
        }
        request.setRawHeader("Authorization", authorization);
        // A brand channel of the account
        if (!m_pageId.isEmpty()) {
            request.setRawHeader("X-Goog-PageId", m_pageId.toUtf8());
            context.insert(QStringLiteral("user"),
                    QJsonObject{{QStringLiteral("onBehalfOfUser"), m_pageId}});
        }
    }
    request.setTransferTimeout(kTimeoutMs);

    QJsonObject full = body;
    full.insert(QStringLiteral("context"), context);
    QNetworkReply* pReply =
            m_pNetwork->post(request, QJsonDocument(full).toJson(QJsonDocument::Compact));
    const quint64 generation = m_generation;
    connect(pReply,
            &QNetworkReply::finished,
            pContext ? pContext : this,
            [this, pReply, generation, done, endpoint, signedRequest]() {
                pReply->deleteLater();
                if (!signedRequest) {
                    // A song lookup: its own answer, empty on any failure
                    done(QJsonDocument::fromJson(pReply->readAll()).object());
                    return;
                }
                if (generation != m_generation) {
                    return;
                }
                const int status =
                        pReply->attribute(
                                      QNetworkRequest::HttpStatusCodeAttribute)
                                .toInt();
                if (status == 401 || status == 403) {
                    kLogger.warning() << endpoint << "refused the sign-in:" << status;
                    signOut();
                    emit failed(tr("YouTube wants a new sign-in"));
                    return;
                }
                if (pReply->error() != QNetworkReply::NoError) {
                    kLogger.warning() << endpoint << "failed:" << status << pReply->errorString();
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
        if (name.isEmpty() && !signedInPage(page)) {
            kLogger.warning() << "YouTube no longer accepts the stored sign-in";
            signOut();
            emit failed(tr("YouTube wants a new sign-in"));
            return;
        }
        if (name.isEmpty()) {
            // The menu's layout changes now and then: the lists still
            // tell whether the sign-in works (refused: 401/403)
            kLogger.info() << "No account name in the account menu";
            return;
        }
        m_name = name;
        save();
        emit accountChanged();
    });
}

void YouTubeAccount::openList(const QString& list) {
    // Only the newest list's pages count (the playlists and channels
    // asked for meanwhile still arrive)
    const quint64 listGeneration = ++m_listGeneration;
    m_continuation.clear();
    const QString id = list == QStringLiteral("liked") ? QStringLiteral("VLLM") : list;
    kLogger.info() << "Opening" << id;
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("browseId"), id}},
            [this, id, listGeneration](const QJsonObject& page) {
                if (listGeneration != m_listGeneration) {
                    return;
                }
                m_continuation = continuation(page);
                const QList<YouTubeVideoInfo> list = tracks(page);
                kLogger.info() << id << "answered" << list.size() << "tracks, signed in:"
                               << signedInPage(page) << ", more:" << !m_continuation.isEmpty();
                if (list.isEmpty() && id == QStringLiteral("VLLM") && signedInPage(page)) {
                    // YouTube Music's liked songs are empty when the likes
                    // were given on YouTube: its liked videos then
                    openList(QStringLiteral("VLLL"));
                    return;
                }
                if (list.isEmpty() && !signedInPage(page)) {
                    kLogger.warning() << "YouTube answered the list as signed out";
                    emit failed(tr(
                            "YouTube did not accept the sign-in. Sign out, "
                            "then sign in again"));
                    return;
                }
                emit tracksReady(list, false);
            });
}

void YouTubeAccount::more() {
    if (m_continuation.isEmpty()) {
        emit tracksReady({}, true);
        return;
    }
    const QString token = m_continuation;
    m_continuation.clear();
    const quint64 listGeneration = m_listGeneration;
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("continuation"), token}},
            [this, listGeneration](const QJsonObject& page) {
                if (listGeneration != m_listGeneration) {
                    return;
                }
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
            // the like, nor the liked songs (their own button) and
            // "Episodes for later"
            if (!id.startsWith(QStringLiteral("VL")) || id == QStringLiteral("VLLM") ||
                    id == QStringLiteral("VLLL") || id == QStringLiteral("VLSE")) {
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
        kLogger.info() << "Playlists:" << playlists.size();
        emit playlistsReady(playlists);
        return;
    }
    post(QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("continuation"), token}},
            [this, playlists, pages](const QJsonObject& next) {
                collectPlaylists(next, playlists, pages + 1);
            });
}

void YouTubeAccount::fetchAccounts() {
    post(QStringLiteral("account/accounts_list"), {}, [this](const QJsonObject& page) {
        QVariantList accounts;
        walk(page, [this, &accounts](const QJsonObject& object) {
            const QJsonObject item = object.value(QStringLiteral("accountItem")).toObject();
            if (item.isEmpty()) {
                return false;
            }
            QString pageId;
            walk(item.value(QStringLiteral("serviceEndpoint")),
                    [&pageId](const QJsonObject& o) {
                        pageId = o.value(QStringLiteral("pageIdToken"))
                                         .toObject()
                                         .value(QStringLiteral("pageId"))
                                         .toString();
                        return !pageId.isEmpty();
                    });
            accounts.append(QVariantMap{
                    {QStringLiteral("name"), text(item.value(QStringLiteral("accountName")))},
                    {QStringLiteral("handle"),
                            text(item.value(QStringLiteral("channelHandle")))},
                    {QStringLiteral("pageId"), pageId},
                    {QStringLiteral("selected"), pageId == m_pageId},
            });
            return true;
        });
        emit accountsReady(accounts);
    });
}

void YouTubeAccount::selectAccount(const QString& pageId, const QString& name) {
    m_pageId = pageId;
    m_name = name;
    m_continuation.clear();
    ++m_generation;
    save();
    emit accountChanged();
}

void YouTubeAccount::findSong(const QString& query,
        int durationSec,
        QObject* pContext,
        const std::function<void(const QString&)>& done) {
    // The "Songs" filter of YouTube Music's search: the releases, not videos
    post(
            QStringLiteral("search"),
            QJsonObject{{QStringLiteral("query"), query},
                    {QStringLiteral("params"),
                            QStringLiteral("EgWKAQIIAWoMEA4QChADEAQQCRAF")}},
            [durationSec, done](const QJsonObject& page) {
                const QList<YouTubeVideoInfo> found = tracks(page);
                // The first close in length; the top hit when none is
                QString best;
                for (const YouTubeVideoInfo& song : found) {
                    if (durationSec <= 0 || std::abs(song.durationSec - durationSec) <= 6) {
                        best = song.id;
                        break;
                    }
                }
                if (best.isEmpty() && !found.isEmpty() &&
                        std::abs(found.first().durationSec - durationSec) <= 30) {
                    best = found.first().id;
                }
                done(best);
            },
            false,
            pContext);
}

} // namespace mixxx
