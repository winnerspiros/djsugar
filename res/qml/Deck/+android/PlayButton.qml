import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Shapes
import QtQuick.Controls 2.12
import ".." as Skin
import "../Theme"

Skin.ControlButton {
    id: root

    property bool minimized: false

    activeColor: "#00e676"
    key: "play"
    toggleable: trackLoadedControl.value == 1

    contentItem: Item {
        id: content

        Shape {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 4
            antialiasing: true
            height: 28
            layer.enabled: true
            layer.samples: 4
            visible: !root.minimized
            width: 26

            ShapePath {
                capStyle: ShapePath.RoundCap
                fillColor: root.value > 0 ? "#00e676" : (trackLoadedControl.value > 0 ? "#ffffff" : Theme.buttonDisableColor)
                fillRule: ShapePath.WindingFill
                startX: 4
                startY: 0
                strokeColor: 'transparent'

                PathLine {
                    x: 24
                    y: 14
                }
                PathLine {
                    x: 4
                    y: 28
                }
                PathLine {
                    x: 4
                    y: 0
                }
            }
        }
        Label {
            id: label

            anchors.bottom: parent.bottom
            anchors.bottomMargin: 4
            anchors.horizontalCenter: parent.horizontalCenter
            color: root.value > 0 ? "#00e676" : (trackLoadedControl.value > 0 ? "#ffffff" : "#626262")
            font.bold: true
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.pixelSize: Theme.buttonFontPixelSize
            horizontalAlignment: Text.AlignHCenter
            text: "Play"
            verticalAlignment: Text.AlignVCenter

            states: [
                State {
                    when: root.minimized

                    AnchorChanges {
                        anchors.bottom: undefined
                        anchors.verticalCenter: content.verticalCenter
                        target: label
                    }
                }
            ]
        }
    }

    Mixxx.ControlProxy {
        id: trackLoadedControl

        group: root.group
        key: "track_loaded"
    }
}
