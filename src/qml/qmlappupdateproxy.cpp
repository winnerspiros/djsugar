#include "qml/qmlappupdateproxy.h"

#include <QCryptographicHash>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFutureWatcher>
#ifdef Q_OS_ANDROID
#include <QCoreApplication>
#include <QJniEnvironment>
#include <QJniObject>
#endif
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStorageInfo>
#include <QVersionNumber>
#include <QtConcurrentRun>

#include "moc_qmlappupdateproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "qml/qmlmobileappproxy.h"

namespace {

const QString kReleasesUrl = QStringLiteral(
        "https://api.github.com/repos/winnerspiros/djsugar/releases?per_page=20");
// Releases of the app are tagged e.g. "v1.5.3-android"; others have no APK
const QRegularExpression kReleaseTag(QStringLiteral("^v(\\d+\\.\\d+\\.\\d+)-android$"));
// The version in the name of a downloaded APK, e.g. mixxx-android-v1.5.3.apk
const QRegularExpression kApkVersion(QStringLiteral("v(\\d+\\.\\d+\\.\\d+)\\.apk$"));
const char* const kJavaClass = "org/mixxx/AppUpdater";

const ConfigKey kAutoCheckKey(QStringLiteral("[Updates]"), QStringLiteral("AutoCheck"));
const ConfigKey kLastCheckKey(QStringLiteral("[Updates]"), QStringLiteral("LastCheck"));
const ConfigKey kSkippedVersionKey(QStringLiteral("[Updates]"), QStringLiteral("SkippedVersion"));

// Not right at startup, while the library and the decks load
constexpr int kAutoCheckDelayMillis = 20000;
constexpr qint64 kAutoCheckIntervalSecs = 20 * 60 * 60;
constexpr int kPollIntervalMillis = 500;
// Room left on the storage besides the APK
constexpr qint64 kSpareBytes = 64 * 1024 * 1024;

#ifdef Q_OS_ANDROID
// android.app.DownloadManager.STATUS_*
constexpr jlong kDownloadSuccessful = 8;
constexpr jlong kDownloadFailed = 16;
// AppUpdater.INSTALL_*
constexpr jint kInstallNeedsPermission = 1;
constexpr jint kInstallFailed = 2;

QJniObject context() {
    return QNativeInterface::QAndroidApplication::context();
}

/// The folder of the downloaded APKs
QString updatesDir() {
    return QJniObject::callStaticMethod<jstring>(kJavaClass,
            "updatesDir",
            "(Landroid/content/Context;)Ljava/lang/String;",
            context().object())
            .toString();
}
#endif // Q_OS_ANDROID

QVersionNumber installedVersion() {
    return QVersionNumber::fromString(mixxx::qml::QmlMobileAppProxy::packageVersion());
}

/// The release notes without the comments and the install instructions
QString cleanReleaseNotes(QString body) {
    static const QRegularExpression kComments(QStringLiteral("<!--.*?-->"),
            QRegularExpression::DotMatchesEverythingOption);
    body.remove(kComments);
    body.replace(QStringLiteral("\r\n"), QStringLiteral("\n"));
    const int install = body.indexOf(QStringLiteral("\n## Install"));
    if (install >= 0) {
        body.truncate(install);
    }
    // The title repeats the version
    static const QRegularExpression kTitle(QStringLiteral("^\\s*# [^\\n]*\\n"));
    body.remove(kTitle);
    return body.trimmed();
}

} // namespace

namespace mixxx {
namespace qml {

QmlAppUpdateProxy::QmlAppUpdateProxy(QObject* pParent)
        : QObject(pParent),
          m_state(State::Idle),
          m_downloadSize(0),
          m_progress(0),
          m_error(Error::None),
          m_automaticCheck(false),
          m_downloadId(-1) {
    m_pollTimer.setInterval(kPollIntervalMillis);
    connect(&m_pollTimer, &QTimer::timeout, this, &QmlAppUpdateProxy::slotPollDownload);
#ifdef Q_OS_ANDROID
    QTimer::singleShot(kAutoCheckDelayMillis, this, &QmlAppUpdateProxy::slotAutoCheck);
    removeOldDownloads();
#endif
}

// static
QmlAppUpdateProxy* QmlAppUpdateProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlAppUpdateProxy(pQmlEngine);
}

bool QmlAppUpdateProxy::latestSkipped() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig && !m_latestVersion.isEmpty() &&
            pConfig->getValueString(kSkippedVersionKey) == m_latestVersion;
}

bool QmlAppUpdateProxy::autoCheck() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig ? pConfig->getValue(kAutoCheckKey, true) : true;
}

void QmlAppUpdateProxy::setAutoCheck(bool autoCheck) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || autoCheck == this->autoCheck()) {
        return;
    }
    pConfig->setValue(kAutoCheckKey, autoCheck);
    pConfig->save();
    emit autoCheckChanged();
}

void QmlAppUpdateProxy::check() {
    startCheck(false);
}

