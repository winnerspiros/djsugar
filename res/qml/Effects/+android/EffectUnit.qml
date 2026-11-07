import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick
import "../Theme"

Item {
    id: root

    readonly property bool expanded: expandedControl.value > 0
    // On narrow screens (tablets showing two units side by side) the three
    // slots don't fit in one row: stack them so names and knobs stay usable
    readonly property bool stacked: root.expanded || effectContainer.width < 480
    readonly property string group: unit.group
    readonly property Mixxx.EffectUnitProxy unit: Mixxx.EffectsManager.getEffectUnit(unitNumber)
    required property int unitNumber

    implicitHeight: effectContainer.height
    height: implicitHeight

    Item {
        id: effectContainer

        anchors.left: parent.left
        anchors.margins: 5
        anchors.right: effectUnitControlsFrame.left
        anchors.top: parent.top
        height: root.expanded ? 160 : (root.stacked ? 132 : 116)

        EffectSlot {
            id: effect1

            anchors.left: parent.left
            anchors.top: parent.top
            effectNumber: 1
            expanded: root.expanded
            height: root.expanded ? 50 : 44
            unitNumber: root.unitNumber
            width: root.stacked ? parent.width : parent.width / 3
        }
        EffectSlot {
            id: effect2

            anchors.left: root.stacked ? parent.left : effect1.right
            anchors.top: root.stacked ? effect1.bottom : parent.top
            effectNumber: 2
            expanded: root.expanded
            height: root.expanded ? 50 : 44
            unitNumber: root.unitNumber
            width: root.stacked ? parent.width : parent.width / 3
        }
        EffectSlot {
            id: effect3

            anchors.left: root.stacked ? parent.left : effect2.right
            anchors.top: root.stacked ? effect2.bottom : parent.top
            effectNumber: 3
            expanded: root.expanded
            height: root.expanded ? 50 : 44
            unitNumber: root.unitNumber
            width: root.stacked ? parent.width : parent.width / 3
        }
    }
    Rectangle {
        id: effectUnitControlsFrame

        anchors.bottom: parent.bottom
        anchors.margins: 5
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.knobBackgroundColor
        radius: 5
        width: 150

        Column {
            anchors.fill: parent
            anchors.margins: 5
            spacing: 5

            Row {
                spacing: 5

                Skin.ControlButton {
                    activeColor: Theme.effectUnitColor
                    group: root.group
                    height: 26
                    key: "show_parameters"
                    text: root.expanded ? "▲" : "▼"
                    toggleable: true
                    width: 36
                }
                Skin.Button {
                    activeColor: Theme.effectUnitColor
                    height: 26
                    text: "FX " + root.unitNumber
                    width: 56

                    onClicked: presetPopup.open()
                }
                Skin.ControlButton {
                    activeColor: Theme.effectUnitColor
                    group: root.group
                    height: 26
                    key: "group_[Headphone]_enable"
                    text: "PFL"
                    toggleable: true
                    width: 42
                }
            }
            Row {
                spacing: 5

                Skin.ControlButton {
                    activeColor: Theme.effectUnitColor
                    group: root.group
                    height: 30
                    key: "mix_mode"
                    text: "D/W"
                    toggleable: true
                    width: 36
                }
                Skin.ControlKnob {
                    arcStart: Skin.Knob.ArcStart.Minimum
                    color: Theme.effectUnitColor
                    group: root.group
                    height: 40
                    key: "mix"
                    width: 40
                }
                Skin.ControlKnob {
                    arcStart: Skin.Knob.ArcStart.Minimum
                    color: Theme.effectUnitColor
                    group: root.group
                    height: 40
                    key: "super1"
                    width: 40
                }
            }
            // Which channels go through this effect unit
            Row {
                spacing: 5

                Repeater {
                    model: [
                        {
                            "label": "1",
                            "channel": "[Channel1]"
                        },
                        {
                            "label": "2",
                            "channel": "[Channel2]"
                        },
                        {
                            "label": "M",
                            "channel": "[Master]"
                        }
                    ]

                    Skin.ControlButton {
                        required property var modelData

                        activeColor: Theme.effectUnitColor
                        group: root.group
                        height: 26
                        key: "group_" + modelData.channel + "_enable"
                        text: modelData.label
                        toggleable: true
                        width: 43
                    }
                }
            }
        }
    }
    EffectChainPresetPopup {
        id: presetPopup

        slot1: effect1.slot
        slot2: effect2.slot
        slot3: effect3.slot
        unit: root.unit
    }
    Mixxx.ControlProxy {
        id: expandedControl

        group: root.group
        key: "show_parameters"
    }
}
