#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QTimer>
#include <QUrl>

namespace mixxx {
namespace qml {

/// Update of the app from the GitHub releases of Mixxx Android, exposed as
/// the `Mixxx.AppUpdate` singleton. Once a day at most (and when the user
/// asks) the releases are checked for a newer version; its APK is only
/// downloaded and installed when the user wants, by the Android
/// DownloadManager and installer (org.mixxx.AppUpdater).
class QmlAppUpdateProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(State state READ state NOTIFY stateChanged)
    /// The newest version, e.g. "1.5.4", once a check found it
    Q_PROPERTY(QString latestVersion READ latestVersion NOTIFY stateChanged)
    /// What's new in it (Markdown)
    Q_PROPERTY(QString releaseNotes READ releaseNotes NOTIFY stateChanged)
    Q_PROPERTY(QUrl releaseUrl READ releaseUrl NOTIFY stateChanged)
    /// Size of its APK in bytes
    Q_PROPERTY(qint64 downloadSize READ downloadSize NOTIFY stateChanged)
    /// 0 to 1 while downloading
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    /// What went wrong, in the Failed state
    Q_PROPERTY(Error error READ error NOTIFY stateChanged)
    /// True if the check was started by the app, not by the user
    Q_PROPERTY(bool automaticCheck READ automaticCheck NOTIFY stateChanged)
    /// True if the user chose to skip the latest version
    Q_PROPERTY(bool latestSkipped READ latestSkipped NOTIFY stateChanged)
    /// Check at startup (once a day at most)
    Q_PROPERTY(bool autoCheck READ autoCheck WRITE setAutoCheck NOTIFY autoCheckChanged)
    QML_NAMED_ELEMENT(AppUpdate)
    QML_SINGLETON

  public:
    enum class State {
        Idle,
        Checking,
        UpToDate,
        Available,
        Downloading,
        Verifying,
        Downloaded,
        Failed,
    };
    Q_ENUM(State)

    enum class Error {
        None,
        /// The releases could not be read (no connection)
        Network,
        NoSpace,
        Download,
        /// The downloaded APK doesn't match the release
        Damaged,
        Installer,
    };
    Q_ENUM(Error)

    explicit QmlAppUpdateProxy(QObject* pParent = nullptr);

    static QmlAppUpdateProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    State state() const {
        return m_state;
    }
    QString latestVersion() const {
        return m_latestVersion;
    }
    QString releaseNotes() const {
        return m_releaseNotes;
    }
    QUrl releaseUrl() const {
        return m_releaseUrl;
    }
    qint64 downloadSize() const {
        return m_downloadSize;
    }
    double progress() const {
        return m_progress;
    }
    Error error() const {
        return m_error;
    }
    bool automaticCheck() const {
        return m_automaticCheck;
    }
    bool latestSkipped() const;
    bool autoCheck() const;
    void setAutoCheck(bool autoCheck);

    /// Looks for a newer version now
    Q_INVOKABLE void check();
    /// Downloads the APK of the latest version
    Q_INVOKABLE void download();
    Q_INVOKABLE void cancelDownload();
    /// Opens the Android installer for the downloaded APK. Returns false if
    /// Android first needs the permission to install apps from Mixxx (its
    /// setting is opened).
    Q_INVOKABLE bool install();
    /// Don't tell about the latest version at startup anymore
    Q_INVOKABLE void skipLatest();

  signals:
    void stateChanged();
    void progressChanged();
    void autoCheckChanged();

  private slots:
    void slotAutoCheck();
    void slotPollDownload();

  private:
    void startCheck(bool automatic);
    void setState(State state, Error error = Error::None);
    void onReleasesFetched(const QString& json);
    void verifyDownload();
    QString apkPath() const;
    /// Removes the APKs of versions that are not newer than the installed one
    void removeOldDownloads() const;

    State m_state;
    QString m_latestVersion;
    QString m_releaseNotes;
    QUrl m_releaseUrl;
    QString m_apkUrl;
    QString m_apkName;
    QString m_apkSha256;
    qint64 m_downloadSize;
    double m_progress;
    Error m_error;
    bool m_automaticCheck;
    qint64 m_downloadId;
    QTimer m_pollTimer;
};

} // namespace qml
} // namespace mixxx
