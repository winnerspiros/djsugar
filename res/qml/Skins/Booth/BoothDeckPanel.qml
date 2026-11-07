import "../.." as Skin
import "../../Deck" as DeckComponent
import Mixxx 1.0 as Mixxx
import QtQuick
import "../../Theme"

// The deck display of the booth skin, in the style of the lower half of a
// booth player screen: the title bar, the deck number, the remaining time,
// the tempo, the BPM and the overview of the whole track. The transport and
// the hot cues are left to the controller; touching the display only
// changes what it shows (remaining/elapsed time), the tempo settings
// (master tempo, sync, leader) and opens the beatgrid panel (BPM).
Rectangle {
    id: root

    required property string group
    required property int deckNumber
    // Only the title bar and the overview, while browsing the library
    property bool slim: false

    readonly property var deckPlayer: Mixxx.PlayerManager.getPlayer(group)
    readonly property var currentTrack: deckPlayer?.currentTrack
    readonly property bool loaded: trackLoadedControl.value > 0
    readonly property bool playing: playControl.value > 0
    readonly property bool leader: syncLeaderControl.value > 0
    property bool showElapsed: false
    readonly property real seconds: {
        const position = Math.max(0, Math.min(1, positionControl.value)) * durationControl.value;
        return root.showElapsed ? position : durationControl.value - position;
    }
    // Text sizes follow the panel size
    readonly property real unit: Math.max(9, Math.min(height / 11, width / 30))

    function pad(number, digits) {
        return ("000" + number).slice(-digits);
    }

    color: Theme.backgroundColor
    border.color: root.playing ? Theme.lightGray2 : Theme.midGray3
    border.width: 1

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
        id: durationControl

        group: root.group
        key: "duration"
    }
    Mixxx.ControlProxy {
        id: positionControl

        group: root.group
        key: "playposition"
        updateInterval: 100
    }
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
        id: rateRangeControl

        group: root.group
        key: "rateRange"
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
        id: keylockControl

        group: root.group
        key: "keylock"
    }

    // A small grey caption, like "REMAIN" or "TEMPO"
    component Caption: Text {
        color: Theme.lightGray3
        font.pixelSize: Math.round(root.unit * 0.95)
    }
    // A small outlined switch, where a player has A.HOT CUE / AUTO CUE
    component Chip: Rectangle {
        id: chip

        required property string label
        property bool on: false
        property color onColor: Theme.warningColor
        signal clicked
        signal pressAndHold

        border.color: on ? onColor : Theme.lightGray3
        border.width: 1
        color: "transparent"
        height: Math.round(root.unit * 1.8)
        width: Math.round(root.unit * 6.6)

        Text {
            anchors.fill: parent
            color: chip.on ? chip.onColor : Theme.lightGray2
            fontSizeMode: Text.HorizontalFit
            font.pixelSize: Math.round(root.unit * 0.95)
            horizontalAlignment: Text.AlignHCenter
            minimumPixelSize: 7
            text: chip.label
            verticalAlignment: Text.AlignVCenter
        }
        MouseArea {
            anchors.fill: parent
            anchors.margins: -3
            onClicked: chip.clicked()
            onPressAndHold: chip.pressAndHold()
        }
    }

    // ---- Title bar ----
    Rectangle {
        id: titleBar

        readonly property real ytDeckProgress: {
            const _ = Mixxx.YouTube.deckDownloadProgress;
            return Mixxx.YouTube.deckProgress(root.group);
        }
        readonly property string ytDeckTitle: {
            const _ = Mixxx.YouTube.deckDownloadProgress;
            return Mixxx.YouTube.deckDownloadingTitle(root.group);
        }

        anchors.left: parent.left
        anchors.leftMargin: 1
        anchors.right: parent.right
        anchors.rightMargin: 1
        anchors.top: parent.top
        anchors.topMargin: 1
        color: Theme.darkGray3
        height: Math.round(root.unit * 2.3)
        clip: true

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            visible: titleBar.ytDeckProgress >= 0
            width: parent.width * Math.max(0.04, Math.min(1.0, titleBar.ytDeckProgress))
            color: "#2ae63380"
            Behavior on width {
                NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            color: root.deckNumber === 1 ? Theme.libraryDeck1Background : Theme.libraryDeck2Background
            height: 2
        }
        Column {
            id: deckLabel

            anchors.left: parent.left
            anchors.leftMargin: 6
            anchors.verticalCenter: parent.verticalCenter
            spacing: -Math.round(root.unit * 0.2)

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                color: Theme.lightGray3
                font.pixelSize: Math.round(root.unit * 0.62)
                text: qsTr("DECK")
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                color: root.playing ? Theme.warningColor : Theme.buttonActiveColor
                font.bold: true
                font.pixelSize: Math.round(root.unit * 1.0)
                text: root.playing ? `((${root.deckNumber}))` : root.deckNumber
            }
        }
        Text {
            anchors.left: deckLabel.right
            anchors.leftMargin: 10
            anchors.right: keyLabel.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            color: titleBar.ytDeckProgress >= 0 ? "#ff99ca" : Theme.buttonActiveColor
            elide: Text.ElideRight
            font.pixelSize: Math.round(root.unit * 1.45)
            text: {
                if (titleBar.ytDeckProgress >= 0) {
                    const pct = Math.round(titleBar.ytDeckProgress * 100);
                    return qsTr("⬇ Downloading %1% · %2").arg(pct).arg(titleBar.ytDeckTitle);
                }
                return root.loaded && root.currentTrack ? "♪ " + root.currentTrack.title : "";
            }
        }
        Text {
            id: keyLabel

            anchors.right: parent.right
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.buttonActiveColor
            font.pixelSize: Math.round(root.unit * 1.3)
            text: root.loaded ? (root.currentTrack?.keyText ?? "") : ""
        }

        // Linear progress bar across bottom of deck song box when downloading
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 3
            visible: titleBar.ytDeckProgress >= 0
            color: Theme.darkGray2

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * Math.max(0.04, Math.min(1.0, titleBar.ytDeckProgress))
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop { position: 0.0; color: Theme.accentColor }
                    GradientStop { position: 0.55; color: "#ff99ca" }
                    GradientStop { position: 1.0; color: Theme.blue }
                }
                Behavior on width {
                    NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
                }
            }
        }
    }

    // ---- Remaining time, tempo and BPM ----
    Item {
        id: info

        anchors.left: parent.left
        anchors.leftMargin: 6
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.top: titleBar.bottom
        anchors.topMargin: Math.round(root.unit * 0.5)
        height: Math.round(root.unit * 4.4)
        visible: !root.slim

        Column {
            id: chips

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Math.round(root.unit * 0.5)

            Chip {
                label: qsTr("MASTER TEMPO")
                on: keylockControl.value > 0
                onClicked: keylockControl.value = keylockControl.value > 0 ? 0 : 1
            }
            // A long press makes this deck the leader (MASTER)
            Chip {
                label: "SYNC"
                on: syncControl.value > 0
                onColor: Theme.libraryDeck1Background
                onClicked: syncControl.value = syncControl.value > 0 ? 0 : 1
                onPressAndHold: syncLeaderControl.value = root.leader ? 0 : 1
            }
        }

        // Tap: remaining or elapsed time
        Item {
            id: time

            anchors.bottom: parent.bottom
            anchors.left: chips.right
            anchors.leftMargin: Math.round(root.unit * 0.8)
            anchors.right: tempo.left
            anchors.rightMargin: 6
            anchors.top: parent.top

            Row {
                anchors.left: parent.left
                anchors.top: parent.top
                spacing: 3

                Caption {
                    color: root.showElapsed ? "Theme.midGray3" : "#d8dadd"
                    text: "• " + qsTr("REMAIN")
                }
                Caption {
                    text: "/"
                }
                Caption {
                    color: root.showElapsed ? "#d8dadd" : "Theme.midGray3"
                    text: qsTr("TIME")
                }
            }
            Row {
                id: timeDigits

                // Shrinks to fit narrow panels instead of running out
                readonly property real fit: Math.min(1, time.width / Math.max(1, implicitWidth))

                anchors.bottom: parent.bottom
                anchors.left: parent.left
                scale: fit
                transformOrigin: Item.BottomLeft

                Text {
                    id: minutesSeconds

                    color: root.loaded ? "#ffffff" : "Theme.midGray3"
                    font.pixelSize: Math.round(root.unit * 3.4)
                    text: {
                        const total = Math.floor(Math.max(0, root.seconds));
                        return root.pad(Math.floor(total / 60), 2) + ":" + root.pad(total % 60, 2);
                    }
                }
                Text {
                    anchors.baseline: minutesSeconds.baseline
                    color: minutesSeconds.color
                    font.pixelSize: Math.round(root.unit * 1.7)
                    text: "." + root.pad(Math.floor((Math.max(0, root.seconds) % 1) * 1000), 3)
                }
            }
            MouseArea {
                anchors.fill: parent
                onClicked: root.showElapsed = !root.showElapsed
            }
        }

        // Tempo: range and pitch
        Item {
            id: tempo

            anchors.bottom: parent.bottom
            anchors.right: bpmBox.left
            anchors.rightMargin: Math.round(root.unit * 0.8)
            anchors.top: parent.top
            width: Math.round(root.unit * 6.2)

            Caption {
                anchors.left: parent.left
                anchors.top: parent.top
                text: qsTr("TEMPO")
            }
            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                color: Theme.libraryDeck2Background
                height: rangeText.implicitHeight + 2
                width: rangeText.implicitWidth + 8

                Text {
                    id: rangeText

                    anchors.centerIn: parent
                    color: Theme.buttonActiveColor
                    font.pixelSize: Math.round(root.unit * 0.95)
                    text: Math.round(rateRangeControl.value * 100) + "%"
                }
            }
            Text {
                anchors.bottom: parent.bottom
                anchors.right: parent.right
                color: root.loaded ? "#d8dadd" : "Theme.midGray3"
                font.pixelSize: Math.round(root.unit * 1.45)
                text: {
                    const percent = rateControl.value * rateRangeControl.value * 100;
                    return (percent >= 0.005 ? "+" : "") + percent.toFixed(2) + "%";
                }
            }
        }

        // BPM, with MASTER for the sync leader. Tap: beatgrid panel
        Rectangle {
            id: bpmBox

            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.top: parent.top
            border.color: root.leader ? Theme.warningColor : Theme.lightGray2
            border.width: 1
            color: "transparent"
            width: Math.max(Math.round(root.unit * 6.2), bpmCaption.implicitWidth + masterText.implicitWidth + 12)

            Row {
                anchors.left: parent.left
                anchors.leftMargin: 1
                anchors.top: parent.top
                anchors.topMargin: 1

                Caption {
                    id: bpmCaption

                    color: Theme.lightGray2
                    leftPadding: 4
                    rightPadding: 4
                    text: "BPM"
                }
                Rectangle {
                    color: Theme.warningColor
                    height: bpmCaption.implicitHeight
                    visible: root.leader
                    width: masterText.implicitWidth + 8

                    Text {
                        id: masterText

                        anchors.centerIn: parent
                        color: Theme.backgroundColor
                        font.pixelSize: Math.round(root.unit * 0.95)
                        text: qsTr("MASTER")
                    }
                }
            }
            Row {
                anchors.bottom: parent.bottom
                anchors.right: parent.right
                anchors.rightMargin: 5

                Text {
                    id: bpmWhole

                    color: root.leader ? "#ff6a13" : (root.loaded ? "#ffffff" : "Theme.midGray3")
                    font.pixelSize: Math.round(root.unit * 2.6)
                    text: root.loaded ? Math.floor(bpmControl.value) : "--"
                }
                Text {
                    anchors.baseline: bpmWhole.baseline
                    color: bpmWhole.color
                    font.pixelSize: Math.round(root.unit * 1.5)
                    text: root.loaded ? "." + Math.floor((bpmControl.value % 1) * 10) : ".-"
                }
            }
            MouseArea {
                anchors.fill: parent
                onClicked: beatgridPopup.open()
            }
            DeckComponent.BeatgridPopup {
                id: beatgridPopup

                group: root.group
            }
        }
    }

    // ---- Overview of the whole track, with its cue points ----
    Skin.WaveformOverview {
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 3
        anchors.left: parent.left
        anchors.leftMargin: 4
        anchors.right: parent.right
        anchors.rightMargin: 4
        anchors.top: root.slim ? titleBar.bottom : info.bottom
        anchors.topMargin: 4
        group: root.group
    }
}
