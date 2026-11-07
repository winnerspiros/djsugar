import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Layouts
import QtQuick.Shapes
import QtQuick.Controls 2.12
import "../Theme"

Item {
    id: root

    required property var currentTrack
    required property string group

    property int activePadMode: 0 // 0: Hotcue, 1: Loop, 2: Stems

    // The names of the stems of the stem files made by the app
    function stemName(label) {
        switch (label) {
        case "Drums":
            return qsTr("DRUMS");
        case "Bass":
            return qsTr("BASS");
        case "Vocals":
            return qsTr("VOCALS");
        case "Other":
            return qsTr("OTHER");
        default:
            return label.toUpperCase();
        }
    }

    // Plays only one stem, or all again if it is the only one playing
    function soloStem(index) {
        let soloed = true;
        for (let i = 0; i < stemRepeater.count; ++i) {
            const muted = stemRepeater.itemAt(i).muteControl.value > 0;
            if (muted !== (i !== index))
                soloed = false;
        }
        for (let i = 0; i < stemRepeater.count; ++i)
            stemRepeater.itemAt(i).muteControl.value = soloed ? 0 : (i === index ? 0 : 1);
    }

    Mixxx.ControlProxy {
        id: stemCountControl

        group: root.group
        key: "stem_count"

        onValueChanged: {
            if (value <= 0 && root.activePadMode === 2)
                root.activePadMode = 0;
        }
    }

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
                            color: hotcue.isSet ? hotcue.color : (activator.pressed ? Theme.darkGray3 : Theme.darkGray4)
                            radius: 10
                            border.color: hotcue.isSet ? Theme.white : Theme.darkGray3
                            border.width: 1

                            MouseArea {
                                id: activator

                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                anchors.fill: parent
                            }
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

        // The stems of the track, two by two like the pads of the booth
        // skin: tap to mute, hold to play only that one
        GridLayout {
            id: stemTab

            anchors.fill: parent
            anchors.margins: root.width < 260 ? 4 : 8
            columns: 2
            columnSpacing: root.width < 260 ? 3 : 5
            rowSpacing: root.width < 260 ? 3 : 5
            visible: root.activePadMode === 2

            Repeater {
                id: stemRepeater

                model: root.currentTrack ? root.currentTrack.stemsModel : 0

                ColumnLayout {
                    id: stem

                    required property int index
                    required property color color
                    required property string label
                    readonly property string group: `${root.group.substr(0, root.group.length - 1)}_Stem${index + 1}]`
                    readonly property bool muted: muteControl.value > 0
                    property alias muteControl: muteControl

                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    // The stems share the space equally
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: 1
                    Layout.minimumWidth: 0
                    Layout.minimumHeight: 0
                    spacing: 0

                    Mixxx.ControlProxy {
                        id: muteControl

                        group: stem.group
                        key: "mute"
                    }

                    Rectangle {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        radius: 4
                        // A playing stem is tinted with its color
                        color: stemMouse.pressed ? "#2e3440" : (stem.muted ? "#14171d" : Qt.darker(stem.color, 2.2))
                        border.color: stemMouse.pressed ? "#ffffff" : stem.color
                        border.width: stem.muted ? 1 : 2

                        Text {
                            anchors.fill: parent
                            anchors.margins: 3
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            // Long names shrink to fit narrow decks
                            fontSizeMode: Text.Fit
                            minimumPixelSize: 7
                            text: root.stemName(stem.label)
                            color: stem.muted ? "#5c6068" : "#ffffff"
                            font.bold: true
                            font.pixelSize: root.width < 260 ? 11 : 14
                            font.strikeout: stem.muted
                        }

                        MouseArea {
                            id: stemMouse

                            anchors.fill: parent
                            onClicked: muteControl.value = stem.muted ? 0 : 1
                            onPressAndHold: root.soloStem(stem.index)
                        }
                    }
                }
            }
        }
    }
    // The pad modes: one segmented column, the mode in use in the color of
    // the deck
    Rectangle {
        id: tabs

        readonly property color deckColor: root.group === "[Channel2]" ? "#e65100" : "#1565c0"

        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.rightMargin: root.width < 260 ? 3 : 6
        anchors.top: parent.top
        border.color: "#2c3544"
        border.width: 1
        color: "#14171d"
        radius: 5
        width: root.width < 260 ? 38 : 46

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 1
            spacing: 0

            Repeater {
                model: stemCountControl.value > 0 ? ["CUE", "LOOP", "STEM"] : ["CUE", "LOOP"]

                Rectangle {
                    id: modeTab

                    required property int index
                    required property string modelData
                    readonly property bool active: root.activePadMode === index

                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: modeTabMouse.pressed ? "#2e3440" : (active ? Qt.darker(tabs.deckColor, 1.4) : "transparent")
                    radius: 4

                    // The line between two modes
                    Rectangle {
                        anchors.left: parent.left
                        anchors.leftMargin: 6
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        anchors.top: parent.top
                        color: "#2c3544"
                        height: 1
                        visible: modeTab.index > 0 && !modeTab.active && root.activePadMode !== modeTab.index - 1
                    }
                    // The mark of the mode in use
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 5
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.topMargin: 5
                        color: "#ffffff"
                        radius: 1.5
                        visible: modeTab.active
                        width: 3
                    }
                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 4
                        anchors.rightMargin: 2
                        color: modeTab.active ? "#ffffff" : "#8293a7"
                        font.bold: true
                        font.pixelSize: root.width < 260 ? 9 : 10
                        fontSizeMode: Text.HorizontalFit
                        horizontalAlignment: Text.AlignHCenter
                        minimumPixelSize: 7
                        text: modeTab.modelData
                        verticalAlignment: Text.AlignVCenter
                    }
                    MouseArea {
                        id: modeTabMouse

                        anchors.fill: parent
                        onClicked: root.activePadMode = modeTab.index
                    }
                }
            }
        }
    }
}
