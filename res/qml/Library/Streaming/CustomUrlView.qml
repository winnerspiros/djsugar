import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import "../.." as Skin

Item {
    id: root

    property string targetDeck: "[Channel1]"
    property string statusMessage: qsTr("Enter a direct audio link (MP3, WAV, FLAC, stream)")
    property bool isDownloading: false

    Connections {
        target: Mixxx.StreamingManager

        function onDownloadProgress(trackId, progress, bytesReceived, bytesTotal) {
            root.statusMessage = qsTr("Downloading... %1% (%2 MB)").arg(Math.round(progress * 100)).arg((bytesReceived / 1048576).toFixed(1));
        }

        function onDownloadFinished(trackId, localPath, targetDeck, autoPlay) {
            root.isDownloading = false;
            // QmlStreamingProxy already loaded the track safely into targetDeck in C++
            root.statusMessage = qsTr("✔ Stream downloaded and loaded into %1").arg(targetDeck === "[Channel1]" ? "DECK 1" : "DECK 2");
        }

        function onDownloadFailed(trackId, errorMessage) {
            root.isDownloading = false;
            root.statusMessage = qsTr("❌ Stream error: %1").arg(errorMessage);
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 12

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 52
            color: "#1f1b0a"
            border.color: "#ffb30044"
            border.width: 1
            radius: 4

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                spacing: 10

                Text {
                    text: qsTr("🔗 DIRECT URL / WEB STREAMING")
                    color: Theme.yellow
                    font.bold: true
                    font.pixelSize: 14
                    font.letterSpacing: 1
                }

                Text {
                    Layout.fillWidth: true
                    text: root.statusMessage
                    color: Theme.yellow
                    font.pixelSize: 11
                    elide: Text.ElideRight
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 110
            color: Theme.darkGray4
            border.color: Theme.midGray3
            border.width: 1
            radius: 6

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 8

                Text {
                    text: qsTr("URL of the audio file or stream:")
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: 12
                }

                TextField {
                    id: customUrlInput
                    Layout.fillWidth: true
                    Layout.preferredHeight: 38
                    placeholderText: qsTr("https://example.com/dj-set.mp3")
                    text: "https://cdn.freesound.org/previews/682/682245_11861866-lq.mp3"
                    color: "#ffffff"
                    font.pixelSize: 12
                    background: Rectangle {
                        color: Theme.darkGray3
                        radius: 4
                        border.color: customUrlInput.activeFocus ? Theme.yellow : Theme.midGray3
                        border.width: 1
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    TextField {
                        id: trackTitleInput
                        Layout.fillWidth: true
                        Layout.preferredHeight: 32
                        placeholderText: qsTr("Optional title (e.g. Future Rave Set)")
                        color: "#ffffff"
                        font.pixelSize: 11
                        background: Rectangle {
                            color: Theme.darkGray3
                            radius: 3
                            border.color: Theme.midGray3
                        }
                    }

                    Skin.Button {
                        Layout.preferredHeight: 34
                        Layout.preferredWidth: 140
                        activeColor: Theme.libraryDeck1Background
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 11
                        arrowLeft: true
                        text: qsTr("Load into DECK 1")

                        onClicked: {
                            if (!customUrlInput.text.trim()) return;
                            root.isDownloading = true;
                            var title = trackTitleInput.text.trim() || "Web Stream Audio";
                            Mixxx.StreamingManager.downloadAndLoad(
                                customUrlInput.text.trim(),
                                "custom",
                                "stream_" + Date.now(),
                                title,
                                "Web Stream",
                                "[Channel1]",
                                false
                            );
                        }
                    }

                    Skin.Button {
                        Layout.preferredHeight: 34
                        Layout.preferredWidth: 140
                        activeColor: Theme.libraryDeck2Background
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 11
                        arrowRight: true
                        text: qsTr("Load into DECK 2")

                        onClicked: {
                            if (!customUrlInput.text.trim()) return;
                            root.isDownloading = true;
                            var title = trackTitleInput.text.trim() || "Web Stream Audio";
                            Mixxx.StreamingManager.downloadAndLoad(
                                customUrlInput.text.trim(),
                                "custom",
                                "stream_" + Date.now(),
                                title,
                                "Web Stream",
                                "[Channel2]",
                                false
                            );
                        }
                    }
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
