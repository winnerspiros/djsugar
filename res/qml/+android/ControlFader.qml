import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12

Skin.Fader {
    id: root

    required property string group
    required property string key

    value: control.parameter

    onMoved: function (value) {
        control.parameter = value;
    }

    Mixxx.ControlProxy {
        id: control

        group: root.group
        key: root.key
    }

    // Touch dragging is relative to where the finger lands, like a real
    // fader: touching the handle to fine-tune never makes the value jump.
    MouseArea {
        id: touchArea

        property real startParameter: 0
        property real startPosition: 0

        function touchPosition(mouse) {
            return root.vertical ? 1.0 - mouse.y / height : mouse.x / width;
        }

        anchors.fill: parent
        preventStealing: true

        onPressed: mouse => {
            startParameter = control.parameter;
            startPosition = touchPosition(mouse);
        }
        onPositionChanged: mouse => {
            if (pressed) {
                const delta = touchPosition(mouse) - startPosition;
                control.parameter = Math.max(0.0, Math.min(1.0, startParameter + delta));
            }
        }
        onDoubleClicked: control.reset()
    }
}
