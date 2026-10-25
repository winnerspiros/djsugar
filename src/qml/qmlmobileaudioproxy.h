#pragma once

#include <QObject>
#include <QQmlEngine>

namespace mixxx {
namespace qml {

/// Controller functions used by the Android UI, exposed as the
/// `Mixxx.MobileAudio` singleton. Only the config keys are shared with
/// SoundManagerAndroid; the proxy itself is platform-independent.
/// Ported from JaviLabarumDJ/mixxx-android (android-stable).
class QmlMobileAudioProxy : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(MobileAudio)
    QML_SINGLETON

  public:
    explicit QmlMobileAudioProxy(QObject* pParent = nullptr);

    static QmlMobileAudioProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    Q_INVOKABLE int getAndroidUsbDirectMode() const;
    Q_INVOKABLE void setAndroidUsbDirectMode(int mode);

    Q_INVOKABLE bool getAutoRouteController() const;
    Q_INVOKABLE void setAutoRouteController(bool enabled);
};

} // namespace qml
} // namespace mixxx
