import "../.." as Skin
import "../../Deck" as DeckComponent
import "." as Booth
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts

// The controls of a deck in the DJ view of the booth skin, for playing
// without a controller, in the style of the PERFORMANCE layout of
// rekordbox: the pads (hot cues, beat jump or beat loop), the loop size,
// CUE and PLAY, the jog and the tempo. Deck 2 is mirrored, with its pads on
// the outer side.
Rectangle {
    id: root

    required property string group
    required property int deckNumber
    property bool mirrored: false
    // Landscape phones
    property bool compact: false
    // hotcue, beatjump or beatloop
    property string padMode: "hotcue"
    readonly property color deckColor: deckNumber === 1 ? "#1f6fd1" : "#e8650f"
    readonly property bool loaded: trackLoadedControl.value > 0
    readonly property real unit: Math.max(9, Math.min(height / 14, width / 34))

    function pad(number, digits) {
        return ("000" + number).slice(-digits);
    }
    function formatTime(seconds) {
        const total = Math.floor(Math.max(0, seconds));
        return root.pad(Math.floor(total / 60), 2) + ":" + root.pad(total % 60, 2);
    }

    border.color: "#2a2c30"
    border.width: 1
    color: "#0c0d0f"

    Mixxx.ControlProxy {
        id: trackLoadedControl

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
        id: cueIndicatorControl

        group: root.group
        key: "cue_indicator"
    }
    Mixxx.ControlProxy {
        id: syncControl

        group: root.group
        key: "sync_enabled"
    }
    Mixxx.ControlProxy {
        id: syncLeaderControl

        group: root.group
        key: "sync_leader"
    }
    Mixxx.ControlProxy {
        id: quantizeControl

        group: root.group
        key: "quantize"
    }
    Mixxx.ControlProxy {
        id: keylockControl

        group: root.group
        key: "keylock"
    }
    Mixxx.ControlProxy {
        id: slipControl

        group: root.group
        key: "slip_enabled"
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
        id: rateRangeControl

        group: root.group
        key: "rateRange"
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
    Mixxx.ControlProxy {
        id: sampleRateControl

        group: root.group
        key: "track_samplerate"
    }

    // A square button with a label, lit when on
    component SmallButton: Rectangle {
        id: smallButton

        property string label: ""
        property bool on: false
        property color onColor: "#ff6a13"
        signal pressed
        signal released
        signal pressAndHold

        border.color: on ? onColor : "#3a3c40"
        border.width: 1
        color: buttonTap.pressed ? "#2a2c30" : (on ? Qt.alpha(onColor, 0.18) : "#141518")
        radius: 2

        Text {
            anchors.fill: parent
            color: smallButton.on ? smallButton.onColor : "#d8dadd"
            fontSizeMode: Text.Fit
            font.bold: true
            font.pixelSize: Math.round(root.unit * 1.05)
            horizontalAlignment: Text.AlignHCenter
            minimumPixelSize: 6
            text: smallButton.label
            verticalAlignment: Text.AlignVCenter
        }
        TapHandler {
            id: buttonTap

            gesturePolicy: TapHandler.WithinBounds
            onLongPressed: smallButton.pressAndHold()
            onPressedChanged: pressed ? smallButton.pressed() : smallButton.released()
        }
    }

    // A pad: a hot cue, or a beat jump or loop of a size
    component Pad: Rectangle {
        id: padItem

        required property int index
        readonly property int number: index + 1
        // Beat jump: the left column has the short jumps, the right one the
        // long ones
        readonly property int sizeIndex: Math.floor(index / 2) + (index % 2) * 4
        readonly property var jumpSizes: [0.03125, 0.125, 0.25, 0.5, 1, 2, 4, 8]
        readonly property var jumpLabels: [qsTr("FINE"), "1/8", "1/4", "1/2", "1", "2", "4", "8"]
        readonly property var loopSizes: [0.25, 0.5, 1, 2, 4, 8, 16, 32]
        readonly property var loopLabels: ["1/4", "1/2", "1", "2", "4", "8", "16", "32"]
        readonly property bool hotcueSet: hotcueStatus.value > 0
        readonly property color hotcueColor: {
            const rgb = hotcueColorControl.value;
            return rgb >= 0 ? Qt.rgba(((rgb >> 16) & 255) / 255, ((rgb >> 8) & 255) / 255, (rgb & 255) / 255, 1) : "#3ad33a";
        }
        readonly property bool lit: {
            switch (root.padMode) {
            case "hotcue":
                return hotcueSet;
            case "beatloop":
                return loopEnabledControl.value > 0 && loopSizeControl.value === loopSizes[index];
            default:
                return false;
            }
        }
        readonly property color litColor: root.padMode === "hotcue" ? hotcueColor : "#2f9bff"

        border.color: padTap.pressed ? "#ffffff" : (lit ? litColor : "#2a2c30")
        border.width: 1
        color: padTap.pressed ? "#2a2c30" : "#141518"

        Mixxx.ControlProxy {
            id: hotcueStatus

            group: root.group
            key: `hotcue_${padItem.number}_status`
        }
        Mixxx.ControlProxy {
            id: hotcueColorControl

            group: root.group
            key: `hotcue_${padItem.number}_color`
        }
        Mixxx.ControlProxy {
            id: hotcuePosition

            group: root.group
            key: `hotcue_${padItem.number}_position`
        }
        Mixxx.ControlProxy {
            id: hotcueActivate

            group: root.group
            key: `hotcue_${padItem.number}_activate`
        }
        Mixxx.ControlProxy {
            id: hotcueClear

            group: root.group
            key: `hotcue_${padItem.number}_clear`
        }
        Mixxx.ControlProxy {
            id: beatloopToggle

            group: root.group
            key: `beatloop_${padItem.loopSizes[padItem.index]}_toggle`
        }

        // The letter of the hot cue, like a player
        Rectangle {
            id: letter

            anchors.left: parent.left
            anchors.leftMargin: 3
            anchors.verticalCenter: parent.verticalCenter
            color: padItem.hotcueSet ? padItem.hotcueColor : "transparent"
            height: Math.min(parent.height - 6, root.unit * 1.5)
            visible: root.padMode === "hotcue"
            width: height

            Text {
                anchors.centerIn: parent
                color: padItem.hotcueSet ? "#000000" : "#5c6068"
                font.bold: true
                font.pixelSize: Math.round(letter.height * 0.75)
                text: String.fromCharCode(65 + padItem.index)
            }
        }
        Text {
            anchors.left: root.padMode === "hotcue" ? letter.right : parent.left
            anchors.leftMargin: root.padMode === "hotcue" ? 5 : 0
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            color: padItem.lit ? "#ffffff" : "#9a9ea6"
            elide: Text.ElideRight
            font.pixelSize: Math.round(root.unit * 1.05)
            horizontalAlignment: root.padMode === "hotcue" ? Text.AlignLeft : Text.AlignHCenter
            text: {
                switch (root.padMode) {
                case "hotcue":
                    return padItem.hotcueSet && sampleRateControl.value > 0 ? root.formatTime(hotcuePosition.value / sampleRateControl.value / 2) : "";
                case "beatjump":
                    return padItem.jumpLabels[padItem.sizeIndex];
                default:
                    return padItem.loopLabels[padItem.index];
                }
            }
        }
        // Beat jump: the left half jumps back, the right half forward
        Repeater {
            model: root.padMode === "beatjump" ? [false, true] : []

            Skin.Triangle {
                required property bool modelData

                anchors.left: modelData ? undefined : parent.left
                anchors.leftMargin: Math.round(padItem.width * 0.12)
                anchors.right: modelData ? parent.right : undefined
                anchors.rightMargin: Math.round(padItem.width * 0.12)
                anchors.verticalCenter: parent.verticalCenter
                color: "#e5332a"
                height: Math.round(root.unit * 1.1)
                pointsRight: modelData
                width: Math.round(height * 0.8)
            }
        }
        TapHandler {
            id: padTap

            gesturePolicy: TapHandler.WithinBounds

            onLongPressed: {
                if (root.padMode === "hotcue" && padItem.hotcueSet) {
                    hotcueClear.trigger();
                }
            }
            onPressedChanged: {
                switch (root.padMode) {
                case "hotcue":
                    hotcueActivate.value = pressed ? 1 : 0;
                    break;
                case "beatjump":
                    if (pressed) {
                        const back = padTap.point.position.x < padItem.width / 2;
                        jumpControl.key = `beatjump_${padItem.jumpSizes[padItem.sizeIndex]}_${back ? "backward" : "forward"}`;
                        jumpControl.trigger();
                    }
                    break;
                default:
                    if (pressed) {
                        beatloopToggle.trigger();
                    }
                }
            }
        }
    }
    Mixxx.ControlProxy {
        id: jumpControl

        group: root.group
        key: ""
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 4
        layoutDirection: root.mirrored ? Qt.RightToLeft : Qt.LeftToRight
        spacing: 4

        // ---- Pads and their mode ----
        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: root.unit * 10
            Layout.preferredWidth: root.unit * 16
            spacing: 3

            GridLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                columnSpacing: 3
                columns: 2
                rowSpacing: 3
                visible: root.padMode !== "sampler"

                Repeater {
                    model: 8

                    Pad {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                    }
                }
            }
            // Sampler pads: 8 samplers per deck side ([Sampler1-8] left,
            // [Sampler9-16] right). Styled like Pad but driving sampler
            // play/cue through ControlProxy, no deck logic needed.
            GridLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                columnSpacing: 3
                columns: 2
                rowSpacing: 3
                visible: root.padMode === "sampler"

                Repeater {
                    model: 8

                    Booth.SamplerPad {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        // Left deck plays samplers 1-8, right deck 9-16
                        samplerGroup: "[Sampler" + (index + 1 + (root.mirrored ? 8 : 0)) + "]"
                    }
                }
            }
            // Tap: the next mode
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 1.8)
                border.color: "#2a2c30"
                color: modeTap.pressed ? "#2a2c30" : "#141518"

                Text {
                    anchors.centerIn: parent
                    color: "#d8dadd"
                    font.pixelSize: Math.round(root.unit * 1.0)
                    text: {
                        switch (root.padMode) {
                        case "beatjump":
                            return qsTr("BEAT JUMP") + "  ⇅";
                        case "beatloop":
                            return qsTr("BEAT LOOP") + "  ⇅";
                        case "sampler":
                            return qsTr("SAMPLER") + "  ⇅";
                        default:
                            return qsTr("HOT CUE") + "  ⇅";
                        }
                    }
                }
                TapHandler {
                    id: modeTap

                    onTapped: {
                        const modes = ["hotcue", "beatjump", "beatloop", "sampler"];
                        root.padMode = modes[(modes.indexOf(root.padMode) + 1) % modes.length];
                    }
                }
            }
        }

        // ---- Loop: the size turns a loop of that many beats on and off,
        // ½ and ×2 change it, IN and OUT set a loop by hand ----
        ColumnLayout {
            readonly property bool looping: loopEnabledControl.value > 0

            Layout.fillHeight: true
            // Layouts fill by default: a fixed width, or the pads and the
            // jog leave the loop only a sliver
            Layout.fillWidth: false
            Layout.minimumWidth: Math.round(root.unit * 5.4)
            Layout.preferredWidth: Math.round(root.unit * 5.4)
            spacing: 3

            Mixxx.ControlProxy {
                id: loopActivate

                group: root.group
                key: "beatloop_activate"
            }
            Mixxx.ControlProxy {
                id: reloopToggle

                group: root.group
                key: "reloop_toggle"
            }
            Mixxx.ControlProxy {
                id: loopHalve

                group: root.group
                key: "loop_halve"
            }
            Mixxx.ControlProxy {
                id: loopDouble

                group: root.group
                key: "loop_double"
            }
            Mixxx.ControlProxy {
                id: loopIn

                group: root.group
                key: "loop_in"
            }
            Mixxx.ControlProxy {
                id: loopOut

                group: root.group
                key: "loop_out"
            }

            Text {
                Layout.fillWidth: true
                color: parent.looping ? "#ff6a13" : "#8a8e96"
                font.bold: true
                font.letterSpacing: 1
                font.pixelSize: Math.round(root.unit * 0.95)
                horizontalAlignment: Text.AlignHCenter
                text: "LOOP"
            }
            // The size: tap for a loop of that many beats, tap again to exit
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 3.8)
                border.color: parent.looping ? "#ff6a13" : "#5c6068"
                border.width: parent.looping ? 2 : 1
                color: loopTap.pressed ? "#5a2a08" : (parent.looping ? "#ff6a13" : "#141518")
                radius: 3

                Column {
                    anchors.centerIn: parent
                    spacing: 0

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: loopEnabledControl.value > 0 ? "#000000" : "#ff9a52"
                        font.bold: true
                        font.pixelSize: Math.round(root.unit * 1.8)
                        text: {
                            const size = loopSizeControl.value;
                            return size >= 1 ? Math.round(size) : "1/" + Math.round(1 / size);
                        }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: loopEnabledControl.value > 0 ? "#000000" : "#8a8e96"
                        font.bold: true
                        font.pixelSize: Math.round(root.unit * 0.8)
                        text: loopEnabledControl.value > 0 ? qsTr("EXIT") : qsTr("BEATS")
                    }
                }
                TapHandler {
                    id: loopTap

                    onTapped: loopEnabledControl.value > 0 ? reloopToggle.trigger() : loopActivate.trigger()
                }
            }
            RowLayout {
                Layout.fillHeight: false
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 2.4)
                spacing: 3

                SmallButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    label: "½"
                    onPressed: loopHalve.trigger()
                }
                SmallButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    label: "×2"
                    onPressed: loopDouble.trigger()
                }
            }
            RowLayout {
                Layout.fillHeight: false
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 2.4)
                spacing: 3

                SmallButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    label: "IN"
                    on: loopIn.value > 0
                    onPressed: loopIn.value = 1
                    onReleased: loopIn.value = 0
                }
                SmallButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    label: "OUT"
                    on: loopOut.value > 0
                    onPressed: loopOut.value = 1
                    onReleased: loopOut.value = 0
                }
            }
            Item {
                Layout.fillHeight: true
            }
            // The beatgrid panel, as the BPM of the WAVE view
            SmallButton {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 2.2)
                label: qsTr("GRID")
                onPressed: {
                    if (root.loaded) {
                        beatgridPopup.open();
                    }
                }

                DeckComponent.BeatgridPopup {
                    id: beatgridPopup

                    group: root.group
                }
            }
            SmallButton {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 2.2)
                label: qsTr("SLIP")
                on: slipControl.value > 0
                onPressed: slipControl.value = slipControl.value > 0 ? 0 : 1
            }
        }

        // ---- SYNC, CUE and PLAY ----
        ColumnLayout {
            readonly property real buttonSize: Math.min(Math.round(root.unit * 4.2), (root.height - 12) / 3.4)

            Layout.fillHeight: true
            Layout.preferredWidth: buttonSize
            spacing: 4

            // Tap: sync, hold: leader (MASTER)
            SmallButton {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.round(root.unit * 2.2)
                label: syncLeaderControl.value > 0 ? qsTr("MASTER") : "SYNC"
                on: syncControl.value > 0 || syncLeaderControl.value > 0
                onColor: syncLeaderControl.value > 0 ? "#ff6a13" : "#2f9bff"
                onPressAndHold: syncLeaderControl.value = syncLeaderControl.value > 0 ? 0 : 1
                onPressed: syncControl.value = syncControl.value > 0 ? 0 : 1
            }
            Item {
                Layout.fillHeight: true
            }
            Rectangle {
                Layout.preferredHeight: parent.buttonSize
                Layout.preferredWidth: parent.buttonSize
                border.color: cueIndicatorControl.value > 0 ? "#ff9a1f" : "#8a5a20"
                border.width: 2
                color: cueTap.pressed ? "#3a2408" : "#141518"
                radius: width / 2

                Text {
                    anchors.centerIn: parent
                    color: cueIndicatorControl.value > 0 ? "#ff9a1f" : "#c08040"
                    font.bold: true
                    font.pixelSize: Math.round(parent.height * 0.28)
                    text: "CUE"
                }
                TapHandler {
                    id: cueTap

                    gesturePolicy: TapHandler.WithinBounds
                    onPressedChanged: cueControl.value = pressed ? 1 : 0
                }
            }
            Rectangle {
                Layout.preferredHeight: parent.buttonSize
                Layout.preferredWidth: parent.buttonSize
                border.color: playControl.value > 0 ? "#3ad33a" : "#2a6a2a"
                border.width: 2
                color: playTap.pressed ? "#0c2a0c" : "#141518"
                radius: width / 2

                // Drawn, as the fonts of some phones lack the pause sign and
                // draw the play sign as an emoji
                Row {
                    readonly property real barHeight: Math.round(parent.height * 0.34)

                    anchors.centerIn: parent
                    spacing: Math.round(barHeight * 0.3)
                    visible: playControl.value > 0

                    Repeater {
                        model: 2

                        Rectangle {
                            color: "#3ad33a"
                            height: parent.barHeight
                            width: Math.round(parent.barHeight * 0.3)
                        }
                    }
                }
                Skin.Triangle {
                    anchors.centerIn: parent
                    anchors.horizontalCenterOffset: Math.round(width * 0.1)
                    color: "#6ab86a"
                    height: Math.round(parent.height * 0.36)
                    visible: playControl.value <= 0
                    width: Math.round(height * 0.9)
                }
                TapHandler {
                    id: playTap

                    onPressedChanged: {
                        if (pressed) {
                            playControl.value = playControl.value > 0 ? 0 : 1;
                        }
                    }
                }
            }
        }

        // ---- Jog: turns with the track, touching it scratches ----
        Item {
            id: jogArea

            readonly property real size: Math.min(width, height)

            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: root.unit * 8
            Layout.preferredWidth: root.unit * 14

            Rectangle {
                id: platter

                anchors.centerIn: parent
                border.color: "#3a3c40"
                border.width: Math.max(2, Math.round(jogArea.size * 0.04))
                color: "#000000"
                height: jogArea.size
                radius: width / 2
                width: jogArea.size

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: Math.round(jogArea.size * 0.1)
                    border.color: "#1c1e22"
                    border.width: 1
                    color: "transparent"
                    radius: width / 2
                }
                DeckComponent.TouchSpinny {
                    id: spinny

                    anchors.fill: parent
                    group: root.group
                    indicatorVisible: root.loaded

                    indicator: Item {
                        height: spinny.height
                        width: spinny.width

                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.top: parent.top
                            anchors.topMargin: 2
                            color: root.deckColor
                            height: Math.round(jogArea.size * 0.12)
                            radius: width / 2
                            width: Math.max(3, Math.round(jogArea.size * 0.035))
                        }
                    }
                }
                Column {
                    anchors.centerIn: parent

                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter

                        Text {
                            id: bpmWhole

                            color: syncLeaderControl.value > 0 ? "#ff6a13" : "#ffffff"
                            font.pixelSize: Math.round(jogArea.size * 0.17)
                            text: root.loaded ? Math.floor(bpmControl.value) : "--"
                        }
                        Text {
                            anchors.baseline: bpmWhole.baseline
                            color: bpmWhole.color
                            font.pixelSize: Math.round(jogArea.size * 0.09)
                            text: root.loaded ? "." + root.pad(Math.floor((bpmControl.value % 1) * 100), 2) : ".--"
                        }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: "#9a9ea6"
                        font.pixelSize: Math.round(jogArea.size * 0.075)
                        text: {
                            const percent = (rateRatioControl.value - 1) * 100;
                            return (percent >= 0.005 ? "+" : "") + percent.toFixed(1) + "%  ±" + Math.round(rateRangeControl.value * 100);
                        }
                    }
                }
            }
            // Quantize and master tempo (key lock), like the Q and MT of a
            // player screen
            Column {
                anchors.bottom: parent.bottom
                anchors.left: root.mirrored ? parent.left : undefined
                anchors.right: root.mirrored ? undefined : parent.right
                spacing: 3

                SmallButton {
                    height: Math.round(root.unit * 2)
                    label: "Q"
                    on: quantizeControl.value > 0
                    width: Math.round(root.unit * 2.6)
                    onPressed: quantizeControl.value = quantizeControl.value > 0 ? 0 : 1
                }
                SmallButton {
                    height: Math.round(root.unit * 2)
                    label: "MT"
                    on: keylockControl.value > 0
                    width: Math.round(root.unit * 2.6)
                    onPressed: keylockControl.value = keylockControl.value > 0 ? 0 : 1
                }
            }
        }

        // ---- Tempo: the top is minus, like a player ----
        BoothFader {
            Layout.fillHeight: true
            Layout.preferredWidth: Math.round(root.unit * 2.6)
            centerMark: true
            color: root.deckColor
            group: root.group
            inverted: true
            key: "rate"
        }
    }
}
