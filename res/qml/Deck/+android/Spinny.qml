import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls 2.12
import "../Theme"

Item {
    id: root

    required property string group

    implicitWidth: 140
    implicitHeight: 140

    readonly property color deckAccentColor: {
        switch (root.group) {
        case "[Channel1]": return "#00b0ff";
        case "[Channel2]": return "#ff6d00";
        case "[Channel3]": return "#00e676";
        case "[Channel4]": return "#e040fb";
        default: return Theme.accentColor;
        }
    }

    Mixxx.ControlProxy {
        id: trackLoadedControl

        group: root.group
        key: "track_loaded"
    }

    Rectangle {
        id: spinner

        anchors.centerIn: parent
        color: "#181a20"
        border.color: trackLoadedControl.value ? root.deckAccentColor : "#353842"
        border.width: 2.5
        height: Math.min(140, Math.max(80, Math.min(parent.width, parent.height) - 6))
        width: height
        radius: width / 2

        // Outer vinyl groove ring
        Rectangle {
            anchors.fill: parent
            anchors.margins: 6
            color: "transparent"
            border.color: "#282b35"
            border.width: 1.5
            radius: width / 2
        }

        // Inner vinyl groove ring
        Rectangle {
            anchors.fill: parent
            anchors.margins: 14
            color: "transparent"
            border.color: "#232630"
            border.width: 1
            radius: width / 2
        }

        TouchSpinny {
            id: spinnyIndicator

            anchors.fill: parent
            group: root.group
            indicatorVisible: trackLoadedControl.value > 0

            indicator: Item {
                height: spinnyIndicator.height
                width: spinnyIndicator.width

                Rectangle {
                    id: tick

                    color: root.deckAccentColor
                    height: spinner.height / 2 - 12
                    width: 3
                    radius: 1.5

                    anchors {
                        horizontalCenter: parent.horizontalCenter
                        top: parent.top
                        topMargin: 4
                    }
                }
            }
        }

        Rectangle {
            id: tempoInfo

            anchors.centerIn: parent
            clip: true
            color: "#0c0d10"
            border.color: "#282a32"
            border.width: 1
            height: Math.round(spinner.height * 0.44)
            radius: height / 2
            width: Math.round(spinner.width * 0.62)

            Behavior on height {
                SpringAnimation {
                    damping: 0.3
                    duration: 350
                    spring: 2
                }
            }

            Column {
                anchors.centerIn: parent
                spacing: 1

                Label {
                    id: tempo

                    readonly property real bpm: bpmControl.value

                    anchors.horizontalCenter: parent.horizontalCenter
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: Math.max(10, Math.round(tempoInfo.height * 0.34))
                    text: bpm > 0 ? (bpm.toFixed(2) + " BPM") : "--.-- BPM"

                    Mixxx.ControlProxy {
                        id: bpmControl

                        group: root.group
                        key: "bpm"
                    }
                }

                Label {
                    id: pitchRatio

                    readonly property real ratio: ((rateRatioControl.value - 1) * 100)

                    anchors.horizontalCenter: parent.horizontalCenter
                    color: root.deckAccentColor
                    font.pixelSize: Math.max(8, Math.round(tempoInfo.height * 0.26))
                    text: ((ratio > 0) ? "+" + ratio.toFixed(2) : ratio.toFixed(2)) + "%"

                    Mixxx.ControlProxy {
                        id: rateRatioControl

                        group: root.group
                        key: "rate_ratio"
                    }
                }
            }
        }
    }
}
