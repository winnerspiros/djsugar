import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "Theme"

// The sample picker of the pads: the bundled, imported and recorded
// sounds, with preview, import (through the trim popup), record and delete.
// Used by Settings > Pads & FX and by the FX Station X-Pad.
Item {
    id: root

    property string title: ""
    property color accent: "#00e5ff"
    // Path of the sample in use, "" for the default
    property string current: ""
    // When set, a first row that goes back to the default (picked(""))
    property string defaultLabel: ""
    // A sampler group to preview with; otherwise the preview deck plays
    // the current sample on the main mix
    property string previewGroup: ""
    property var samples: []

    signal picked(string path)

    function refresh() {
        root.samples = Mixxx.PlayerManager.bundledSamples();
    }
    function open() {
        root.refresh();
        picker.open();
    }
    function close() {
        picker.close();
    }
    function choose(path) {
        root.current = path;
        root.picked(path);
    }
    // A new import or recording: list it and choose it
    function adopt(path) {
        if (path === "") {
            return;
        }
        root.refresh();
        root.choose(path);
    }
    function preview() {
        if (root.previewGroup !== "") {
            previewPlay.value = 1;
            return;
        }
        if (root.current === "") {
            return;
        }
        if (!previewDeck.active) {
            trimMainMix.saved = trimMainMix.value;
            previewDeck.active = true;
        }
        trimMainMix.value = 1;
        Mixxx.PlayerManager.loadLocationToPlayer(root.current, "[PreviewDeck1]", true);
    }

    component PillButton: Rectangle {
        id: pill

        property string label
        property bool filled: false
        property bool playIcon: false
        property color accent: root.accent

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
                    target: root
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

        group: root.previewGroup !== "" ? root.previewGroup : "[Sampler1]"
        key: "cue_gotoandplay"
    }
    // Set while the preview deck plays a picked sample on the main mix
    QtObject {
        id: previewDeck

        property bool active: false
    }

    Popup {
        id: picker

        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(parent ? parent.width - 40 : 400, 420)
        height: Math.min(parent ? parent.height - 40 : 500, 520)
        modal: true
        padding: 8

        background: Rectangle {
            color: "#000000"
            radius: 8
            border.color: root.accent
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 6

            Text {
                text: root.title
                color: Theme.white
                font.bold: true
                font.pixelSize: 14
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: root.defaultLabel !== "" ? [{
                        "name": root.defaultLabel,
                        "path": "",
                        "user": false
                    }].concat(root.samples) : root.samples
                delegate: Rectangle {
                    id: row

                    required property var modelData

                    width: ListView.view.width
                    height: 44
                    readonly property bool current: root.current === modelData.path

                    color: rowArea.pressed ? "#1a1a1a" : current ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16) : "#000000"
                    radius: 6

                    // Accent bar on the chosen row
                    Rectangle {
                        visible: row.current
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        width: 4
                        height: parent.height - 12
                        radius: 2
                        color: root.accent
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.right: deleteMark.visible ? deleteMark.left : checkMark.left
                        anchors.rightMargin: 8
                        elide: Text.ElideRight
                        text: (row.modelData.user ? "★ " : "") + row.modelData.name
                        color: row.current ? root.accent : Theme.white
                        font.pixelSize: 14
                        font.bold: row.current
                        font.italic: row.modelData.path === ""
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
                        color: root.accent
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
                        onClicked: root.choose(row.modelData.path)
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 8

                PillButton {
                    label: qsTr("Preview")
                    playIcon: true
                    onClicked: root.preview()
                }
                PillButton {
                    label: qsTr("Import")
                    onClicked: importDialog.open()
                }
                PillButton {
                    filled: Mixxx.PlayerManager.sampleRecording
                    label: Mixxx.PlayerManager.sampleRecording ? qsTr("■ Stop %1 s").arg(Mixxx.PlayerManager.sampleRecordSeconds.toFixed(1)) : qsTr("● Rec")
                    onClicked: {
                        if (Mixxx.PlayerManager.sampleRecording) {
                            root.adopt(Mixxx.PlayerManager.stopSampleRecording());
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
                root.adopt(Mixxx.PlayerManager.stopSampleRecording());
            }
            if (previewDeck.active) {
                previewDeck.active = false;
                trimPlay.value = 0;
                trimMainMix.value = trimMainMix.saved;
            }
        }

        Connections {
            target: Mixxx.PlayerManager
            enabled: picker.opened || trim.opened
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
    // The preview deck only plays to headphones; phones often have just
    // the main output, so the trim preview goes to the main mix
    Mixxx.ControlProxy {
        id: trimMainMix

        property real saved: 0

        group: "[PreviewDeck1]"
        key: "main_mix"
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
        readonly property color accent: root.accent
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
            trimMainMix.saved = trimMainMix.value;
            trimMainMix.value = 1;
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
            if (path !== "") {
                trimMainMix.value = trimMainMix.saved;
            }
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
                        root.adopt(saved);
                    }
                }
            }
        }
    }
}
