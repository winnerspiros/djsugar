import "Theme"
import QtQuick

// The DJ Sugar mark: the frosted donut record of the splash screen, drawn at
// any size from plain rectangles, followed by the wordmark. Nothing is loaded
// from disk and nothing animates, so it costs one cheap scene graph node group.
Item {
    id: root

    property real size: 28
    property bool showWordmark: true

    implicitHeight: root.size
    implicitWidth: root.showWordmark ? root.size + 7 + wordmark.implicitWidth : root.size

    // The donut, designed on a 66 px grid and scaled to `size`
    Item {
        id: donut

        anchors.verticalCenter: parent.verticalCenter
        height: root.size
        width: root.size

        Item {
            anchors.centerIn: parent
            height: 66
            scale: root.size / 66
            width: 66

            // Golden dough ring
            Rectangle {
                anchors.fill: parent
                border.color: "#b3722d"
                border.width: 2
                color: "#d5944a"
                radius: 33
            }
            // Pink sugar glaze
            Rectangle {
                anchors.centerIn: parent
                border.color: "#ff99ca"
                border.width: 1.5
                color: "#e63380"
                height: 56
                radius: 28
                width: 56

                Rectangle {
                    anchors.centerIn: parent
                    border.color: "#ba1b5c"
                    border.width: 1
                    color: "transparent"
                    height: 44
                    radius: 22
                    width: 44
                }
                Rectangle { color: "#0091ff"; height: 3; radius: 1.5; rotation: -22; width: 8; x: 24; y: 5 }
                Rectangle { color: "#ffffff"; height: 3; radius: 1.5; rotation: 35; width: 8; x: 41; y: 15 }
                Rectangle { color: "#ffb300"; height: 3; radius: 1.5; rotation: -40; width: 8; x: 42; y: 34 }
                Rectangle { color: "#ff6b57"; height: 3; radius: 1.5; rotation: 15; width: 8; x: 25; y: 46 }
                Rectangle { color: "#7cfc00"; height: 3; radius: 1.5; rotation: 48; width: 8; x: 8; y: 35 }
                Rectangle { color: "#ffffff"; height: 3; radius: 1.5; rotation: -32; width: 8; x: 9; y: 15 }
            }
            // Record label and spindle hole
            Rectangle {
                anchors.centerIn: parent
                border.color: "#2c2e34"
                border.width: 1.5
                color: "#1b1c20"
                height: 26
                radius: 13
                width: 26

                Rectangle {
                    anchors.centerIn: parent
                    border.color: "#cfd4dc"
                    border.width: 1.5
                    color: "#0e1117"
                    height: 6
                    radius: 3
                    width: 6
                }
            }
        }
    }

    // DJ in white, SUGAR in the glaze pink
    Row {
        id: wordmark

        anchors.left: donut.right
        anchors.leftMargin: 7
        anchors.verticalCenter: parent.verticalCenter
        spacing: root.size * 0.12
        visible: root.showWordmark

        Text {
            color: Theme.textColor
            font.bold: true
            font.letterSpacing: root.size * 0.08
            font.pixelSize: root.size * 0.5
            text: "DJ"
        }
        Text {
            color: "#ff4fa8"
            font.bold: true
            font.letterSpacing: root.size * 0.08
            font.pixelSize: root.size * 0.5
            text: "SUGAR"
        }
    }
}
