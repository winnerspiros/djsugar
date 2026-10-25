import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls

Skin.ComboBox {
    id: root

    required property Mixxx.EffectSlotProxy slot

    function syncCurrentEffect() {
        for (let index = 0; index < count; ++index) {
            if (model.get(index).effectId === slot.effectId) {
                currentIndex = index;
                return;
            }
        }
        currentIndex = 0;
    }

    readonly property string observedEffectId: root.slot?.effectId ?? ""

    model: Mixxx.EffectsManager.visibleEffectsModel
    popupMaxItem: 8
    textRole: "display"

    Component.onCompleted: syncCurrentEffect()
    onObservedEffectIdChanged: syncCurrentEffect()
    onActivated: index => {
        const effectId = model.get(index).effectId || "";
        if (slot.effectId !== effectId) {
            slot.effectId = effectId;
        }
    }
    onCountChanged: syncCurrentEffect()

}
