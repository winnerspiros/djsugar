import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"
import "webinput.js" as WebInput

// Navidrome / Subsonic, Audius and direct URLs / radio behind one box: it
// works out what it was fed (see webinput.js). The chips show where it went
// and switch by hand.
Item {
    id: root

    property string targetDeck: "[Channel1]"
    // 0: Navidrome / Subsonic, 1: Audius, 2: URL / radio
    property int source: Mixxx.StreamingManager.isSubsonicConnected ? 0 : 1
    property string hint: ""

    readonly property var sources: [
        {
            "label": qsTr("NAVIDROME"),
            "icon": "qrc:/images/library/ic_library_navidrome.svg",
            "color": "#00e5ff"
        },
        {
            "label": qsTr("AUDIUS"),
            "icon": "qrc:/images/library/ic_library_audius.svg",
            "color": "#ea80fc"
        },
        {
            "label": qsTr("URL / RADIO"),
            "icon": "",
            "color": "#ffb300"
        }
    ]

    function feed(text) {
        const input = WebInput.classify(text);
        switch (input.kind) {
        case "search":
            if (root.source === 2)
                root.source = Mixxx.StreamingManager.isSubsonicConnected ? 0 : 1;
            if (root.source === 0)
                subsonicView.search(input.query);
            else
                audiusView.search(input.query);
            root.hint = root.source === 0 ? qsTr("Searching your Navidrome library") : qsTr("Searching Audius");
            break;
        case "audius":
            root.source = 1;
            if (input.query !== "")
                audiusView.search(input.query);
            root.hint = qsTr("Audius link");
            break;
        case "url":
            root.source = 2;
            urlView.openUrl(input.url);
            root.hint = qsTr("Audio file or stream: pick a deck");
            break;
        case "server":
            root.source = 0;
            subsonicView.openServer(input.url);
            root.hint = qsTr("Navidrome / Subsonic server: log in");
            break;
        default:
            root.hint = "";
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 36
            color: Theme.darkGray3
            border.color: Theme.midGray3
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.margins: 5
                spacing: 6

                TextField {
                    id: smartInput

                    Layout.fillWidth: true
                    Layout.preferredHeight: 26
                    color: "#ffffff"
                    font.pixelSize: 11
                    inputMethodHints: Qt.ImhNoPredictiveText
                    placeholderText: qsTr("Search, or paste a link: Navidrome server, Audius, MP3, radio stream...")
                    background: Rectangle {
                        color: Theme.darkGray2
                        border.color: smartInput.activeFocus ? root.sources[root.source].color : Theme.midGray3
                        border.width: 1
                        radius: 4
                    }
                    onAccepted: root.feed(text)
                }
                Skin.Button {
                    Layout.preferredHeight: 26
                    Layout.preferredWidth: 64
                    activeColor: root.sources[root.source].color
                    normalColor: "#ffffff"
                    font.bold: true
                    font.pixelSize: 10
                    text: qsTr("GO")
                    onClicked: root.feed(smartInput.text)
                }
                Repeater {
                    model: root.sources

                    delegate: Skin.Button {
                        required property int index
                        required property var modelData

                        Layout.preferredHeight: 26
                        Layout.preferredWidth: modelData.icon !== "" ? 118 : 90
                        activeColor: modelData.color
                        normalColor: root.source === index ? modelData.color : "#9e9e9e"
                        font.bold: true
                        font.pixelSize: 9
                        text: modelData.label
                        onClicked: {
                            root.source = index;
                            root.hint = "";
                            // The box's text goes along when it fits there
                            const input = WebInput.classify(smartInput.text);
                            if (input.kind === "search" && index === 0)
                                subsonicView.search(input.query);
                            else if (input.kind === "search" && index === 1)
                                audiusView.search(input.query);
                            else if (input.kind === "url" && index === 2)
                                urlView.openUrl(input.url);
                        }

                        Image {
                            anchors.left: parent.left
                            anchors.leftMargin: 5
                            anchors.verticalCenter: parent.verticalCenter
                            height: 16
                            source: parent.modelData.icon
                            sourceSize.height: 32
                            sourceSize.width: 32
                            visible: source != ""
                            width: 16
                        }
                    }
                }
            }
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 8
            color: "#9a9ea6"
            elide: Text.ElideRight
            font.pixelSize: 10
            text: root.hint
            visible: root.hint !== ""
        }
        StackLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            currentIndex: root.source

            SubsonicView {
                id: subsonicView

            }
            OnlineStreamView {
                id: audiusView

            }
            CustomUrlView {
                id: urlView

                targetDeck: root.targetDeck
            }
        }
    }
}
