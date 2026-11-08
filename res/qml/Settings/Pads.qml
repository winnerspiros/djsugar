import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../Theme"

// Pick what each performance pad plays, the way rekordbox does it: the pads
// are drawn like the controller (2 rows of 4), tap one and choose a sample
// or an effect from the list.
//   SAMPLER: bank A = samplers 1-8, bank B = samplers 9-16
//   PAD FX:  PAD FX 1 = pads 1-8, PAD FX 2 = pads 9-16
Category {
    id: root

    label: qsTr("Pads & FX")
    selectedIndex: 0

    property var samples: []
    property var effects: []
    property var beatEffects: []
    // Bumped after each change so the pad labels re-read their value
    property int revision: 0

    function refresh() {
        root.samples = Mixxx.PlayerManager.bundledSamples();
        root.effects = Mixxx.EffectsManager.padFxCatalog();
        root.beatEffects = Mixxx.EffectsManager.beatFxCatalog();
        root.revision++;
    }

    function nameIn(list, key) {
        for (let i = 0; i < list.length; i++) {
            if (list[i].key === key) {
                return list[i].name;
            }
        }
        return key;
    }
    function effectName(key) {
        return root.nameIn(root.effects, key);
    }

    Component.onCompleted: refresh()

    Flickable {
        anchors.fill: parent
        contentHeight: column.implicitHeight + 30
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: column

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 12
            spacing: 12

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: "#cfd8dc"
                font.pixelSize: 12
                text: qsTr("Tap a pad to choose what it plays. Your choices are used by the controller pads in SAMPLER and PAD FX mode, and by BEAT FX SELECT.")
            }

            // ================= SAMPLER =================
            Text {
                text: qsTr("🥁 SAMPLER")
                font.bold: true
                font.pixelSize: 14
                color: "#00e5ff"
            }
            PadGrid {
                title: qsTr("Bank A (pads 1-8)")
                first: 1
                sampler: true
            }
            PadGrid {
                title: qsTr("Bank B (SHIFT, pads 9-16)")
                first: 9
                sampler: true
            }

            // ================= PAD FX =================
            Text {
                text: qsTr("✨ PAD FX")
                font.bold: true
                font.pixelSize: 14
                color: "#ff4081"
            }
            PadGrid {
                title: qsTr("PAD FX 1")
                first: 1
                sampler: false
            }
            PadGrid {
                title: qsTr("PAD FX 2")
                first: 9
                sampler: false
            }
            Button {
                text: qsTr("Reset PAD FX to defaults")
                onClicked: {
                    Mixxx.EffectsManager.resetPadFx();
                    root.revision++;
                }
            }

            // ================= BEAT FX =================
            Text {
                text: qsTr("🎛 BEAT FX")
                font.bold: true
                font.pixelSize: 14
                color: "#ffab00"
            }
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: "#cfd8dc"
                font.pixelSize: 12
                text: qsTr("BEAT FX SELECT steps through these 10 effects in order; BEAT ◀ ▶ sets the length, LEVEL/DEPTH the amount.")
            }
            GridLayout {
                Layout.fillWidth: true
                columns: 5
                rowSpacing: 6
                columnSpacing: 6

                Repeater {
                    model: 10
                    delegate: PadButton {
                        required property int index

                        pad: index + 1
                        mode: 2
                    }
                }
            }
            Button {
                text: qsTr("Reset BEAT FX to defaults")
                onClicked: {
                    Mixxx.EffectsManager.resetBeatFx();
                    root.revision++;
                }
            }
        }
    }

    // 2 x 4 pads like on the controller
    component PadGrid: ColumnLayout {
        id: grid

        property string title
        property int first: 1
        property bool sampler: true

        Layout.fillWidth: true
        spacing: 6

        Text {
            text: grid.title
            color: Theme.white
            font.pixelSize: 12
        }
        GridLayout {
            Layout.fillWidth: true
            columns: 4
            rowSpacing: 6
            columnSpacing: 6

            Repeater {
                model: 8
                delegate: PadButton {
                    required property int index

                    pad: grid.first + index
                    mode: grid.sampler ? 0 : 1
                }
            }
        }
    }

    component PadButton: Rectangle {
        id: padButton

        property int pad: 1
        // 0 = sampler, 1 = PAD FX, 2 = BEAT FX
        property int mode: 0
        readonly property bool sampler: mode === 0
        readonly property string samplerGroup: "[Sampler" + pad + "]"
        readonly property var player: sampler ? Mixxx.PlayerManager.getPlayer(samplerGroup) : null
        readonly property string text: {
            root.revision;
            if (sampler) {
                const track = player ? player.currentTrack : null;
                return track && track.title !== "" ? track.title : qsTr("(empty)");
            }
            if (mode === 2) {
                return root.nameIn(root.beatEffects, Mixxx.EffectsManager.beatFxSlot(pad));
            }
            return root.effectName(Mixxx.EffectsManager.padFxEffect(pad));
        }

        Layout.fillWidth: true
        Layout.preferredHeight: 56
        radius: 6
        color: "#000000"
        border.width: 2
        border.color: mode === 0 ? "#00e5ff" : mode === 1 ? "#ff4081" : "#ffab00"

        Column {
            anchors.centerIn: parent
            width: parent.width - 8
            spacing: 2

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: padButton.pad
                color: "#78909c"
                font.pixelSize: 10
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                text: padButton.text
                color: Theme.white
                font.pixelSize: 12
                font.bold: true
            }
        }

        MouseArea {
            anchors.fill: parent
            onClicked: picker.openFor(padButton.pad, padButton.mode)
        }
    }

    Mixxx.ControlProxy {
        id: previewPlay

        group: picker.sampler ? "[Sampler" + picker.pad + "]" : "[Sampler1]"
        key: "cue_gotoandplay"
    }

    Popup {
        id: picker

        property int pad: 1
        property int mode: 0
        readonly property bool sampler: mode === 0

        function openFor(newPad, newMode) {
            picker.pad = newPad;
            picker.mode = newMode;
            picker.open();
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(parent ? parent.width - 40 : 400, 420)
        height: Math.min(parent ? parent.height - 40 : 500, 520)
        modal: true
        padding: 8

        background: Rectangle {
            color: "#000000"
            radius: 8
            border.color: picker.mode === 0 ? "#00e5ff" : picker.mode === 1 ? "#ff4081" : "#ffab00"
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 6

            Text {
                text: (picker.mode === 0 ? qsTr("Sample for pad %1") : picker.mode === 1 ? qsTr("Effect for pad %1") : qsTr("BEAT FX %1")).arg(picker.pad)
                color: Theme.white
                font.bold: true
                font.pixelSize: 14
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: picker.mode === 0 ? root.samples : picker.mode === 1 ? root.effects : root.beatEffects
                delegate: Rectangle {
                    id: row

                    required property var modelData

                    width: ListView.view.width
                    height: 44
                    color: rowArea.pressed ? "#1a1a1a" : "#000000"

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 10
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        elide: Text.ElideRight
                        text: row.modelData.name
                        color: Theme.white
                        font.pixelSize: 14
                    }
                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: "#1f1f1f"
                    }
                    MouseArea {
                        id: rowArea

                        anchors.fill: parent
                        onClicked: {
                            if (picker.sampler) {
                                Mixxx.PlayerManager.loadLocationToPlayer(row.modelData.path, "[Sampler" + picker.pad + "]", false);
                            } else if (picker.mode === 2) {
                                Mixxx.EffectsManager.setBeatFxSlot(picker.pad, row.modelData.key);
                                picker.close();
                            } else {
                                Mixxx.EffectsManager.setPadFxEffect(picker.pad, row.modelData.key);
                                picker.close();
                            }
                            root.revision++;
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Button {
                    visible: picker.sampler
                    text: qsTr("▶ Preview")
                    onClicked: previewPlay.value = 1
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    text: qsTr("Done")
                    onClicked: picker.close()
                }
            }
        }
    }
}
