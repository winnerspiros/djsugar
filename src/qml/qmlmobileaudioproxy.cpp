#include "qml/qmlmobileaudioproxy.h"

#include "moc_qmlmobileaudioproxy.cpp"
#include "qml/qmlconfigproxy.h"

namespace mixxx {
namespace qml {

namespace {

// Same keys as in SoundManagerAndroid, which is only built on Android
const ConfigKey kUsbDirectModeKey =
        ConfigKey(QStringLiteral("[Soundcard]"), QStringLiteral("AndroidUsbDirectMode"));
const ConfigKey kAutoRouteControllerKey =
        ConfigKey(QStringLiteral("[Soundcard]"), QStringLiteral("AndroidAutoRouteController"));

} // namespace

QmlMobileAudioProxy::QmlMobileAudioProxy(QObject* pParent)
        : QObject(pParent) {
}

// static
QmlMobileAudioProxy* QmlMobileAudioProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileAudioProxy(pQmlEngine);
}

int QmlMobileAudioProxy::getAndroidUsbDirectMode() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig ? pConfig->getValue(kUsbDirectModeKey, 0) : 0;
}

void QmlMobileAudioProxy::setAndroidUsbDirectMode(int mode) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (pConfig) {
        pConfig->setValue(kUsbDirectModeKey, mode);
    }
}

bool QmlMobileAudioProxy::getAutoRouteController() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig ? pConfig->getValue(kAutoRouteControllerKey, true) : true;
}

void QmlMobileAudioProxy::setAutoRouteController(bool enabled) {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (pConfig) {
        pConfig->setValue(kAutoRouteControllerKey, enabled);
    }
}

} // namespace qml
} // namespace mixxx
