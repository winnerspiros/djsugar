import QtQuick
import Mixxx 1.0 as Mixxx

// The beat of the bar of a deck, as 4 cells lit up to the current beat (the
// first one in its own color), counted from the downbeats of the beatgrid
Row {
    id: root

    required property string group
    property color color: "#00b0ff"
    property color downbeatColor: "#ff1744"
    property real cellWidth: 14
    property real cellHeight: 7

    // 1 to 4, 0 without a beatgrid
    readonly property int beat: beatInBar.value

    spacing: 3
    visible: beat > 0

    Mixxx.ControlProxy {
        id: beatInBar

        group: root.group
        key: "beat_in_bar"
    }
    Repeater {
        model: 4

        Rectangle {
            required property int index

            border.color: index === 0 ? root.downbeatColor : root.color
            border.width: 1
            color: index < root.beat ? (index === 0 ? root.downbeatColor : root.color) : "#a0101418"
            height: root.cellHeight
            radius: 2
            width: root.cellWidth
        }
    }
}
