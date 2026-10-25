import Mixxx 1.0 as Mixxx
import QtQuick

// A fader of the booth skin: the channel faders, the crossfader and the
// tempo. The handle follows the finger, a double tap resets it. Pointer
// handlers, so that several faders can be moved at once.
Item {
    id: root

    required property string group
    required property string key
    property bool horizontal: false
    // The top (or the left) is the start of the range, like the tempo
    // slider of a player, whose top is minus
    property bool inverted: false
    property color color: "#d8dadd"
    // A mark in the middle of the track, e.g. for the tempo and the
    // crossfader
    property bool centerMark: false
    // 0: bottom or left, 1: top or right
    readonly property real position: inverted ? 1 - control.parameter : control.parameter
    readonly property real handleLength: Math.max(14, Math.round((horizontal ? height : width) * 0.6))
    readonly property real travel: Math.max(1, (horizontal ? width : height) - handleLength)

    function setPosition(position) {
        const clamped = Math.max(0, Math.min(1, position));
        control.parameter = inverted ? 1 - clamped : clamped;
    }

    implicitHeight: horizontal ? 28 : 120
    implicitWidth: horizontal ? 120 : 28

    Mixxx.ControlProxy {
        id: control

        group: root.group
        key: root.key
    }
    Rectangle {
        id: track

        anchors.centerIn: parent
        color: "#2a2c30"
        height: root.horizontal ? 4 : parent.height - root.handleLength / 2
        radius: 2
        width: root.horizontal ? parent.width - root.handleLength / 2 : 4
    }
    Rectangle {
        anchors.centerIn: parent
        color: "#5c6068"
        height: root.horizontal ? parent.height * 0.6 : 1
        visible: root.centerMark
        width: root.horizontal ? 1 : parent.width * 0.6
    }
    Rectangle {
        id: handle

        border.color: dragHandler.active ? "#ffffff" : "#8a8d93"
        border.width: 1
        color: "#3a3c40"
        height: root.horizontal ? parent.height : root.handleLength * 0.55
        radius: 2
        width: root.horizontal ? root.handleLength * 0.55 : parent.width
        x: root.horizontal ? root.position * (root.width - width) : 0
        y: root.horizontal ? 0 : (1 - root.position) * (root.height - height)

        Rectangle {
            anchors.centerIn: parent
            color: root.color
            height: root.horizontal ? parent.height - 6 : 2
            width: root.horizontal ? 2 : parent.width - 6
        }
    }
    DragHandler {
        id: dragHandler

        property real startPosition: 0

        target: null

        onActiveChanged: {
            if (active) {
                startPosition = root.position;
            }
        }
        onTranslationChanged: {
            if (!active) {
                return;
            }
            const moved = root.horizontal ? translation.x / (root.width - handle.width) : -translation.y / (root.height - handle.height);
            root.setPosition(startPosition + moved);
        }
    }
    TapHandler {
        onDoubleTapped: control.reset()
    }
}
