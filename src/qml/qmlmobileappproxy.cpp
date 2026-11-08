#include "qml/qmlmobileappproxy.h"

#include <QDir>
#include <QFileInfo>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QSurfaceFormat>
#if QT_CONFIG(vulkan)
#include <QVulkanInstance>
#endif

#ifdef Q_OS_ANDROID
#include <QCoreApplication>
#include <QGuiApplication>
#include <QJniEnvironment>
#include <QJniObject>
#include <QScreen>
#endif

#include "moc_qmlmobileappproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "util/versionstore.h"

namespace {

const ConfigKey kSkinConfigKey(QStringLiteral("[Config]"), QStringLiteral("AndroidSkin"));
const QString kClassicSkin = QStringLiteral("Classic");
const QString kDefaultSkin = QStringLiteral("Classic");
const ConfigKey kHiddenTopBarItemsKey(
        QStringLiteral("[Skin]"), QStringLiteral("AndroidHiddenTopBarItems"));

const ConfigKey kGraphicsApiKey(QStringLiteral("[Config]"), QStringLiteral("GraphicsApi"));
// Set to the chosen API before the window starts and cleared after its first
// frame: still set at the next start means that start never drew anything
// (driver crash, black window), so the default API is used again.
const ConfigKey kGraphicsApiPendingKey(
        QStringLiteral("[Config]"), QStringLiteral("GraphicsApiPending"));
const ConfigKey kAntialiasingKey(QStringLiteral("[Config]"), QStringLiteral("Antialiasing"));
const QString kAutoApi = QStringLiteral("auto");
constexpr int kDefaultSamples = 4;

bool vulkanAvailable() {
#if QT_CONFIG(vulkan)
    // Creating an instance is cheap and tells whether a Vulkan driver (the
    // loader and at least one device layer) is there at all.
    static const bool available = []() {
        QVulkanInstance instance;
        return instance.create();
    }();
    return available;
#else
    return false;
#endif
}

QStringList supportedApis() {
    QStringList apis{kAutoApi, QStringLiteral("opengl")};
    if (vulkanAvailable()) {
        apis << QStringLiteral("vulkan");
    }
#if defined(Q_OS_MACOS) || defined(Q_OS_IOS)
    apis << QStringLiteral("metal");
#endif
#ifdef Q_OS_WIN
    apis << QStringLiteral("d3d11");
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
    apis << QStringLiteral("d3d12");
#endif
#endif
    return apis;
}

QString apiLabel(const QString& api) {
    if (api == QLatin1String("opengl")) {
#if defined(Q_OS_ANDROID) || defined(Q_OS_IOS)
        return QStringLiteral("OpenGL ES");
#else
        return QStringLiteral("OpenGL");
#endif
    }
    if (api == QLatin1String("vulkan")) {
        return QStringLiteral("Vulkan");
    }
    if (api == QLatin1String("metal")) {
        return QStringLiteral("Metal");
    }
    if (api == QLatin1String("d3d11")) {
        return QStringLiteral("Direct3D 11");
    }
    if (api == QLatin1String("d3d12")) {
        return QStringLiteral("Direct3D 12");
    }
    return QObject::tr("Automatic");
}

int normalizedSamples(int samples) {
    return samples >= 4 ? 4 : (samples >= 2 ? 2 : 0);
}

} // namespace

