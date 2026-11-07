import Qt5Compat.GraphicalEffects
import QtQuick 2
import QtQuick.Controls 2
import "Theme"
import "." as Skin

AbstractButton {
    id: root

    property color activeColor: Theme.buttonActiveColor
    property bool highlight: false
    property color normalColor: Theme.buttonNormalColor
    property color pressedColor: activeColor
    property color backgroundColor: Theme.darkGray4
    // A ◀ before or a ▶ after the text, drawn as triangles: Android draws
    // these signs as emoji
    property bool arrowLeft: false
    property bool arrowRight: false
    readonly property real arrowSize: Math.round(label.font.pixelSize * 0.7)
    readonly property real arrowSpace: Math.round(arrowSize * 0.85 + label.font.pixelSize * 0.35)

    implicitHeight: 26
    implicitWidth: Math.max(52, label.implicitWidth + 16)

    // Flat rounded surface. No shadow or glow effects: they cost a full
    // offscreen pass per control on phone GPUs.
    background: Rectangle {
        id: backgroundImage

        anchors.fill: parent
        color: root.backgroundColor
        radius: 10
    }
    contentItem: Item {
        anchors.fill: parent

        Label {
            id: label

            anchors.fill: parent
            color: root.normalColor
            font.bold: true
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.pixelSize: Theme.buttonFontPixelSize
            horizontalAlignment: Text.AlignHCenter
            // Room for the arrows, keeping them and the text centered
            leftPadding: root.arrowLeft ? root.arrowSpace : 0
            rightPadding: root.arrowRight ? root.arrowSpace : 0
            text: root.text
            verticalAlignment: Text.AlignVCenter
            visible: root.text != null
        }
        Repeater {
            model: [root.arrowLeft ? -1 : 0, root.arrowRight ? 1 : 0].filter(side => side !== 0)

            Skin.Triangle {
                required property int modelData
                // Where the centered text starts and ends
                readonly property real textStart: label.leftPadding + (label.width - label.leftPadding - label.rightPadding - label.contentWidth) / 2

                anchors.verticalCenter: parent.verticalCenter
                color: label.color
                height: root.arrowSize
                pointsRight: modelData > 0
                width: Math.round(root.arrowSize * 0.85)
                x: label.text.length === 0 ? (root.width - width) / 2 : (modelData < 0 ? textStart - root.arrowSpace : textStart + label.contentWidth + root.arrowSpace - width)
            }
        }
        Image {
            id: image

            anchors.centerIn: parent
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            height: icon.height
            source: icon.source
            visible: false
            width: icon.width
        }
        ColorOverlay {
            anchors.fill: image
            antialiasing: true
            color: root.normalColor
            source: image
            visible: icon.source != null
        }
    }
    states: [
        State {
            name: "pressed"
            when: root.pressed

            PropertyChanges {
                color: root.checked ? Theme.accentColor : Theme.darkGray3
                target: backgroundImage
            }
            PropertyChanges {
                color: root.pressedColor
                target: label
            }
        },
        State {
            name: "active"
            when: (root.highlight || root.checked) && !root.pressed

            PropertyChanges {
                color: Theme.accentColor
                target: backgroundImage
            }
            PropertyChanges {
                color: root.activeColor
                target: label
            }
        },
        State {
            name: "inactive"
            when: !root.checked && !root.highlight && !root.pressed

            PropertyChanges {
                color: root.normalColor
                target: label
            }
        }
    ]
}
