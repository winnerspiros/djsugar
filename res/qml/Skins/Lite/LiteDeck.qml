import "../.." as Skin
import "../Booth" as Booth
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts

// A deck of the Lite skin: the track, a position bar instead of waveforms
// (redrawn 5 times a second, not at the frame rate), BPM and tempo, hot
// cues A–D (A–H when there is room), a loop, SYNC, CUE, PLAY and the
// tempo fader with nudge.
Item {
    id: root

    required property string group
    required property int deckNumber
    property color deckColor: "#2f9bff"
    // The tempo fader is on the outer side, like on a controller
    property bool faderOnLeft: true
    // The height of a row of buttons
    readonly property real unit: Math.max(34, Math.min(64, height / 7))
    // A second row of hot cues (E–H) where the height allows it, i.e. on
    // tablets
    readonly property int hotcueCount: height >= unit * 9.5 ? 8 : 4

    readonly property var deckPlayer: Mixxx.PlayerManager.getPlayer(group)
    readonly property var currentTrack: deckPlayer?.currentTrack
    readonly property bool loaded: loadedControl.value > 0
    // The play position, read 5 times a second (see positionTimer)
    property real position: 0

    function formatTime(seconds) {
        const total = Math.max(0, Math.floor(seconds));
        const minutes = Math.floor(total / 60);
        const rest = total % 60;
        return minutes + ":" + (rest < 10 ? "0" : "") + rest;
    }

    Mixxx.ControlProxy {
        id: loadedControl

        group: root.group
        key: "track_loaded"
    }
    Mixxx.ControlProxy {
        id: playControl

        group: root.group
        key: "play"
    }
    Mixxx.ControlProxy {
        id: cueControl

        group: root.group
        key: "cue_default"
    }
    Mixxx.ControlProxy {
        id: syncControl

        group: root.group
        key: "sync_enabled"
    }
    Mixxx.ControlProxy {
        id: durationControl

        group: root.group
        key: "duration"
    }
    // Not bound to the UI: its value changes with every audio buffer, which
    // would redraw the window that often
    Mixxx.ControlProxy {
        id: positionControl

        group: root.group
        key: "playposition"
    }
    Mixxx.ControlProxy {
        id: bpmControl

        group: root.group
        key: "bpm"
    }
    Mixxx.ControlProxy {
        id: rateRatioControl

        group: root.group
        key: "rate_ratio"
    }
    Mixxx.ControlProxy {
        id: loopEnabledControl

        group: root.group
        key: "loop_enabled"
    }
    Mixxx.ControlProxy {
        id: loopSizeControl

        group: root.group
        key: "beatloop_size"
    }
    // Nudge (pitch bend) while held
    Mixxx.ControlProxy {
        id: nudgeUpControl

        group: root.group
        key: "rate_temp_up"
    }
    Mixxx.ControlProxy {
        id: nudgeDownControl

        group: root.group
        key: "rate_temp_down"
    }
    // Loop commands, triggered by key
    Mixxx.ControlProxy {
        id: commandControl

        group: root.group
        key: ""
    }
    Timer {
        id: positionTimer

        interval: 200
        repeat: true
        running: root.loaded && Qt.application.state === Qt.ApplicationActive
        triggeredOnStart: true

        onTriggered: root.position = Math.max(0, Math.min(1, positionControl.value))
        onRunningChanged: {
            if (!running && !root.loaded) {
                root.position = 0;
            }
        }
    }

    // The hot cue pressed long, for its color and delete menu
    Skin.Hotcue {
        id: popupHotcue

        group: root.group
        hotcueNumber: 1
    }
    Skin.HotcuePopup {
        id: hotcuePopup

        hotcue: popupHotcue

        Connections {
            function onIsSetChanged() {
                if (!popupHotcue.isSet) {
                    hotcuePopup.close();
                }
            }

            target: popupHotcue
        }
    }

    RowLayout {
        anchors.fill: parent
        layoutDirection: root.faderOnLeft ? Qt.LeftToRight : Qt.RightToLeft
        spacing: 6

        // ---- Tempo: nudge and fader ----
        ColumnLayout {
            Layout.fillHeight: true
            Layout.preferredWidth: Math.round(root.unit * 0.95)
            spacing: 4

            LiteButton {
                Layout.fillWidth: true
                Layout.preferredHeight: root.unit * 0.8
                text: "+"
                onPressing: down => nudgeUpControl.value = down ? 1 : 0
            }
            Booth.BoothFader {
                Layout.fillHeight: true
                Layout.fillWidth: true
                centerMark: true
                color: root.deckColor
                group: root.group
                inverted: true
                key: "rate"
            }
            LiteButton {
                Layout.fillWidth: true
                Layout.preferredHeight: root.unit * 0.8
                text: "-"
                onPressing: down => nudgeDownControl.value = down ? 1 : 0
            }
        }

        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 5

            // ---- Track ----
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Rectangle {
                    Layout.preferredHeight: root.unit * 0.7
                    Layout.preferredWidth: root.unit * 0.7
                    color: root.deckColor
                    radius: 3

                    Text {
                        anchors.centerIn: parent
                        color: "#000000"
                        font.bold: true
                        font.pixelSize: Math.round(parent.height * 0.6)
                        text: root.deckNumber
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    Text {
                        Layout.fillWidth: true
                        color: "#ffffff"
                        elide: Text.ElideRight
                        font.bold: true
                        font.pixelSize: Math.max(13, Math.round(root.unit * 0.32))
                        text: root.currentTrack?.title || (root.loaded ? "" : qsTr("No track"))
                    }
                    Text {
                        Layout.fillWidth: true
                        color: "#9a9ea6"
                        elide: Text.ElideRight
                        font.pixelSize: Math.max(11, Math.round(root.unit * 0.24))
                        text: root.currentTrack?.artist || ""
                    }
                }
            }

            // ---- Position, time left, BPM and tempo ----
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 0.6)
                border.color: "#3a3d44"
                border.width: 1
                color: "#111214"
                radius: 2

                Rectangle {
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 1
                    anchors.left: parent.left
                    anchors.leftMargin: 1
                    anchors.top: parent.top
                    anchors.topMargin: 1
                    color: Qt.darker(root.deckColor, 2.2)
                    width: (parent.width - 2) * root.position
                }
                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: Math.max(12, Math.round(parent.height * 0.55))
                    text: {
                        if (!root.loaded || bpmControl.value <= 0) {
                            return "--- BPM";
                        }
                        const percent = (rateRatioControl.value - 1) * 100;
                        return bpmControl.value.toFixed(1) + " BPM  " + (percent >= 0.05 ? "+" : "") + percent.toFixed(1) + "%";
                    }
                }
                Text {
                    anchors.right: parent.right
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: Math.max(12, Math.round(parent.height * 0.55))
                    text: root.loaded ? "-" + root.formatTime(durationControl.value * (1 - root.position)) : "-:--"
                }
                // A tap moves the play position there
                TapHandler {
                    enabled: root.loaded

                    onTapped: eventPoint => {
                        const position = Math.max(0, Math.min(1, eventPoint.position.x / parent.width));
                        positionControl.value = position;
                        root.position = position;
                    }
                }
            }

            // ---- Hot cues: a tap sets or plays, a long press opens the
            // color and delete menu. They take the spare height. ----
            GridLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.minimumHeight: root.unit * (root.hotcueCount / 4)
                columnSpacing: 4
                columns: 4
                rowSpacing: 4

                Repeater {
                    model: root.hotcueCount

                    LiteButton {
                        id: pad

                        required property int index
                        readonly property int number: index + 1

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        lit: statusControl.value > 0
                        litColor: colorControl.value >= 0 ? "#" + colorControl.value.toString(16).padStart(6, "0") : root.deckColor
                        text: String.fromCharCode(65 + index)

                        onLongPressed: {
                            if (statusControl.value > 0) {
                                popupHotcue.hotcueNumber = pad.number;
                                const position = pad.mapToItem(root, 0, pad.height);
                                hotcuePopup.x = Math.min(position.x, root.width - hotcuePopup.width);
                                hotcuePopup.y = Math.min(position.y, root.height - hotcuePopup.height);
                                hotcuePopup.open();
                            }
                        }
                        onPressing: down => activateControl.value = down ? 1 : 0

                        Mixxx.ControlProxy {
                            id: statusControl

                            group: root.group
                            key: `hotcue_${pad.number}_status`
                        }
                        Mixxx.ControlProxy {
                            id: colorControl

                            group: root.group
                            key: `hotcue_${pad.number}_color`
                        }
                        Mixxx.ControlProxy {
                            id: activateControl

                            group: root.group
                            key: `hotcue_${pad.number}_activate`
                        }
                    }
                }
            }

            // ---- Loop: on with the size, or off; half and double ----
            RowLayout {
                // Its buttons fill its height, which would make it take the
                // spare height too; the hot cues take it
                Layout.fillHeight: false
                Layout.fillWidth: true
                Layout.preferredHeight: root.unit * 1.25
                spacing: 4

                LiteButton {
                    Layout.fillHeight: true
                    Layout.preferredWidth: root.unit * 0.9
                    text: "½"
                    onClicked: {
                        commandControl.key = "loop_halve";
                        commandControl.trigger();
                    }
                }
                LiteButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    lit: loopEnabledControl.value > 0
                    litColor: "#3ad33a"
                    text: {
                        const size = loopSizeControl.value;
                        const label = size >= 1 ? size.toString() : "1/" + Math.round(1 / size);
                        return qsTr("LOOP %1").arg(label);
                    }
                    onClicked: {
                        commandControl.key = loopEnabledControl.value > 0 ? "reloop_toggle" : "beatloop_activate";
                        commandControl.trigger();
                    }
                }
                LiteButton {
                    Layout.fillHeight: true
                    Layout.preferredWidth: root.unit * 0.9
                    text: "×2"
                    onClicked: {
                        commandControl.key = "loop_double";
                        commandControl.trigger();
                    }
                }
            }

            // ---- SYNC, CUE, PLAY ----
            RowLayout {
                Layout.fillHeight: false
                Layout.fillWidth: true
                Layout.preferredHeight: root.unit * 1.5
                spacing: 4

                LiteButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    lit: syncControl.value > 0
                    litColor: "#2f9bff"
                    text: "SYNC"
                    onClicked: syncControl.value = syncControl.value > 0 ? 0 : 1
                }
                LiteButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    litColor: "#ff9a1f"
                    text: "CUE"
                    onPressing: down => cueControl.value = down ? 1 : 0
                }
                LiteButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    fontPixelSize: 18
                    lit: playControl.value > 0
                    litColor: "#3ad33a"
                    // Lit while playing; Android's fonts have no ❚❚
                    text: "PLAY"
                    onClicked: playControl.value = playControl.value > 0 ? 0 : 1
                }
            }
        }
    }
}
