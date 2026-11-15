#pragma once

#include <QSqlDatabase>
#include <QString>
#include <QStringList>

namespace mixxx {
namespace android {

/// Backups of the library database and the settings on Android, where the
/// settings folder is private to the app and deleted with it.
///
/// A backup is a folder in backupRootDirectory() with a copy of the
/// settings folder: the database, mixxx.cfg, the other configuration files
/// and the user controller mappings. Waveforms and logs are left out.

/// /storage/emulated/0/Mixxx/backup, kept when the app is uninstalled
QString backupRootDirectory();

/// Writes a new backup. The database is copied with "VACUUM INTO" through
/// the open connection `database`, which gives a consistent copy while
/// Mixxx is running. Returns the folder of the backup, or an empty string
/// and an error message in pError.
QString createBackup(const QString& settingsPath, const QSqlDatabase& database, QString* pError);

/// The names of the backup folders, newest first
QStringList listBackups();

/// Prepares restoring the backup `backupName`. The files can't be replaced
/// while Mixxx uses them, so they are copied next to the settings folder
/// and applyPendingRestore() swaps them in on the next start.
bool stageRestore(const QString& settingsPath, const QString& backupName, QString* pError);

/// Called at startup, before anything reads the settings folder. Replaces
/// its files with a staged backup, if any. The replaced files are kept in
/// the "restore_previous" subfolder until the next restore.
void applyPendingRestore(const QString& settingsPath);

} // namespace android
} // namespace mixxx
