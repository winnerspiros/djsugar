import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import ".." as Skin
import "../Theme"
import "../stems.js" as StemsText

// Separates the selected track or all tracks of a list into stems (Android)
Popup {
    id: root

    // The list of the library the tracks come from
    property var trackList
    property url trackUrl
    // All tracks of the list instead of the selected one
    property bool allTracks: false
    property var stats: ({total: 0, withStems: 0})
    property string model: ""
    // Separate again the tracks that have stems
    property bool replace: false
    property string message: ""
    // Free memory of the device when the popup opened, in MB
    property int freeMemoryMb: -1

    readonly property bool trackHasStems: !allTracks && Mixxx.Stems.jobs && Mixxx.Stems.hasStems(trackUrl)
    readonly property int stemTracks: allTracks ? stats.withStems : (trackHasStems ? 1 : 0)
    readonly property int tracksToSeparate: {
        if (allTracks)
            return replace ? stats.total : stats.total - stats.withStems;
        return trackHasStems && !replace ? 0 : 1;
    }
    readonly property bool modelInstalled: model === "spleeter" ? Mixxx.Stems.spleeterInstalled : model === "htdemucs" ? Mixxx.Stems.htdemucsInstalled : false
    readonly property bool anyDeckPlaying: play1.value > 0 || play2.value > 0 || play3.value > 0 || play4.value > 0

    // Without a track URL, all tracks of the list are separated
    function openFor(url) {
        trackUrl = url;
        allTracks = url.toString().length === 0;
        stats = Mixxx.Stems.listStats(trackList);
        replace = false;
        message = "";
        freeMemoryMb = Mixxx.Stems.availableMemoryMb();
        const preferred = Mixxx.Stems.defaultModel;
        if (preferred === "spleeter" || preferred === "htdemucs")
            model = preferred;
        else if (Mixxx.Stems.spleeterInstalled)
            model = "spleeter";
        else if (Mixxx.Stems.htdemucsInstalled)
            model = "htdemucs";
        else
            model = "";
        open();
    }

    function trackName() {
        const urlStr = trackUrl.toString();
        return decodeURIComponent(urlStr.substring(urlStr.lastIndexOf('/') + 1));
    }

    function separate() {
        const added = allTracks
                ? Mixxx.Stems.separateList(trackList, model, replace)
                : Mixxx.Stems.separateTrack(trackUrl, model, replace);
        if (added === 0) {
            message = qsTr("Nothing to separate: the tracks are already in the queue or can't be read.");
            return;
        }
        message = qsTr("%n track(s) added to the queue. Follow the progress in the top bar (🎚) or in Settings → Stems.", "", added);
        closeTimer.start();
    }

    anchors.centerIn: parent
    width: Math.min(560, parent ? parent.width - 20 : 560)
    height: Math.min(contentLayout.implicitHeight + 32, parent ? parent.height - 20 : 600)
    modal: true
    padding: 16

    background: Rectangle {
        color: "#14171f"
        radius: 10
        border.color: "#2a3648"
        border.width: 1
    }

    Mixxx.ControlProxy {
        id: play1
        group: "[Channel1]"
        key: "play"
    }
    Mixxx.ControlProxy {
        id: play2
        group: "[Channel2]"
        key: "play"
    }
    Mixxx.ControlProxy {
        id: play3
        group: "[Channel3]"
        key: "play"
    }
    Mixxx.ControlProxy {
        id: play4
        group: "[Channel4]"
        key: "play"
    }

    Timer {
        id: closeTimer
        interval: 2500
        onTriggered: root.close()
    }

    Flickable {
        anchors.fill: parent
        contentHeight: contentLayout.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: contentLayout

            width: parent.width
            spacing: 10

            Text {
                Layout.fillWidth: true
                text: qsTr("🎚 Separate into stems")
                color: "#ffffff"
                font.bold: true
                font.pixelSize: 15
            }

            // What: the selected track or the whole list
            RowLayout {
                Layout.fillWidth: true
                visible: root.trackUrl.toString().length > 0
                spacing: 8

                Skin.Button {
                    Layout.fillWidth: true
                    implicitHeight: 34
                    activeColor: "#ffffff"
                    highlight: !root.allTracks
                    backgroundColor: !root.allTracks ? "#1e3a5f" : "#222733"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("SELECTED TRACK")
                    onClicked: root.allTracks = false
                }
                Skin.Button {
                    Layout.fillWidth: true
                    implicitHeight: 34
                    activeColor: "#ffffff"
                    highlight: root.allTracks
                    backgroundColor: root.allTracks ? "#1e3a5f" : "#222733"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("WHOLE LIST")
                    onClicked: root.allTracks = true
                }
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: "#cfd8dc"
                font.pixelSize: 12
                text: {
                    if (!root.allTracks)
                        return root.trackHasStems ? qsTr("%1 · already has stems").arg(root.trackName()) : root.trackName();
                    return qsTr("%n track(s) in the list", "", root.stats.total) + ", " + qsTr("%n with stems", "", root.stats.withStems);
                }
            }

            // The model
            Text {
                text: qsTr("Model")
                color: "#90a4ae"
                font.pixelSize: 11
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                ModelChoice {
                    name: "spleeter"
                    installed: Mixxx.Stems.spleeterInstalled
                    summary: qsTr("Fast and light")
                    ranges: qsTr("1–4 min per track")
                }
                ModelChoice {
                    name: "htdemucs"
                    installed: Mixxx.Stems.htdemucsInstalled
                    summary: qsTr("Best quality, slow")
                    ranges: qsTr("8–30 min per track · 2.5 GB of memory")
                }
            }

            Text {
                Layout.fillWidth: true
                visible: root.model === "htdemucs" && root.freeMemoryMb >= 0 && root.freeMemoryMb < 2500
                wrapMode: Text.WordWrap
                color: "#ffb74d"
                font.pixelSize: 11
                text: qsTr("⚠ Only %1 GB of memory is free now: HTDemucs may run out of memory. Close other apps or use Spleeter.").arg((root.freeMemoryMb / 1024).toFixed(1))
            }

            // Warning while playing
            Rectangle {
                Layout.fillWidth: true
                visible: root.anyDeckPlaying
                implicitHeight: playingText.implicitHeight + 16
                radius: 6
                color: "#3a2a12"
                border.color: "#ff9800"
                border.width: 1

                Text {
                    id: playingText

                    anchors.fill: parent
                    anchors.margins: 8
                    wrapMode: Text.WordWrap
                    color: "#ffcc80"
                    font.pixelSize: 11
                    text: Mixxx.Stems.whilePlaying === "pause"
                            ? qsTr("⚠ A deck is playing: the queue will wait until you stop the decks.")
                            : qsTr("⚠ A deck is playing: separating now may cause audio dropouts. It will go slower to leave CPU to the audio. You can make it wait in Settings → Stems.")
                }
            }

            // Separate again
            RowLayout {
                Layout.fillWidth: true
                visible: root.stemTracks > 0
                spacing: 8

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: qsTr("Separate again the tracks that have stems")
                    color: "#cfd8dc"
                    font.pixelSize: 11
                }
                Skin.Button {
                    implicitWidth: 70
                    implicitHeight: 30
                    activeColor: "#ffffff"
                    highlight: root.replace
                    backgroundColor: root.replace ? "#1e3a5f" : "#222733"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("YES")
                    onClicked: root.replace = true
                }
                Skin.Button {
                    implicitWidth: 70
                    implicitHeight: 30
                    activeColor: "#ffffff"
                    highlight: !root.replace
                    backgroundColor: !root.replace ? "#1e3a5f" : "#222733"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("NO")
                    onClicked: root.replace = false
                }
            }

            Text {
                Layout.fillWidth: true
                visible: root.message !== ""
                wrapMode: Text.WordWrap
                color: "#81c784"
                font.pixelSize: 12
                text: root.message
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Skin.Button {
                    visible: root.stemTracks > 0
                    implicitWidth: 150
                    implicitHeight: 38
                    activeColor: "#ffffff"
                    backgroundColor: "#5f1e1e"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: root.allTracks ? qsTr("🗑 DELETE STEMS (%1)").arg(root.stemTracks) : qsTr("🗑 DELETE STEMS")
                    onClicked: {
                        const deleted = root.allTracks
                                ? Mixxx.Stems.deleteListStems(root.trackList)
                                : (Mixxx.Stems.deleteTrackStems(root.trackUrl) ? 1 : 0);
                        root.stats = Mixxx.Stems.listStats(root.trackList);
                        root.message = qsTr("Deleted the stems of %n track(s).", "", deleted);
                    }
                }

                Item {
                    Layout.fillWidth: true
                }

                Skin.Button {
                    implicitWidth: 100
                    implicitHeight: 38
                    activeColor: "#ffffff"
                    backgroundColor: "#2e3440"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("CLOSE")
                    onClicked: root.close()
                }

                Skin.Button {
                    implicitWidth: 170
                    implicitHeight: 38
                    enabled: root.modelInstalled && root.tracksToSeparate > 0 && !closeTimer.running
                    activeColor: "#ffffff"
                    backgroundColor: enabled ? "#1e5f3a" : "#262930"
                    normalColor: enabled ? "#ffffff" : "#78909c"
                    font.bold: true
                    text: root.allTracks ? qsTr("🎚 SEPARATE %1").arg(root.tracksToSeparate) : qsTr("🎚 SEPARATE")
                    onClicked: root.separate()
                }
            }
        }
    }

    // A model to choose, with its download if it is missing
    component ModelChoice: Rectangle {
        id: choice

        property string name
        property bool installed
        property string summary
        property string ranges
        readonly property bool chosen: root.model === name
        // Read again on each opening, after a measurement
        readonly property double speed: root.opened ? Mixxx.Stems.measuredSpeed(name) : 0
        readonly property bool downloading: Mixxx.Stems.downloadingModel === name

        Layout.fillWidth: true
        Layout.preferredWidth: 1
        implicitHeight: choiceLayout.implicitHeight + 16
        radius: 8
        color: chosen ? "#1e3a5f" : "#1a1e27"
        border.color: chosen ? "#42a5f5" : "#2a3648"
        border.width: chosen ? 2 : 1

        MouseArea {
            anchors.fill: parent
            onClicked: root.model = choice.name
        }

        ColumnLayout {
            id: choiceLayout

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 8
            spacing: 3

            Text {
                text: StemsText.modelLabel(choice.name) + (choice.chosen ? "  ✓" : "")
                color: "#ffffff"
                font.bold: true
                font.pixelSize: 12
            }
            Text {
                Layout.fillWidth: true
                text: choice.summary
                color: "#cfd8dc"
                font.pixelSize: 10
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                text: choice.speed > 0 ? qsTr("This device: ~%1 per 5 minute track").arg(StemsText.trackTime(choice.speed)) : choice.ranges
                color: choice.speed > 0 ? "#4fc3f7" : "#90a4ae"
                font.pixelSize: 10
                wrapMode: Text.WordWrap
            }
            Skin.Button {
                Layout.fillWidth: true
                visible: !choice.installed
                implicitHeight: 30
                enabled: choice.downloading || Mixxx.Stems.downloadingModel === ""
                activeColor: "#ffffff"
                backgroundColor: choice.downloading ? "#5f1e1e" : "#1e5f3a"
                normalColor: "#ffffff"
                font.bold: true
                text: choice.downloading
                        ? qsTr("%1% · CANCEL").arg(Math.round(Mixxx.Stems.downloadProgress * 100))
                        : qsTr("⬇ DOWNLOAD %1 MB").arg(Mixxx.Stems.modelSizeMb(choice.name))
                onClicked: {
                    root.model = choice.name;
                    if (choice.downloading)
                        Mixxx.Stems.cancelDownload();
                    else
                        Mixxx.Stems.downloadModel(choice.name);
                }
            }
        }
    }
}
