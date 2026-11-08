import QtQuick
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "." as Skin
import "Theme"
import "stems.js" as StemsText

// The queue of tracks being separated into stems, with its buttons (Android)
ColumnLayout {
    id: root

    spacing: 8

    readonly property var jobs: Mixxx.Stems.jobs
    readonly property bool hasFinished: jobs.some(job => job.state === "done" || job.state === "failed")

    function stageText() {
        switch (Mixxx.Stems.stage) {
        case "decoding":
            return qsTr("Reading the audio");
        case "loading":
            return qsTr("Loading the model");
        case "saving":
            return qsTr("Saving");
        default:
            return qsTr("Separating");
        }
    }

    function holdText() {
        switch (Mixxx.Stems.hold) {
        case "paused":
            return qsTr("Paused");
        case "playing":
            return qsTr("Waiting until the decks stop playing");
        case "charging":
            return qsTr("Waiting for the charger");
        default:
            return "";
        }
    }

    // Status and buttons
    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Text {
            Layout.fillWidth: true
            color: Mixxx.Stems.hold !== "" ? "#ffb74d" : "#cfd8dc"
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            text: {
                if (Mixxx.Stems.pendingCount === 0)
                    return root.jobs.length > 0 ? qsTr("Queue finished") : qsTr("No tracks in the queue. Choose a track or a list in the library and tap 🎚 STEMS.");
                if (Mixxx.Stems.hold !== "")
                    return qsTr("%n track(s) waiting", "", Mixxx.Stems.pendingCount) + " · " + root.holdText();
                return qsTr("%n track(s) waiting", "", Mixxx.Stems.pendingCount);
            }
        }

        Skin.Button {
            visible: Mixxx.Stems.pendingCount > 0
            implicitWidth: 110
            implicitHeight: 34
            activeColor: "#ffffff"
            backgroundColor: Mixxx.Stems.hold === "paused" ? "#1e5f3a" : "#2e3440"
            normalColor: "#ffffff"
            font.bold: true
            text: Mixxx.Stems.hold === "paused" ? qsTr("▶ RESUME") : qsTr("⏸ PAUSE")
            onClicked: Mixxx.Stems.hold === "paused" ? Mixxx.Stems.resume() : Mixxx.Stems.pause()
        }

        Skin.Button {
            visible: Mixxx.Stems.pendingCount > 0
            implicitWidth: 110
            implicitHeight: 34
            activeColor: "#ffffff"
            backgroundColor: "#5f1e1e"
            normalColor: "#ffffff"
            font.bold: true
            text: qsTr("CANCEL ALL")
            onClicked: Mixxx.Stems.cancelAll()
        }

        Skin.Button {
            visible: root.hasFinished
            implicitWidth: 160
            implicitHeight: 34
            activeColor: "#ffffff"
            backgroundColor: "#2e3440"
            normalColor: "#ffffff"
            font.bold: true
            text: qsTr("CLEAR DONE")
            onClicked: Mixxx.Stems.clearFinished()
        }
    }

    // The tracks
    Repeater {
        model: root.jobs

        Rectangle {
            id: row

            required property var modelData
            readonly property bool running: modelData.state === "running"

            Layout.fillWidth: true
            implicitHeight: rowLayout.implicitHeight + 12
            radius: 6
            color: running ? "#1e2a3a" : "#1a1e27"
            border.color: running ? "#42a5f5" : "#262c38"
            border.width: 1

            // Progress of the running track
            Rectangle {
                visible: row.running
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                height: 3
                radius: 2
                width: parent.width * Mixxx.Stems.progress
                color: "#42a5f5"
            }

            RowLayout {
                id: rowLayout

                anchors.fill: parent
                anchors.margins: 6
                spacing: 8

                Text {
                    text: {
                        switch (row.modelData.state) {
                        case "running":
                            return "⏳";
                        case "done":
                            return "✅";
                        case "failed":
                            return row.modelData.error === "canceled" ? "⏹" : "⚠️";
                        default:
                            return "🕒";
                        }
                    }
                    font.pixelSize: 14
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Text {
                        Layout.fillWidth: true
                        text: row.modelData.name
                        color: "#ffffff"
                        font.pixelSize: 12
                        font.bold: row.running
                        elide: Text.ElideMiddle
                    }
                    Text {
                        Layout.fillWidth: true
                        color: row.modelData.state === "failed" ? "#ef9a9a" : "#90a4ae"
                        font.pixelSize: 10
                        elide: Text.ElideRight
                        text: {
                            const model = StemsText.modelLabel(row.modelData.model);
                            switch (row.modelData.state) {
                            case "running": {
                                if (Mixxx.Stems.hold !== "")
                                    return `${model} · ${root.holdText()}`;
                                let text = `${model} · ${root.stageText()} ${Math.round(Mixxx.Stems.progress * 100)}%`;
                                if (Mixxx.Stems.secondsLeft >= 0)
                                    text += " · " + qsTr("%1 left").arg(StemsText.duration(Mixxx.Stems.secondsLeft));
                                return text;
                            }
                            case "done":
                                return `${model} · ${qsTr("Ready: reload the track to use its stems")}`;
                            case "failed":
                                return StemsText.errorText(row.modelData.error);
                            default:
                                return model;
                            }
                        }
                    }
                }

                Skin.Button {
                    visible: row.modelData.state === "queued" || row.running
                    implicitWidth: 36
                    implicitHeight: 30
                    activeColor: "#ffffff"
                    backgroundColor: "#2e3440"
                    normalColor: "#ef9a9a"
                    font.bold: true
                    font.pixelSize: 16
                    text: "×"
                    onClicked: Mixxx.Stems.cancel(row.modelData.location)
                }
            }
        }
    }
}
