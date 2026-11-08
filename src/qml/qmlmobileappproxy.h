#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QRectF>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QVariantList>

#include "preferences/usersettings.h"

class QQuickWindow;

namespace mixxx {
namespace qml {

/// Application information used by the Android UI, exposed as the
/// `Mixxx.MobileApp` singleton: the skin selector backing the Classic/Booth
/// toggle in Settings → Interface, and the hideable top-bar items.
/// Ported from JaviLabarumDJ/mixxx-android (android-stable).
class QmlMobileAppProxy : public QObject {
    Q_OBJECT
    /// The version of the installed package (versionName on Android), or
    /// the Mixxx version elsewhere
    Q_PROPERTY(QString packageVersion READ packageVersion CONSTANT)
    /// The skin of the user interface: "Classic", the main window in
    /// res/qml, or the name of a folder in res/qml/Skins with its own
    /// MainWindow.qml. Remembered across restarts.
    Q_PROPERTY(QString skin READ skin WRITE setSkin NOTIFY skinChanged)
    /// The items of the top bar the user hid in the settings, e.g. "wave",
    /// "xy" (see res/qml/Skins/Booth/MainWindow.qml)
    Q_PROPERTY(QStringList hiddenTopBarItems READ hiddenTopBarItems WRITE
                    setHiddenTopBarItems NOTIFY hiddenTopBarItemsChanged)
    /// The camera cutouts (notch, punch hole) of the display as QRectF in the
    /// QML coordinates of the window, empty without one. The skins draw edge
    /// to edge and move their controls clear of these.
    Q_PROPERTY(QVariantList cutouts READ cutouts NOTIFY cutoutsChanged)
    /// The graphics API of the interface chosen in the settings: "auto",
    /// "opengl", "vulkan", "metal", "d3d11" or "d3d12". Applied at the next
    /// start (Qt Quick picks it once, before the first window).
    Q_PROPERTY(QString graphicsApi READ graphicsApi WRITE setGraphicsApi NOTIFY
                    graphicsApiChanged)
    /// The choices for graphicsApi on this platform and build, as
    /// {value, label} maps
    Q_PROPERTY(QVariantList graphicsApis READ graphicsApis CONSTANT)
    /// The graphics API the interface runs with now, as a label
    Q_PROPERTY(QString activeGraphicsApi READ activeGraphicsApi CONSTANT)
    /// Multisample antialiasing of the window: 0 (off), 2 or 4 samples.
    /// Applied at the next start.
    Q_PROPERTY(int antialiasing READ antialiasing WRITE setAntialiasing NOTIFY
                    antialiasingChanged)
    QML_NAMED_ELEMENT(MobileApp)
    QML_SINGLETON

  public:
    explicit QmlMobileAppProxy(QObject* pParent = nullptr);

    static QmlMobileAppProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    QString packageVersion() const;
    /// The version of the installed package, see packageVersion
    /// (from JaviLabarumDJ/mixxx-android)
    static QString installedVersion();

    QString skin() const;
    void setSkin(const QString& skin);

    /// "Classic" and the skins in `skinsFolder` (res/qml/Skins), i.e. its
    /// subfolders that contain a MainWindow.qml
    Q_INVOKABLE QStringList skinsIn(const QUrl& skinsFolder) const;

    QString graphicsApi() const;
    void setGraphicsApi(const QString& api);
    QVariantList graphicsApis() const;
    QString activeGraphicsApi() const;
    int antialiasing() const;
    void setAntialiasing(int samples);

    /// Picks the graphics API and the surface format from the settings.
    /// Must run before the first QQuickWindow is created.
    static void applyGraphicsSettings(const UserSettingsPointer& pConfig);
    /// Confirms the chosen graphics API once `pWindow` has shown a frame,
    /// and goes back to the default one if the scene graph fails.
    static void watchGraphicsStartup(QQuickWindow* pWindow, const UserSettingsPointer& pConfig);

    QVariantList cutouts() const {
        return m_cutouts;
    }

    QStringList hiddenTopBarItems() const;
    void setHiddenTopBarItems(const QStringList& items);

  signals:
    void skinChanged();
    void graphicsApiChanged();
    void antialiasingChanged();
    void hiddenTopBarItemsChanged();
    void cutoutsChanged();

  private:
    void refreshCutouts();

    QVariantList m_cutouts;
    QTimer m_cutoutTimer;
};

} // namespace qml
} // namespace mixxx
