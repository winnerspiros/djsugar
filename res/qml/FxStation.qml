import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "." as Skin

// The FX Station (src/engine/fxstation): a remix effector on the main mix
// with Scene, Isolate, X-Pad and Release sections and its own tempo. Every
// section is shown at once when there is room: in a row in place of the
// library (strip), or in two rows over the whole window; on phones one
// section at a time, in tabs.
Rectangle {
    id: root

    signal closeRequested

    // In place of the library, under the decks and the mixer: a single row,
    // without the header (the FX ST button closes it)
    readonly property bool strip: height < 560 && width >= 900 && height >= 220
    readonly property bool compact: !strip && (height < 560 || width < 760)
    property int tab: 0
    // The effects of the FX Station, in the order of scene_type and
    // release_type
    readonly property var sceneNames: ["BPF ECHO", "ECHO", "NOISE", "SPIRAL UP", "REVERB UP", "HPF ECHO", "LPF ECHO", "CRUSH ECHO", "SPIRAL DOWN", "REVERB DOWN"]
    readonly property var releaseNames: ["VINYL BRAKE", "ECHO", "BACK SPIN"]
    // The effect on each Scene and Release button, swapped with the edit
    // icon; a button plays its own unless swapped
    property var sceneEffects: sceneNames.map((name, button) => button)
    property var releaseEffects: releaseNames.map((name, button) => button)

    function loadButtonEffects() {
        sceneEffects = sceneNames.map((name, button) => {
            const effect = Mixxx.PlayerManager.fxStationButtonEffect("scene", button);
            return effect >= 0 && effect < sceneNames.length ? effect : button;
        });
        releaseEffects = releaseNames.map((name, button) => {
            const effect = Mixxx.PlayerManager.fxStationButtonEffect("release", button);
            return effect >= 0 && effect < releaseNames.length ? effect : button;
        });
    }

    color: "#0b0e13"
    Component.onCompleted: loadButtonEffects()

    // Touches on the gaps don't reach what is under the panel
    MouseArea {
        anchors.fill: parent
    }

    // A button of the panel: lit when active
    component FxButton: Rectangle {
        id: button

        property string text: ""
        property bool active: false
        property color activeColor: "#00e5ff"
        property bool enabledLook: true
        // Shows the edit icon, which emits edit()
        property bool editable: false
        signal clicked
        signal edit
        signal held
        signal pressed
        signal released

        border.color: active ? activeColor : "#323a48"
        border.width: active ? 2 : 1
        color: buttonArea.pressed ? "#2a3240" : (active ? Qt.darker(activeColor, 2.6) : "#161b24")
        opacity: enabledLook ? 1 : 0.4
        radius: 4

        Text {
            anchors.fill: parent
            anchors.margins: 3
            color: button.active ? "#ffffff" : "#b0bec5"
            font.bold: true
            font.pixelSize: Math.max(9, Math.min(14, button.height * 0.32))
            fontSizeMode: Text.Fit
            horizontalAlignment: Text.AlignHCenter
            minimumPixelSize: 7
            text: button.text
            verticalAlignment: Text.AlignVCenter
            wrapMode: Text.WordWrap
        }
        Skin.EditIcon {
            visible: button.editable
            anchors.right: parent.right
            anchors.rightMargin: 3
            anchors.top: parent.top
            anchors.topMargin: 3
            width: Math.max(10, Math.min(16, button.height * 0.3))
            height: width
            color: button.activeColor
            z: 1
            onClicked: button.edit()
        }
        MouseArea {
            id: buttonArea

            anchors.fill: parent
            enabled: button.enabledLook
            onClicked: button.clicked()
            onPressAndHold: button.held()
            onPressed: button.pressed()
            onReleased: button.released()
            onCanceled: button.released()
        }
    }

    // A section with its title
    component Section: Rectangle {
        id: section

        property string title: ""
        property color titleColor: "#00e5ff"
        default property alias content: body.data
        // Small buttons at the right of the title
        property alias actions: actionsRow.data

        border.color: "#232a35"
        border.width: 1
        color: "#11151c"
        radius: 6

        Text {
            id: sectionTitle

            anchors.left: parent.left
            anchors.leftMargin: 8
            anchors.top: parent.top
            anchors.topMargin: 4
            color: section.titleColor
            font.bold: true
            font.letterSpacing: 1
            font.pixelSize: 12
            text: section.title
        }
        Row {
            id: actionsRow

            anchors.right: parent.right
            anchors.rightMargin: 6
            anchors.top: parent.top
            anchors.topMargin: 3
            height: 18
            spacing: 4
        }
        Item {
            id: body

            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.margins: 6
            anchors.right: parent.right
            anchors.top: sectionTitle.bottom
            anchors.topMargin: 4
        }
    }

    Mixxx.ControlProxy {
        id: sceneTypeControl

        group: "[FxStation]"
        key: "scene_type"
    }
    Mixxx.ControlProxy {
        id: sceneOnControl

        group: "[FxStation]"
        key: "scene_on"
    }
    Mixxx.ControlProxy {
        id: sourceInputControl

        group: "[FxStation]"
        key: "fx_source_input"
    }
    Mixxx.ControlProxy {
        id: sourceXPadControl

        group: "[FxStation]"
        key: "fx_source_xpad"
    }
    Mixxx.ControlProxy {
        id: isolateModeControl

        group: "[FxStation]"
        key: "isolate_mode"
    }
    Mixxx.ControlProxy {
        id: bpmControl

        group: "[FxStation]"
        key: "bpm"
        updateInterval: 250
    }
    Mixxx.ControlProxy {
        id: beatControl

        group: "[FxStation]"
        key: "beat"
        updateInterval: 33
    }
    Mixxx.ControlProxy {
        id: sourceControl

        group: "[FxStation]"
        key: "source"
        updateInterval: 250
    }
    Mixxx.ControlProxy {
        id: bpmAutoControl

        group: "[FxStation]"
        key: "bpm_auto"
    }
    Mixxx.ControlProxy {
        id: quantizeControl

        group: "[FxStation]"
        key: "quantize"
    }
    Mixxx.ControlProxy {
        id: tapControl

        group: "[FxStation]"
        key: "tap"
    }
    Mixxx.ControlProxy {
        id: nudgeControl

        group: "[FxStation]"
        key: "nudge"
    }
    Mixxx.ControlProxy {
        id: releaseTypeControl

        group: "[FxStation]"
        key: "release_type"
    }
    Mixxx.ControlProxy {
        id: releaseControl

        group: "[FxStation]"
        key: "release"
    }
    Mixxx.ControlProxy {
        id: releaseActiveControl

        group: "[FxStation]"
        key: "release_active"
    }
    Mixxx.ControlProxy {
        id: instrumentControl

        group: "[FxStation]"
        key: "xpad_instrument"
    }
    Mixxx.ControlProxy {
        id: divisionControl

        group: "[FxStation]"
        key: "xpad_division"
    }
    Mixxx.ControlProxy {
        id: overdubControl

        group: "[FxStation]"
        key: "xpad_overdub"
    }
    Mixxx.ControlProxy {
        id: clearControl

        group: "[FxStation]"
        key: "xpad_clear"
    }
    Mixxx.ControlProxy {
        id: bankControl

        group: "[FxStation]"
        key: "xpad_bank"
    }
    Mixxx.ControlProxy {
        id: loopStepsControl

        group: "[FxStation]"
        key: "xpad_loop_steps"
    }
    Mixxx.ControlProxy {
        id: reloadControl

        group: "[FxStation]"
        key: "xpad_reload"
    }

    // ---- Scene ----
    component SceneSection: Section {
        id: sceneSection

        readonly property int type: sceneTypeControl.value
        readonly property bool isNoise: type === 2
        readonly property bool isReverb: type === 4 || type === 9
        readonly property bool isSpiral: type === 3 || type === 8

        // Plays the effect on a Scene button, or stops it
        function select(button) {
            const index = root.sceneEffects[button];
            if (sceneTypeControl.value === index && sceneOnControl.value > 0) {
                sceneOnControl.value = 0;
            } else {
                sceneTypeControl.value = index;
                sceneOnControl.value = 1;
            }
        }

        title: "SCENE FX"

        // FX SOURCE: what Scene and Isolate process, the music, the X-Pad
        // or both
        actions: [
            Text {
                color: "#90a4ae"
                font.bold: true
                font.pixelSize: 9
                height: 18
                text: "FX SOURCE"
                verticalAlignment: Text.AlignVCenter
            },
            FxButton {
                active: sourceInputControl.value > 0
                height: 18
                text: "INPUT"
                width: 52
                onClicked: sourceInputControl.value = sourceInputControl.value > 0 ? 0 : 1
            },
            FxButton {
                active: sourceXPadControl.value > 0
                activeColor: "#ff4081"
                height: 18
                text: "X-PAD"
                width: 52
                onClicked: sourceXPadControl.value = sourceXPadControl.value > 0 ? 0 : 1
            }
        ]

        ColumnLayout {
            anchors.fill: parent
            spacing: 4

            Text {
                color: "#ffab40"
                font.bold: true
                font.pixelSize: 10
                text: "BUILD UP"
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 34
                spacing: 3

                Repeater {
                    model: 5

                    FxButton {
                        required property int index
                        readonly property int button: index

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        active: sceneOnControl.value > 0 && sceneTypeControl.value === root.sceneEffects[button]
                        activeColor: "#ffab40"
                        editable: true
                        text: root.sceneNames[root.sceneEffects[button]]
                        onClicked: sceneSection.select(button)
                        onHeld: effectPicker.openFor("scene", button, activeColor)
                        onEdit: effectPicker.openFor("scene", button, activeColor)
                    }
                }
            }
            Text {
                color: "#40c4ff"
                font.bold: true
                font.pixelSize: 10
                text: "BREAK DOWN"
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 34
                spacing: 3

                Repeater {
                    model: 5

                    FxButton {
                        required property int index
                        readonly property int button: index + 5

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        active: sceneOnControl.value > 0 && sceneTypeControl.value === root.sceneEffects[button]
                        activeColor: "#40c4ff"
                        editable: true
                        text: root.sceneNames[root.sceneEffects[button]]
                        onClicked: sceneSection.select(button)
                        onHeld: effectPicker.openFor("scene", button, activeColor)
                        onEdit: effectPicker.openFor("scene", button, activeColor)
                    }
                }
            }
            RowLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                spacing: 6

                Skin.FxStationKnob {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 2
                    accentColor: sceneSection.type < 5 ? "#ffab40" : "#40c4ff"
                    defaultValue: 0.5
                    key: "scene_dial"
                    label: "SCENE"
                }
                ColumnLayout {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    spacing: 2

                    Skin.FxStationKnob {
                        id: param1Knob

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        key: "scene_param1"
                        label: sceneSection.isNoise ? "TONE" : (sceneSection.isReverb ? "SIZE" : "TIME")
                        valueText: sceneSection.isNoise || sceneSection.isReverb ? "" : ["1/8", "1/4", "3/8", "1/2", "3/4", "1"][Math.min(5, Math.floor(value * 6))]
                    }
                    Skin.FxStationKnob {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        key: "scene_param2"
                        label: sceneSection.isNoise ? "RESO" : (sceneSection.isReverb ? "TONE" : (sceneSection.isSpiral ? "STEP" : "FEEDBACK"))
                        valueText: sceneSection.isSpiral ? Math.round(1 + 6 * value) + " st" : ""
                    }
                }
            }
        }
    }

    // ---- Isolate ----
    component IsolateSection: Section {
        id: isolateSection

        readonly property var modes: ["ISOLATOR", "CUT / ADD", "TRANS / ROLL", "GATE / DRIVE"]
        readonly property var hints: ["KILL ← → BOOST", "CUT ← → ADD", "TRANS ← → ROLL", "GATE ← → DRIVE"]

        title: "ISOLATE FX"
        titleColor: "#69f0ae"

        ColumnLayout {
            anchors.fill: parent
            spacing: 4

            GridLayout {
                Layout.fillWidth: true
                columnSpacing: 3
                columns: 2
                rowSpacing: 3

                Repeater {
                    model: isolateSection.modes

                    FxButton {
                        required property int index
                        required property string modelData

                        Layout.fillWidth: true
                        Layout.preferredHeight: 30
                        active: isolateModeControl.value === index
                        activeColor: "#69f0ae"
                        text: modelData
                        onClicked: isolateModeControl.value = index
                    }
                }
            }
            Item {
                Layout.fillHeight: true
            }
            RowLayout {
                id: isolateKnobs

                // Not stretched by the knobs: centered with the hint
                Layout.fillHeight: false
                Layout.fillWidth: true
                // Round knobs: as tall as a third of the width, plus the label
                Layout.preferredHeight: width / 3 + 20
                spacing: 4

                Repeater {
                    model: [["isolate_low", "LOW"], ["isolate_mid", "MID"], ["isolate_high", "HIGH"]]

                    Skin.FxStationKnob {
                        required property var modelData

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        accentColor: "#69f0ae"
                        bipolar: true
                        key: modelData[0]
                        label: modelData[1]
                    }
                }
            }
            // What each side of the knobs does in this mode
            Text {
                Layout.fillWidth: true
                color: "#69f0ae"
                font.bold: true
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                text: isolateSection.hints[Math.max(0, Math.min(3, isolateModeControl.value))]
            }
            Item {
                Layout.fillHeight: true
            }
        }
    }

    // ---- X-Pad ----
    component XPadSection: Section {
        id: xpadSection

        readonly property var instruments: ["KICK", "SNARE", "CLAP", "HI-HAT"]
        // The name on each pad: the picked sound, or the instrument
        property var padNames: instruments
        function loadPadNames() {
            padNames = instruments.map((name, index) => {
                const path = Mixxx.PlayerManager.fxStationSound(bankControl.value, index);
                if (!path) {
                    return name;
                }
                const file = path.substring(path.lastIndexOf("/") + 1);
                return file.substring(0, file.lastIndexOf(".") > 0 ? file.lastIndexOf(".") : file.length).replace(/[-_]+/g, " ").toUpperCase();
            });
        }

        Component.onCompleted: loadPadNames()

        Connections {
            target: bankControl
            function onValueChanged() {
                xpadSection.loadPadNames();
            }
        }
        Connections {
            target: soundPicker
            function onPicked() {
                Qt.callLater(xpadSection.loadPadNames);
            }
        }
        readonly property var divisions: ["1/1", "1/2", "1/4", "1/8", "1/16"]
        // The reload of the user banks, at the right of the title
        property bool showReload: false

        title: "X-PAD"
        titleColor: "#ff4081"

        actions: [
            FxButton {
                height: 18
                text: "RELOAD"
                visible: xpadSection.showReload
                width: 64
                onPressed: reloadControl.value = 1
                onReleased: reloadControl.value = 0
            }
        ]

        // The instruments, the ribbon under them, then the bank, the sound
        // and the loop
        ColumnLayout {
            anchors.fill: parent
            spacing: 4

            RowLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.minimumHeight: 28
                Layout.preferredHeight: 2
                spacing: 3

                Repeater {
                    model: xpadSection.instruments

                    FxButton {
                        id: instrumentButton

                        required property int index
                        required property string modelData

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        active: instrumentControl.value === index
                        activeColor: "#ff4081"
                        text: muteControl.value > 0 ? xpadSection.padNames[index] + " ✕" : xpadSection.padNames[index]
                        editable: true
                        // Plays the sound and makes it the one the rolls play
                        onPressed: {
                            instrumentControl.value = index;
                            hitControl.value = 1;
                        }
                        onReleased: hitControl.value = 0
                        onHeld: soundPicker.openFor(bankControl.value, index, modelData)
                        onEdit: soundPicker.openFor(bankControl.value, index, modelData)

                        Mixxx.ControlProxy {
                            id: hitControl

                            group: "[FxStation]"
                            key: "xpad_hit_" + (instrumentButton.index + 1)
                        }
                        Mixxx.ControlProxy {
                            id: muteControl

                            group: "[FxStation]"
                            key: "xpad_mute_" + (instrumentButton.index + 1)
                        }
                    }
                }
            }
            // The ribbon: 1/1 on the left to 1/16 on the right, while
            // touched
            Rectangle {
                id: ribbon

                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.minimumHeight: 36
                Layout.preferredHeight: 3
                border.color: "#ff4081"
                border.width: 1
                color: "#1a1016"
                radius: 6

                Row {
                    anchors.fill: parent
                    anchors.margins: 1

                    Repeater {
                        model: xpadSection.divisions

                        Rectangle {
                            id: divisionCell

                            required property int index
                            required property string modelData
                            // Touched here, or its roll button held (e.g.
                            // on a pad controller)
                            readonly property bool lit: divisionControl.value === index + 1 || rollControl.value > 0

                            color: lit ? "#ff4081" : "transparent"
                            height: parent.height
                            opacity: lit ? 0.8 : 1
                            radius: 5
                            width: parent.width / 5

                            Rectangle {
                                anchors.right: parent.right
                                color: "#3a2430"
                                height: parent.height
                                visible: parent.index < 4
                                width: 1
                            }
                            Text {
                                anchors.centerIn: parent
                                color: "#ffffff"
                                font.bold: true
                                font.pixelSize: Math.max(11, Math.min(20, parent.height * 0.3))
                                text: parent.modelData
                            }
                            Mixxx.ControlProxy {
                                id: rollControl

                                group: "[FxStation]"
                                key: "xpad_roll_" + (divisionCell.index + 1)
                            }
                        }
                    }
                }
                MouseArea {
                    function divisionAt(x) {
                        return Math.max(1, Math.min(5, 1 + Math.floor(x / width * 5)));
                    }

                    anchors.fill: parent
                    preventStealing: true

                    onCanceled: divisionControl.value = 0
                    onPositionChanged: mouse => divisionControl.value = divisionAt(mouse.x)
                    onPressed: mouse => divisionControl.value = divisionAt(mouse.x)
                    onReleased: divisionControl.value = 0
                }
            }
            RowLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.minimumHeight: 70
                Layout.preferredHeight: 3
                spacing: 4

                GridLayout {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 130
                    columnSpacing: 3
                    columns: 2
                    rowSpacing: 3

                    Repeater {
                        model: 4

                        FxButton {
                            id: bankButton

                            required property int index

                            Layout.fillHeight: true
                            Layout.fillWidth: true
                            active: bankControl.value === index
                            activeColor: "#ff4081"
                            // An empty bank stays reachable, to give its pads sounds
                            opacity: bankSounds.value > 0 ? 1 : 0.55
                            text: "BANK " + (index + 1)
                            onClicked: bankControl.value = index

                            Mixxx.ControlProxy {
                                id: bankSounds

                                group: "[FxStation]"
                                key: "xpad_bank_sounds_" + (bankButton.index + 1)
                            }
                        }
                    }
                }
                Skin.FxStationKnob {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.minimumWidth: 56
                    Layout.preferredWidth: 80
                    accentColor: "#ff4081"
                    bipolar: true
                    key: "xpad_pitch"
                    label: "PITCH"
                    valueText: (value > 0 ? "+" : "") + Math.round(value * 12)
                }
                Skin.FxStationKnob {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.minimumWidth: 56
                    Layout.preferredWidth: 80
                    accentColor: "#ff4081"
                    defaultValue: 0.8
                    key: "xpad_volume"
                    label: "LEVEL"
                }
                ColumnLayout {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.minimumWidth: 80
                    Layout.preferredWidth: 120
                    spacing: 3

                    FxButton {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        active: overdubControl.value > 0
                        activeColor: "#ff1744"
                        text: loopStepsControl.value > 0 ? "OVERDUB ●" : "OVERDUB"
                        onClicked: overdubControl.value = overdubControl.value > 0 ? 0 : 1
                    }
                    FxButton {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        active: selectedMute.value > 0
                        activeColor: "#ffd740"
                        text: "MUTE " + xpadSection.padNames[Math.max(0, Math.min(3, instrumentControl.value))]
                        onClicked: selectedMute.value = selectedMute.value > 0 ? 0 : 1

                        Mixxx.ControlProxy {
                            id: selectedMute

                            group: "[FxStation]"
                            key: "xpad_mute_" + (instrumentControl.value + 1)
                        }
                    }
                    FxButton {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        text: "CLEAR"
                        onPressed: clearControl.value = 1
                        onReleased: clearControl.value = 0
                    }
                }
            }
        }
    }

    // ---- Release ----
    component ReleaseSection: Section {
        title: "RELEASE FX"
        titleColor: "#ff6e40"

        ColumnLayout {
            anchors.fill: parent
            spacing: 3

            Repeater {
                model: 3

                FxButton {
                    required property int index
                    readonly property int effect: root.releaseEffects[index]

                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    active: releaseActiveControl.value > 0 && releaseTypeControl.value === effect
                    activeColor: "#ff6e40"
                    editable: true
                    text: root.releaseNames[effect]
                    onEdit: effectPicker.openFor("release", index, activeColor)
                    onPressed: {
                        releaseTypeControl.value = effect;
                        releaseControl.value = 1;
                    }
                    onReleased: releaseControl.value = 0
                }
            }
        }
    }

    // ---- Tempo ----
    component ClockSection: Section {
        title: "BPM"
        titleColor: "#e0e0e0"

        ColumnLayout {
            anchors.fill: parent
            spacing: 3

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Text {
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: 26
                    text: Number(bpmControl.value).toFixed(1)
                }
                Text {
                    Layout.fillWidth: true
                    color: "#90a4ae"
                    font.bold: true
                    font.pixelSize: 11
                    text: sourceControl.value > 0 ? "DECK " + sourceControl.value : (bpmAutoControl.value > 0 ? "AUTO" : "TAP")
                }
                // The beats of the bar
                Row {
                    spacing: 4

                    Repeater {
                        model: 4

                        Rectangle {
                            required property int index

                            color: Math.floor(beatControl.value) === index ? (index === 0 ? "#ff1744" : "#00e5ff") : "#263040"
                            height: 12
                            radius: 6
                            width: 12
                        }
                    }
                }
            }
            // Narrow (in the strip): in two rows
            GridLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                columnSpacing: 3
                columns: width >= 330 ? 5 : 3
                rowSpacing: 3

                FxButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    active: bpmAutoControl.value > 0
                    text: "AUTO"
                    onClicked: bpmAutoControl.value = bpmAutoControl.value > 0 ? 0 : 1
                }
                FxButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    text: "TAP"
                    onPressed: tapControl.value = 1
                    onReleased: tapControl.value = 0
                }
                FxButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    active: quantizeControl.value > 0
                    text: "QUANTIZE"
                    onClicked: quantizeControl.value = quantizeControl.value > 0 ? 0 : 1
                }
                FxButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    text: "NUDGE −"
                    onPressed: nudgeControl.value = -1
                    onReleased: nudgeControl.value = 0
                }
                FxButton {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    text: "NUDGE +"
                    onPressed: nudgeControl.value = 1
                    onReleased: nudgeControl.value = 0
                }
            }
        }
    }

    // ---- Header ----
    RowLayout {
        id: header

        anchors.left: parent.left
        anchors.margins: 6
        anchors.right: parent.right
        anchors.top: parent.top
        height: root.strip ? 0 : 36
        spacing: 6
        visible: !root.strip

        Text {
            color: "#ffffff"
            font.bold: true
            font.letterSpacing: 2
            font.pixelSize: 16
            text: "FX STATION"
        }
        // Phones: one section at a time
        Repeater {
            model: root.compact ? ["SCENE", "ISOLATE", "X-PAD", "RELEASE"] : []

            FxButton {
                required property int index
                required property string modelData

                Layout.fillHeight: true
                Layout.preferredWidth: 90
                active: root.tab === index
                text: modelData
                onClicked: root.tab = index
            }
        }
        Item {
            Layout.fillWidth: true
        }
        FxButton {
            Layout.fillHeight: true
            Layout.preferredWidth: 140
            text: "RELOAD SAMPLES"
            onPressed: reloadControl.value = 1
            onReleased: reloadControl.value = 0
        }
        FxButton {
            Layout.fillHeight: true
            Layout.preferredWidth: 80
            text: "CLOSE"
            onClicked: root.closeRequested()
        }
    }

    // ---- In place of the library: everything in a row ----
    Loader {
        anchors.fill: parent
        anchors.margins: 4
        active: root.strip
        sourceComponent: RowLayout {
            spacing: 4

            IsolateSection {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 230
            }
            SceneSection {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 340
            }
            XPadSection {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 450
                showReload: true
            }
            ColumnLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 240
                spacing: 4

                ReleaseSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredHeight: 3
                }
                ClockSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredHeight: 2
                }
            }
        }
    }

    // ---- Tablets: everything at once ----
    Loader {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.margins: 6
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: 6
        active: !root.compact && !root.strip
        sourceComponent: ColumnLayout {
            spacing: 6

            RowLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredHeight: 3
                spacing: 6

                IsolateSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 3
                }
                SceneSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 4
                }
                ColumnLayout {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.preferredWidth: 3
                    spacing: 6

                    ReleaseSection {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        Layout.preferredHeight: 3
                    }
                    ClockSection {
                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        Layout.preferredHeight: 2
                    }
                }
            }
            XPadSection {
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredHeight: 2
            }
        }
    }

    // ---- Phones: a section at a time ----
    Loader {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.margins: 6
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: 6
        active: root.compact
        sourceComponent: Item {
            SceneSection {
                anchors.fill: parent
                visible: root.tab === 0
            }
            IsolateSection {
                anchors.fill: parent
                visible: root.tab === 1
            }
            XPadSection {
                anchors.fill: parent
                visible: root.tab === 2
            }
            RowLayout {
                anchors.fill: parent
                spacing: 6
                visible: root.tab === 3

                ReleaseSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                }
                ClockSection {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                }
            }
        }
    }

    // The sound of an X-Pad pad: any sample of the pads (bundled,
    // imported or recorded), or back to the bank's default
    Skin.SamplePicker {
        id: soundPicker

        property int bank: 0
        property int instrument: 0

        function openFor(bankIndex, instrumentIndex, name) {
            bank = bankIndex;
            instrument = instrumentIndex;
            title = "BANK " + (bankIndex + 1) + " · " + name;
            defaultLabel = bankIndex === 0 ? qsTr("Default %1").arg(name.toLowerCase()) : qsTr("From bank folder");
            current = Mixxx.PlayerManager.fxStationSound(bankIndex, instrumentIndex);
            open();
        }

        accent: "#ff4081"
        onPicked: path => Mixxx.PlayerManager.setFxStationSound(bank, instrument, path)
    }

    // The effect of a Scene or Release button: any FX Station effect of
    // its kind, or back to the button's own
    Popup {
        id: effectPicker

        property string kind: "scene"
        property int button: 0
        property color accent: "#ffab40"
        readonly property var names: kind === "scene" ? root.sceneNames : root.releaseNames
        readonly property int current: (kind === "scene" ? root.sceneEffects : root.releaseEffects)[button]

        function openFor(newKind, newButton, color) {
            // The FX Station is loaded outside the window, so the overlay
            // is looked up when opening
            parent = root.Overlay.overlay;
            kind = newKind;
            button = newButton;
            accent = color;
            open();
        }
        function pick(effect) {
            Mixxx.PlayerManager.setFxStationButtonEffect(kind, button, effect === button ? -1 : effect);
            root.loadButtonEffects();
        }

        anchors.centerIn: parent
        width: Math.min(parent ? parent.width - 40 : 400, 380)
        height: Math.min(parent ? parent.height - 40 : 500, 56 * names.length + 110)
        modal: true
        padding: 8

        background: Rectangle {
            color: "#000000"
            radius: 8
            border.color: effectPicker.accent
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 6

            Text {
                text: (effectPicker.kind === "scene" ? "SCENE FX " : "RELEASE FX ") + (effectPicker.button + 1) + " · " + effectPicker.names[effectPicker.current]
                color: "#ffffff"
                font.bold: true
                font.pixelSize: 14
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: effectPicker.names
                delegate: Rectangle {
                    id: row

                    required property int index
                    required property string modelData
                    readonly property bool current: effectPicker.current === index

                    width: ListView.view.width
                    height: 44
                    radius: 6
                    color: rowArea.pressed ? "#1a1a1a" : current ? Qt.rgba(effectPicker.accent.r, effectPicker.accent.g, effectPicker.accent.b, 0.16) : "#000000"

                    // Accent bar on the chosen row
                    Rectangle {
                        visible: row.current
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        width: 4
                        height: parent.height - 12
                        radius: 2
                        color: effectPicker.accent
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.right: checkMark.left
                        anchors.rightMargin: 8
                        elide: Text.ElideRight
                        text: row.modelData + (row.index === effectPicker.button ? qsTr(" (default)") : "")
                        color: row.current ? effectPicker.accent : "#ffffff"
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
                        color: effectPicker.accent
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
                        onClicked: effectPicker.pick(row.index)
                    }
                }
            }
            Rectangle {
                Layout.alignment: Qt.AlignRight
                Layout.preferredWidth: 90
                Layout.preferredHeight: 34
                radius: 17
                color: effectPicker.accent

                Text {
                    anchors.centerIn: parent
                    text: qsTr("Done")
                    color: "#000000"
                    font.bold: true
                    font.pixelSize: 13
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: effectPicker.close()
                }
            }
        }
    }
}
