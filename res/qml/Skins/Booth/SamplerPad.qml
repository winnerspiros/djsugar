import Mixxx 1.0 as Mixxx
import QtQuick

// A sampler pad for the Booth skin's SAMPLER pad mode: tapping plays or
// cues the sampler, holding repeats it while held. Lit orange when loaded,
// brighter while playing.
Rectangle {
    id: padItem

    // Repeater index (0-7) of this pad; the Repeater assigns it automatically
    // when SamplerPad is used as a delegate. Combined with mirrored to pick
    // the sampler group: left deck [Sampler1-8], right deck [Sampler9-16].
    required property int index
    required property bool mirrored

    readonly property string samplerGroup: "[Sampler" + (index + 1 + (mirrored ? 8 : 0)) + "]"

    Mixxx.ControlProxy {
        id: loadedControl

        group: padItem.samplerGroup
        key: "track_loaded"
    }
    Mixxx.ControlProxy {
        id: playControl

        group: padItem.samplerGroup
        key: "play"
    }
    Mixxx.ControlProxy {
        id: cueControl

        group: padItem.samplerGroup
        key: "cue_gotoandplay"
    }

    readonly property bool lit: loadedControl.value > 0
    readonly property bool playing: playControl.value > 0

    border.color: padTap.pressed ? "#ffffff" : (playing ? "#ff6a13" : (lit ? "#e8650f" : "#2a2c30"))
    border.width: playing ? 2 : 1
    color: padTap.pressed ? "#2a2c30" : (playing ? Qt.alpha("#ff6a13", 0.3) : (lit ? Qt.alpha("#e8650f", 0.18) : "#141518"))
    radius: 2

    Text {
        anchors.centerIn: parent
        color: padItem.playing ? "#ffffff" : (padItem.lit ? "#e8650f" : "#5c6068")
        font.bold: true
        font.pixelSize: Math.round(parent.height * 0.28)
        text: padItem.samplerGroup.replace("[Sampler", "S").replace("]", "")
    }
    TapHandler {
        id: padTap

        gesturePolicy: TapHandler.WithinBounds
        // Hold: keep repeating from the cue while held
        onPressedChanged: cueControl.value = pressed ? 1 : 0
    }
}
