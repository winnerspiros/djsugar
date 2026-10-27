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

    /// When Mixxx drives USB sound cards itself, see
    /// mixxx::androidusb::DirectMode (0 = auto, 1 = always, 2 = never).
    /// Takes effect after a restart.
    Q_INVOKABLE int getAndroidUsbDirectMode() const;
    Q_INVOKABLE void setAndroidUsbDirectMode(int mode);
    /// Whether a DJ controller sound card automatically gets the Main
    /// output on channels 1-2 and the Headphones on channels 3-4
    Q_INVOKABLE bool getAutoRouteController() const;
    Q_INVOKABLE void setAutoRouteController(bool enabled);
};

} // namespace qml
} // namespace mixxx
