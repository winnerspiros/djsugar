import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../Theme"
import ".." as Skin

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
            PillButton {
                Layout.alignment: Qt.AlignRight
                accent: "#00e5ff"
                label: qsTr("Reset SAMPLER")
                onClicked: {
                    Mixxx.PlayerManager.resetSamplers();
                    picker.pickedPaths = {};
                    root.revision++;
                }
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
            PillButton {
                Layout.alignment: Qt.AlignRight
                accent: "#ff4081"
                label: qsTr("Reset PAD FX")
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
            PillButton {
                Layout.alignment: Qt.AlignRight
                accent: "#ffab00"
                label: qsTr("Reset BEAT FX")
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

    // Rounded button in the picker's accent colour: outlined, or filled for
    // the main action; the play icon is drawn, not an emoji glyph
    component PillButton: Rectangle {
        id: pill

        property string label
        property bool filled: false
        property bool playIcon: false
        property color accent: picker.accent

        signal clicked

        implicitWidth: pillRow.implicitWidth + 32
        implicitHeight: 40
        radius: height / 2
        color: filled ? pill.accent : pillArea.pressed ? Qt.rgba(pill.accent.r, pill.accent.g, pill.accent.b, 0.2) : "transparent"
        border.color: pill.accent
        border.width: filled ? 0 : 1.5
        opacity: pillArea.pressed && filled ? 0.8 : 1

        Row {
            id: pillRow

            anchors.centerIn: parent
            spacing: 8

            Canvas {
                id: playCanvas

                visible: pill.playIcon
                width: 12
                height: 14
                anchors.verticalCenter: parent.verticalCenter
                onPaint: {
                    const ctx = getContext("2d");
                    ctx.reset();
                    ctx.fillStyle = pill.filled ? "#000000" : pill.accent;
                    ctx.beginPath();
                    ctx.moveTo(1, 1);
                    ctx.lineTo(width - 1, height / 2);
                    ctx.lineTo(1, height - 1);
                    ctx.closePath();
                    ctx.fill();
                }
                Connections {
                    target: picker
                    function onAccentChanged() {
                        playCanvas.requestPaint();
                    }
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: pill.label
                color: pill.filled ? "#000000" : pill.accent
                font.pixelSize: 14
                font.bold: true
            }
        }
        MouseArea {
            id: pillArea

            anchors.fill: parent
            onClicked: pill.clicked()
        }
    }

    Popup {
        id: picker

        property int pad: 1
        property int mode: 0
        readonly property bool sampler: mode === 0
        readonly property color accent: mode === 0 ? "#00e5ff" : mode === 1 ? "#ff4081" : "#ffab00"
        // Sample paths picked in this session (the sampler's track only
        // knows its title, which may differ from the sample name)
        property var pickedPaths: ({})

        // The row that is assigned to the pad right now
        function isCurrent(item) {
            root.revision;
            if (mode === 2) {
                return item.key === Mixxx.EffectsManager.beatFxSlot(pad);
            }
            if (mode === 1) {
                return item.key === Mixxx.EffectsManager.padFxEffect(pad);
            }
            return false;
        }
        // The sample on a sampler pad, as a path
        function samplePath(samplerPad) {
            if (pickedPaths[samplerPad] !== undefined) {
                return pickedPaths[samplerPad];
            }
            const player = Mixxx.PlayerManager.getPlayer("[Sampler" + samplerPad + "]");
            const track = player ? player.currentTrack : null;
            if (!track || track.title === "") {
                return "";
            }
            for (let i = 0; i < root.samples.length; i++) {
                if (root.samples[i].name === track.title) {
                    return root.samples[i].path;
                }
            }
            return "";
        }

        // Put a sample on the pad and remember it as the pad's choice
        function assign(path) {
            Mixxx.PlayerManager.loadLocationToPlayer(path, "[Sampler" + picker.pad + "]", false);
            const paths = picker.pickedPaths;
            paths[picker.pad] = path;
            picker.pickedPaths = paths;
            root.revision++;
        }

        function openFor(newPad, newMode) {
            picker.pad = newPad;
            picker.mode = newMode;
            if (newMode === 0) {
                samplePicker.title = qsTr("Sample for pad %1").arg(newPad);
                samplePicker.previewGroup = "[Sampler" + newPad + "]";
                samplePicker.current = picker.samplePath(newPad);
                samplePicker.open();
                return;
            }
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
            border.color: picker.accent
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 6

            Text {
                text: (picker.mode === 1 ? qsTr("Effect for pad %1") : qsTr("BEAT FX %1")).arg(picker.pad)
                color: Theme.white
                font.bold: true
                font.pixelSize: 14
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: picker.mode === 1 ? root.effects : root.beatEffects
                delegate: Rectangle {
                    id: row

                    required property var modelData

                    width: ListView.view.width
                    height: 44
                    readonly property bool current: picker.isCurrent(modelData)

                    color: rowArea.pressed ? "#1a1a1a" : current ? Qt.rgba(picker.accent.r, picker.accent.g, picker.accent.b, 0.16) : "#000000"
                    radius: 6

                    // Accent bar on the assigned row
                    Rectangle {
                        visible: row.current
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        width: 4
                        height: parent.height - 12
                        radius: 2
                        color: picker.accent
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.right: checkMark.left
                        anchors.rightMargin: 8
                        elide: Text.ElideRight
                        text: row.modelData.name
                        color: row.current ? picker.accent : Theme.white
                        font.pixelSize: 14
                        font.bold: row.current
                    }
                    Text {
                        id: checkMark

                        anchors.verticalCenter: parent.verticalCenter
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        text: "✓"
                        visible: row.current
                        color: picker.accent
                        font.pixelSize: 16
                        font.bold: true
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
                            if (picker.mode === 2) {
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
                Layout.topMargin: 4
                spacing: 8

                Item {
                    Layout.fillWidth: true
                }
                PillButton {
                    label: qsTr("Done")
                    filled: true
                    onClicked: picker.close()
                }
            }
        }
    }

    Skin.SamplePicker {
        id: samplePicker

        accent: "#00e5ff"
        onPicked: path => picker.assign(path)
    }
}
