#include "qml/qmlmobileappproxy.h"

#include <QDir>
#include <QFileInfo>

#ifdef Q_OS_ANDROID
#include <QCoreApplication>
#include <QJniObject>
#endif

#include "moc_qmlmobileappproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "util/versionstore.h"

namespace {

const ConfigKey kSkinConfigKey(QStringLiteral("[Config]"), QStringLiteral("AndroidSkin"));
const QString kClassicSkin = QStringLiteral("Classic");
const QString kDefaultSkin = QStringLiteral("Booth");
const ConfigKey kHiddenTopBarItemsKey(
        QStringLiteral("[Skin]"), QStringLiteral("AndroidHiddenTopBarItems"));

} // namespace

namespace mixxx {
namespace qml {

QmlMobileAppProxy::QmlMobileAppProxy(QObject* pParent)
        : QObject(pParent) {
}

// static
QmlMobileAppProxy* QmlMobileAppProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileAppProxy(pQmlEngine);
}

QString QmlMobileAppProxy::packageVersion() const {
#ifdef Q_OS_ANDROID
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    QJniObject packageManager = context.callObjectMethod(
            "getPackageManager", "()Landroid/content/pm/PackageManager;");
    QJniObject packageName = context.callObjectMethod("getPackageName", "()Ljava/lang/String;");
    QJniObject packageInfo = packageManager.callObjectMethod("getPackageInfo",
            "(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;",
            packageName.object<jstring>(),
            jint(0));
    if (packageInfo.isValid()) {
        const QString versionName =
                packageInfo.getObjectField<jstring>("versionName").toString();
        if (!versionName.isEmpty()) {
            return versionName;
        }
    }
#endif
    return VersionStore::version();
}

QString QmlMobileAppProxy::skin() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (pConfig) {
        const QString skin = pConfig->getValue(kSkinConfigKey, QString());
        if (!skin.isEmpty()) {
            return skin;
        }
    }
    // Booth is the mobile UI; desktop keeps the Classic main window unless
    // the user picked a skin in Settings -> Interface.
#ifdef Q_OS_ANDROID
    return kDefaultSkin;
#else
    return kClassicSkin;
#endif
}

void QmlMobileAppProxy::setSkin(const QString& skin) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || skin.isEmpty() || skin == this->skin()) {
        return;
    }
    pConfig->setValue(kSkinConfigKey, skin);
    pConfig->save();
    emit skinChanged();
}

QStringList QmlMobileAppProxy::skinsIn(const QUrl& skinsFolder) const {
    QStringList skins{kClassicSkin};
    // Without full storage access the QML is loaded from the APK
    // ("assets:/qml/..."), which QDir reads with the scheme as a prefix
    QString path;
    if (skinsFolder.isLocalFile()) {
        path = skinsFolder.toLocalFile();
    } else if (skinsFolder.scheme() == QStringLiteral("qrc")) {
        path = QChar(':') + skinsFolder.path();
    } else if (!skinsFolder.scheme().isEmpty()) {
        path = skinsFolder.scheme() + QChar(':') + skinsFolder.path();
    } else {
        path = skinsFolder.path();
    }
    const QDir dir(path);
    const QStringList folders = dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QString& folder : folders) {
        if (folder != kClassicSkin &&
                QFileInfo::exists(dir.filePath(folder + QStringLiteral("/MainWindow.qml")))) {
            skins.append(folder);
        }
    }
    return skins;
}

QStringList QmlMobileAppProxy::hiddenTopBarItems() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig) {
        return {};
    }
    return pConfig->getValue(kHiddenTopBarItemsKey, QString())
            .split(QChar(','), Qt::SkipEmptyParts);
}

void QmlMobileAppProxy::setHiddenTopBarItems(const QStringList& items) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || items == hiddenTopBarItems()) {
        return;
    }
    pConfig->setValue(kHiddenTopBarItemsKey, items.join(QChar(',')));
    // Android may close the app without the usual shutdown
    pConfig->save();
    emit hiddenTopBarItemsChanged();
}

} // namespace qml
} // namespace mixxx
