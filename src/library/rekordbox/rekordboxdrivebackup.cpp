#include "library/rekordbox/rekordboxdrivebackup.h"

#include <QDir>
#include <QDirIterator>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QSet>
#include <QStorageInfo>
#include <QVariantMap>
#include <QtConcurrentRun>
#include <QtDebug>
#include <algorithm>
#include <cstdlib>

#include "library/rekordbox/rekordboxandroid.h"
#include "moc_rekordboxdrivebackup.cpp"
#include "sources/soundsourceproxy.h"
#include "util/removablestorage.h"
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

namespace {

const QString kTargetRoot = QStringLiteral("/storage/emulated/0/Music/Mixxx/Rekordbox");
const QString kInternalStorage = QStringLiteral("/storage/emulated/0");
const QStringList kPdbPaths = {
        QStringLiteral("PIONEER/rekordbox/export.pdb"),
        QStringLiteral(".PIONEER/rekordbox/export.pdb"),
};
/// The folders of a Rekordbox export: the database and the analysis, and
/// the tracks. They are backed up completely.
const QStringList kExportFolders = {
        QStringLiteral("PIONEER"),
        QStringLiteral(".PIONEER"),
        QStringLiteral("Contents"),
};
/// Folders of the systems that are never backed up
const QStringList kSystemFolders = {
        QStringLiteral("System Volume Information"),
        QStringLiteral("$RECYCLE.BIN"),
        QStringLiteral("LOST.DIR"),
        QStringLiteral("Android"),
};
/// In every backup folder, with the serial number of its stick
const QString kStickMarkerFileName = QStringLiteral(".mixxx-stick");
/// Kept free on the device
constexpr qint64 kReservedBytes = 200LL * 1024 * 1024;
constexpr int kProgressIntervalMs = 250;

struct File {
    QString relativePath;
    qint64 size;
    QDateTime modified;
};

/// FAT stores the time with 2 s resolution
bool sameTime(const QDateTime& a, const QDateTime& b) {
    return a.isValid() && b.isValid() && std::abs(a.secsTo(b)) <= 2;
}

/// Whether a file of the stick (path relative to its root) is backed up:
/// everything in the export folders, and the audio files anywhere else,
/// e.g. tracks in the root folder that Rekordbox plays from there
bool isBackedUp(const QString& relativePath) {
    const QString topLevel = relativePath.section(QChar('/'), 0, 0);
    if (relativePath.contains(QChar('/'))) {
        if (kExportFolders.contains(topLevel, Qt::CaseInsensitive)) {
            return true;
        }
        // Hidden folders (.Trashes, .Spotlight-V100...) and system folders
        if (topLevel.startsWith(QChar('.')) ||
                kSystemFolders.contains(topLevel, Qt::CaseInsensitive)) {
            return false;
        }
    }
    const QString fileName = relativePath.section(QChar('/'), -1);
    return !fileName.startsWith(QChar('.')) &&
            SoundSourceProxy::getSupportedFileNamesRegex().match(fileName).hasMatch();
}

/// Sets *pComplete to false if a folder could not be read, in which case
/// files may be missing from the list
QList<File> listMountedFiles(const QString& root, bool* pComplete) {
    QList<File> files;
    const QDir rootDir(root);
    QDirIterator it(root,
            QDir::Files | QDir::Dirs | QDir::Hidden | QDir::NoDotAndDotDot,
            QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QFileInfo info(it.next());
        const QString relativePath = rootDir.relativeFilePath(info.filePath());
        if (info.isDir()) {
            // QDirIterator silently skips the folders it can't open
            if (!info.isReadable() || !QDir(info.filePath()).isReadable()) {
                *pComplete = false;
            }
            continue;
        }
        if (isBackedUp(relativePath)) {
            files.append({relativePath, info.size(), info.lastModified()});
        }
    }
    return files;
}

/// Identifies a stick: the serial number of its file system, which Android
/// also uses as the name of the folder it is mounted at
QString stickId(const QString& drivePath) {
#ifdef __ANDROID_USB_DRIVE__
    const QString serial = mixxx::usbdrive::driveSerial(drivePath);
    if (!serial.isEmpty()) {
        return serial;
    }
#endif
    return QFileInfo(drivePath).fileName();
}

QString readStickMarker(const QString& target) {
    QFile marker(target + QChar('/') + kStickMarkerFileName);
    if (!marker.open(QIODevice::ReadOnly)) {
        return QString();
    }
    return QString::fromUtf8(marker.readAll()).trimmed();
}

/// Deletes the backed up files (see isBackedUp()) of the backup `target`
/// that are not in `stickFiles` (relative paths in lower case), left over
/// partial files, and the folders that become empty. Returns the number of
/// deleted files.
int removeDeletedFiles(const QString& target, const QSet<QString>& stickFiles) {
    int deleted = 0;
    const QDir targetDir(target);
    QStringList folders;
    QDirIterator it(target,
            QDir::Files | QDir::Dirs | QDir::Hidden | QDir::NoDotAndDotDot,
            QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QFileInfo info(it.next());
        if (info.isDir()) {
            folders.append(info.filePath());
            continue;
        }
        const QString relativePath = targetDir.relativeFilePath(info.filePath());
        const bool removed = (isBackedUp(relativePath) &&
                                     !stickFiles.contains(relativePath.toLower())) ||
                relativePath.endsWith(QLatin1String(".part"));
        if (removed && QFile::remove(info.filePath())) {
            ++deleted;
        }
    }
    // Deepest first; rmdir() only removes empty folders
    std::sort(folders.begin(), folders.end(), [](const QString& a, const QString& b) {
        return a.size() > b.size();
    });
    for (const QString& path : std::as_const(folders)) {
        QDir().rmdir(path);
    }
    return deleted;
}

void setModified(const QString& path, const QDateTime& modified) {
    if (!modified.isValid()) {
        return;
    }
    QFile file(path);
    if (file.open(QIODevice::ReadWrite)) {
        file.setFileTime(modified, QFileDevice::FileModificationTime);
    }
}

} // namespace