void QmlAppUpdateProxy::slotAutoCheck() {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || !autoCheck()) {
        return;
    }
    const qint64 lastCheck = pConfig->getValueString(kLastCheckKey).toLongLong();
    if (QDateTime::currentSecsSinceEpoch() - lastCheck < kAutoCheckIntervalSecs) {
        return;
    }
    startCheck(true);
}

void QmlAppUpdateProxy::startCheck(bool automatic) {
#ifndef Q_OS_ANDROID
    // In-app updates are delivered through the Android DownloadManager
    // and installer; anywhere else the proxy stays idle.
    Q_UNUSED(automatic);
    return;
#else
    if (m_state == State::Checking || m_state == State::Downloading ||
            m_state == State::Verifying) {
        return;
    }
    m_automaticCheck = automatic;
    setState(State::Checking);
    const QString userAgent = QStringLiteral("Mixxx-Android/") +
            QmlMobileAppProxy::packageVersion();
    auto* pWatcher = new QFutureWatcher<QString>(this);
    connect(pWatcher, &QFutureWatcher<QString>::finished, this, [this, pWatcher]() {
        onReleasesFetched(pWatcher->result());
        pWatcher->deleteLater();
    });
    pWatcher->setFuture(QtConcurrent::run([userAgent]() {
        return QJniObject::callStaticMethod<jstring>(kJavaClass,
                "fetch",
                "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                QJniObject::fromString(kReleasesUrl).object<jstring>(),
                QJniObject::fromString(userAgent).object<jstring>())
                .toString();
    }));
#endif
}

void QmlAppUpdateProxy::onReleasesFetched(const QString& json) {
    const QJsonDocument document = QJsonDocument::fromJson(json.toUtf8());
    if (!document.isArray()) {
        qWarning() << "Unable to read the releases of Mixxx Android";
        setState(State::Failed, Error::Network);
        return;
    }
    if (const UserSettingsPointer pConfig = QmlConfigProxy::get()) {
        pConfig->setValue(kLastCheckKey, QString::number(QDateTime::currentSecsSinceEpoch()));
    }

    QVersionNumber latest;
    QJsonObject latestRelease;
    QJsonObject latestApk;
    for (const QJsonValue& value : document.array()) {
        const QJsonObject release = value.toObject();
        if (release.value(QStringLiteral("draft")).toBool() ||
                release.value(QStringLiteral("prerelease")).toBool()) {
            continue;
        }
        const QRegularExpressionMatch match =
                kReleaseTag.match(release.value(QStringLiteral("tag_name")).toString());
        if (!match.hasMatch()) {
            continue;
        }
        const QVersionNumber version = QVersionNumber::fromString(match.captured(1));
        if (version <= latest) {
            continue;
        }
        for (const QJsonValue& asset : release.value(QStringLiteral("assets")).toArray()) {
            const QJsonObject apk = asset.toObject();
            if (apk.value(QStringLiteral("name")).toString().endsWith(QStringLiteral(".apk"))) {
                latest = version;
                latestRelease = release;
                latestApk = apk;
                break;
            }
        }
    }

    const QVersionNumber installed = installedVersion();
    if (latest.isNull() || installed.isNull() || latest <= installed) {
        setState(State::UpToDate);
        return;
    }
    m_latestVersion = latest.toString();
    m_releaseNotes = cleanReleaseNotes(latestRelease.value(QStringLiteral("body")).toString());
    m_releaseUrl = QUrl(latestRelease.value(QStringLiteral("html_url")).toString());
    m_apkUrl = latestApk.value(QStringLiteral("browser_download_url")).toString();
    // Only the file name: it becomes a path in the updates folder
    m_apkName = QFileInfo(latestApk.value(QStringLiteral("name")).toString()).fileName();
    m_downloadSize = static_cast<qint64>(latestApk.value(QStringLiteral("size")).toDouble());
    // e.g. "sha256:c98e…"; older releases of GitHub have no digest
    const QString digest = latestApk.value(QStringLiteral("digest")).toString();
    m_apkSha256 = digest.startsWith(QStringLiteral("sha256:")) ? digest.mid(7).toLower()
                                                               : QString();
    qInfo() << "Mixxx Android" << m_latestVersion << "is available";

    // Downloaded before, e.g. while Mixxx was closed
    if (QFileInfo(apkPath()).size() == m_downloadSize) {
        verifyDownload();
        return;
    }
    setState(State::Available);
}

void QmlAppUpdateProxy::download() {
#ifdef Q_OS_ANDROID
    if (m_state != State::Available && m_state != State::Failed) {
        return;
    }
    if (m_apkUrl.isEmpty()) {
        return;
    }
    const QString dir = updatesDir();
    if (QStorageInfo(dir).bytesAvailable() < m_downloadSize + kSpareBytes) {
        setState(State::Failed, Error::NoSpace);
        return;
    }
    m_downloadId = QJniObject::callStaticMethod<jlong>(kJavaClass,
            "startDownload",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/"
            "String;)J",
            context().object(),
            QJniObject::fromString(m_apkUrl).object<jstring>(),
            QJniObject::fromString(m_apkName).object<jstring>(),
            QJniObject::fromString(QStringLiteral("Mixxx ") + m_latestVersion).object<jstring>());
    if (m_downloadId < 0) {
        setState(State::Failed, Error::Download);
        return;
    }
    m_progress = 0;
    emit progressChanged();
    setState(State::Downloading);
    m_pollTimer.start();
#else
    Q_UNUSED(this);
#endif
}