namespace mixxx {
namespace qml {

QmlMobileAppProxy::QmlMobileAppProxy(QObject* pParent)
        : QObject(pParent) {
#ifdef Q_OS_ANDROID
    // The cutout moves to the other side when the phone is turned around,
    // which Android reports through no signal that is cheap to hook here.
    m_cutoutTimer.setInterval(1000);
    connect(&m_cutoutTimer, &QTimer::timeout, this, &QmlMobileAppProxy::refreshCutouts);
    m_cutoutTimer.start();
    refreshCutouts();
#endif
}

void QmlMobileAppProxy::refreshCutouts() {
#ifdef Q_OS_ANDROID
    QJniObject activity = QNativeInterface::QAndroidApplication::context();
    if (!activity.isValid()) {
        return;
    }
    QJniObject rects = QJniObject::callStaticObjectMethod(
            "org/mixxx/AndroidScreenManager",
            "getCutoutRects",
            "(Landroid/app/Activity;)[I",
            activity.object());
    QVariantList cutouts;
    if (rects.isValid()) {
        QJniEnvironment env;
        const auto array = rects.object<jintArray>();
        const jsize count = env->GetArrayLength(array);
        QList<jint> values(count);
        env->GetIntArrayRegion(array, 0, count, values.data());
        // QML pixels are device pixels divided by the density
        const QScreen* pScreen = QGuiApplication::primaryScreen();
        const qreal ratio = pScreen && pScreen->devicePixelRatio() > 0
                ? pScreen->devicePixelRatio()
                : 1.0;
        for (jsize i = 0; i + 3 < count; i += 4) {
            cutouts.append(QRectF(values[i] / ratio,
                    values[i + 1] / ratio,
                    (values[i + 2] - values[i]) / ratio,
                    (values[i + 3] - values[i + 1]) / ratio));
        }
    }
    if (cutouts != m_cutouts) {
        m_cutouts = cutouts;
        emit cutoutsChanged();
    }
#endif
}

// static
QmlMobileAppProxy* QmlMobileAppProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileAppProxy(pQmlEngine);
}

QString QmlMobileAppProxy::packageVersion() const {
    return installedVersion();
}

// static
QString QmlMobileAppProxy::installedVersion() {
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
    path.replace(QStringLiteral("/+android/"), QStringLiteral("/"));
    path.replace(QStringLiteral("/Settings/"), QStringLiteral("/"));
    const QDir dir(path);
    const QStringList folders = dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QString& folder : folders) {
        if (folder != kClassicSkin &&
                QFileInfo::exists(dir.filePath(folder + QStringLiteral("/MainWindow.qml")))) {
            skins.append(folder);
        }
    }
#ifdef Q_OS_ANDROID
    // APK assets fallback: QDir::entryList() cannot list assets:/ subdirectories
    if (skins.size() <= 1) {
        const QStringList knownSkins{
                QStringLiteral("Booth"),
                QStringLiteral("Lite"),
        };
        for (const QString& folder : knownSkins) {
            if (!skins.contains(folder) &&
                    QFileInfo::exists(
                            dir.filePath(folder + QStringLiteral("/MainWindow.qml")))) {
                skins.append(folder);
            }
        }
    }
#endif
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

QString QmlMobileAppProxy::graphicsApi() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    const QString api = pConfig ? pConfig->getValue(kGraphicsApiKey, kAutoApi) : kAutoApi;
    return supportedApis().contains(api) ? api : kAutoApi;
}

void QmlMobileAppProxy::setGraphicsApi(const QString& api) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || !supportedApis().contains(api) || api == graphicsApi()) {
        return;
    }
    pConfig->setValue(kGraphicsApiKey, api);
    pConfig->save();
    emit graphicsApiChanged();
}

bool QmlMobileAppProxy::showVinyl() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return !pConfig ||
            pConfig->getValue(ConfigKey(QStringLiteral("[Config]"),
                                      QStringLiteral("AndroidShowVinyl")),
                    true);
}

void QmlMobileAppProxy::setShowVinyl(bool show) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || show == showVinyl()) {
        return;
    }
    pConfig->setValue(ConfigKey(QStringLiteral("[Config]"), QStringLiteral("AndroidShowVinyl")),
            show);
    emit showVinylChanged();
}

double QmlMobileAppProxy::uiScale() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    const double scale = pConfig
            ? pConfig->getValue(ConfigKey(QStringLiteral("[Config]"),
                                        QStringLiteral("AndroidUiScale")),
                      0.45)
            : 0.45;
    return (scale >= 0.3 && scale <= 1.5) ? scale : 0.45;
}

void QmlMobileAppProxy::setUiScale(double scale) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig || scale < 0.3 || scale > 1.5 || qFuzzyCompare(scale, uiScale())) {
        return;
    }
    // Read by main.cpp before Qt starts, so it applies at the next start
    pConfig->setValue(ConfigKey(QStringLiteral("[Config]"), QStringLiteral("AndroidUiScale")),
            QString::number(scale, 'f', 2));
    pConfig->save();
    emit uiScaleChanged();
}

QVariantList QmlMobileAppProxy::graphicsApis() const {
    QVariantList list;
    for (const QString& api : supportedApis()) {
        list.append(QVariantMap{{QStringLiteral("value"), api},
                {QStringLiteral("label"), apiLabel(api)}});
    }
    return list;
}

