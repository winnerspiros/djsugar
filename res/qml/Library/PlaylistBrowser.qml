import ".." as Skin
import Mixxx 1.0 as Mixxx
import Qt5Compat.GraphicalEffects
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Theme"

// The playlists, with a search and a field for a new one. Picking a
// playlist emits playlistPicked; with `checkable` each row has a check box,
// checked when in `checkedIds`.
ColumnLayout {
    id: root

    property bool checkable: false
    property var checkedIds: []
    property var playlists: []
    property string filter: ""

    signal playlistPicked(int playlistId)

    function reload() {
        playlists = Mixxx.MobileLibrary.playlists();
    }
    function create() {
        // The keyboard's word in progress counts too
        Qt.inputMethod.commit();
        const name = newName.text.trim();
        if (name.length === 0) {
            return;
        }
        const id = Mixxx.MobileLibrary.createNamedPlaylist(name);
        newName.text = "";
        if (id >= 0) {
            reload();
            root.playlistPicked(id);
        } else {
            newName.placeholderText = qsTr("%1 exists").arg(name);
        }
    }

    readonly property var entries: {
        const query = filter.trim().toLowerCase();
        return query ? playlists.filter(p => p.name.toLowerCase().includes(query)) : playlists;
    }

    spacing: 8

    Component.onCompleted: reload()

    Connections {
        target: Mixxx.MobileLibrary
        function onPlaylistsChanged() {
            root.reload();
        }
    }

    Skin.TextField {
        Layout.fillWidth: true
        Layout.preferredHeight: 38
        font.pixelSize: 14
        placeholderText: qsTr("Search playlists")
        visible: root.playlists.length > 6

        onTextChanged: root.filter = text
    }
    ListView {
        id: list

        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(44, Math.min(contentHeight, 5 * 48))
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        model: root.entries
        spacing: 4

        delegate: Rectangle {
            id: entry

            required property var modelData
            readonly property bool checked: root.checkedIds.includes(modelData.id)

            border.color: Theme.accentColor
            border.width: checked ? 1 : 0
            color: entryArea.pressed ? Theme.darkGray3 : Qt.lighter(Theme.darkGray2, 1.25)
            height: 44
            opacity: modelData.locked ? 0.4 : 1
            radius: 10
            width: ListView.view.width

            Image {
                id: entryIcon

                anchors.left: parent.left
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                height: 18
                source: "qrc:/images/library/ic_library_playlist.svg"
                sourceSize: Qt.size(18, 18)
                visible: false
                width: 18
            }
            ColorOverlay {
                anchors.fill: entryIcon
                color: entry.checked ? Theme.accentColor : Theme.textColor
                source: entryIcon
            }
            Text {
                anchors.left: entryIcon.right
                anchors.leftMargin: 12
                anchors.right: parent.right
                anchors.rightMargin: 40
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.white
                elide: Text.ElideRight
                font.pixelSize: 14
                text: (entry.modelData.locked ? "🔒 " : "") + entry.modelData.name
            }
            Rectangle {
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                border.color: entry.checked ? Theme.accentColor : Theme.lightGray2
                border.width: 2
                color: entry.checked ? Theme.accentColor : "transparent"
                height: 22
                radius: 6
                visible: root.checkable
                width: 22

                Text {
                    anchors.centerIn: parent
                    color: Theme.white
                    font.bold: true
                    font.pixelSize: 14
                    text: "✓"
                    visible: entry.checked
                }
            }
            MouseArea {
                id: entryArea

                anchors.fill: parent
                enabled: !entry.modelData.locked

                onClicked: root.playlistPicked(entry.modelData.id)
            }
        }

        Text {
            anchors.centerIn: parent
            color: Theme.lightGray2
            font.pixelSize: 13
            text: root.filter.length > 0 ? qsTr("Nothing found") : qsTr("No playlists yet")
            visible: list.count === 0
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

            onAccepted: root.create()
        }
        Skin.Button {
            Layout.preferredHeight: 40
            activeColor: Theme.accentColor
            enabled: newName.displayText.trim().length > 0
            text: qsTr("ADD")

            onClicked: root.create()
        }
    }
}
