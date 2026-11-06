#include "qml/qmlmobilecontrollersproxy.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QStandardPaths>
#include <QVariantMap>

#if defined(Q_OS_ANDROID)
#include <QtCore/private/qandroidextras_p.h>
#include <jni.h>

#include <QJniEnvironment>
#include <QJniObject>
#endif

#include "controllers/controller.h"
#include "controllers/mappingmatcher.h"
#include "moc_qmlmobilecontrollersproxy.cpp"

namespace mixxx {
namespace qml {

QmlMobileControllersProxy::QmlMobileControllersProxy(QObject* pParent)
        : QObject(pParent),
          m_bleStatus(tr("Scan to connect Bluetooth / BLE MIDI controllers (e.g. DDJ-FLX4)")) {
    m_blePollTimer.setInterval(750);
    connect(&m_blePollTimer, &QTimer::timeout, this, [this]() {
        refreshBleDevices();
        if (m_bleScanning) {
            ++m_bleScanTicks;
            if (m_bleScanTicks >= 8) {
                stopBleScan();
            }
        }
    });
}

// static
QmlMobileControllersProxy* QmlMobileControllersProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileControllersProxy(pQmlEngine);
}

QString QmlMobileControllersProxy::mappingsDirectory() const {
#if defined(Q_OS_ANDROID)
    return QStringLiteral("/storage/emulated/0/Mixxx/controllers/");
#else
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation) +
            QStringLiteral("/controllers/");
#endif
}

void QmlMobileControllersProxy::copyToClipboard(const QString& text) const {
    if (auto* pClipboard = QGuiApplication::clipboard()) {
        pClipboard->setText(text);
    }
}

QmlControllerMappingProxy* QmlMobileControllersProxy::suggestedMapping(
        QmlControllerDeviceProxy* pDevice,
        const QmlControllerMappingProxyList& mappings) const {
    if (!pDevice || !pDevice->internal()) {
        return nullptr;
    }
    QmlControllerMappingProxy* pBestMatch = nullptr;
    int bestScore = 0;
    for (auto* pMapping : mappings) {
        const int score = mixxx::mappingNameMatchScore(
                pDevice->getName(), pMapping->getName());
        if (score <= 0) {
            continue;
        }
        if (score > bestScore ||
                (score == bestScore &&
                        pMapping->getName().size() < pBestMatch->getName().size())) {
            bestScore = score;
            pBestMatch = pMapping;
        }
    }
    return pBestMatch;
}

void QmlMobileControllersProxy::startBleScan() {
#if defined(Q_OS_ANDROID)
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid()) {
        m_bleStatus = tr("Android context unavailable");
        emit bleStateChanged();
        return;
    }
    const bool hasPerms = QJniObject::callStaticMethod<jboolean>(
            "org/mixxx/BleMidiScanner",
            "hasRequiredPermissions",
            "(Landroid/content/Context;)Z",
            context.object<jobject>());
    if (!hasPerms) {
        QJniObject::callStaticMethod<void>(
                "org/mixxx/BleMidiScanner",
                "requestPermissionsIfNeeded",
                "(Landroid/app/Activity;)V",
                context.object<jobject>());
        m_bleStatus =
                tr("Please allow Nearby Devices / Bluetooth permission, then "
                   "tap Scan Bluetooth again");
        emit bleStateChanged();
        return;
    }
    const bool btEnabled = QJniObject::callStaticMethod<jboolean>(
            "org/mixxx/BleMidiScanner",
            "isBluetoothEnabled",
            "(Landroid/content/Context;)Z",
            context.object<jobject>());
    if (!btEnabled) {
        m_bleStatus =
                tr("Bluetooth is turned off. Turn on Bluetooth in Android "
                   "settings and scan again.");
        emit bleStateChanged();
        return;
    }

    const bool started = QJniObject::callStaticMethod<jboolean>(
            "org/mixxx/BleMidiScanner",
            "startScan",
            "(Landroid/content/Context;Ljava/lang/String;)Z",
            context.object<jobject>(),
            QJniObject::fromString(
                    QStringLiteral("03B80E5A-EDE8-4B33-A751-6CE34EC4C700"))
                    .object<jstring>());
    m_bleScanning = started;
    m_bleScanTicks = 0;
    m_bleStatus = started
            ? tr("Scanning for Bluetooth MIDI controllers...")
            : tr("Could not start Bluetooth scan");
    emit bleStateChanged();
    refreshBleDevices();
    if (started) {
        m_blePollTimer.start();
    }
