import ".." as Skin
import "." as DeckComponent
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Layouts
import "../Theme"

Rectangle {
    id: root

    readonly property var currentTrack: deckPlayer?.currentTrack
    property var deckPlayer: Mixxx.PlayerManager.getPlayer(group)
    property bool editMode: false
    required property string group
    property bool minimized: false
    required property int rightColumnWidth

    readonly property bool isDeck1: group === "[Channel1]"
    readonly property bool isDeck2: group === "[Channel2]"
    readonly property color deckColor: isDeck1 ? "#00b0ff" : (isDeck2 ? "#ff6d00" : (group === "[Channel3]" ? "#00e676" : "#e040fb"))
    readonly property color deckBgColor: isDeck1 ? "#0d47a1" : (isDeck2 ? "#bf360c" : "#263238")
    readonly property string deckName: isDeck1 ? "DECK 1" : (isDeck2 ? "DECK 2" : (group === "[Channel3]" ? "DECK 3" : "DECK 4"))

    radius: 6
    color: "#181b22"
    border.color: isDeck1 ? "#00b0ff44" : (isDeck2 ? "#ff6d0044" : "#303644")
    border.width: 1

    signal triggerLoadTrack()

    Mixxx.ControlProxy {
        id: bpmControl
        group: root.group
        key: "bpm"
    }

    Mixxx.ControlProxy {
        id: rateControl
        group: root.group
        key: "rate"
    }

    Mixxx.ControlProxy {
        id: durationControl
        group: root.group
        key: "duration"
    }

    Mixxx.ControlProxy {
        id: playPositionControl
        group: root.group
        key: "playposition"
    }

    Mixxx.ControlProxy {
        id: keyControl
        group: root.group
        key: "visual_key"
    }

    Mixxx.ControlProxy {
        id: pflControl
        group: root.group
        key: "pfl"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        spacing: 8

        // ================= 1. DECK BADGE =================
        Rectangle {
            Layout.preferredHeight: root.minimized ? 22 : 36
            Layout.preferredWidth: (root.width < 360) ? 48 : (root.minimized ? 54 : 62)
            radius: 4
            color: root.deckBgColor
            border.color: root.deckColor
            border.width: 1

            Text {
                anchors.centerIn: parent
                font.bold: true
                font.pixelSize: (root.width < 360 || root.minimized) ? 9 : 11
                font.letterSpacing: 1
                color: "#ffffff"
                text: root.deckName
            }
        }

        // ================= 2. COVER ART THUMBNAIL =================
        Rectangle {
            Layout.preferredHeight: root.minimized ? 22 : 36
            Layout.preferredWidth: root.minimized ? 22 : 36
            radius: 4
            color: "#222734"
            clip: true
            visible: !root.minimized && root.width >= 400

            Image {
                anchors.fill: parent
                asynchronous: true
                source: root.currentTrack?.coverArtUrl || ""
                fillMode: Image.PreserveAspectCrop
                visible: status === Image.Ready
            }

            Text {
                anchors.centerIn: parent
                text: "♫"
                font.pixelSize: 14
                color: "#90a4ae"
                visible: !root.currentTrack?.coverArtUrl
            }
        }

        // ================= 3. TITLE & ARTIST =================
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 1

            Text {
                Layout.fillWidth: true
                text: (root.deckPlayer?.isLoaded && root.currentTrack?.title)
                    ? root.currentTrack.title
                    : qsTr("No track - press LOAD")
                color: root.deckPlayer?.isLoaded ? "#ffffff" : "#90a4ae"
                font.bold: true
                font.pixelSize: (root.width < 360 || root.minimized) ? 10 : 12
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                visible: !root.minimized && root.deckPlayer?.isLoaded && root.width >= 340
                text: (root.currentTrack?.artist ? root.currentTrack.artist : "") +
                      (root.currentTrack?.album ? (" • " + root.currentTrack.album) : "")
                color: root.deckColor
                font.pixelSize: 10
                elide: Text.ElideRight
            }
        }

        // ================= 4. BPM & PITCH PILL (PROMINENT) =================
        // Tapping it opens the beatgrid corrections
        Rectangle {
            Layout.preferredHeight: root.minimized ? 22 : 36
            Layout.preferredWidth: bpmCol.implicitWidth + 10
            radius: 4
            color: bpmMouseArea.pressed ? "#263043" : "#12151c"
            border.color: root.deckColor
            border.width: 1

            ColumnLayout {
                id: bpmCol
                anchors.centerIn: parent
                spacing: 0

                Text {
                    id: bpmText
                    Layout.alignment: Qt.AlignHCenter
                    font.bold: true
                    font.pixelSize: (root.width < 360 || root.minimized) ? 10 : 12
                    color: bpmControl.value > 0 ? "#ffffff" : "#78909c"
                    text: bpmControl.value > 0 ? (bpmControl.value.toFixed(2) + " BPM") : "--.-- BPM"
                }

                Text {
                    visible: !root.minimized && bpmControl.value > 0
                    Layout.alignment: Qt.AlignHCenter
                    font.bold: true
                    font.pixelSize: 9
                    color: root.deckColor
                    text: {
                        var p = rateControl.value * 8.0;
                        return (p > 0 ? "+" : "") + p.toFixed(1) + "%";
                    }
                }
            }

            MouseArea {
                id: bpmMouseArea

                anchors.fill: parent
                enabled: bpmControl.value > 0 && !root.editMode
                onClicked: beatgridPopup.open()
            }
            DeckComponent.BeatgridPopup {
                id: beatgridPopup

                group: root.group
            }
        }

        // ================= 5. KEY PILL =================
        Rectangle {
            visible: !root.minimized && keyControl.value !== "" && keyControl.value !== undefined && root.width >= 360
            Layout.preferredHeight: 28
            Layout.preferredWidth: keyText.implicitWidth + 10
            radius: 4
            color: "#1e2433"
            border.color: "#374151"
            border.width: 1

            Text {
                id: keyText
                anchors.centerIn: parent
                font.bold: true
                font.pixelSize: 10
                color: "#e0e0e0"
                text: keyControl.value ? keyControl.value.toString() : ""
            }
        }

        // ================= 6. TIME REMAINING =================
        // ================= 6. TIME (ELAPSED / TOTAL / REMAINING) =================
        Rectangle {
            id: timeBadge
            property int timeMode: 0 // 0: Elapsed / Total (ej: 0:15 / 3:45), 1: Remaining (-3:30), 2: Elapsed (0:15)
            visible: root.width >= 330
            Layout.preferredHeight: root.minimized ? 22 : 36
            Layout.preferredWidth: Math.max(76, timeText.implicitWidth + 14)
            radius: 4
            color: "#141720"
            border.color: timeMouse.containsMouse ? "#64b5f6" : "#293040"
            border.width: 1

            Text {
                id: timeText
                anchors.centerIn: parent
                font.bold: true
                font.pixelSize: (root.width < 360 || root.minimized) ? 10 : 11
                color: "#e0e6ed"
                text: {
                    var total = durationControl.value || 0;
                    var pos = playPositionControl.value || 0;
                    if (total <= 0) return "--:--";

                    var cur = Math.max(0, Math.min(total, total * pos));
                    var rem = Math.max(0, total - cur);

                    function fmt(sec) {
                        var m = Math.floor(sec / 60);
                        var s = Math.floor(sec % 60);
                        return m + ":" + (s < 10 ? "0" : "") + s;
                    }

                    if (timeBadge.timeMode === 0) {
                        return fmt(cur) + " / " + fmt(total);
                    } else if (timeBadge.timeMode === 1) {
                        return "-" + fmt(rem);
                    } else {
                        return fmt(cur);
                    }
                }
            }

            MouseArea {
                id: timeMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    timeBadge.timeMode = (timeBadge.timeMode + 1) % 3;
                }
            }
        }

        // ================= 7. PFL / HEADPHONE CUE =================
        Rectangle {
            id: pflButton
            Layout.preferredHeight: root.minimized ? 24 : 36
            Layout.preferredWidth: (root.width < 360) ? 36 : 44
            radius: 4
            color: pflControl.value > 0 ? "#ff9800" : (pflMouse.pressed ? "#2a2d36" : "#1a1e27")
            border.color: pflControl.value > 0 ? "#ffe082" : "#323947"
            border.width: 1

            RowLayout {
                anchors.centerIn: parent
                spacing: 2
                Text {
                    text: "🎧"
                    font.pixelSize: (root.width < 360 || root.minimized) ? 11 : 13
                }
                Text {
                    visible: !root.minimized && root.width >= 420
                    text: "CUE"
                    font.bold: true
                    font.pixelSize: 10
                    color: pflControl.value > 0 ? "#000000" : "#b0bec5"
                }
            }

            MouseArea {
                id: pflMouse
                anchors.fill: parent
                onClicked: {
                    pflControl.value = (pflControl.value > 0) ? 0 : 1;
                }
            }
        }

        // ================= 8. LOAD / CARGAR BUTTON =================
        Skin.Button {
            id: loadButton
            Layout.preferredHeight: root.minimized ? 24 : 36
            Layout.preferredWidth: (root.width < 360) ? 58 : (root.minimized ? 66 : 76)
            activeColor: root.isDeck1 ? "#64b5f6" : "#ffb74d"
            backgroundColor: root.isDeck1 ? "#1565c0" : "#e65100"
            normalColor: "#ffffff"
            font.bold: true
            font.pixelSize: (root.width < 360 || root.minimized) ? 9 : 11
            text: (root.width < 360) ? "LOAD" : qsTr("LOAD")

            onClicked: {
                if (typeof root.Window?.window?.openQuickLoad === "function") {
                    root.Window.window.openQuickLoad(root.group);
                } else {
                    root.triggerLoadTrack();
                }
            }
        }
    }
}
