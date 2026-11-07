import QtQuick

// A flat button of the Lite skin: a rectangle and a text, no images,
// gradients or animations. Lit in its color while `lit`.
Rectangle {
    id: root

    property string text: ""
    property bool lit: false
    property color litColor: "#2f9bff"
    property int fontPixelSize: 14
    readonly property bool pressed: tapHandler.pressed

    signal clicked
    // Held down or released, for buttons that act while held (CUE, hot cues)
    signal pressing(bool down)
    signal longPressed

    border.color: lit ? litColor : "#3a3d44"
    border.width: 1
    color: pressed ? "#33363d" : (lit ? Qt.darker(litColor, 3) : "#1b1d21")
    implicitHeight: 44
    implicitWidth: 64
    radius: 3

    Text {
        anchors.fill: parent
        anchors.margins: 2
        color: root.lit ? root.litColor : "#d8dadd"
        elide: Text.ElideRight
        font.bold: true
        font.pixelSize: root.fontPixelSize
        horizontalAlignment: Text.AlignHCenter
        text: root.text
        verticalAlignment: Text.AlignVCenter
    }
    TapHandler {
        id: tapHandler

        gesturePolicy: TapHandler.WithinBounds

        onLongPressed: root.longPressed()
        onPressedChanged: root.pressing(pressed)
        onTapped: root.clicked()
    }
}
