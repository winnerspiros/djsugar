#include "qml/librarycoverloader.h"

#include <QCache>
#include <QSqlQuery>
#include <algorithm>
#include <mutex>
#include <optional>

#include "library/coverart.h"
#include "library/library.h"
#include "qml/qmllibraryproxy.h"
#include "util/db/dbconnectionpooled.h"
#include "util/db/dbconnectionpooler.h"

namespace {

// The covers shown recently, so scrolling back doesn't load them again
std::mutex s_imageCacheMutex;
QCache<QString, QImage> s_imageCache(32 * 1024); // KiB

/// Reads the cover info of a track in the library. Tracks that aren't in
/// the library get an empty cover info. Returns nothing if the database
/// can't be read.
std::optional<CoverInfoRelative> coverInfoFromLibrary(const QString& trackLocation) {
    Library* pLibrary = mixxx::qml::QmlLibraryProxy::get();
    if (!pLibrary) {
        return std::nullopt;
    }
    const mixxx::DbConnectionPooler dbConnectionPooler(pLibrary->dbConnectionPool());
    QSqlDatabase database = mixxx::DbConnectionPooled(pLibrary->dbConnectionPool());
    if (!database.isOpen()) {
        return std::nullopt;
    }
    QSqlQuery query(database);
    query.prepare(QStringLiteral(
            "SELECT library.coverart_source, library.coverart_type,"
            " library.coverart_location, library.coverart_digest, library.coverart_hash"
            " FROM library INNER JOIN track_locations"
            " ON library.location = track_locations.id"
            " WHERE track_locations.location = :location"));
    query.bindValue(QStringLiteral(":location"), trackLocation);
    if (!query.exec()) {
        return std::nullopt;
    }
    CoverInfoRelative info;
    if (!query.next()) {
        return info;
    }
    info.source = static_cast<CoverInfoRelative::Source>(query.value(0).toInt());
    info.type = static_cast<CoverInfoRelative::Type>(query.value(1).toInt());
    info.coverLocation = query.value(2).toString();
    info.setImageDigest(query.value(3).toByteArray(), static_cast<quint16>(query.value(4).toUInt()));
    return info;
}

} // namespace

namespace mixxx {
namespace qml {

bool loadLibraryCover(const QString& id,
        const QString& trackLocation,
        const QSize& requestedSize,
        QImage* pImage) {
    const QString cacheKey = id + QChar('@') + QString::number(requestedSize.width()) +
            QChar('x') + QString::number(requestedSize.height());
    {
        std::lock_guard lock(s_imageCacheMutex);
        if (const QImage* pCachedImage = s_imageCache.object(cacheKey)) {
            *pImage = *pCachedImage;
            return true;
        }
    }
    const auto coverInfo = coverInfoFromLibrary(trackLocation);
    if (!coverInfo) {
        return false;
    }
    const CoverInfo::LoadedImage loadedImage = CoverInfo(*coverInfo, trackLocation).loadImage();
    if (loadedImage.result == CoverInfo::LoadedImage::Result::Ok) {
        *pImage = requestedSize.isValid() ? loadedImage.image.scaled(requestedSize)
                                          : loadedImage.image;
    }
    std::lock_guard lock(s_imageCacheMutex);
    s_imageCache.insert(cacheKey,
            new QImage(*pImage),
            std::max<qsizetype>(1, pImage->sizeInBytes() / 1024));
    return true;
}

} // namespace qml
} // namespace mixxx