QString QmlMobileAppProxy::activeGraphicsApi() const {
    switch (QQuickWindow::graphicsApi()) {
    case QSGRendererInterface::OpenGL:
        return apiLabel(QStringLiteral("opengl"));
    case QSGRendererInterface::Vulkan:
        return apiLabel(QStringLiteral("vulkan"));
    case QSGRendererInterface::Metal:
        return apiLabel(QStringLiteral("metal"));
    case QSGRendererInterface::Direct3D11:
        return apiLabel(QStringLiteral("d3d11"));
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
    case QSGRendererInterface::Direct3D12:
        return apiLabel(QStringLiteral("d3d12"));
#endif
    case QSGRendererInterface::Software:
        return QStringLiteral("Software");
    default:
        return QString();
    }
}

int QmlMobileAppProxy::antialiasing() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return normalizedSamples(
            pConfig ? pConfig->getValue(kAntialiasingKey, kDefaultSamples) : kDefaultSamples);
}

void QmlMobileAppProxy::setAntialiasing(int samples) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    samples = normalizedSamples(samples);
    if (!pConfig || samples == antialiasing()) {
        return;
    }
    pConfig->setValue(kAntialiasingKey, samples);
    pConfig->save();
    emit antialiasingChanged();
}

// static
void QmlMobileAppProxy::applyGraphicsSettings(const UserSettingsPointer& pConfig) {
    QString api = pConfig->getValue(kGraphicsApiKey, kAutoApi);
    const QString pending = pConfig->getValue(kGraphicsApiPendingKey, QString());
    if (!pending.isEmpty() && pending == api && api != kAutoApi) {
        qWarning() << "The" << apiLabel(api)
                   << "interface did not start last time, using the default graphics API";
        api = kAutoApi;
        pConfig->setValue(kGraphicsApiKey, api);
    }
    if (!supportedApis().contains(api)) {
        api = kAutoApi;
    }
    pConfig->setValue(kGraphicsApiPendingKey, api == kAutoApi ? QString() : api);
    pConfig->save();

    // Automatic: the native API on macOS and Windows; OpenGL (ES) elsewhere,
    // which the offscreen rendering of controller screens needs.
    if (api == QLatin1String("opengl")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
    } else if (api == QLatin1String("vulkan")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Vulkan);
    } else if (api == QLatin1String("metal")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Metal);
    } else if (api == QLatin1String("d3d11")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Direct3D11);
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
    } else if (api == QLatin1String("d3d12")) {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Direct3D12);
#endif
    } else {
#if !defined(Q_OS_MACOS) && !defined(Q_OS_WIN)
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
#endif
    }

    QSurfaceFormat format = QSurfaceFormat::defaultFormat();
    // Multisampling antialiases the QtQuick.Shapes strokes (knob arcs,
    // waveform markers); their "antialiasing" property does nothing without
    // it. 4x costs four samples per pixel of the whole window, which a
    // 3088x1440 phone screen feels: 2x or off trade edges for speed.
    format.setSamples(normalizedSamples(pConfig->getValue(kAntialiasingKey, kDefaultSamples)));
    // Nothing draws through the window background: no alpha channel spares
    // the compositor blending the whole window every frame.
    format.setAlphaBufferSize(0);
    // Qt Quick only needs depth/stencil for opaque batching and clipping;
    // 24/8 is what every GPU stores natively.
    format.setDepthBufferSize(24);
    format.setStencilBufferSize(8);
    format.setSwapBehavior(QSurfaceFormat::DoubleBuffer);
    QSurfaceFormat::setDefaultFormat(format);
    qInfo() << "Graphics:" << apiLabel(api) << "requested," << format.samples()
            << "x multisampling";
}

// static
void QmlMobileAppProxy::watchGraphicsStartup(
        QQuickWindow* pWindow, const UserSettingsPointer& pConfig) {
    if (!pWindow || pConfig->getValue(kGraphicsApiPendingKey, QString()).isEmpty()) {
        return;
    }
    QObject::connect(
            pWindow,
            &QQuickWindow::frameSwapped,
            pWindow,
            [pConfig]() {
                pConfig->setValue(kGraphicsApiPendingKey, QString());
                pConfig->save();
            },
            Qt::SingleShotConnection);
    QObject::connect(pWindow,
            &QQuickWindow::sceneGraphError,
            pWindow,
            [pConfig](QQuickWindow::SceneGraphError, const QString& message) {
                qWarning() << "Scene graph failed:" << message
                           << "- using the default graphics API from the next start";
                pConfig->setValue(kGraphicsApiKey, kAutoApi);
                pConfig->setValue(kGraphicsApiPendingKey, QString());
                pConfig->save();
            });
}

} // namespace qml
} // namespace mixxx
