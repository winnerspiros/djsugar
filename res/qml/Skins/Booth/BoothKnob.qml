import Mixxx 1.0 as Mixxx
import QtQuick

// A knob of the booth skin: a ring that lights from its middle (EQ, trim)
// or from its start. Dragging up or down turns it, a double tap resets it.
// Pointer handlers, so that several knobs and faders can be moved at once
// with several fingers.
Item {
    id: root

    required property string group
    required property string key
    property string label: ""
    // The ring lights from the middle, for controls whose default is there
    property bool bipolar: true
    property color color: "#2f9bff"
    // The drag that turns the knob from one end to the other
    property real dragRange: 160
    readonly property real value: control.parameter
    readonly property real ringSize: Math.max(12, Math.min(width, height - (labelText.visible ? labelText.height + 1 : 0)))

    function clamp(value) {
        return Math.max(0, Math.min(1, value));
    }

    implicitHeight: 40 + (labelText.visible ? labelText.implicitHeight + 1 : 0)
    implicitWidth: 40

    onColorChanged: ring.requestPaint()
    onValueChanged: ring.requestPaint()

    Mixxx.ControlProxy {
        id: control

        group: root.group
        key: root.key
    }
    Canvas {
        id: ring

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        height: root.ringSize
        width: root.ringSize

        onHeightChanged: requestPaint()
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            const center = width / 2;
            const lineWidth = Math.max(2, width / 11);
            const radius = center - lineWidth;
            // From the bottom left over the top to the bottom right
            const start = Math.PI * 0.75;
            const end = Math.PI * 2.25;
            const at = start + root.clamp(root.value) * (end - start);
            const from = root.bipolar ? Math.PI * 1.5 : start;
            ctx.lineCap = "round";
            ctx.lineWidth = lineWidth;
            ctx.strokeStyle = "#2a2c30";
            ctx.beginPath();
            ctx.arc(center, center, radius, start, end);
            ctx.stroke();
            if (Math.abs(at - from) > 0.01) {
                ctx.strokeStyle = root.color;
                ctx.beginPath();
                ctx.arc(center, center, radius, Math.min(from, at), Math.max(from, at));
                ctx.stroke();
            }
            ctx.lineWidth = Math.max(1.5, width / 18);
            ctx.strokeStyle = "#ffffff";
            ctx.beginPath();
            ctx.moveTo(center + Math.cos(at) * radius * 0.3, center + Math.sin(at) * radius * 0.3);
            ctx.lineTo(center + Math.cos(at) * radius * 0.8, center + Math.sin(at) * radius * 0.8);
            ctx.stroke();
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
                    control.parameter = root.clamp(startValue - translation.y / root.dragRange);
                }
            }
        }
        TapHandler {
            onDoubleTapped: control.reset()
        }
    }
    Text {
        id: labelText

        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        color: "#9a9ea6"
        font.pixelSize: Math.max(8, Math.min(13, Math.round(root.width * 0.22)))
        text: root.label
        visible: root.label.length > 0
    }
}
