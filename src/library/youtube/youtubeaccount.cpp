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
// The YouTube Music web player, whose requests these are; the live
// version is read from music.youtube.com (an old one gets empty lists)
const QString kClientVersion = QStringLiteral("1.20261104.00.00");
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

/// What a page holds, for the log when it holds no tracks: the renderers
/// and any message YouTube put on it
QString pageOutline(const QJsonObject& page) {
    QSet<QString> kinds;
    QStringList messages;
    walk(page, [&](const QJsonObject& object) {
        for (auto it = object.begin(); it != object.end(); ++it) {
            if (it.key().endsWith(QStringLiteral("Renderer")) ||
                    it.key().endsWith(QStringLiteral("ViewModel"))) {
                kinds.insert(it.key());
            }
        }
        for (const QString& key : {QStringLiteral("messageRenderer"),
                     QStringLiteral("alertRenderer"),
                     QStringLiteral("alertWithButtonRenderer")}) {
            const QJsonObject message = object.value(key).toObject();
            if (!message.isEmpty()) {
                messages << text(message.value(QStringLiteral("text"))) +
                                text(message.value(QStringLiteral("subtext")));
            }
        }
        return false;
    });
    QStringList sorted(kinds.begin(), kinds.end());
    sorted.sort();
    return QStringLiteral("top: ") + page.keys().join(QLatin1Char(',')) +
            QStringLiteral(" | renderers: ") + sorted.join(QLatin1Char(',')) +
            QStringLiteral(" | messages: ") + messages.join(QStringLiteral(" / "));
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
        : m_pNetwork(new QNetworkAccessManager(this)),
          m_clientVersion(kClientVersion) {
    QFile file(cookieFile());
    if (file.open(QIODevice::ReadOnly)) {
        // Cookies, then the chosen channel's page id and name
        const QStringList lines = QString::fromUtf8(file.readAll()).split(QLatin1Char('\n'));
        m_cookies = lines.value(0).trimmed();
        m_pageId = lines.value(1).trimmed();
        m_name = lines.value(2).trimmed();
    }
    fetchClientVersion();
    if (signedIn()) {
        fetchName();
    }
}

void YouTubeAccount::fetchClientVersion() {
    QNetworkRequest request(QUrl(kOrigin + QLatin1Char('/')));
    request.setRawHeader("User-Agent", kUserAgent);
    request.setRawHeader("Cookie", "SOCS=CAI");
    request.setTransferTimeout(kTimeoutMs);
    // Its own connection: the page's visitor cookies must not join the
    // account's requests (they made YouTube answer them as signed out)
    auto* pPageNetwork = new QNetworkAccessManager(this);
    QNetworkReply* pReply = pPageNetwork->get(request);
    connect(pReply, &QNetworkReply::finished, this, [this, pReply, pPageNetwork]() {
        pReply->deleteLater();
        pPageNetwork->deleteLater();
        static const QRegularExpression kVersion(
                QStringLiteral(R"re("INNERTUBE_CLIENT_VERSION":"([0-9.]+)")re"));
        const QString html = QString::fromUtf8(pReply->readAll());
        const QRegularExpressionMatch match = kVersion.match(html);
        // The visitor id the web player sends with every request: without
        // it YouTube answers the account's lists with empty shelves
        static const QRegularExpression kVisitor(
                QStringLiteral(R"re("VISITOR_DATA":"([^"]+)")re"));
        const QRegularExpressionMatch visitor = kVisitor.match(html);
        if (visitor.hasMatch()) {
            m_visitorData = visitor.captured(1);
        }
        kLogger.info() << "Visitor id" << (m_visitorData.isEmpty() ? "missing" : "found");
        if (match.hasMatch()) {
            m_clientVersion = match.captured(1);
        }
        kLogger.info() << "Web player version" << m_clientVersion;
    });
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
    // The previous account's pending answers are not this one's
    m_continuation.clear();
    ++m_generation;
    clearRequestsInFlight();
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
    clearRequestsInFlight();
    emit accountChanged();
}

void YouTubeAccount::post(const QString& endpoint,
        const QJsonObject& body,
        const std::function<void(const QJsonObject&)>& done,
        bool signedRequest,
        QObject* pContext,
        std::function<void()> onFailure) {
    QNetworkRequest request(QUrl(kOrigin + QStringLiteral("/youtubei/v1/") + endpoint +
            QStringLiteral("?prettyPrint=false")));
    request.setHeader(QNetworkRequest::ContentTypeHeader, QStringLiteral("application/json"));
    request.setRawHeader("User-Agent", kUserAgent);
    request.setRawHeader("Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Origin", kOrigin.toUtf8());
    request.setRawHeader("X-Goog-AuthUser", "0");
    request.setRawHeader("X-YouTube-Client-Name", "67");
    request.setRawHeader("X-YouTube-Client-Version", m_clientVersion.toUtf8());
    request.setRawHeader("Referer", (kOrigin + QLatin1Char('/')).toUtf8());
    QJsonObject client{{QStringLiteral("clientName"), QStringLiteral("WEB_REMIX")},
            {QStringLiteral("clientVersion"), m_clientVersion},
            {QStringLiteral("hl"), QStringLiteral("en")}};
    if (!m_visitorData.isEmpty()) {
        request.setRawHeader("X-Goog-Visitor-Id", m_visitorData.toUtf8());
        client.insert(QStringLiteral("visitorData"), m_visitorData);
    }
    QJsonObject context{{QStringLiteral("client"), client}};
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
            [this, pReply, generation, done, endpoint, signedRequest, onFailure]() {
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
                    // Asked again, a failed request runs again
                    if (onFailure) {
                        onFailure();
                    }
                    emit failed(pReply->errorString());
                    return;
                }
                done(QJsonDocument::fromJson(pReply->readAll()).object());
            });
}

void YouTubeAccount::clearRequestsInFlight() {
    m_listInFlight.clear();
    m_playlistsInFlight = false;
    m_accountsInFlight = false;
}

void YouTubeAccount::fetchName() {
    // The channels too: their names stand in when the menu has none
    fetchAccounts();
    post(QStringLiteral("account/account_menu"), {}, [this](const QJsonObject& page) {
        QString name;
        walk(page, [&name](const QJsonObject& object) {
            if (!object.contains(QStringLiteral("accountName"))) {
                return false;
            }
            name = text(object.value(QStringLiteral("accountName")));
            return true;
        });
        // Not a YouTube page (captive portal, cut-off reply): no verdict
        const bool innerTubePage = page.contains(QStringLiteral("responseContext"));
        if (name.isEmpty() && innerTubePage && !signedInPage(page)) {
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
    const QString id = list == QStringLiteral("liked") ? QStringLiteral("VLLM") : list;
    if (id == m_listInFlight) {
        // Already asked for: its answer goes to every view
        return;
    }
    // Only the newest list's pages count (the playlists and channels
    // asked for meanwhile still arrive)
    const quint64 listGeneration = ++m_listGeneration;
    m_continuation.clear();
    m_listInFlight = id;
    kLogger.info() << "Opening" << id;
    post(
            QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("browseId"), id}},
            [this, id, listGeneration](const QJsonObject& page) {
                if (listGeneration != m_listGeneration) {
                    return;
                }
                m_listInFlight.clear();
                m_continuation = continuation(page);
                const QList<YouTubeVideoInfo> list = tracks(page);
                kLogger.info() << id << "answered" << list.size() << "tracks, signed in:"
                               << signedInPage(page) << ", more:" << !m_continuation.isEmpty();
                if (list.isEmpty()) {
                    kLogger.info() << id << pageOutline(page);
                    walk(page, [&id](const QJsonObject& object) {
                        const QJsonValue shelf =
                                object.value(QStringLiteral("musicPlaylistShelfRenderer"));
                        if (shelf.isUndefined()) {
                            return false;
                        }
                        kLogger.info() << id << "shelf:"
                                       << QJsonDocument(shelf.toObject())
                                                  .toJson(QJsonDocument::Compact)
                                                  .left(1500);
                        return true;
                    });
                }
                if (list.isEmpty() && id == QStringLiteral("VLLM") && signedInPage(page)) {
                    // YouTube Music's liked songs are empty when the likes
                    // were given on YouTube: its liked videos then
                    openList(QStringLiteral("VLLL"));
                    return;
                }
                if (list.isEmpty() && !signedInPage(page) && m_clientVersion != kClientVersion) {
                    kLogger.info() << "Signed out with version" << m_clientVersion
                                   << ": again with" << kClientVersion;
                    m_clientVersion = kClientVersion;
                    openList(id == QStringLiteral("VLLM") ? QStringLiteral("liked") : id);
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
            },
            /*signedRequest=*/true,
            nullptr,
            [this, listGeneration]() {
                if (listGeneration == m_listGeneration) {
                    m_listInFlight.clear();
                }
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
    if (m_playlistsInFlight) {
        return;
    }
    m_playlistsInFlight = true;
    post(
            QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("browseId"),
                    QStringLiteral("FEmusic_liked_playlists")}},
            [this](const QJsonObject& page) { collectPlaylists(page, {}, 1); },
            /*signedRequest=*/true,
            nullptr,
            [this]() { m_playlistsInFlight = false; });
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
            kLogger.debug() << "Library item" << id;
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
        m_playlistsInFlight = false;
        kLogger.info() << "Playlists:" << playlists.size();
        if (playlists.isEmpty() && pages == 1) {
            kLogger.info() << "Library page" << pageOutline(page);
        }
        emit playlistsReady(playlists);
        return;
    }
    post(
            QStringLiteral("browse"),
            QJsonObject{{QStringLiteral("continuation"), token}},
            [this, playlists, pages](const QJsonObject& next) {
                collectPlaylists(next, playlists, pages + 1);
            },
            /*signedRequest=*/true,
            nullptr,
            [this]() { m_playlistsInFlight = false; });
}

void YouTubeAccount::fetchAccounts() {
    if (m_accountsInFlight) {
        return;
    }
    m_accountsInFlight = true;
    const auto onAccounts = [this](const QJsonObject& page) {
        m_accountsInFlight = false;
        QVariantList accounts;
        walk(page, [this, &accounts](const QJsonObject& object) {
            const QJsonObject item = object.value(QStringLiteral("accountItem")).toObject();
            if (item.isEmpty()) {
                return false;
            }
            // The channel's page id: in a pageIdToken anywhere in the item
            // (the switch endpoint moved over time), else the first half
            // of its datasync id ("<pageId>||<user id>"); "" = main channel
            QString pageId;
            QString datasyncId;
            walk(item, [&pageId, &datasyncId](const QJsonObject& o) {
                const QString id = o.value(QStringLiteral("pageIdToken"))
                                           .toObject()
                                           .value(QStringLiteral("pageId"))
                                           .toString();
                if (!id.isEmpty()) {
                    pageId = id;
                    return true;
                }
                const QString sync = o.value(QStringLiteral("datasyncIdToken"))
                                             .toObject()
                                             .value(QStringLiteral("datasyncIdToken"))
                                             .toString();
                if (!sync.isEmpty() && datasyncId.isEmpty()) {
                    datasyncId = sync;
                }
                return false;
            });
            if (pageId.isEmpty() && datasyncId.contains(QStringLiteral("||"))) {
                const QString first = datasyncId.section(QStringLiteral("||"), 0, 0);
                const QString second = datasyncId.section(QStringLiteral("||"), 1);
                // "<id>||" alone is the main channel
                if (!second.isEmpty() && first != second) {
                    pageId = first;
                }
            }
            kLogger.info() << "Channel" << text(item.value(QStringLiteral("accountName")))
                           << "page id" << (pageId.isEmpty() ? QStringLiteral("(main)") : pageId);
            accounts.append(QVariantMap{
                    {QStringLiteral("name"), text(item.value(QStringLiteral("accountName")))},
                    {QStringLiteral("handle"),
                            text(item.value(QStringLiteral("channelHandle")))},
                    {QStringLiteral("pageId"), pageId},
                    {QStringLiteral("selected"), pageId == m_pageId},
            });
            return true;
        });
        kLogger.info() << "Channels:" << accounts.size();
        if (m_name.isEmpty()) {
            // The account menu gave no name: the chosen channel's
            for (const QVariant& account : std::as_const(accounts)) {
                const QVariantMap map = account.toMap();
                if (map.value(QStringLiteral("pageId")).toString() == m_pageId ||
                        accounts.size() == 1) {
                    m_name = map.value(QStringLiteral("name")).toString();
                    save();
                    emit accountChanged();
                    break;
                }
            }
        }
        emit accountsReady(accounts);
    };
    post(QStringLiteral("account/accounts_list"),
            {},
            onAccounts,
            /*signedRequest=*/true,
            nullptr,
            [this]() { m_accountsInFlight = false; });
}

void YouTubeAccount::selectAccount(const QString& pageId, const QString& name) {
    m_pageId = pageId;
    m_name = name;
    m_continuation.clear();
    ++m_generation;
    clearRequestsInFlight();
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
