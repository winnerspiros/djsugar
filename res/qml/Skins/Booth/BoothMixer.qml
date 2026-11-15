import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts
import "../../Theme"

// Booth skin mixer: per-deck TRIM, EQ, FILTER, CUE, level meter, fader.
Rectangle {
    id: root

    readonly property real unit: Math.max(9, Math.min(height / 14, width / 11))

    border.color: Theme.midGray3
    border.width: 1
    color: Theme.darkGray4

component LevelMeter: Item {
        id: meter

        required property string group

        Mixxx.ControlProxy {
            id: vuControl

            group: meter.group
            key: "vu_meter"
            updateInterval: 33
        }
        Mixxx.ControlProxy {
            id: peakControl

            group: meter.group
            key: "peak_indicator"
            updateInterval: 33
        }
        Rectangle {
            anchors.fill: parent
            color: Theme.darkGray4
        }
        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: peakControl.value > 0 ? Theme.warningColor : (vuControl.value > 0.8 ? Theme.yellow : Theme.green)
            height: Math.round(parent.height * Math.max(0, Math.min(1, vuControl.value)))
        }
    }
component Channel: RowLayout {
        id: channel

        required property string group
        required property color deckColor
        readonly property string eqGroup: `[EqualizerRack1_${group}_Effect1]`
        readonly property string quickEffectGroup: `[QuickEffectRack1_${group}]`

        spacing: 3

        Mixxx.ControlProxy {
            id: pflControl

            group: channel.group
            key: "pfl"
        }
        Mixxx.ControlProxy {
            id: quickEffectEnabledControl

            group: channel.quickEffectGroup
            key: "enabled"
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
// Quick effect (filter by default). Dimmed when off.
            BoothKnob {
                Layout.fillHeight: true
                Layout.fillWidth: true
                color: Theme.green
                group: channel.quickEffectGroup
                key: "super1"
                label: qsTr("FILTER")
                opacity: quickEffectEnabledControl.value > 0 ? 1 : 0.4
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
                border.color: pflControl.value > 0 ? Theme.warningColor : Theme.midGray2
                color: pflControl.value > 0 ? "#3a1a08" : Theme.darkGray4
                radius: 2

                Text {
                    anchors.centerIn: parent
                    color: pflControl.value > 0 ? Theme.warningColor : Theme.lightGray2
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
            deckColor: Theme.libraryDeck1Background
            group: "[Channel1]"
        }
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 1
            color: Theme.midGray3
        }
        Channel {
            Layout.fillHeight: true
            Layout.fillWidth: true
            deckColor: Theme.libraryDeck2Background
            group: "[Channel2]"
        }
    }
}
