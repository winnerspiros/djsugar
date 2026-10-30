#pragma once

#include <QString>

#include "effects/defs.h"

class EffectsManager;

namespace mixxx {
namespace android {

/// The name an effect chain preset is shown with. The QuickEffect presets
/// are generated on the first run, named after their effect in the language
/// used then; with the language switchable at runtime they are shown with
/// the effect's name in the current language. Other presets keep their name.
QString presetDisplayName(const EffectChainPresetPointer& pPreset, EffectsManager* pEffectsManager);

} // namespace android
} // namespace mixxx
