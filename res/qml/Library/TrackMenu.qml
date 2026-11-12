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

    function act(action) {
        action();
        close();
    }

    anchors.centerIn: Overlay.overlay
    focus: true
    modal: true
    padding: 16
    width: Math.min(360, Overlay.overlay ? Overlay.overlay.width - 24 : 360)

    Overlay.modal: Rectangle {
        color: "#99000000"
    }
    background: Rectangle {
        border.color: Theme.darkGray3
        color: Theme.darkGray2
        radius: 16
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
        Rectangle {
            Layout.fillWidth: true
            color: Theme.darkGray3
            implicitHeight: 1
        }
        Text {
            color: Theme.lightGray2
            font.pixelSize: 12
            font.weight: Font.DemiBold
            text: qsTr("ADD TO PLAYLIST")
        }
        // Tap to add or remove, as many as wanted; the menu stays open
        PlaylistBrowser {
            id: playlistBrowser

            readonly property bool isFile: root.track.trackUrl.toString() !== ""

            function refresh() {
                if (isFile) {
                    checkedIds = Mixxx.MobileLibrary.playlistsOfTrack(root.track.trackUrl);
                }
            }

            Layout.fillWidth: true
            checkable: true

            Component.onCompleted: refresh()
            onPlaylistPicked: id => {
                if (checkedIds.includes(id)) {
                    // A streaming track is a file only after its download
                    if (isFile && Mixxx.MobileLibrary.removeTrackFromPlaylist(id, root.track.trackUrl)) {
                        checkedIds = checkedIds.filter(c => c !== id);
                    }
                    return;
                }
                root.track.addToPlaylist(id);
                checkedIds = checkedIds.concat([id]);
            }
        }
        Text {
            Layout.fillWidth: true
            color: Theme.lightGray2
            elide: Text.ElideRight
            font.pixelSize: 11
            text: qsTr("In %n playlist(s)", "", playlistBrowser.checkedIds.length)
        }
        Skin.Button {
            Layout.fillWidth: true
            Layout.preferredHeight: 42
            activeColor: Theme.accentColor
            font.bold: true
            text: qsTr("DONE")

            onClicked: root.close()
        }
    }

    onClosed: destroy()
}
