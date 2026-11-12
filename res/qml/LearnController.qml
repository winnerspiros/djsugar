import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx

// Learn controller (Android): makes a mapping by pressing or moving each
// control of a MIDI controller, see QmlControllerLearningProxy. For players
// like CDJs, each one is learned with its own deck, so that two of them can
// be used at once.
Rectangle {
    id: root

    signal closeRequested

    property int deck: 1
    property string waitingFor: ""
    // "group,key" -> description of what was learned
    property var learnedText: ({})
    property string status: ""

    readonly property string deckGroup: "[Channel" + deck + "]"
    readonly property var sections: [
        {
            "title": qsTr("Transport"),
            "items": [["play", qsTr("Play / pause")], ["cue_default", qsTr("Cue")], ["sync_enabled", qsTr("Sync")], ["keylock", qsTr("Master tempo (key lock)")], ["quantize", qsTr("Quantize")], ["slip_enabled", qsTr("Slip")]]
        },
        {
            "title": qsTr("Tempo and jog"),
            "items": [["rate", qsTr("Tempo fader")], ["rate_temp_up", qsTr("Pitch bend +")], ["rate_temp_down", qsTr("Pitch bend −")], ["rateRange", qsTr("Tempo range")], ["jog", qsTr("Jog wheel (nudge)")], ["beatjump_4_forward", qsTr("Beat jump forward")], ["beatjump_4_backward", qsTr("Beat jump back")]]
        },
        {
            "title": qsTr("Hot cues"),
            "items": [["hotcue_1_activate", "Hot cue A"], ["hotcue_2_activate", "Hot cue B"], ["hotcue_3_activate", "Hot cue C"], ["hotcue_4_activate", "Hot cue D"], ["hotcue_5_activate", "Hot cue E"], ["hotcue_6_activate", "Hot cue F"], ["hotcue_7_activate", "Hot cue G"], ["hotcue_8_activate", "Hot cue H"]]
        },
        {
            "title": qsTr("Loops"),
            "items": [["loop_in", qsTr("Loop in")], ["loop_out", qsTr("Loop out")], ["reloop_toggle", qsTr("Reloop / exit")], ["beatloop_4_toggle", qsTr("Loop of 4 beats")], ["loop_halve", qsTr("Loop ½")], ["loop_double", qsTr("Loop ×2")]]
        },
        {
            "title": qsTr("Mixer of the deck"),
            "items": [["volume", qsTr("Channel fader")], ["pregain", qsTr("Trim")], ["pfl", qsTr("Headphones cue")], ["eq_high", qsTr("EQ high")], ["eq_mid", qsTr("EQ mid")], ["eq_low", qsTr("EQ low")], ["filter", qsTr("Filter")]]
        },
        {
            "title": qsTr("Library"),
            "items": [["LoadSelectedTrack", qsTr("Load the selected track")], ["library_browse", qsTr("Browse knob")], ["library_focus", qsTr("Browse press (open / back)")], ["eject", qsTr("Eject")]]
        },
        {
            // "fx:" are controls of the FX Station, the same for every deck
            "title": qsTr("FX Station"),
            "items": [["fx:scene_on", qsTr("Scene FX on / off")], ["fx:scene_type", qsTr("Next scene")], ["fx:scene_dial", qsTr("Scene dial")], ["fx:scene_param1", qsTr("Scene time")], ["fx:scene_param2", qsTr("Scene feedback")], ["fx:isolate_mode", qsTr("Next isolate mode")], ["fx:isolate_low", qsTr("Isolate low")], ["fx:isolate_mid", qsTr("Isolate mid")], ["fx:isolate_high", qsTr("Isolate high")], ["fx:xpad_instrument", qsTr("Next X-Pad instrument")], ["fx:xpad_roll_2", qsTr("X-Pad roll 1/2 (hold)")], ["fx:xpad_roll_3", qsTr("X-Pad roll 1/4 (hold)")], ["fx:xpad_roll_4", qsTr("X-Pad roll 1/8 (hold)")], ["fx:xpad_roll_5", qsTr("X-Pad roll 1/16 (hold)")], ["fx:xpad_volume", qsTr("X-Pad level")], ["fx:xpad_pitch", qsTr("X-Pad pitch")], ["fx:xpad_overdub", qsTr("X-Pad overdub")], ["fx:xpad_clear", qsTr("X-Pad clear")], ["fx:release", qsTr("Release FX (hold)")], ["fx:release_type", qsTr("Next release type")], ["fx:fx_source_input", qsTr("FX source: input")], ["fx:fx_source_xpad", qsTr("FX source: X-Pad")], ["fx:tap", qsTr("Tap tempo")], ["fx:quantize", qsTr("Quantize of the FX Station")]]
        },
        {
            "title": qsTr("Main"),
            "items": [["crossfader", qsTr("Crossfader")], ["headMix", qsTr("Headphones mix")], ["headGain", qsTr("Headphones level")], ["main_gain", qsTr("Main level")]]
        }
    ]

    // Where each function of the list goes in Mixxx
    function controlOf(item) {
        const n = root.deck;
        switch (item) {
        case "eq_high":
            return ["[EqualizerRack1_[Channel" + n + "]_Effect1]", "parameter3"];
        case "eq_mid":
            return ["[EqualizerRack1_[Channel" + n + "]_Effect1]", "parameter2"];
        case "eq_low":
            return ["[EqualizerRack1_[Channel" + n + "]_Effect1]", "parameter1"];
        case "filter":
            return ["[QuickEffectRack1_[Channel" + n + "]]", "super1"];
        case "library_browse":
            return ["[Library]", "MoveVertical"];
        case "library_focus":
            return ["[Library]", "GoToItem"];
        case "crossfader":
            return ["[Master]", "crossfader"];
        case "headMix":
            return ["[Master]", "headMix"];
        case "headGain":
            return ["[Master]", "headGain"];
        case "main_gain":
            return ["[Master]", "gain"];
        default:
            if (item.startsWith("fx:"))
                return ["[FxStation]", item.slice(3)];
            return [root.deckGroup, item];
        }
    }
    function keyOf(item) {
        const control = controlOf(item);
        return control[0] + "," + control[1];
    }

    color: "#0f1319"

    Component.onCompleted: {
        if (Mixxx.ControllerLearning.controllers.length > 0)
            Mixxx.ControllerLearning.start(controllerChoice.currentIndex);
    }
    Component.onDestruction: Mixxx.ControllerLearning.cancel()

    Connections {
        function onControlLearned(group, key, description) {
            const learned = root.learnedText;
            learned[group + "," + key] = description;
            root.learnedText = learned;
            root.waitingFor = "";
            root.status = qsTr("Learned: %1").arg(description);
        }
        function onControlNotLearned(group, key) {
            root.waitingFor = "";
            root.status = qsTr("Nothing usable was received, try again");
        }

        target: Mixxx.ControllerLearning
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                Layout.fillWidth: true
                color: "#ffffff"
                font.bold: true
                font.pixelSize: 18
                text: qsTr("🎛 Learn controller")
            }
            Button {
                text: qsTr("Close")
                onClicked: root.closeRequested()
            }
        }
        Text {
            Layout.fillWidth: true
            color: "#90a4ae"
            font.pixelSize: 12
            text: qsTr("Tap LEARN next to a function, then press or move that control on your controller. With two players (e.g. two CDJs), learn each one with its own deck.")
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                color: "#cfd8dc"
                font.pixelSize: 13
                text: qsTr("Controller")
            }
            ComboBox {
                id: controllerChoice

                Layout.fillWidth: true
                model: Mixxx.ControllerLearning.controllers
                onActivated: index => {
                    root.learnedText = ({});
                    Mixxx.ControllerLearning.start(index);
                }
            }
            Text {
                color: "#cfd8dc"
                font.pixelSize: 13
                text: qsTr("Deck")
            }
            Repeater {
                model: 4

                Button {
                    required property int index

                    checkable: true
                    checked: root.deck === index + 1
                    implicitWidth: 44
                    text: index + 1
                    onClicked: root.deck = index + 1
                }
            }
        }
        // What the controller sends right now
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: Mixxx.ControllerLearning.listening ? "#3b2a10" : "#161b24"
            radius: 6

            Text {
                anchors.fill: parent
                anchors.margins: 8
                color: Mixxx.ControllerLearning.listening ? "#ffd740" : "#b0bec5"
                elide: Text.ElideRight
                font.pixelSize: 13
                text: Mixxx.ControllerLearning.controllers.length === 0 ? qsTr("No MIDI controller is connected and enabled") : (Mixxx.ControllerLearning.listening ? qsTr("Press or move the control now…  %1").arg(Mixxx.ControllerLearning.lastMessage) : (root.status.length > 0 ? root.status : qsTr("Last message: %1").arg(Mixxx.ControllerLearning.lastMessage || "—")))
                verticalAlignment: Text.AlignVCenter
            }
        }
        ListView {
            id: list

            Layout.fillHeight: true
            Layout.fillWidth: true
            clip: true
            model: root.sections
            spacing: 10

            delegate: ColumnLayout {
                id: sectionDelegate

                required property var modelData

                spacing: 4
                width: list.width

                Text {
                    color: "#00e5ff"
                    font.bold: true
                    font.pixelSize: 13
                    text: sectionDelegate.modelData.title
                }
                Repeater {
                    model: sectionDelegate.modelData.items

                    Rectangle {
                        id: row

                        required property var modelData
                        readonly property string learnKey: root.keyOf(modelData[0])
                        readonly property bool learned: root.learnedText[learnKey] !== undefined
                        readonly property bool waiting: root.waitingFor === learnKey

                        Layout.fillWidth: true
                        Layout.preferredHeight: 42
                        border.color: waiting ? "#ffd740" : (learned ? "#69f0ae" : "#232a35")
                        color: "#141921"
                        radius: 6

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 6
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                color: "#ffffff"
                                elide: Text.ElideRight
                                font.pixelSize: 13
                                text: (row.learned ? "✓ " : "") + row.modelData[1]
                            }
                            Text {
                                color: "#69f0ae"
                                elide: Text.ElideLeft
                                font.pixelSize: 11
                                text: row.learned ? root.learnedText[row.learnKey] : ""
                                Layout.maximumWidth: row.width * 0.4
                            }
                            Button {
                                enabled: Mixxx.ControllerLearning.active
                                text: row.waiting ? qsTr("WAITING…") : qsTr("LEARN")
                                onClicked: {
                                    if (row.waiting) {
                                        Mixxx.ControllerLearning.stopListening();
                                        root.waitingFor = "";
                                        return;
                                    }
                                    const control = root.controlOf(row.modelData[0]);
                                    root.waitingFor = row.learnKey;
                                    root.status = "";
                                    Mixxx.ControllerLearning.listen(control[0], control[1]);
                                }
                            }
                        }
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            TextField {
                id: nameField

                Layout.fillWidth: true
                placeholderText: qsTr("Name of the mapping")
                text: controllerChoice.currentText.length > 0 ? controllerChoice.currentText + qsTr(" (deck %1)").arg(root.deck) : ""
            }
            Button {
                enabled: Mixxx.ControllerLearning.learned.length > 0
                text: qsTr("SAVE (%1)").arg(Mixxx.ControllerLearning.learned.length)
                onClicked: {
                    const path = Mixxx.ControllerLearning.save(nameField.text);
                    root.status = path.length > 0 ? qsTr("Saved and in use: %1").arg(path) : qsTr("The mapping couldn't be saved");
                    if (path.length > 0) {
                        root.learnedText = ({});
                        Mixxx.ControllerLearning.start(controllerChoice.currentIndex);
                    }
                }
            }
        }
    }
}
