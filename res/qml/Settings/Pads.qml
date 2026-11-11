import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
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
            PillButton {
                Layout.alignment: Qt.AlignRight
                accent: "#00e5ff"
                label: qsTr("↺ Reset SAMPLER")
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
                label: qsTr("↺ Reset PAD FX")
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
                label: qsTr("↺ Reset BEAT FX")
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
            if (pickedPaths[pad] !== undefined) {
                return pickedPaths[pad] === item.path;
            }
            const player = Mixxx.PlayerManager.getPlayer("[Sampler" + pad + "]");
            const track = player ? player.currentTrack : null;
            return !!track && track.title !== "" && track.title === item.name;
        }

        // Put a sample on the pad and remember it as the pad's choice
        function assign(path) {
            Mixxx.PlayerManager.loadLocationToPlayer(path, "[Sampler" + picker.pad + "]", false);
            const paths = picker.pickedPaths;
            paths[picker.pad] = path;
            picker.pickedPaths = paths;
            root.revision++;
        }
        // A new import or recording: list it and put it on the pad
        function adopt(path) {
            if (path === "") {
                return;
            }
            root.refresh();
            picker.assign(path);
        }

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
            border.color: picker.accent
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
                        anchors.right: deleteMark.visible ? deleteMark.left : checkMark.left
                        anchors.rightMargin: 8
                        elide: Text.ElideRight
                        text: (row.modelData.user ? "★ " : "") + row.modelData.name
                        color: row.current ? picker.accent : Theme.white
                        font.pixelSize: 14
                        font.bold: row.current
                    }
                    Text {
                        id: deleteMark

                        visible: !!row.modelData.user
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.right: checkMark.left
                        anchors.rightMargin: 12
                        text: "🗑"
                        font.pixelSize: 16
                        z: 1

                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -10
                            onClicked: {
                                Mixxx.PlayerManager.deleteSample(row.modelData.path);
                                root.refresh();
                            }
                        }
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
                            if (picker.sampler) {
                                picker.assign(row.modelData.path);
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
                Layout.topMargin: 4
                spacing: 8

                PillButton {
                    visible: picker.sampler
                    label: qsTr("Preview")
                    playIcon: true
                    onClicked: previewPlay.value = 1
                }
                PillButton {
                    visible: picker.sampler
                    label: qsTr("Import")
                    onClicked: importDialog.open()
                }
                PillButton {
                    visible: picker.sampler
                    filled: Mixxx.PlayerManager.sampleRecording
                    label: Mixxx.PlayerManager.sampleRecording ? qsTr("■ Stop %1 s").arg(Mixxx.PlayerManager.sampleRecordSeconds.toFixed(1)) : qsTr("● Rec")
                    onClicked: {
                        if (Mixxx.PlayerManager.sampleRecording) {
                            picker.adopt(Mixxx.PlayerManager.stopSampleRecording());
                        } else {
                            Mixxx.PlayerManager.startSampleRecording();
                        }
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                PillButton {
                    label: qsTr("Done")
                    filled: true
                    onClicked: picker.close()
                }
            }
            Text {
                id: errorText

                Layout.fillWidth: true
                visible: text !== ""
                wrapMode: Text.WordWrap
                color: "#ff5252"
                font.pixelSize: 12
            }
        }

        // A take still running when the picker closes is saved, not lost
        onClosed: {
            errorText.text = "";
            if (Mixxx.PlayerManager.sampleRecording) {
                picker.adopt(Mixxx.PlayerManager.stopSampleRecording());
            }
        }

        Connections {
            target: Mixxx.PlayerManager
            function onSampleError(reason) {
                errorText.text = reason;
                trim.close();
            }
            function onSampleOpened(sound) {
                trim.load(sound);
            }
        }

        FileDialog {
            id: importDialog

            title: qsTr("Import sample")
            fileMode: FileDialog.OpenFile
            // Android's picker maps filters to MIME types and greys out
            // valid audio; the trim popup checks the format instead
            nameFilters: Qt.platform.os === "android" ? [] : Mixxx.PlayerManager.supportedAudioFileNameFilters.concat([qsTr("All files (*)")])
            onAccepted: {
                trim.loading = true;
                trim.open();
                Mixxx.PlayerManager.openSampleForTrim(importDialog.selectedFile);
            }
        }
    }

    // Trim an imported sound before it becomes a pad sample: drag the two
    // handles (or the selection) over the waveform, preview, then save
    Mixxx.ControlProxy {
        id: trimPlay

        group: "[PreviewDeck1]"
        key: "play"
    }
    Mixxx.ControlProxy {
        id: trimPosition

        group: "[PreviewDeck1]"
        key: "playposition"
        onValueChanged: {
            // Stop at the end of the selection
            if (trimPlay.value > 0 && trim.duration > 0 && value * trim.duration >= trim.end) {
                trimPlay.value = 0;
            }
        }
    }

    Popup {
        id: trim

        property bool loading: false
        property string path: ""
        property string name: ""
        property real duration: 0
        property var peaks: []
        property real start: 0
        property real end: 0
        readonly property color accent: "#00e5ff"
        readonly property real minLength: Math.min(0.05, duration)
        // Pads are for short sounds: a long file opens on its first 10 s
        readonly property real defaultLength: 10

        function load(sound) {
            path = sound.path;
            name = sound.name;
            duration = sound.duration;
            peaks = sound.peaks;
            start = 0;
            end = Math.min(duration, defaultLength);
            nameField.text = name;
            loading = false;
            Mixxx.PlayerManager.loadLocationToPlayer(path, "[PreviewDeck1]", false);
            wave.requestPaint();
        }
        function play() {
            trimPosition.value = start / duration;
            trimPlay.value = 1;
        }
        function format(seconds) {
            return seconds < 60 ? seconds.toFixed(1) + " s" : Math.floor(seconds / 60) + ":" + ("0" + (seconds % 60).toFixed(1)).slice(-4);
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(parent.width - 24, 560)
        modal: true
        closePolicy: Popup.CloseOnEscape
        padding: 14
        onClosed: {
            trimPlay.value = 0;
            loading = false;
            path = "";
        }

        background: Rectangle {
            color: "#0d0d0d"
            radius: 8
            border.color: trim.accent
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 10

            Text {
                text: trim.loading ? qsTr("Opening…") : qsTr("Trim sample")
                color: Theme.white
                font.bold: true
                font.pixelSize: 14
            }

            // The waveform with the selection
            Item {
                id: waveArea

                readonly property real secondsPerPixel: trim.duration / Math.max(1, width)
                readonly property real startX: trim.duration > 0 ? trim.start / trim.duration * width : 0
                readonly property real endX: trim.duration > 0 ? trim.end / trim.duration * width : width

                Layout.fillWidth: true
                Layout.preferredHeight: 110
                opacity: trim.loading ? 0.3 : 1

                Canvas {
                    id: wave

                    anchors.fill: parent
                    onPaint: {
                        const ctx = getContext("2d");
                        ctx.reset();
                        const peaks = trim.peaks;
                        const n = peaks.length;
                        if (n === 0) {
                            return;
                        }
                        const bar = width / n;
                        const mid = height / 2;
                        for (let i = 0; i < n; i++) {
                            const x = i * bar;
                            const inside = x + bar >= waveArea.startX && x <= waveArea.endX;
                            ctx.fillStyle = inside ? trim.accent : "#37474f";
                            const h = Math.max(1, peaks[i] * (mid - 2));
                            ctx.fillRect(x, mid - h, Math.max(1, bar - 0.5), 2 * h);
                        }
                    }
                }
                // Dim outside the selection
                Rectangle {
                    x: 0
                    width: waveArea.startX
                    height: parent.height
                    color: "#a0000000"
                }
                Rectangle {
                    x: waveArea.endX
                    width: parent.width - waveArea.endX
                    height: parent.height
                    color: "#a0000000"
                }
                // Selection frame: drag it to move the whole part
                Rectangle {
                    x: waveArea.startX
                    width: waveArea.endX - waveArea.startX
                    height: parent.height
                    color: "transparent"
                    border.color: trim.accent
                    border.width: 2
                    radius: 4

                    MouseArea {
                        property real grabX: 0

                        anchors.fill: parent
                        anchors.leftMargin: 20
                        anchors.rightMargin: 20
                        onPressed: mouse => grabX = mouse.x
                        onPositionChanged: mouse => {
                            const length = trim.end - trim.start;
                            const shift = (mouse.x - grabX) * waveArea.secondsPerPixel;
                            const start = Math.max(0, Math.min(trim.duration - length, trim.start + shift));
                            trim.start = start;
                            trim.end = start + length;
                            wave.requestPaint();
                        }
                        onReleased: trim.play()
                    }
                }
                // Playhead
                Rectangle {
                    visible: trimPlay.value > 0
                    x: trimPosition.value * waveArea.width
                    width: 2
                    height: parent.height
                    color: Theme.white
                }
                Repeater {
                    model: 2

                    delegate: Rectangle {
                        id: handle

                        required property int index
                        readonly property bool isStart: index === 0

                        x: (isStart ? waveArea.startX : waveArea.endX) - width / 2
                        y: -6
                        width: 18
                        height: waveArea.height + 12
                        radius: 9
                        color: trim.accent

                        Rectangle {
                            anchors.centerIn: parent
                            width: 2
                            height: 24
                            radius: 1
                            color: "#0d0d0d"
                        }
                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -12
                            onPositionChanged: mouse => {
                                const seconds = (handle.x + handle.width / 2 + mouse.x - width / 2) * waveArea.secondsPerPixel;
                                if (handle.isStart) {
                                    trim.start = Math.max(0, Math.min(trim.end - trim.minLength, seconds));
                                } else {
                                    trim.end = Math.min(trim.duration, Math.max(trim.start + trim.minLength, seconds));
                                }
                                wave.requestPaint();
                            }
                            onReleased: trim.play()
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Text {
                    text: trim.format(trim.start)
                    color: "#78909c"
                    font.pixelSize: 12
                }
                Item {
                    Layout.fillWidth: true
                }
                Text {
                    text: trim.format(trim.end - trim.start)
                    color: trim.accent
                    font.bold: true
                    font.pixelSize: 14
                }
                Item {
                    Layout.fillWidth: true
                }
                Text {
                    text: trim.format(trim.end)
                    color: "#78909c"
                    font.pixelSize: 12
                }
            }

            TextField {
                id: nameField

                Layout.fillWidth: true
                placeholderText: qsTr("Name")
                color: Theme.white
                background: Rectangle {
                    color: "#000000"
                    radius: 6
                    border.color: nameField.activeFocus ? trim.accent : "#37474f"
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                PillButton {
                    accent: trim.accent
                    playIcon: trimPlay.value === 0
                    label: trimPlay.value > 0 ? qsTr("■ Stop") : qsTr("Play")
                    onClicked: trimPlay.value > 0 ? trimPlay.value = 0 : trim.play()
                }
                Item {
                    Layout.fillWidth: true
                }
                PillButton {
                    accent: trim.accent
                    label: qsTr("Cancel")
                    onClicked: trim.close()
                }
                PillButton {
                    accent: trim.accent
                    filled: true
                    label: qsTr("Save")
                    onClicked: {
                        if (trim.loading || trim.path === "") {
                            return;
                        }
                        trimPlay.value = 0;
                        const saved = Mixxx.PlayerManager.saveTrimmedSample(trim.start, trim.end, nameField.text);
                        trim.close();
                        picker.adopt(saved);
                    }
                }
            }
        }
    }
}
