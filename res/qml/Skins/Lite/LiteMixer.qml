import "../Booth" as Booth
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts

// The mixer of the Lite skin: for each deck HIGH, MID, LOW and FILTER as
// bars, the headphones CUE and the channel fader; the crossfader below.
// No level meters: they change with every audio buffer.
Rectangle {
    id: root

    readonly property real unit: Math.max(30, Math.min(52, height / 8))

    border.color: "#2a2c30"
    border.width: 1
    color: "#0e0f11"

    component Channel: ColumnLayout {
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
        Mixxx.ControlProxy {
            id: quickEffectEnabledControl

            group: `[QuickEffectRack1_${channel.group}]`
            key: "enabled"
        }
        LiteKnob {
            Layout.fillWidth: true
            Layout.preferredHeight: root.unit
            color: channel.deckColor
            group: channel.eqGroup
            key: "parameter3"
            label: qsTr("HIGH")
        }
        LiteKnob {
            Layout.fillWidth: true
            Layout.preferredHeight: root.unit
            color: channel.deckColor
            group: channel.eqGroup
            key: "parameter2"
            label: qsTr("MID")
        }
        LiteKnob {
            Layout.fillWidth: true
            Layout.preferredHeight: root.unit
            color: channel.deckColor
            group: channel.eqGroup
            key: "parameter1"
            label: qsTr("LOW")
        }
        LiteKnob {
            Layout.fillWidth: true
            Layout.preferredHeight: root.unit
            color: "#2fbf9f"
            group: `[QuickEffectRack1_${channel.group}]`
            key: "super1"
            label: qsTr("FILTER")
            opacity: quickEffectEnabledControl.value > 0 ? 1 : 0.4
        }
        LiteButton {
            Layout.fillWidth: true
            Layout.preferredHeight: root.unit * 0.9
            fontPixelSize: 12
            lit: pflControl.value > 0
            litColor: "#ff9a1f"
            text: "CUE"
            onClicked: pflControl.value = pflControl.value > 0 ? 0 : 1
        }
        Booth.BoothFader {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillHeight: true
            Layout.minimumHeight: root.unit * 1.5
            Layout.preferredWidth: Math.round(root.unit * 0.9)
            color: channel.deckColor
            group: channel.group
            key: "volume"
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 5
        spacing: 6

        RowLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 8

            Channel {
                Layout.fillHeight: true
                Layout.fillWidth: true
                deckColor: "#2f9bff"
                group: "[Channel1]"
            }
            Channel {
                Layout.fillHeight: true
                Layout.fillWidth: true
                deckColor: "#ff7a1f"
                group: "[Channel2]"
            }
        }
        Booth.BoothFader {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.round(root.unit * 0.8)
            centerMark: true
            group: "[Master]"
            horizontal: true
            key: "crossfader"
        }
    }
}