#else
    m_bleStatus = tr("Bluetooth MIDI scanning is only supported on Android");
    emit bleStateChanged();
#endif
}

void QmlMobileControllersProxy::stopBleScan() {
#if defined(Q_OS_ANDROID)
    QJniObject::callStaticMethod<void>(
            "org/mixxx/BleMidiScanner",
            "stopScan",
            "()V");
#endif
    m_blePollTimer.stop();
    m_bleScanning = false;
    refreshBleDevices();
    if (m_bleDevices.isEmpty()) {
        m_bleStatus =
                tr("No Bluetooth MIDI devices found. Put your controller in "
                   "pairing mode (e.g. hold SHIFT+BROWSE on DDJ-FLX4) and tap "
                   "Scan Bluetooth.");
    } else {
        m_bleStatus = tr(
                "Found %1 Bluetooth device(s). Tap Connect to pair & enable.")
                              .arg(m_bleDevices.size());
    }
    emit bleStateChanged();
}

void QmlMobileControllersProxy::refreshBleDevices() {
#if defined(Q_OS_ANDROID)
    QJniObject arrObj = QJniObject::callStaticObjectMethod(
            "org/mixxx/BleMidiScanner",
            "getDiscoveredDeviceStrings",
            "()[Ljava/lang/String;");
    if (!arrObj.isValid()) {
        return;
    }
    QJniEnvironment env;
    jobjectArray arr = arrObj.object<jobjectArray>();
    const jsize count = env->GetArrayLength(arr);

    QVariantList midiList;
    QVariantList otherList;
    for (jsize i = 0; i < count; ++i) {
        QJniObject elem = QJniObject::fromLocalRef(env->GetObjectArrayElement(arr, i));
        if (!elem.isValid()) {
            continue;
        }
        const QString line = elem.toString();
        const QStringList parts = line.split(QLatin1Char('|'));
        if (parts.size() < 2) {
            continue;
        }
        const QString address = parts.at(0);
        const QString name = parts.at(1);
        const bool isMidi = parts.size() >= 3 && parts.at(2) == QStringLiteral("1");
        const bool connected = parts.size() >= 4 && parts.at(3) == QStringLiteral("1");

        QVariantMap item;
        item[QStringLiteral("address")] = address;
        item[QStringLiteral("name")] = name;
        item[QStringLiteral("isMidi")] = isMidi;
        item[QStringLiteral("connected")] = connected;
        if (isMidi || connected) {
            midiList.append(item);
        } else {
            otherList.append(item);
        }
    }
    QVariantList combined = midiList;
    combined.append(otherList);
    if (combined != m_bleDevices) {
        m_bleDevices = combined;
        emit bleDevicesChanged();
    }
#endif
}

bool QmlMobileControllersProxy::connectBleDevice(
        const QString& address, const QString& name) {
#if defined(Q_OS_ANDROID)
    QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid() || address.isEmpty()) {
        return false;
    }
    m_bleStatus = tr("Connecting to %1...").arg(name.isEmpty() ? address : name);
    emit bleStateChanged();

    const bool ok = QJniObject::callStaticMethod<jboolean>(
            "org/mixxx/BleMidiScanner",
            "connectDevice",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Z",
            context.object<jobject>(),
            QJniObject::fromString(address).object<jstring>(),
            QJniObject::fromString(name).object<jstring>());

    refreshBleDevices();
    m_bleStatus = ok ? tr("Connected to %1 — configuring controller mapping...")
                               .arg(name.isEmpty() ? address : name)
                     : tr("Could not connect to %1. Ensure Bluetooth MIDI "
                          "pairing is active.")
                               .arg(name.isEmpty() ? address : name);
    emit bleStateChanged();
    return ok;
#else
    Q_UNUSED(address);
    Q_UNUSED(name);
    return false;
#endif
}

void QmlMobileControllersProxy::disconnectBleDevice(const QString& address) {
#if defined(Q_OS_ANDROID)
    if (address.isEmpty()) {
        return;
    }
    QJniObject::callStaticMethod<void>(
            "org/mixxx/BleMidiScanner",
            "disconnectDevice",
            "(Ljava/lang/String;)V",
            QJniObject::fromString(address).object<jstring>());
    refreshBleDevices();
    m_bleStatus = tr("Disconnected %1").arg(address);
    emit bleStateChanged();
#else
    Q_UNUSED(address);
#endif
}

} // namespace qml
} // namespace mixxx
