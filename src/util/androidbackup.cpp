#include "util/androidbackup.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSqlError>
#include <QSqlQuery>
#include <QtDebug>
#include <algorithm>

namespace mixxx {
namespace android {

namespace {

const QString kDatabaseFileName = QStringLiteral("mixxxdb.sqlite");
const QString kPendingDirName = QStringLiteral("restore_pending");
const QString kPreviousDirName = QStringLiteral("restore_previous");
/// Written last when staging, so an interrupted copy is never applied
const QString kCompleteMarker = QStringLiteral(".complete");
const QString kInfoFileName = QStringLiteral("backup-info.txt");
const QString kConfigFileName = QStringLiteral("mixxx.cfg");
/// Secrets that must not be written to the shared storage
const QString kStreamingGroup = QStringLiteral("[Streaming]");
const QString kSubsonicPasswordKey = QStringLiteral("SubsonicPassword");

/// Entries of the settings folder that are not backed up
bool isExcluded(const QString& name) {
    return name.startsWith(kDatabaseFileName) || // copied separately
            name.startsWith(QStringLiteral("mixxx.log")) ||
            name == QStringLiteral("analysis") ||        // waveforms, regenerated
            name == QStringLiteral("streaming_cache") || // downloaded tracks
            name == kPendingDirName || name == kPreviousDirName;
}

bool copyRecursively(const QString& source, const QString& target) {
    const QFileInfo sourceInfo(source);
    if (!sourceInfo.isDir()) {
        QFile::remove(target);
        return QFile::copy(source, target);
    }
    if (!QDir().mkpath(target)) {
        return false;
    }
    const QStringList entries =
            QDir(source).entryList(QDir::AllEntries | QDir::Hidden | QDir::NoDotAndDotDot);
    for (const auto& entry : entries) {
        if (!copyRecursively(source + QChar('/') + entry, target + QChar('/') + entry)) {
            return false;
        }
    }
    return true;
}

bool isSubsonicPasswordLine(const QString& line) {
    return line.startsWith(kSubsonicPasswordKey + QChar(' '));
}

/// The configuration without the Subsonic password, which would be readable
/// by every app with storage access in the backup folder
bool copyConfigWithoutSecrets(const QString& source, const QString& target) {
    QFile in(source);
    if (!in.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }
    QStringList lines = QString::fromUtf8(in.readAll()).split(QChar('\n'));
    lines.erase(std::remove_if(lines.begin(), lines.end(), isSubsonicPasswordLine), lines.end());
    QFile out(target);
    return out.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text) &&
            out.write(lines.join(QChar('\n')).toUtf8()) >= 0;
}

/// Puts the Subsonic password of the replaced configuration (backups don't
/// contain it) into the restored one
void keepSubsonicPassword(const QString& previousConfig, const QString& restoredConfig) {
    QFile previous(previousConfig);
    if (!previous.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return;
    }
    QString passwordLine;
    QString group;
    for (const auto& line : QString::fromUtf8(previous.readAll()).split(QChar('\n'))) {
        if (line.startsWith(QChar('['))) {
            group = line.trimmed();
        } else if (group == kStreamingGroup && isSubsonicPasswordLine(line)) {
            passwordLine = line;
        }
    }
    if (passwordLine.isEmpty()) {
        return;
    }
    QFile restored(restoredConfig);
    if (!restored.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return;
    }
    QStringList lines = QString::fromUtf8(restored.readAll()).split(QChar('\n'));
    restored.close();
    const auto groupLine = std::find_if(lines.begin(), lines.end(), [](const QString& line) {
        return line.trimmed() == kStreamingGroup;
    });
    if (groupLine == lines.end()) {
        lines << QString() << kStreamingGroup << passwordLine;
    } else {
        lines.insert(groupLine + 1, passwordLine);
    }
    if (!restored.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text) ||
            restored.write(lines.join(QChar('\n')).toUtf8()) < 0) {
        qWarning() << "Restore: cannot keep the Subsonic password in" << restoredConfig;
    }
}

bool removePath(const QString& path) {
    const QFileInfo info(path);
    if (!info.exists() && !info.isSymLink()) {
        return true;
    }
    return info.isDir() ? QDir(path).removeRecursively() : QFile::remove(path);
}

} // namespace

QString backupRootDirectory() {
    return QStringLiteral("/storage/emulated/0/Mixxx/backup");
}

QString createBackup(const QString& settingsPath, QSqlDatabase database, QString* pError) {
    const QString name = QDateTime::currentDateTime().toString(
            QStringLiteral("yyyy-MM-dd_HH-mm-ss"));
    const QString backupDir = backupRootDirectory() + QChar('/') + name;
    if (!QDir().mkpath(backupDir)) {
        *pError = QCoreApplication::translate("AndroidBackup", "Cannot create %1").arg(backupDir);
        return {};
    }

    QSqlQuery query(database);
    query.prepare(QStringLiteral("VACUUM INTO :path"));
    query.bindValue(QStringLiteral(":path"), QString(backupDir + QChar('/') + kDatabaseFileName));
    if (!query.exec()) {
        *pError = QCoreApplication::translate(
                "AndroidBackup", "Cannot copy the library database: %1")
                          .arg(query.lastError().text());
        QDir(backupDir).removeRecursively();
        return {};
    }

    const QStringList entries =
            QDir(settingsPath).entryList(QDir::AllEntries | QDir::Hidden | QDir::NoDotAndDotDot);
    for (const auto& entry : entries) {
        if (isExcluded(entry)) {
            continue;
        }
        const QString source = settingsPath + QChar('/') + entry;
        const QString target = backupDir + QChar('/') + entry;
        const bool copied = entry == kConfigFileName
                ? copyConfigWithoutSecrets(source, target)
                : copyRecursively(source, target);
        if (!copied) {
            *pError = QCoreApplication::translate("AndroidBackup", "Cannot copy %1").arg(entry);
            QDir(backupDir).removeRecursively();
            return {};
        }
    }

    QFile info(backupDir + QChar('/') + kInfoFileName);
    if (info.open(QIODevice::WriteOnly | QIODevice::Text)) {
        info.write(QStringLiteral("Mixxx %1 backup, created %2\n")
                        .arg(QCoreApplication::applicationVersion(),
                                QDateTime::currentDateTime().toString(Qt::ISODate))
                        .toUtf8());
    }
    qInfo() << "Created backup" << backupDir;
    return backupDir;
}

