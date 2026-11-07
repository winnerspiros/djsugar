import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import "Theme"

Rectangle {
    id: root

    required property string group
    property alias knob: knob

    color: Theme.knobBackgroundColor
    height: 42
    opacity: statusControl.value ? 1 : 0.4
    radius: 5
    width: 42

    Skin.ControlKnob {
        id: knob

        anchors.horizontalCenter: root.horizontalCenter
        anchors.top: root.top
        group: root.group
        height: 36
        key: "super1"
        width: 36
    }
    // On/off button in the corner. A double tap on the whole knob toggled the
    // effect by accident while turning it on a touch screen.
    Rectangle {
        anchors.right: root.right
        anchors.top: root.top
        anchors.margins: 1
        border.color: Theme.white
        border.width: 1
        color: statusControl.value ? Theme.white : "transparent"
        height: 10
        radius: 5
        width: 10

        TapHandler {
            margin: 8

            onTapped: {
                statusControl.value = !statusControl.value;
            }
        }
    }
    Mixxx.ControlProxy {
        id: statusControl

        group: root.group
        key: "enabled"
    }
}
