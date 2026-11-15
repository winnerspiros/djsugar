#pragma once

#include <QObject>
#include <QQmlEngine>

namespace mixxx {
namespace qml {

/// Controller functions for the Android UI, exposed as
/// `Mixxx.MobileAudio`. Only config keys are shared with
/// SoundManagerAndroid; the proxy is platform-independent.
class QmlMobileAudioProxy : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(MobileAudio)
    QML_SINGLETON

  public:
    explicit QmlMobileAudioProxy(QObject* pParent = nullptr);

    static QmlMobileAudioProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    /// USB direct mode for sound cards
    /// (0 = auto, 1 = always, 2 = never). Requires restart.
    Q_INVOKABLE int getAndroidUsbDirectMode() const;
    Q_INVOKABLE void setAndroidUsbDirectMode(int mode);
    /// DJ controller sound card auto-routes Main to ch 1-2, Headphones to ch 3-4
    Q_INVOKABLE bool getAutoRouteController() const;
    Q_INVOKABLE void setAutoRouteController(bool enabled);
};

} // namespace qml
} // namespace mixxx
