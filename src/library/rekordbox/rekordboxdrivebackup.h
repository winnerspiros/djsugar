#pragma once

#include <QFuture>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <atomic>

namespace mixxx {
namespace android {

/// Rekordbox USB backup: copies PIONEER and Contents folders plus audio files to /storage/emulated/0/Music/Mixxx/Rekordbox/<name>. Mirroring: new/changed files copied, removed files deleted (only if nothing failed).
class RekordboxDriveBackup : public QObject {
    Q_OBJECT
  public:
    explicit RekordboxDriveBackup(QObject* pParent = nullptr);
    ~RekordboxDriveBackup() override;

    /// The Rekordbox sticks that are connected, as a list of {path, name}.
    /// Looking for the drives may wait for the USB permission, so this must
    /// not be called on the main thread.
    static QVariantList findDrives();

    /// The folder a stick named `name` is backed up to
    static QString targetDirectory(const QString& name);

    bool isRunning() const {
        return m_running;
    }
    int filesDone() const {
        return m_filesDone;
    }
    int filesTotal() const {
        return m_filesTotal;
    }
    qint64 bytesDone() const {
        return m_bytesDone;
    }
    qint64 bytesTotal() const {
        return m_bytesTotal;
    }

    /// Returns an error message, or an empty string if the backup started
    QString start(const QString& drivePath, const QString& name);
    void cancel();

  signals:
    void progressChanged();
    void finished(const QString& message, bool error);

  private:
    void run(const QString& drivePath, const QString& target);

    bool m_running = false;
    std::atomic<int> m_filesDone = 0;
    std::atomic<int> m_filesTotal = 0;
    std::atomic<qint64> m_bytesDone = 0;
    std::atomic<qint64> m_bytesTotal = 0;
    std::atomic<bool> m_canceled = false;
    QFuture<void> m_future;
};

} // namespace android
} // namespace mixxx
