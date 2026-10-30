import QtQuick
import "." as Skin

// A text with a ◀ before it or a ▶ after it, drawn as triangles (see
// Triangle.qml), e.g. "◀ LOW PASS"
Row {
    id: root

    property alias text: label.text
    property alias color: label.color
    property alias font: label.font
    property bool arrowLeft: false
    property bool arrowRight: false
    readonly property real arrowSize: Math.round(label.font.pixelSize * 0.7)

    spacing: Math.round(label.font.pixelSize * 0.35)

    Skin.Triangle {
        anchors.verticalCenter: parent.verticalCenter
        color: label.color
        height: root.arrowSize
        pointsRight: false
        visible: root.arrowLeft
        width: Math.round(root.arrowSize * 0.85)
    }
    Text {
        id: label

        anchors.verticalCenter: parent.verticalCenter
    }
    Skin.Triangle {
        anchors.verticalCenter: parent.verticalCenter
        color: label.color
        height: root.arrowSize
        visible: root.arrowRight
        width: Math.round(root.arrowSize * 0.85)
    }
}
