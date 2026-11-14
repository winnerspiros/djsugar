#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>

#include "preferences/usersettings.h"

namespace mixxx {
namespace qml {

/// Exposes the UI language to QML as the `Mixxx.Language` singleton.
/// Lives in its own file so merging upstream Mixxx stays conflict-free.
class QmlLanguageProxy : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(Language)
    QML_SINGLETON

    /// The locale stored as [Config],Locale. Empty string = system language.
    Q_PROPERTY(QString locale READ locale WRITE setLocale NOTIFY localeChanged)
    /// Languages with mixxx-android_<lang>.qm plus English, as
    /// {locale, name, nativeName} maps.
    Q_PROPERTY(QVariantList availableLocales READ availableLocales CONSTANT)

  public:
    explicit QmlLanguageProxy(QObject* pParent = nullptr);

    static QmlLanguageProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    /// Installs Android-only translation catalogs after
    /// `Translations::initializeTranslations()`.
    /// `pConfig` is needed before QML singletons get the settings.
    static void installAndroidTranslations(UserSettingsPointer pConfig = UserSettingsPointer());

    QString locale() const;
    void setLocale(const QString& locale);

    QVariantList availableLocales() const;

  signals:
    /// Emitted before texts are re-translated, so views can keep state
    void localeAboutToChange();
    void localeChanged();
};

} // namespace qml
} // namespace mixxx
