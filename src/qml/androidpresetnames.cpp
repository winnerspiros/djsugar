#include "qml/androidpresetnames.h"

#include <QCoreApplication>
#include <QLocale>
#include <QTranslator>

#include "effects/backends/effectmanifest.h"
#include "effects/effectsmanager.h"
#include "effects/presets/effectchainpreset.h"
#include "effects/presets/effectpreset.h"
#include "qml/qmlconfigproxy.h"

namespace {

// The Android user interface is available in English and Spanish, so a
// preset generated on a Spanish first run has a Spanish effect name
const QTranslator& spanishTranslator() {
    static QTranslator s_translator;
    static const bool s_loaded = [] {
        const UserSettingsPointer pConfig = mixxx::qml::QmlConfigProxy::get();
        return pConfig &&
                s_translator.load(QLocale(QLocale::Spanish),
                        QStringLiteral("mixxx"),
                        QStringLiteral("_"),
                        pConfig->getResourcePath() + QStringLiteral("translations/"));
    }();
    Q_UNUSED(s_loaded);
    return s_translator;
}

} // namespace

namespace mixxx {
namespace android {

QString presetDisplayName(const EffectChainPresetPointer& pPreset,
        EffectsManager* pEffectsManager) {
    const QString name = pPreset->name();
    EffectManifestPointer pManifest;
    for (const auto& pEffectPreset : pPreset->effectPresets()) {
        if (pEffectPreset->isEmpty()) {
            continue;
        }
        if (pManifest) {
            // Several effects: made by the user
            return name;
        }
        pManifest = pEffectsManager->getBackendManager()->getManifest(pEffectPreset);
        if (!pManifest) {
            return name;
        }
    }
    if (!pManifest) {
        return name;
    }
    const QString effectName = pManifest->displayName();
    if (name == effectName) {
        return name;
    }
    if (QCoreApplication::translate("QObject", name.toUtf8().constData()) == effectName ||
            spanishTranslator().translate("QObject", effectName.toUtf8().constData()) ==
                    name) {
        return effectName;
    }
    return name;
}

} // namespace android
} // namespace mixxx
