import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick
import "../Theme"

Skin.Button {
    id: root

    required property int effectNumber
    required property string effectUnitGroup
    readonly property bool focused: Math.round(focusedEffectControl.value) === effectNumber

    activeColor: Theme.effectColor
    highlight: focused
    text: "F"
    visible: showFocusControl.value > 0

    TapHandler {
        acceptedButtons: Qt.LeftButton

        // Never remove the focus: controllers (e.g. the DDJ-400 BEAT FX)
        // act on the focused effect
        onTapped: focusedEffectControl.value = root.effectNumber
    }
    TapHandler {
        acceptedButtons: Qt.RightButton

        onTapped: focusedEffectControl.value = 0
    }
    Mixxx.ControlProxy {
        id: focusedEffectControl

        group: root.effectUnitGroup
        key: "focused_effect"
    }
    Mixxx.ControlProxy {
        id: showFocusControl

        group: root.effectUnitGroup
        key: "show_focus"
    }
}