namespace mixxx {
namespace android {

RekordboxDriveBackup::RekordboxDriveBackup(QObject* pParent)
        : QObject(pParent) {
}

RekordboxDriveBackup::~RekordboxDriveBackup() {
    m_canceled = true;
    m_future.waitForFinished();
}

// static
QVariantList RekordboxDriveBackup::findDrives() {
    QVariantList drives;
    const QFileInfoList roots = mixxx::removableStorageRoots();
    for (const QFileInfo& root : roots) {
        const QString path = root.filePath();
        // Neither the internal storage nor the backups themselves
        if (path == kInternalStorage || path.startsWith(kTargetRoot + QChar('/'))) {
            continue;
        }
#ifdef __ANDROID_USB_DRIVE__
        // The database of drives that Mixxx reads itself is in their local
        // folder once it was copied
        mixxx::usbdrive::prepareFiles(path, kPdbPaths);
#endif
        const bool hasExport = std::any_of(kPdbPaths.cbegin(),
                kPdbPaths.cend(),
                [&path](const QString& pdbPath) {
                    return QFileInfo(path + QChar('/') + pdbPath).isFile();
                });
        if (hasExport) {
            drives.append(QVariantMap{
                    {QStringLiteral("path"), path},
                    {QStringLiteral("name"), root.fileName()},
            });
        }
    }
    return drives;
}

// static
QString RekordboxDriveBackup::targetDirectory(const QString& name) {
    QString folderName = safeFileName(name);
    if (folderName.isEmpty()) {
        folderName = QStringLiteral("USB");
    }
    return kTargetRoot + QChar('/') + folderName;
}

QString RekordboxDriveBackup::start(const QString& drivePath, const QString& name) {
    if (m_running) {
        return tr("A stick is already being backed up");
    }
    if (name.trimmed().isEmpty()) {
        return tr("Enter a name for the backup");
    }
    const QString target = targetDirectory(name.trimmed());
    // A backup folder only ever mirrors one stick: mirroring another stick
    // into it would delete the files of the first one
    const QString id = stickId(drivePath);
    const QString markedId = readStickMarker(target);
    if (markedId.isEmpty()
                    ? !QDir(target).isEmpty(QDir::AllEntries | QDir::NoDotAndDotDot | QDir::Hidden)
                    : markedId != id) {
        return tr(
                "The folder %1 already has the backup of another stick or other files. "
                "Choose another name.")
                .arg(target);
    }
    if (!QDir().mkpath(target)) {
        return tr(
                "Could not create the folder %1. Grant Mixxx access to all files and "
                "try again.")
                .arg(target);
    }
    m_filesDone = 0;
    m_filesTotal = 0;
    m_bytesDone = 0;
    m_bytesTotal = 0;
    // Keeps Android's media library (and other music apps) from listing the
    // copies as duplicates
    QFile noMedia(kTargetRoot + QStringLiteral("/.nomedia"));
    if (!noMedia.exists() && noMedia.open(QIODevice::WriteOnly)) {
        noMedia.close();
    }
    if (markedId.isEmpty()) {
        QFile marker(target + QChar('/') + kStickMarkerFileName);
        if (!marker.open(QIODevice::WriteOnly) || marker.write(id.toUtf8()) < 0) {
            return tr("Could not write to the folder %1.").arg(target);
        }
    }
    m_canceled = false;
    m_running = true;
    emit progressChanged();
    qInfo() << "Backing up the Rekordbox stick" << drivePath << "(" << id << ") to" << target;
    m_future = QtConcurrent::run([this, drivePath, target]() {
        run(drivePath, target);
    });
    return QString();
}

void RekordboxDriveBackup::cancel() {
    m_canceled = true;
}

// Runs on the worker thread
void RekordboxDriveBackup::run(const QString& drivePath, const QString& target) {
    const auto finish = [this](const QString& message, bool error) {
        QMetaObject::invokeMethod(
                this,
                [this, message, error]() {
                    m_running = false;
                    emit progressChanged();
                    emit finished(message, error);
                },
                Qt::QueuedConnection);
    };
    const auto reportProgress = [this]() {
        QMetaObject::invokeMethod(
                this,
                [this]() {
                    emit progressChanged();
                },
                Qt::QueuedConnection);
    };

    QList<File> files;
    bool isDrive = false;
    // Whether every folder could be read, so that a file missing from the
    // list was really removed from the stick
    bool complete = true;
#ifdef __ANDROID_USB_DRIVE__
    if (const auto driveFiles =
                    mixxx::usbdrive::listFiles(drivePath, &complete)) {
        isDrive = true;
        for (const auto& file : *driveFiles) {
            if (isBackedUp(file.relativePath)) {
                files.append({file.relativePath, file.size, file.modified});
            }
        }
    }
#endif
    if (!isDrive) {
        files = listMountedFiles(drivePath, &complete);
    }
    if (files.isEmpty()) {
        finish(tr("No Rekordbox export found on the stick"), true);
        return;
    }

    // Only what is new or changed. The copies get the date of the file on
    // the stick, so a changed date means the file was changed.
    QList<File> pending;
    qint64 pendingBytes = 0;
    QSet<QString> stickFiles;
    for (const File& file : std::as_const(files)) {
        stickFiles.insert(file.relativePath.toLower());
        const QFileInfo existing(target + QChar('/') + file.relativePath);
        const bool up - to - date = existing.isFile() && existing.size() == file.size &&
                sameTime(existing.lastModified(), file.modified);
        if (!up - to - date) {
            pending.append(file);
            pendingBytes += file.size;
        }
    }
    const qint64 available = QStorageInfo(kInternalStorage).bytesAvailable();
    if (available >= 0 && pendingBytes + kReservedBytes > available) {
        finish(tr("Not enough space: the backup needs %1 MB, %2 MB are free")
                        .arg(pendingBytes / (1024 * 1024))
                        .arg(available / (1024 * 1024)),
                true);
        return;
    }
    m_filesTotal = static_cast<int>(pending.size());
    m_bytesTotal = pendingBytes;
    reportProgress();

    int failed = 0;
    QElapsedTimer progressTimer;
    progressTimer.start();
    for (const File& file : std::as_const(pending)) {
        if (m_canceled) {
            break;
        }
        const QString destination = target + QChar('/') + file.relativePath;
        QDir().mkpath(QFileInfo(destination).absolutePath());
        bool ok = false;
#ifdef __ANDROID_USB_DRIVE__
        if (isDrive) {
            ok = mixxx::usbdrive::copyFileTo(drivePath, file.relativePath, destination);
        }
#endif
        if (!isDrive) {
            const QString partialPath = destination + QStringLiteral(".part");
            QFile::remove(partialPath);
            ok = QFile::copy(drivePath + QChar('/') + file.relativePath, partialPath) &&
                    (!QFile::exists(destination) || QFile::remove(destination)) &&
                    QFile::rename(partialPath, destination);
            if (!ok) {
                QFile::remove(partialPath);
            }
        }
        if (ok) {
            // Compared on the next backup
            setModified(destination, file.modified);
        } else {
            qWarning() << "Unable to back up" << file.relativePath;
            ++failed;
        }
        ++m_filesDone;
        m_bytesDone += file.size;
        if (progressTimer.elapsed() >= kProgressIntervalMs) {
            progressTimer.restart();
            reportProgress();
        }
    }

    const int copied = m_filesDone.load() - failed;
    // Mirrors the stick only if it was read completely and everything was
    // copied, so a read error never deletes files of the backup
    int deleted = 0;
    if (!m_canceled && failed == 0 && complete) {
        deleted = removeDeletedFiles(target, stickFiles);
    } else if (!complete) {
        qWarning() << "The stick" << drivePath
                   << "could not be read completely, no files are deleted from the backup";
    }
    const QString deletedText = deleted > 0 ? QStringLiteral(" ") +
                    tr("%1 files that are no longer on the stick were deleted.")
                            .arg(deleted)
                                            : QString();
    if (m_canceled) {
        finish(tr("Backup stopped after %1 of %2 files. Back up again to continue.")
                        .arg(m_filesDone.load())
                        .arg(m_filesTotal.load()),
                true);
    } else if (failed > 0) {
        finish(tr("%1 files backed up, %2 failed. Back up again to retry them.")
                        .arg(copied)
                        .arg(failed),
                true);
    } else if (copied == 0 && deleted == 0) {
        finish(tr("The backup is up to date"), false);
    } else {
        finish(tr("%1 files backed up to %2.").arg(copied).arg(target) + deletedText, false);
    }
    qInfo() << "Backup of" << drivePath << "done:" << copied << "copied," << failed << "failed,"
            << deleted << "deleted";
}

} // namespace android
} // namespace mixxx
