import ".." as Skin
import Mixxx 1.0 as Mixxx
import Qt5Compat.GraphicalEffects
import QtQuick 2.12
import QtQuick.Layouts
import QtQuick.Shapes
import QtQuick.Controls 2.12
import "../Theme"

Item {
    id: root

    required property var currentTrack
    required property string group

    property int activePadMode: 0 // 0: Hotcue, 1: Loop

    Rectangle {
        color: '#0E0E0E'

        anchors {
            bottom: parent.bottom
            left: parent.left
            right: tabs.left
            rightMargin: 6
            top: parent.top
        }
        Item {
            id: hotcueTab

            anchors.fill: parent
            visible: root.activePadMode === 0

            Skin.FadeBehavior on visible {
                fadeTarget: hotcueTab
            }

            GridLayout {
                anchors.bottomMargin: 8
                anchors.fill: parent
                anchors.leftMargin: root.width < 260 ? 4 : 16
                anchors.rightMargin: root.width < 260 ? 4 : 16
                anchors.topMargin: 8
                columnSpacing: root.width < 260 ? 3 : 6
                rowSpacing: 6

                Repeater {
                    model: 8

                    Item {
                        required property int index

                        Layout.column: index % 4
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        Layout.row: parseInt(index / 4)

                        Skin.Hotcue {
                            id: hotcue

                            readonly property var label: (isSet && root.currentTrack && root.currentTrack.hotcuesModel) ? root.currentTrack.hotcuesModel.get(index).label : null

                            activate: activator.pressedButtons == Qt.LeftButton
                            // onIsSetChanged: {
                            //     if (!isSet)
                            //         popup.close();
                            // }
                            group: root.group
                            hotcueNumber: index + 1
                        }
                        Rectangle {
                            id: backgroundImage

                            anchors.fill: parent
                            color: hotcue.isSet ? hotcue.color : (activator.pressed ? "#3b4252" : "#20242b")
                            radius: 3
                            border.color: hotcue.isSet ? "#ffffff" : "#323946"
                            border.width: 1

                            MouseArea {
                                id: activator

                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                anchors.fill: parent
                            }
                        }
                        DropShadow {
                            id: effect1

                            anchors.fill: backgroundImage
                            color: "#80000000"
                            horizontalOffset: 0
                            radius: 1.0
                            source: backgroundImage
                            verticalOffset: 0
                        }
                        InnerShadow {
                            id: effect2

                            anchors.fill: parent
                            color: "#353535"
                            horizontalOffset: 1
                            radius: 12
                            samples: 24
                            source: effect1
                            spread: 0.2
                            verticalOffset: 1
                        }
                        InnerShadow {
                            anchors.fill: parent
                            color: "#353535"
                            horizontalOffset: -1
                            radius: 12
                            samples: 24
                            source: effect2
                            spread: 0.2
                            verticalOffset: -1
                        }
                        ColumnLayout {
                            anchors.centerIn: backgroundImage
                            spacing: 0

                            Label {
                                Layout.alignment: Qt.AlignHCenter
                                color: hotcue.isSet ? "#ffffff" : "#9aaec4"
                                font.pixelSize: 13
                                font.weight: Font.Bold
                                text: `${index + 1}`
                            }
                            Label {
                                Layout.alignment: Qt.AlignHCenter
                                color: hotcue.isSet ? "#ffffff" : "#7c8d9e"
                                font.pixelSize: 10
                                text: hotcue.label ?? ""
                                visible: hotcue.label
                            }
                        }
                    }
                }
            }
        }
        Item {
            id: loopTab

            anchors.fill: parent
            visible: root.activePadMode === 1

            Skin.FadeBehavior on visible {
                fadeTarget: loopTab
            }

            GridLayout {
                anchors.bottomMargin: 8
                anchors.fill: parent
                anchors.leftMargin: root.width < 260 ? 4 : 16
                anchors.rightMargin: root.width < 260 ? 4 : 16
                anchors.topMargin: 8
                columnSpacing: root.width < 260 ? 3 : 6
                rowSpacing: 6

                Repeater {
                    model: [
                        { label: "1/4", size: "0.25" },
                        { label: "1/2", size: "0.5" },
                        { label: "1", size: "1" },
                        { label: "2", size: "2" },
                        { label: "4", size: "4" },
                        { label: "8", size: "8" },
                        { label: "16", size: "16" },
                        { label: "32", size: "32" }
                    ]

                    Item {
                        id: loopPadItem
                        required property int index
                        required property var modelData

                        Layout.column: index % 4
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        Layout.row: parseInt(index / 4)

                        Mixxx.ControlProxy {
                            id: loopToggleProxy
                            group: root.group
                            key: "beatloop_" + loopPadItem.modelData.size + "_toggle"
                        }

                        Mixxx.ControlProxy {
                            id: loopEnabledProxy
                            group: root.group
                            key: "beatloop_" + loopPadItem.modelData.size + "_enabled"
                        }

                        Rectangle {
                            id: loopPadBg
                            anchors.fill: parent
                            radius: 4
                            color: (loopEnabledProxy.value > 0)
                                ? "#00e5ff"
                                : (loopPadMouse.pressed ? "#374151" : "#1a222e")
                            border.color: (loopEnabledProxy.value > 0) ? "#ffffff" : "#2c3b4d"
                            border.width: (loopEnabledProxy.value > 0) ? 2 : 1

                            ColumnLayout {
                                anchors.centerIn: parent
                                spacing: 1

                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    font.bold: true
                                    font.pixelSize: 15
                                    color: (loopEnabledProxy.value > 0) ? "#001018" : "#00e5ff"
                                    text: loopPadItem.modelData.label
                                }

                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    font.bold: true
                                    font.pixelSize: 8
                                    color: (loopEnabledProxy.value > 0) ? "#00364a" : "#78909c"
                                    text: "BEATS"
                                }
                            }

                            MouseArea {
                                id: loopPadMouse
                                anchors.fill: parent
                                onClicked: {
                                    loopToggleProxy.trigger();
                                }
                            }
                        }
                    }
                }
            }
        }

    }
    ColumnLayout {
        id: tabs

        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.rightMargin: root.width < 260 ? 3 : 6
        anchors.top: parent.top
        spacing: root.width < 260 ? 3 : 6
        width: root.width < 260 ? 36 : 44

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: 5
            color: root.activePadMode === 0 ? ((root.group === "[Channel2]") ? "#e65100" : "#1565c0") : (cueTabMouse.pressed ? "#2e3440" : "#181c24")
            border.color: root.activePadMode === 0 ? "#ffffff" : "#2c3544"
            border.width: root.activePadMode === 0 ? 1.5 : 1

            ColumnLayout {
                anchors.centerIn: parent
                spacing: 3

                Shape {
                    Layout.alignment: Qt.AlignHCenter
                    width: 12
                    height: 10
                    antialiasing: true
                    ShapePath {
                        fillColor: root.activePadMode === 0 ? "#ffffff" : "#8293a7"
                        strokeColor: "transparent"
                        startX: 1; startY: 0
                        PathLine { x: 11; y: 5 }
                        PathLine { x: 1; y: 10 }
                        PathLine { x: 1; y: 0 }
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: "CUE"
                    font.bold: true
                    font.pixelSize: 9
                    color: root.activePadMode === 0 ? "#ffffff" : "#8293a7"
                }
            }

            MouseArea {
                id: cueTabMouse
                anchors.fill: parent
                onClicked: {
                    root.activePadMode = 0;
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: 5
            color: root.activePadMode === 1 ? "#00838f" : (loopTabMouse.pressed ? "#2e3440" : "#181c24")
            border.color: root.activePadMode === 1 ? "#ffffff" : "#2c3544"
            border.width: root.activePadMode === 1 ? 1.5 : 1

            ColumnLayout {
                anchors.centerIn: parent
                spacing: 3

                Shape {
                    Layout.alignment: Qt.AlignHCenter
                    width: 14
                    height: 10
                    antialiasing: true
                    ShapePath {
                        fillColor: "transparent"
                        strokeColor: root.activePadMode === 1 ? "#ffffff" : "#8293a7"
                        strokeWidth: 1.6
                        capStyle: ShapePath.RoundCap
                        startX: 2; startY: 2.5
                        PathLine { x: 12; y: 2.5 }
                        PathLine { x: 12; y: 7.5 }
                        PathLine { x: 2; y: 7.5 }
                    }
                    ShapePath {
                        fillColor: root.activePadMode === 1 ? "#ffffff" : "#8293a7"
                        strokeColor: "transparent"
                        startX: 0.5; startY: 5.5
                        PathLine { x: 4; y: 5.5 }
                        PathLine { x: 2.2; y: 2 }
                        PathLine { x: 0.5; y: 5.5 }
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: "LOOP"
                    font.bold: true
                    font.pixelSize: 9
                    color: root.activePadMode === 1 ? "#ffffff" : "#8293a7"
                }
            }

            MouseArea {
                id: loopTabMouse
                anchors.fill: parent
                onClicked: {
                    root.activePadMode = 1;
                }
            }
        }
    }
}
