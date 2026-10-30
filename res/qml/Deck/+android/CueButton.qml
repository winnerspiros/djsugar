import ".." as Skin
import "../Theme"

Skin.ControlButton {
    id: root

    property bool minimized: false

    activeColor: "#ff9100"
    key: "cue_default"
    text: "CUE"
    font.bold: true
}
