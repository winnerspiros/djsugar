import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts

// The mixer of the DJ view of the booth skin, like the one of the
// PERFORMANCE layout of rekordbox: for each of the two decks TRIM, HIGH,
// MID and LOW, the headphones CUE, the level meter and the channel fader.
Rectangle {
    id: root

    readonly property real unit: Math.max(9, Math.min(height / 14, width / 11))

    border.color: "#2a2c30"
    border.width: 1
    color: "#0c0d0f"

    // A meter of the level of a channel
    component LevelMeter: Item {
        id: meter

        required property string group

        Mixxx.ControlProxy {
            id: vuControl

            group: meter.group
            key: "vu_meter"
        }
        Mixxx.ControlProxy {
            id: peakControl

            group: meter.group
            key: "peak_indicator"
        }
        Rectangle {
            anchors.fill: parent
            color: "#141518"
        }
        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: peakControl.value > 0 ? "#e5332a" : (vuControl.value > 0.8 ? "#f0a020" : "#2fbf4f")
            height: Math.round(parent.height * Math.max(0, Math.min(1, vuControl.value)))
        }
    }
    // The strip of a deck: the knobs beside the headphones CUE, the meter
    // and the fader
    component Channel: RowLayout {
        id: channel

        required property string group
        required property color deckColor
        readonly property string eqGroup: `[EqualizerRack1_${group}_Effect1]`

        spacing: 3

        Mixxx.ControlProxy {
            id: pflControl

            group: channel.group
            key: "pfl"
        }
        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 2

            Repeater {
                model: [
                    {
                        "label": qsTr("TRIM"),
                        "group": channel.group,
                        "key": "pregain"
                    },
                    {
                        "label": qsTr("HIGH"),
                        "group": channel.eqGroup,
                        "key": "parameter3"
                    },
                    {
                        "label": qsTr("MID"),
                        "group": channel.eqGroup,
                        "key": "parameter2"
                    },
                    {
                        "label": qsTr("LOW"),
                        "group": channel.eqGroup,
                        "key": "parameter1"
                    }
                ]

                BoothKnob {
                    required property var modelData

                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    group: modelData.group
                    key: modelData.key
                    label: modelData.label
                }
            }
        }
        ColumnLayout {
            Layout.fillHeight: true
            Layout.preferredWidth: Math.round(root.unit * 2.8)
            spacing: 3

            // Headphones
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 1.8)
                border.color: pflControl.value > 0 ? "#ff6a13" : "#3a3c40"
                color: pflControl.value > 0 ? "#3a1a08" : "#141518"
                radius: 2

                Text {
                    anchors.centerIn: parent
                    color: pflControl.value > 0 ? "#ff6a13" : "#d8dadd"
                    font.bold: true
                    font.pixelSize: Math.round(root.unit * 0.95)
                    text: "CUE"
                }
                TapHandler {
                    onTapped: pflControl.value = pflControl.value > 0 ? 0 : 1
                }
            }
            RowLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                spacing: 2

                LevelMeter {
                    Layout.fillHeight: true
                    Layout.preferredWidth: Math.max(3, Math.round(root.unit * 0.45))
                    group: channel.group
                }
                BoothFader {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    color: channel.deckColor
                    group: channel.group
                    key: "volume"
                }
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 4
        spacing: 6

        Channel {
            Layout.fillHeight: true
            Layout.fillWidth: true
            deckColor: "#1f6fd1"
            group: "[Channel1]"
        }
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 1
            color: "#2a2c30"
        }
        Channel {
            Layout.fillHeight: true
            Layout.fillWidth: true
            deckColor: "#e8650f"
            group: "[Channel2]"
        }
    }
}
