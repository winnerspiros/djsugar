import QtQuick
import QtQuick.Shapes
import Mixxx 1.0 as Mixxx

// A knob of the FX Station: drag up or down to turn it, double tap to set it
// back. Bipolar knobs (-1..1) are off in the middle.
Item {
    id: root

    required property string key
    property string label: ""
    // The text under the value, e.g. the meaning of each side
    property string hint: ""
    property bool bipolar: false
    property real defaultValue: bipolar ? 0 : 0.5
    property color accentColor: "#00e5ff"
    // Shown instead of the percentage
    property string valueText: ""
    readonly property real value: control.value
    readonly property real minimum: bipolar ? -1 : 0
    readonly property real normalized: (value - minimum) / (1 - minimum)

    implicitWidth: 80
    implicitHeight: implicitWidth + labelText.implicitHeight + 4

    Mixxx.ControlProxy {
        id: control

        group: "[FxStation]"
        key: root.key
    }

    Item {
        id: dial

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        height: Math.min(root.width, root.height - labelText.implicitHeight - 4)
        width: height

        Rectangle {
            id: knobFace

            anchors.fill: parent
            anchors.margins: parent.width * 0.12
            border.color: "#3a4150"
            border.width: 1
            color: dragArea.pressed ? "#262c38" : "#1a1f29"
            radius: width / 2

            // The pointer
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                antialiasing: true
                color: "#ffffff"
                // Short, so that it doesn't cover the value
                height: parent.height * 0.2
                radius: width / 2
                transform: Rotation {
                    angle: -135 + 270 * root.normalized
                    origin.x: 1.5
                    origin.y: knobFace.height / 2 - 3
                }
                width: 3
                y: 3
            }
        }
        // The value as an arc, from the middle for bipolar knobs
        Shape {
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer

            ShapePath {
                capStyle: ShapePath.RoundCap
                fillColor: "transparent"
                strokeColor: "#2a303b"
                strokeWidth: Math.max(3, dial.width * 0.06)

                PathAngleArc {
                    centerX: dial.width / 2
                    centerY: dial.height / 2
                    radiusX: dial.width / 2 - 4
                    radiusY: dial.height / 2 - 4
                    startAngle: 135
                    sweepAngle: 270
                }
            }
            ShapePath {
                capStyle: ShapePath.RoundCap
                fillColor: "transparent"
                strokeColor: root.accentColor
                strokeWidth: Math.max(3, dial.width * 0.06)

                PathAngleArc {
                    centerX: dial.width / 2
                    centerY: dial.height / 2
                    radiusX: dial.width / 2 - 4
                    radiusY: dial.height / 2 - 4
                    startAngle: root.bipolar ? 270 : 135
                    sweepAngle: root.bipolar ? 135 * root.value : 270 * root.normalized
                }
            }
        }
        Text {
            anchors.centerIn: parent
            color: "#cfd8dc"
            font.bold: true
            font.pixelSize: Math.max(9, dial.width * 0.16)
            text: root.valueText.length > 0 ? root.valueText : (root.bipolar ? (root.value > 0 ? "+" : "") + Math.round(root.value * 100) : Math.round(root.value * 100))
        }
        MouseArea {
            id: dragArea

            property real startValue: 0
            property real startY: 0

            anchors.fill: parent
            preventStealing: true

            onDoubleClicked: control.value = root.defaultValue
            onPositionChanged: mouse => {
                // A drag of three knob heights turns it from end to end
                const range = 1 - root.minimum;
                const delta = (startY - mouse.y) / (dial.height * 3) * range;
                control.value = Math.max(root.minimum, Math.min(1, startValue + delta));
            }
            onPressed: mouse => {
                startY = mouse.y;
                startValue = control.value;
            }
        }
    }
    Text {
        id: labelText

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: dial.bottom
        anchors.topMargin: 2
        color: "#90a4ae"
        font.bold: true
        font.pixelSize: Math.max(9, Math.min(13, root.width * 0.12))
        // Narrow knobs: the hint shrinks instead of overflowing
        fontSizeMode: Text.HorizontalFit
        horizontalAlignment: Text.AlignHCenter
        minimumPixelSize: 7
        width: root.width
        text: root.hint.length > 0 ? root.label + "\n" + root.hint : root.label
    }
}