QStringList listBackups() {
    QStringList names;
    const QDir root(backupRootDirectory());
    const QStringList dirs = root.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    for (const auto& dir : dirs) {
        // Only folders that contain a database are usable backups
        if (QFileInfo::exists(root.filePath(dir + QChar('/') + kDatabaseFileName))) {
            names.append(dir);
        }
    }
    // The names are timestamps, so they sort chronologically
    std::sort(names.begin(), names.end(), std::greater<>());
    return names;
}

bool stageRestore(const QString& settingsPath, const QString& backupName, QString* pError) {
    const QString backupDir = backupRootDirectory() + QChar('/') + backupName;
    if (backupName.isEmpty() || backupName.contains(QChar('/')) ||
            !QFileInfo::exists(backupDir + QChar('/') + kDatabaseFileName)) {
        *pError = QCoreApplication::translate(
                "AndroidBackup", "Backup %1 not found")
                          .arg(backupName);
        return false;
    }
    const QString pendingDir = settingsPath + QChar('/') + kPendingDirName;
    removePath(pendingDir);
    if (!copyRecursively(backupDir, pendingDir)) {
        *pError = QCoreApplication::translate(
                "AndroidBackup", "Cannot copy backup %1")
                          .arg(backupName);
        removePath(pendingDir);
        return false;
    }
    QFile::remove(pendingDir + QChar('/') + kInfoFileName);
    QFile marker(pendingDir + QChar('/') + kCompleteMarker);
    if (!marker.open(QIODevice::WriteOnly)) {
        *pError = QCoreApplication::translate(
                "AndroidBackup", "Cannot copy backup %1")
                          .arg(backupName);
        removePath(pendingDir);
        return false;
    }
    qInfo() << "Staged backup" << backupName << "for restoring at the next start";
    return true;
}

void applyPendingRestore(const QString& settingsPath) {
    const QDir settingsDir(settingsPath);
    const QString pendingDir = settingsDir.filePath(kPendingDirName);
    if (!QFileInfo::exists(pendingDir)) {
        return;
    }
    if (!QFileInfo::exists(pendingDir + QChar('/') + kCompleteMarker)) {
        qWarning() << "Discarding incomplete backup restore in" << pendingDir;
        removePath(pendingDir);
        return;
    }
    QFile::remove(pendingDir + QChar('/') + kCompleteMarker);

    const QString previousDir = settingsDir.filePath(kPreviousDirName);
    removePath(previousDir);
    if (!QDir().mkpath(previousDir)) {
        qWarning() << "Restore: cannot create" << previousDir << ", nothing was restored";
        removePath(pendingDir);
        return;
    }

    // A journal left over from the replaced database must not be applied
    // to the restored one
    QStringList names = QDir(pendingDir)
                                .entryList(QDir::AllEntries | QDir::Hidden |
                                        QDir::NoDotAndDotDot);
    names.append(kDatabaseFileName + QStringLiteral("-journal"));

    // First move the current files away. If one can't be moved, put back
    // the ones already moved and keep the current state: nothing is deleted.
    QStringList movedAway;
    for (const auto& name : std::as_const(names)) {
        const QString target = settingsDir.filePath(name);
        if (!QFileInfo::exists(target)) {
            continue;
        }
        if (!QDir().rename(target, previousDir + QChar('/') + name)) {
            qWarning() << "Restore: cannot move away" << target << ", nothing was restored";
            for (const auto& moved : std::as_const(movedAway)) {
                QDir().rename(previousDir + QChar('/') + moved, settingsDir.filePath(moved));
            }
            removePath(pendingDir);
            return;
        }
        movedAway.append(name);
    }

    // Then move the restored files in. On failure, go back to the previous
    // files.
    QStringList movedIn;
    for (const auto& name : std::as_const(names)) {
        const QString source = pendingDir + QChar('/') + name;
        if (!QFileInfo::exists(source)) {
            continue;
        }
        if (!QDir().rename(source, settingsDir.filePath(name))) {
            qWarning() << "Restore: cannot move" << source << ", going back to the previous files";
            for (const auto& moved : std::as_const(movedIn)) {
                removePath(settingsDir.filePath(moved));
            }
            for (const auto& moved : std::as_const(movedAway)) {
                QDir().rename(previousDir + QChar('/') + moved, settingsDir.filePath(moved));
            }
            removePath(pendingDir);
            return;
        }
        movedIn.append(name);
    }
    keepSubsonicPassword(previousDir + QChar('/') + kConfigFileName,
            settingsDir.filePath(kConfigFileName));
    removePath(pendingDir);
    qInfo() << "Restored a backup, the previous files are in" << previousDir;
}

} // namespace android
} // namespace mixxx
