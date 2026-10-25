#include "qml/qmllanguageproxy.h"

#include <QCoreApplication>
#include <QDir>
#include <QLocale>
#include <QPointer>
#include <QTranslator>
#include <algorithm>

#include "moc_qmllanguageproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "util/translations.h"

namespace {

const ConfigKey kLocaleConfigKey(QStringLiteral("[Config]"), QStringLiteral("Locale"));
const QString kAndroidTranslationsPrefix = QStringLiteral("mixxx-android");

QPointer<QTranslator> s_pAndroidTranslator;

QString translationsPath(const UserSettingsPointer& pConfig) {
    return pConfig->getResourcePath() + QStringLiteral("translations/");
}

} // namespace

namespace mixxx {
namespace qml {

QmlLanguageProxy::QmlLanguageProxy(QObject* pParent)
        : QObject(pParent) {
}

// static
QmlLanguageProxy* QmlLanguageProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlLanguageProxy(pQmlEngine);
}

// static
void QmlLanguageProxy::installAndroidTranslations(UserSettingsPointer pConfig) {
    auto* pApp = QCoreApplication::instance();
    if (!pConfig) {
        pConfig = QmlConfigProxy::get();
    }
    if (!pApp || !pConfig) {
        return;
    }
    if (s_pAndroidTranslator) {
        pApp->removeTranslator(s_pAndroidTranslator);
        delete s_pAndroidTranslator;
    }
    const QLocale locale;
    if (locale.language() == QLocale::C || locale.language() == QLocale::English) {
        // English is the source language
        return;
    }
    auto* pTranslator = new QTranslator(pApp);
    // Only the language of the locale itself, e.g. "es_MX" then "es". Loading
    // by QLocale would also try the other languages the user prefers in the
    // system settings and could pick Spanish for an English or Greek UI.
    if (!pTranslator->load(kAndroidTranslationsPrefix + QChar('_') + locale.name(),
                translationsPath(pConfig))) {
        delete pTranslator;
        return;
    }
    qInfo() << "Loaded" << kAndroidTranslationsPrefix << "translations for locale"
            << locale.name() << "from" << pTranslator->filePath();
    // Installed last, so it takes precedence over the upstream catalog
    pApp->installTranslator(pTranslator);
    s_pAndroidTranslator = pTranslator;
}

QString QmlLanguageProxy::locale() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig ? pConfig->getValue(kLocaleConfigKey, QString()) : QString();
}

void QmlLanguageProxy::setLocale(const QString& locale) {
    auto* pApp = QCoreApplication::instance();
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pApp || !pConfig || locale == this->locale()) {
        return;
    }
    emit localeAboutToChange();
    if (locale.isEmpty()) {
        pConfig->remove(kLocaleConfigKey);
        // Translations::initializeTranslations() keeps the previous default
        // locale when none is configured.
        QLocale::setDefault(QLocale::system());
    } else {
        pConfig->setValue(kLocaleConfigKey, locale);
    }
    // Android may close the app without the usual shutdown, which would
    // lose the change
    pConfig->save();

    // Translations::initializeTranslations() parents all translators it
    // installs to the application, so remove them before reloading.
    const auto translators = pApp->findChildren<QTranslator*>(Qt::FindDirectChildrenOnly);
    for (auto* pTranslator : translators) {
        pApp->removeTranslator(pTranslator);
        delete pTranslator;
    }
    mixxx::Translations::initializeTranslations(pConfig, pApp, QString());
    installAndroidTranslations();

    auto* pEngine = qobject_cast<QQmlEngine*>(parent());
    if (!pEngine) {
        pEngine = qmlEngine(this);
    }
    qInfo() << "UI language changed to" << QLocale().name() << ", retranslating"
            << (pEngine ? "the QML engine" : "nothing (no QML engine)");
    if (pEngine) {
        pEngine->setUiLanguage(QLocale().name());
        // Re-evaluates all qsTr() bindings with the new translators
        pEngine->retranslate();
    }
    emit localeChanged();
}

QVariantList QmlLanguageProxy::availableLocales() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    if (!pConfig) {
        return {};
    }
    const QDir translationsDir(translationsPath(pConfig));
    // Only the languages the Android UI is translated to. The upstream
    // catalogs (mixxx_*.qm) don't cover the QML UI, so the other languages
    // would show the UI in English anyway.
    const QStringList fileNames =
            translationsDir.entryList({kAndroidTranslationsPrefix + QStringLiteral("_*.qm")});
    // Source language
    QStringList localeNames{QStringLiteral("en_US")};
    for (const auto& fileName : fileNames) {
        QString localeName = fileName.mid(fileName.indexOf('_') + 1);
        localeName.truncate(localeName.lastIndexOf('.'));
        // The translation files use both '-' and '_' as separator
        localeName.replace('-', '_');
        if (!localeNames.contains(localeName)) {
            localeNames.append(localeName);
        }
    }

    QVariantList locales;
    for (const auto& localeName : std::as_const(localeNames)) {
        const QLocale locale(localeName);
        if (locale.language() == QLocale::C) {
            continue;
        }
        QString name = QLocale::languageToString(locale.language());
        QString nativeName = locale.nativeLanguageName();
        if (locale.language() == QLocale::English) {
            // The source language, stored as "en_US" like in the desktop
            // preferences
            nativeName = name;
        } else if (localeName.contains('_')) {
            name += QStringLiteral(" (") + QLocale::territoryToString(locale.territory()) + ')';
            nativeName += QStringLiteral(" (") + locale.nativeTerritoryName() + ')';
        }
        if (!nativeName.isEmpty()) {
            // e.g. "español" -> "Español"
            nativeName = locale.toUpper(nativeName.left(1)) + nativeName.mid(1);
        }
        locales.append(QVariantMap{
                {QStringLiteral("locale"), localeName},
                {QStringLiteral("name"), name},
                {QStringLiteral("nativeName"), nativeName.isEmpty() ? name : nativeName},
        });
    }
    std::sort(locales.begin(), locales.end(), [](const QVariant& a, const QVariant& b) {
        return a.toMap()
                       .value(QStringLiteral("name"))
                       .toString()
                       .localeAwareCompare(b.toMap().value(QStringLiteral("name")).toString()) < 0;
    });
    return locales;
}

} // namespace qml
} // namespace mixxx
