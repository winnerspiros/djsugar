import ".." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Theme"

// The menu of a held track, local or streaming: a card in the style of the
// library's popups. `track` is its TrackDrag.
Popup {
    id: root

    required property Item track
    // Local tracks: queue an analysis
    property var analyze: null
    property var playlists: []

    function loadPlaylists() {
        playlists = Mixxx.MobileLibrary.playlists().filter(p => !p.locked);
    }
    function createPlaylist() {
        // The keyboard's word in progress counts too
        Qt.inputMethod.commit();
        const name = newName.text.trim();
        if (name.length === 0) {
            return;
        }
        const id = Mixxx.MobileLibrary.createNamedPlaylist(name);
        if (id >= 0) {
            root.act(() => root.track.addToPlaylist(id));
        } else {
            newName.placeholderText = qsTr("%1 exists: pick it above").arg(name);
            newName.text = "";
        }
    }

    function act(action) {
        action();
        close();
    }

    anchors.centerIn: Overlay.overlay
    focus: true
    modal: true
    padding: 12
    width: Math.min(340, Overlay.overlay ? Overlay.overlay.width - 24 : 340)

    Overlay.modal: Rectangle {
        color: "#99000000"
    }
    background: Rectangle {
        border.color: Theme.darkGray3
        color: Theme.darkGray2
        radius: 12
    }
    contentItem: ColumnLayout {
        spacing: 10

        Text {
            Layout.fillWidth: true
            color: Theme.white
            elide: Text.ElideRight
            font.bold: true
            font.pixelSize: 15
            text: root.track.label
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                activeColor: Theme.libraryDeck1Pressed
                arrowLeft: true
                font.bold: true
                normalColor: Theme.libraryDeck1Background
                text: qsTr("DECK 1")

                onClicked: root.act(() => root.track.loadToGroup("[Channel1]"))
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                activeColor: Theme.libraryDeck2Pressed
                arrowRight: true
                font.bold: true
                normalColor: Theme.libraryDeck2Background
                text: qsTr("DECK 2")

                onClicked: root.act(() => root.track.loadToGroup("[Channel2]"))
            }
        }
        Skin.Button {
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            activeColor: Theme.accentColor
            normalColor: Theme.libraryActionText
            text: root.analyze ? qsTr("ANALYZE") : qsTr("DOWNLOAD")
            visible: root.analyze || root.track.fetch

            onClicked: root.act(root.analyze ?? root.track.fetch)
        }
        Text {
            color: Theme.lightGray2
            font.pixelSize: 12
            text: qsTr("Add to playlist")
        }
        ListView {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 4 * 44)
            clip: true
            model: root.playlists
            spacing: 4

            delegate: Skin.Button {
                required property var modelData

                activeColor: Theme.accentColor
                backgroundColor: Theme.libraryActionDisabledBackground
                font.pixelSize: 14
                height: 40
                normalColor: Theme.white
                text: modelData.name
                width: ListView.view.width

                onClicked: root.act(() => root.track.addToPlaylist(modelData.id))
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Skin.TextField {
                id: newName

                Layout.fillWidth: true
                Layout.preferredHeight: 40
                font.pixelSize: 14
                placeholderText: qsTr("New playlist")
                onAccepted: root.createPlaylist()
            }
            Skin.Button {
                Layout.preferredHeight: 40
                activeColor: Theme.accentColor
                enabled: newName.displayText.trim().length > 0
                text: qsTr("ADD")

                onClicked: root.createPlaylist()
            }
        }
    }

    Component.onCompleted: loadPlaylists()
    onClosed: destroy()

    Connections {
        target: Mixxx.MobileLibrary
        function onPlaylistsChanged() {
            root.loadPlaylists();
        }
    }
}
