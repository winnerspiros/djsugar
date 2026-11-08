import Mixxx 1.0 as Mixxx
import QtQuick

// A knob of the Lite skin drawn as a bar, without a Canvas: the bar fills
// from the middle (EQ, filter) or from the left. Dragging up or down turns
// it, a double tap resets it.
Item {
    id: root

    required property string group
    required property string key
    property string label: ""
    property bool bipolar: true
    property color color: "#2f9bff"
    // The drag that turns the knob from one end to the other
    property real dragRange: 160
    readonly property real value: Math.max(0, Math.min(1, control.parameter))

    implicitHeight: 34
    implicitWidth: 60

    Mixxx.ControlProxy {
        id: control

        group: root.group
        key: root.key
    }
    Rectangle {
        id: bar

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        border.color: dragHandler.active ? "#ffffff" : "#3a3d44"
        border.width: 1
        color: "#1b1d21"
        height: Math.max(8, parent.height - labelText.height - 2)
        radius: 2

        Rectangle {
            readonly property real from: root.bipolar ? 0.5 : 0

            anchors.bottom: parent.bottom
            anchors.bottomMargin: 2
            anchors.top: parent.top
            anchors.topMargin: 2
            color: root.color
            radius: 1
            width: Math.abs(root.value - from) * (parent.width - 4)
            x: 2 + Math.min(root.value, from) * (parent.width - 4)
        }
        // The middle
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.verticalCenter: parent.verticalCenter
            color: "#5c6068"
            height: parent.height - 4
            visible: root.bipolar
            width: 1
        }
    }
    Text {
        id: labelText

        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        color: "#9a9ea6"
        font.pixelSize: 10
        text: root.label
    }
    DragHandler {
        id: dragHandler

        property real startValue: 0

        target: null
        xAxis.enabled: false

        onActiveChanged: {
            if (active) {
                startValue = control.parameter;
            }
        }
        onTranslationChanged: {
            if (active) {
                control.parameter = Math.max(0, Math.min(1, startValue - translation.y / root.dragRange));
            }
        }
    }
    TapHandler {
        onDoubleTapped: control.reset()
    }
}
