import QtQuick

// A ◀ or ▶ drawn as a triangle: Android draws these signs as emoji, and
// its fonts lack the other triangles. Rotate it for ▲ and ▼.
Canvas {
    id: root

    // Points to the right, else to the left
    property bool pointsRight: true
    property color color: "#c0c4ca"

    implicitHeight: 10
    implicitWidth: 10

    onColorChanged: requestPaint()
    onHeightChanged: requestPaint()
    onPointsRightChanged: requestPaint()
    onWidthChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.fillStyle = root.color;
        ctx.beginPath();
        if (root.pointsRight) {
            ctx.moveTo(0, 0);
            ctx.lineTo(width, height / 2);
            ctx.lineTo(0, height);
        } else {
            ctx.moveTo(width, 0);
            ctx.lineTo(0, height / 2);
            ctx.lineTo(width, height);
        }
        ctx.closePath();
        ctx.fill();
    }
}
