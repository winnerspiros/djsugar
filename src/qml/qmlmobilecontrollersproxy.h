#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QTimer>
#include <QVariantList>

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
    Q_PROPERTY(bool bleScanning READ bleScanning NOTIFY bleStateChanged)
    Q_PROPERTY(QString bleStatus READ bleStatus NOTIFY bleStateChanged)
    Q_PROPERTY(QVariantList bleDevices READ bleDevices NOTIFY bleDevicesChanged)
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

    bool bleScanning() const {
        return m_bleScanning;
    }
    QString bleStatus() const {
        return m_bleStatus;
    }
    QVariantList bleDevices() const {
        return m_bleDevices;
    }

    Q_INVOKABLE void startBleScan();
    Q_INVOKABLE void stopBleScan();
    Q_INVOKABLE void refreshBleDevices();
    Q_INVOKABLE bool connectBleDevice(const QString& address, const QString& name);
    Q_INVOKABLE void disconnectBleDevice(const QString& address);

  signals:
    void bleStateChanged();
    void bleDevicesChanged();

  private:
    QTimer m_blePollTimer;
    int m_bleScanTicks = 0;
    bool m_bleScanning = false;
    QString m_bleStatus;
    QVariantList m_bleDevices;
};

} // namespace qml
} // namespace mixxx
