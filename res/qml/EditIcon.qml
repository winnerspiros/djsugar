import QtQuick

// A pencil, drawn: the edit glyph of the fonts is missing on Android
Item {
    id: icon

    property color color: "#ffffff"
    signal clicked

    implicitWidth: 14
    implicitHeight: 14

    Canvas {
        id: canvas

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d");
            const s = Math.min(width, height);
            ctx.reset();
            ctx.strokeStyle = icon.color;
            ctx.fillStyle = icon.color;
            ctx.lineWidth = Math.max(1.5, s * 0.14);
            ctx.lineJoin = "round";
            // Body, from the top right down to the tip
            ctx.beginPath();
            ctx.moveTo(s * 0.72, s * 0.08);
            ctx.lineTo(s * 0.92, s * 0.28);
            ctx.lineTo(s * 0.34, s * 0.86);
            ctx.lineTo(s * 0.14, s * 0.66);
            ctx.closePath();
            ctx.stroke();
            // Tip
            ctx.beginPath();
            ctx.moveTo(s * 0.14, s * 0.66);
            ctx.lineTo(s * 0.34, s * 0.86);
            ctx.lineTo(s * 0.06, s * 0.94);
            ctx.closePath();
            ctx.fill();
        }
    }
    onColorChanged: canvas.requestPaint()
    onWidthChanged: canvas.requestPaint()
    onHeightChanged: canvas.requestPaint()

    MouseArea {
        anchors.fill: parent
        anchors.margins: -8
        onClicked: icon.clicked()
    }
}
