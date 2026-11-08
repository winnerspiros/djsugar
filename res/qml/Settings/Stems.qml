import QtQuick
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import ".." as Skin
import "../Theme"
import "../stems.js" as StemsText

// Separation of tracks into stems (Android)
Category {
    id: root

    label: qsTr("Stems")
    selectedIndex: 0

    // Shown while a measurement or a download fails
    property string message: ""

    Connections {
        target: Mixxx.Stems
        function onDownloadFailed(model, error) {
            root.message = qsTr("The download of %1 failed: %2").arg(StemsText.modelLabel(model)).arg(error);
        }
        function onBenchmarkFinished(model, speed, error) {
            root.message = error === "" ? "" : StemsText.errorText(error);
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: column.implicitHeight + 30
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: column

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 12
            spacing: 12

            // ================= WHAT IT IS =================
            Card {
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 6

                    Text {
                        text: qsTr("🎚 STEMS")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#00e5ff"
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#cfd8dc"
                        font.pixelSize: 11
                        text: qsTr("Splits the tracks you choose into drums, bass, vocals and the rest, so that you can mute or mix each part on the deck. In the library, select a track or open a list and tap 🎚 STEMS. The stems are saved next to each track, in a hidden .stems folder, and they are used whenever you load the track. If its folder can't be written, they stay in the app.")
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#ffb74d"
                        font.pixelSize: 11
                        text: qsTr("⚠ Separating uses a lot of CPU. While you play it may cause audio dropouts: better prepare your tracks before the session.")
                    }
                }
            }

            // ================= MODELS =================
            Card {
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("🧠 MODELS")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#00e5ff"
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#90a4ae"
                        font.pixelSize: 11
                        text: qsTr("Download the models you want to use. The times are for a 5 minute track and depend a lot on the device: tap MEASURE to know yours.")
                    }

                    ModelRow {
                        model: "spleeter"
                        installed: Mixxx.Stems.spleeterInstalled
                        description: qsTr("Fast and light. Good quality, the vocals may sound a bit metallic. Its bass is reinforced with the low end of the track.")
                        ranges: qsTr("1–4 min per track · ~250 MB of memory")
                    }

                    ModelRow {
                        model: "htdemucs"
                        installed: Mixxx.Stems.htdemucsInstalled
                        description: qsTr("The best quality: cleaner vocals and bass. Slow and memory hungry: best to prepare playlists with the charger plugged in and without playing.")
                        ranges: qsTr("8–30 min per track · 2–2.5 GB of memory")
                        warning: Mixxx.Stems.totalMemoryMb > 0 && Mixxx.Stems.totalMemoryMb < 5000
                                ? qsTr("This device has %1 GB of memory: HTDemucs may not fit.").arg((Mixxx.Stems.totalMemoryMb / 1024).toFixed(1))
                                : ""
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: root.message !== ""
                        wrapMode: Text.WordWrap
                        color: "#ef9a9a"
                        font.pixelSize: 11
                        text: root.message
                    }
                }
            }

            // ================= OPTIONS =================
            Card {
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("⚙️ OPTIONS")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#00e5ff"
                    }

                    ChoiceRow {
                        label: qsTr("Model")
                        value: Mixxx.Stems.defaultModel
                        choices: [
                            {value: "ask", text: qsTr("ASK")},
                            {value: "spleeter", text: "SPLEETER"},
                            {value: "htdemucs", text: "HTDEMUCS"}
                        ]
                        onChosen: value => Mixxx.Stems.defaultModel = value
                    }

                    ChoiceRow {
                        label: qsTr("Size of the stem files")
                        hint: qsTr("Per 5 minute track")
                        value: Mixxx.Stems.fileSize
                        choices: [
                            {value: "light", text: qsTr("LIGHT ~18 MB")},
                            {value: "normal", text: qsTr("NORMAL ~24 MB")},
                            {value: "high", text: qsTr("HIGH ~36 MB")}
                        ]
                        onChosen: value => Mixxx.Stems.fileSize = value
                    }

                    ChoiceRow {
                        label: qsTr("While a deck plays")
                        hint: Mixxx.Stems.whilePlaying === "pause"
                                ? qsTr("The queue waits until the decks stop")
                                : qsTr("Separates with half the CPU, more slowly")
                        value: Mixxx.Stems.whilePlaying
                        choices: [
                            {value: "slow", text: qsTr("GO ON SLOWER")},
                            {value: "pause", text: qsTr("PAUSE")}
                        ]
                        onChosen: value => Mixxx.Stems.whilePlaying = value
                    }

                    ChoiceRow {
                        label: qsTr("Only while charging")
                        value: Mixxx.Stems.chargingOnly ? "on" : "off"
                        choices: [
                            {value: "on", text: qsTr("YES")},
                            {value: "off", text: qsTr("NO")}
                        ]
                        onChosen: value => Mixxx.Stems.chargingOnly = value === "on"
                    }
                }
            }

            // ================= QUEUE =================
            Card {
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("📋 QUEUE")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#00e5ff"
                    }

                    Skin.StemsQueue {
                        Layout.fillWidth: true
                    }
                }
            }

            // ================= STORAGE =================
            Card {
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 8

                    Text {
                        text: qsTr("💾 STORAGE")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#00e5ff"
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Text {
                            id: storedText

                            property int storedMb: Mixxx.Stems.storedStemsMb()

                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: "#cfd8dc"
                            font.pixelSize: 11
                            text: qsTr("Stems kept in the app (tracks of folders that can't be written): %1 MB. The stems next to your tracks are deleted from the library, with 🎚 STEMS.").arg(storedMb)

                            Connections {
                                target: Mixxx.Stems
                                function onTrackFinished() {
                                    storedText.storedMb = Mixxx.Stems.storedStemsMb();
                                }
                            }
                        }

                        Skin.Button {
                            implicitWidth: 110
                            implicitHeight: 34
                            enabled: storedText.storedMb > 0
                            activeColor: "#ffffff"
                            backgroundColor: "#5f1e1e"
                            normalColor: "#ffffff"
                            font.bold: true
                            text: qsTr("DELETE")
                            onClicked: {
                                Mixxx.Stems.deleteStoredStems();
                                storedText.storedMb = Mixxx.Stems.storedStemsMb();
                            }
                        }
                    }
                }
            }
        }
    }

    // A card holding one layout, anchored to fill it with margins of 12
    component Card: Rectangle {
        default property alias content: holder.data

        Layout.fillWidth: true
        implicitHeight: (holder.children.length > 0 ? holder.children[0].implicitHeight : 0) + 24
        color: "#181b24"
        radius: 8
        border.color: "#282d3b"
        border.width: 1

        Item {
            id: holder

            anchors.fill: parent
        }
    }

    // A row of buttons to choose one value
    component ChoiceRow: RowLayout {
        id: choiceRow

        property string label
        property string hint: ""
        property string value
        property var choices: []

        signal chosen(string value)

        Layout.fillWidth: true
        spacing: 8

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Text {
                Layout.fillWidth: true
                text: choiceRow.label
                color: "#ffffff"
                font.pixelSize: 12
            }
            Text {
                Layout.fillWidth: true
                visible: choiceRow.hint !== ""
                text: choiceRow.hint
                color: "#78909c"
                font.pixelSize: 10
                wrapMode: Text.WordWrap
            }
        }

        Repeater {
            model: choiceRow.choices

            Skin.Button {
                required property var modelData

                implicitWidth: Math.max(80, choiceMetrics.width + 24)
                implicitHeight: 32
                activeColor: "#ffffff"
                backgroundColor: choiceRow.value === modelData.value ? "#1e3a5f" : "#222733"
                normalColor: choiceRow.value === modelData.value ? "#ffffff" : "#90a4ae"
                highlight: choiceRow.value === modelData.value
                font.bold: true
                font.pixelSize: 11
                text: modelData.text
                onClicked: choiceRow.chosen(modelData.value)

                TextMetrics {
                    id: choiceMetrics

                    font.bold: true
                    font.pixelSize: 11
                    text: modelData.text
                }
            }
        }
    }

    // A model: what it is, its download and its measured speed
    component ModelRow: Rectangle {
        id: modelRow

        property string model
        property bool installed
        property string description
        property string ranges
        property string warning: ""
        readonly property bool downloading: Mixxx.Stems.downloadingModel === model
        property double speed: Mixxx.Stems.measuredSpeed(model)

        Layout.fillWidth: true
        implicitHeight: modelLayout.implicitHeight + 16
        radius: 6
        color: "#1a1e27"
        border.color: installed ? "#2e7d32" : "#262c38"
        border.width: 1

        Connections {
            target: Mixxx.Stems
            function onBenchmarkFinished(model, speed, error) {
                if (model === modelRow.model && error === "")
                    modelRow.speed = speed;
            }
        }

        RowLayout {
            id: modelLayout

            anchors.fill: parent
            anchors.margins: 8
            spacing: 10

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Text {
                    text: StemsText.modelLabel(modelRow.model) + (modelRow.installed ? "  ✓" : "")
                    color: modelRow.installed ? "#81c784" : "#ffffff"
                    font.bold: true
                    font.pixelSize: 12
                }
                Text {
                    Layout.fillWidth: true
                    text: modelRow.description
                    color: "#cfd8dc"
                    font.pixelSize: 10
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    text: modelRow.ranges
                    color: "#90a4ae"
                    font.pixelSize: 10
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    visible: modelRow.speed > 0
                    text: qsTr("This device: ~%1 per 5 minute track").arg(StemsText.trackTime(modelRow.speed))
                    color: "#4fc3f7"
                    font.pixelSize: 10
                    font.bold: true
                }
                Text {
                    Layout.fillWidth: true
                    visible: modelRow.warning !== ""
                    text: modelRow.warning
                    color: "#ffb74d"
                    font.pixelSize: 10
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    visible: modelRow.downloading
                    text: qsTr("Downloading… %1%").arg(Math.round(Mixxx.Stems.downloadProgress * 100))
                    color: "#4fc3f7"
                    font.pixelSize: 10
                }
            }

            Skin.Button {
                visible: modelRow.installed
                implicitWidth: 100
                implicitHeight: 34
                enabled: !Mixxx.Stems.benchmarking && Mixxx.Stems.pendingCount === 0
                activeColor: "#ffffff"
                backgroundColor: "#2e3440"
                normalColor: "#ffffff"
                font.bold: true
                text: Mixxx.Stems.benchmarking ? qsTr("MEASURING…") : qsTr("⏱ MEASURE")
                onClicked: {
                    root.message = "";
                    Mixxx.Stems.benchmark(modelRow.model);
                }
            }

            Skin.Button {
                implicitWidth: 130
                implicitHeight: 34
                enabled: modelRow.downloading || Mixxx.Stems.downloadingModel === ""
                activeColor: "#ffffff"
                backgroundColor: modelRow.installed || modelRow.downloading ? "#5f1e1e" : "#1e5f3a"
                normalColor: "#ffffff"
                font.bold: true
                text: {
                    if (modelRow.downloading)
                        return qsTr("CANCEL");
                    if (modelRow.installed)
                        return qsTr("DELETE");
                    return qsTr("⬇ %1 MB").arg(Mixxx.Stems.modelSizeMb(modelRow.model));
                }
                onClicked: {
                    root.message = "";
                    if (modelRow.downloading)
                        Mixxx.Stems.cancelDownload();
                    else if (modelRow.installed)
                        Mixxx.Stems.removeModel(modelRow.model);
                    else
                        Mixxx.Stems.downloadModel(modelRow.model);
                }
            }
        }
    }
}
