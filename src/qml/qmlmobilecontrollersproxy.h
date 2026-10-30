#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>

#include "qml/qmlpreferencesproxy.h"

namespace mixxx {
namespace qml {

/// Controller functions used by the Android UI, exposed as the
/// `Mixxx.MobileControllers` singleton. Kept apart from
/// QmlControllerManagerProxy so that merging upstream Mixxx stays
/// conflict-free.
class QmlMobileControllersProxy : public QObject {
    Q_OBJECT
    /// The public folder where users can add their own mappings
    Q_PROPERTY(QString mappingsDirectory READ mappingsDirectory CONSTANT)
    QML_NAMED_ELEMENT(MobileControllers)
    QML_SINGLETON

  public:
    explicit QmlMobileControllersProxy(QObject* pParent = nullptr);

    static QmlMobileControllersProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    QString mappingsDirectory() const;
    Q_INVOKABLE void copyToClipboard(const QString& text) const;

    /// Returns the mapping of `mappings` whose name best matches the device
    /// name, or nullptr. Used to suggest a mapping for new devices.
    Q_INVOKABLE mixxx::qml::QmlControllerMappingProxy* suggestedMapping(
            mixxx::qml::QmlControllerDeviceProxy* pDevice,
            const mixxx::qml::QmlControllerMappingProxyList& mappings) const;
};

} // namespace qml
} // namespace mixxx
