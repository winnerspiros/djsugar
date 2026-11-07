import Mixxx 1.0 as Mixxx
import QtQuick.Shapes
import QtQuick 2.12
import ".." as Skin
import "." as DeckComponent
import "../Theme"

Item {
    id: root

    property color buttonColor: trackLoadedControl.value > 0 ? Theme.buttonActiveColor : Theme.buttonDisableColor
    required property string group
    property var deck: null

    Mixxx.ControlProxy {
        id: trackLoadedControl

        group: root.group
        key: "track_loaded"
    }
    Skin.ControlButton {
        id: reverseButton

        activeColor: Theme.deckActiveColor
        anchors.left: parent.left
        group: root.group
        implicitHeight: 22
        implicitWidth: 22
        key: "reverse"

        contentItem: Shape {
            anchors.fill: parent
            antialiasing: true
            layer.enabled: true
            layer.samples: 4

            ShapePath {
                fillColor: root.buttonColor
                startX: 5
                startY: 11
                strokeColor: 'transparent'

                PathLine {
                    x: 20
                    y: 4
                }
                PathLine {
                    x: 20
                    y: 18
                }
                PathLine {
                    x: 5
                    y: 11
                }
            }
        }
    }
    // Opens the beatgrid corrections. Compact next to the pad switcher of
    // narrow decks.
    Skin.Button {
        id: beatgridButton

        readonly property bool compact: padSwitcher.visible

        anchors.right: ejectButton.left
        anchors.rightMargin: 5
        implicitHeight: 22
        implicitWidth: Math.max(40, implicitContentWidth + leftPadding + rightPadding)
        enabled: trackLoadedControl.value > 0
        text: compact ? qsTr("GRID") : qsTr("Beatgrid")
        visible: compact ? (root.width - padSwitcher.width) / 2 >= implicitWidth + ejectButton.width + 10 : root.width > 165

        onClicked: beatgridPopup.open()
    }
    DeckComponent.BeatgridPopup {
        id: beatgridPopup

        group: root.group
    }
    Row {
        id: padSwitcher
        anchors.centerIn: parent
        spacing: 4
        visible: root.deck ? root.deck.width < 520 : false

        Rectangle {
            id: cueBtn
            width: 50
            height: 22
            radius: 3
            color: (root.deck && root.deck.mobilePadView === 0) ? ((root.group === "[Channel2]") ? "#e65100" : "#1565c0") : "#1a1e24"
            border.color: (root.deck && root.deck.mobilePadView === 0) ? "#ffffff" : "#2f3745"
            border.width: 1

            Text {
                anchors.centerIn: parent
                text: "PADS"
                font.bold: true
                font.pixelSize: 9
                color: (root.deck && root.deck.mobilePadView === 0) ? "#ffffff" : "#9da5b4"
            }

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (root.deck) root.deck.mobilePadView = 0;
                }
            }
        }

        Rectangle {
            id: loopBtn
            width: 54
            height: 22
            radius: 3
            color: (root.deck && root.deck.mobilePadView === 1) ? "#00838f" : "#1a1e24"
            border.color: (root.deck && root.deck.mobilePadView === 1) ? "#ffffff" : "#2f3745"
            border.width: 1

            Text {
                anchors.centerIn: parent
                text: "IN / OUT"
                font.bold: true
                font.pixelSize: 8
                color: (root.deck && root.deck.mobilePadView === 1) ? "#ffffff" : "#9da5b4"
            }

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (root.deck) root.deck.mobilePadView = 1;
                }
            }
        }
    }
    Skin.ControlButton {
        id: ejectButton

        activeColor: Theme.deckActiveColor
        anchors.right: parent.right
        group: root.group
        implicitHeight: 22
        implicitWidth: 22
        key: "eject"

        contentItem: Item {
            anchors.fill: parent

            Shape {
                antialiasing: true
                height: 10
                layer.enabled: true
                layer.samples: 4
                width: 15

                anchors {
                    horizontalCenter: parent.horizontalCenter
                    top: parent.top
                    topMargin: 5
                }
                ShapePath {
                    fillColor: root.buttonColor
                    startX: 7.5
                    startY: 0
                    strokeColor: 'transparent'

                    PathLine {
                        x: 15
                        y: 10
                    }
                    PathLine {
                        x: 0
                        y: 10
                    }
                    PathLine {
                        x: 7.5
                        y: 0
                    }
                }
            }
            Rectangle {
                color: root.buttonColor
                height: 2
                width: 15

                anchors {
                    bottom: parent.bottom
                    bottomMargin: 3
                    horizontalCenter: parent.horizontalCenter
                }
            }
        }
    }
}