void QmlAppUpdateProxy::slotPollDownload() {
#ifdef Q_OS_ANDROID
    QJniEnvironment env;
    const QJniObject result = QJniObject::callStaticMethod<jlongArray>(kJavaClass,
            "queryDownload",
            "(Landroid/content/Context;J)[J",
            context().object(),
            static_cast<jlong>(m_downloadId));
    jlong values[3] = {-1, 0, 0};
    if (result.isValid()) {
        env->GetLongArrayRegion(result.object<jlongArray>(), 0, 3, values);
    }
    const jlong status = values[0];
    if (status == kDownloadSuccessful) {
        m_pollTimer.stop();
        m_downloadId = -1;
        verifyDownload();
        return;
    }
    if (status == kDownloadFailed || status < 0) {
        // Failed, or removed from the notification
        m_pollTimer.stop();
        m_downloadId = -1;
        setState(State::Failed, Error::Download);
        return;
    }
    const jlong total = values[2] > 0 ? values[2] : m_downloadSize;
    if (total > 0) {
        m_progress = static_cast<double>(values[1]) / total;
        emit progressChanged();
    }
#else
    Q_UNUSED(this);
#endif
}

void QmlAppUpdateProxy::cancelDownload() {
#ifdef Q_OS_ANDROID
    if (m_state != State::Downloading) {
        return;
    }
    m_pollTimer.stop();
    QJniObject::callStaticMethod<void>(kJavaClass,
            "cancelDownload",
            "(Landroid/content/Context;J)V",
            context().object(),
            static_cast<jlong>(m_downloadId));
    m_downloadId = -1;
    setState(State::Available);
#else
    Q_UNUSED(this);
#endif
}

void QmlAppUpdateProxy::verifyDownload() {
    setState(State::Verifying);
    const QString path = apkPath();
    const QString expectedSha256 = m_apkSha256;
    const qint64 expectedSize = m_downloadSize;
    auto* pWatcher = new QFutureWatcher<bool>(this);
    connect(pWatcher, &QFutureWatcher<bool>::finished, this, [this, pWatcher, path]() {
        if (pWatcher->result()) {
            setState(State::Downloaded);
        } else {
            qWarning() << "The downloaded update" << path << "doesn't match the release";
            QFile::remove(path);
            setState(State::Failed, Error::Damaged);
        }
        pWatcher->deleteLater();
    });
    pWatcher->setFuture(QtConcurrent::run([path, expectedSha256, expectedSize]() {
        QFile file(path);
        if (file.size() != expectedSize || !file.open(QIODevice::ReadOnly)) {
            return false;
        }
        if (expectedSha256.isEmpty()) {
            return true;
        }
        QCryptographicHash hash(QCryptographicHash::Sha256);
        return hash.addData(&file) &&
                QString::fromLatin1(hash.result().toHex()) == expectedSha256;
    }));
}

bool QmlAppUpdateProxy::install() {
#ifdef Q_OS_ANDROID
    if (m_state != State::Downloaded) {
        return true;
    }
    const jint result = QJniObject::callStaticMethod<jint>(kJavaClass,
            "install",
            "(Landroid/content/Context;Ljava/lang/String;)I",
            context().object(),
            QJniObject::fromString(apkPath()).object<jstring>());
    if (result == kInstallNeedsPermission) {
        return false;
    }
    if (result == kInstallFailed) {
        setState(State::Failed, Error::Installer);
    }
    return true;
#else
    Q_UNUSED(this);
    return true;
#endif
}

void QmlAppUpdateProxy::skipLatest() {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || m_latestVersion.isEmpty()) {
        return;
    }
    pConfig->setValue(kSkippedVersionKey, m_latestVersion);
    pConfig->save();
    emit stateChanged();
}

void QmlAppUpdateProxy::setState(State state, Error error) {
    m_state = state;
    m_error = error;
    emit stateChanged();
}

void QmlAppUpdateProxy::removeOldDownloads() const {
#ifdef Q_OS_ANDROID
    const QVersionNumber installed = installedVersion();
    const QFileInfoList apks =
            QDir(updatesDir())
                    .entryInfoList({QStringLiteral("*.apk")}, QDir::Files);
    for (const QFileInfo& apk : apks) {
        const QRegularExpressionMatch match = kApkVersion.match(apk.fileName());
        if (!match.hasMatch() || QVersionNumber::fromString(match.captured(1)) <= installed) {
            QFile::remove(apk.absoluteFilePath());
        }
    }
#else
    Q_UNUSED(this);
#endif
}

} // namespace qml
} // namespace mixxx
