#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>

#include "preferences/usersettings.h"

namespace mixxx {
namespace qml {

/// Exposes the UI language to QML as the `Mixxx.Language` singleton.
///
/// This lives in its own file on purpose: it only builds on top of
/// `mixxx::Translations` and `QmlConfigProxy` without modifying them, so
/// that merging upstream Mixxx stays conflict-free.
class QmlLanguageProxy : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(Language)
    QML_SINGLETON

    /// The locale stored as [Config],Locale. An empty string means the
    /// system language.
    Q_PROPERTY(QString locale READ locale WRITE setLocale NOTIFY localeChanged)
    /// The languages the Android UI is translated to (those with a
    /// res/translations/mixxx-android_<lang>.qm) plus English, as a list
    /// of {locale, name, nativeName} maps sorted by name.
    Q_PROPERTY(QVariantList availableLocales READ availableLocales CONSTANT)

  public:
    explicit QmlLanguageProxy(QObject* pParent = nullptr);

    static QmlLanguageProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    /// Installs the catalog of the strings that only exist in the Android
    /// UI (res/translations/mixxx-android_*.qm) for the current default
    /// locale. It has to be called after `Translations::initializeTranslations()`.
    /// `pConfig` is needed before the QML singletons get the settings, at
    /// the first load of the QML.
    static void installAndroidTranslations(UserSettingsPointer pConfig = UserSettingsPointer());

    QString locale() const;
    void setLocale(const QString& locale);

    QVariantList availableLocales() const;

  signals:
    /// Emitted before the texts are translated again, so that views can
    /// keep their state while their qsTr() models are rebuilt.
    void localeAboutToChange();
    void localeChanged();
};

} // namespace qml
} // namespace mixxx
