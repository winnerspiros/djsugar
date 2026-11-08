#include "stems/stemcompanion.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFileInfo>
#include <QStandardPaths>

namespace mixxx {
namespace stems {

namespace {

const QString kStemsFolder = QStringLiteral(".stems");
const QString kStemSuffix = QStringLiteral(".stem.mp4");

} // namespace

QString companionPath(const QString& trackLocation) {
    const QFileInfo fileInfo(trackLocation);
    return fileInfo.dir().filePath(kStemsFolder + QChar('/') + fileInfo.fileName() + kStemSuffix);
}

QString fallbackPath(const QString& trackLocation) {
    const QByteArray hash = QCryptographicHash::hash(
            trackLocation.toUtf8(), QCryptographicHash::Sha1);
    return QDir(QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation))
            .filePath(QStringLiteral("stems/") + QString::fromLatin1(hash.toHex()) + kStemSuffix);
}

QString findStemFile(const QString& trackLocation) {
    if (trackLocation.isEmpty() || trackLocation.endsWith(kStemSuffix)) {
        return {};
    }
    QString path = companionPath(trackLocation);
    if (QFileInfo::exists(path)) {
        return path;
    }
    path = fallbackPath(trackLocation);
    if (QFileInfo::exists(path)) {
        return path;
    }
    return {};
}

} // namespace stems
} // namespace mixxx
