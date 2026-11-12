import ".." as Skin
import Mixxx 1.0 as Mixxx
import Qt5Compat.GraphicalEffects
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Theme"

// Playlists in their folders, to pick a playlist (e.g. for a track) or a
// folder (to move a playlist or folder into). Opens folders in place, with
// the way back on top; a search looks through all of them.
ColumnLayout {
    id: root

    // Pick a folder instead of a playlist
    property bool pickFolder: false
    // The folder being moved: it and the folders inside it cannot be picked
    property int movingFolder: -1
    // The folder shown, -1 for the top level
    property int folderId: -1
    property var folders: []
    property var playlists: []
    property string filter: ""
    // The new name field makes a folder rather than a playlist
    property bool newIsFolder: pickFolder

    signal playlistPicked(int playlistId)
    signal folderPicked(int folderId)

    function reload() {
        folders = Mixxx.MobileLibrary.playlistFolders();
        playlists = Mixxx.MobileLibrary.playlists();
        if (folderId > 0 && !folders.some(f => f.id === folderId)) {
            folderId = -1;
        }
    }
    function folderOf(id) {
        return folders.find(f => f.id === id) ?? null;
    }
    // The folders from the top down to `id`
    function pathTo(id) {
        const path = [];
        for (let folder = folderOf(id); folder; folder = folderOf(folder.parentId)) {
            path.unshift(folder);
            if (path.length > 64) {
                break;
            }
        }
        return path;
    }
    function isInside(id, ancestor) {
        return pathTo(id).some(f => f.id === ancestor);
    }
    function pathText(id) {
        return pathTo(id).map(f => f.name).join(" / ");
    }
    function create() {
        // The keyboard's word in progress counts too
        Qt.inputMethod.commit();
        const name = newName.text.trim();
        if (name.length === 0) {
            return;
        }
        if (newIsFolder) {
            const id = Mixxx.MobileLibrary.createPlaylistFolder(name, folderId);
            newName.text = "";
            if (id > 0) {
                folderId = id;
            }
            return;
        }
        const id = Mixxx.MobileLibrary.createNamedPlaylist(name, folderId);
        if (id >= 0) {
            newName.text = "";
            root.playlistPicked(id);
        } else {
            newName.placeholderText = qsTr("%1 exists").arg(name);
            newName.text = "";
        }
    }

    readonly property var entries: {
        const query = filter.trim().toLowerCase();
        const result = [];
        const folderEntry = f => ({
                    kind: "folder",
                    id: f.id,
                    name: f.name,
                    sub: query ? pathText(f.parentId) : "",
                    disabled: pickFolder && (f.id === movingFolder || isInside(f.id, movingFolder))
                });
        const playlistEntry = p => ({
                    kind: "playlist",
                    id: p.id,
                    name: p.name,
                    sub: query ? pathText(p.folderId) : "",
                    disabled: p.locked
                });
        if (query) {
            for (const f of folders) {
                if (f.name.toLowerCase().includes(query)) {
                    result.push(folderEntry(f));
                }
            }
            if (!pickFolder) {
                for (const p of playlists) {
                    if (p.name.toLowerCase().includes(query)) {
                        result.push(playlistEntry(p));
                    }
                }
            }
            return result;
        }
        for (const f of folders) {
            if (f.parentId === folderId) {
                result.push(folderEntry(f));
            }
        }
        if (!pickFolder) {
            for (const p of playlists) {
                if (p.folderId === folderId || (p.folderId > 0 && !folderOf(p.folderId) && folderId === -1)) {
                    result.push(playlistEntry(p));
                }
            }
        }
        return result;
    }

    spacing: 8

    Component.onCompleted: reload()

    Connections {
        target: Mixxx.MobileLibrary
        function onPlaylistsChanged() {
            root.reload();
        }
    }

    // Where we are, and the way back
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        Rectangle {
            Layout.preferredHeight: 32
            Layout.preferredWidth: 32
            color: backArea.pressed ? Theme.darkGray3 : "transparent"
            radius: 16
            visible: root.folderId > 0 && root.filter.length === 0

            Text {
                anchors.centerIn: parent
                color: Theme.white
                font.pixelSize: 22
                text: "‹"
            }
            MouseArea {
                id: backArea

                anchors.fill: parent

                onClicked: root.folderId = root.folderOf(root.folderId)?.parentId ?? -1
            }
        }
        Text {
            Layout.fillWidth: true
            color: Theme.lightGray2
            elide: Text.ElideLeft
            font.pixelSize: 12
            font.weight: Font.DemiBold
            text: root.filter.length > 0 ? qsTr("Search") : [qsTr("Playlists")].concat(root.pathTo(root.folderId).map(f => f.name)).join("  ›  ")
        }
    }
    Skin.TextField {
        Layout.fillWidth: true
        Layout.preferredHeight: 38
        font.pixelSize: 14
        placeholderText: qsTr("Search playlists")
        visible: root.playlists.length + root.folders.length > 6

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
            readonly property bool isFolder: modelData.kind === "folder"

            color: entryArea.pressed ? Theme.darkGray3 : Qt.lighter(Theme.darkGray2, 1.25)
            height: 44
            opacity: modelData.disabled ? 0.4 : 1
            radius: 10
            width: ListView.view.width

            Image {
                id: entryIcon

                anchors.left: parent.left
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                height: 18
                source: entry.isFolder ? "qrc:/images/library/ic_library_crates.svg" : "qrc:/images/library/ic_library_playlist.svg"
                sourceSize: Qt.size(18, 18)
                visible: false
                width: 18
            }
            ColorOverlay {
                anchors.fill: entryIcon
                color: entry.isFolder ? Theme.accentColor : Theme.textColor
                source: entryIcon
            }
            Column {
                anchors.left: entryIcon.right
                anchors.leftMargin: 12
                anchors.right: chevron.left
                anchors.rightMargin: 6
                anchors.verticalCenter: parent.verticalCenter

                Text {
                    color: Theme.white
                    elide: Text.ElideRight
                    font.pixelSize: 14
                    text: (entry.modelData.disabled && !entry.isFolder ? "🔒 " : "") + entry.modelData.name
                    width: parent.width
                }
                Text {
                    color: Theme.lightGray2
                    elide: Text.ElideLeft
                    font.pixelSize: 11
                    text: entry.modelData.sub
                    visible: text.length > 0
                    width: parent.width
                }
            }
            Text {
                id: chevron

                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.lightGray2
                font.pixelSize: 20
                text: entry.isFolder ? "›" : ""
            }
            MouseArea {
                id: entryArea

                anchors.fill: parent
                enabled: !entry.modelData.disabled

                onClicked: {
                    if (entry.isFolder) {
                        root.folderId = entry.modelData.id;
                        root.filter = "";
                    } else {
                        root.playlistPicked(entry.modelData.id);
                    }
                }
            }
        }

        Text {
            anchors.centerIn: parent
            color: Theme.lightGray2
            font.pixelSize: 13
            text: root.filter.length > 0 ? qsTr("Nothing found") : root.pickFolder ? qsTr("No folders inside") : qsTr("Empty folder")
            visible: list.count === 0
        }
    }
    // New playlist (or folder) here
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        Skin.Button {
            Layout.preferredHeight: 40
            Layout.preferredWidth: 52
            activeColor: Theme.accentColor
            checkable: true
            checked: root.newIsFolder
            font.pixelSize: 11
            text: root.newIsFolder ? qsTr("FOLDER") : qsTr("LIST")
            visible: !root.pickFolder

            onClicked: root.newIsFolder = !root.newIsFolder
        }
        Skin.TextField {
            id: newName

            Layout.fillWidth: true
            Layout.preferredHeight: 40
            font.pixelSize: 14
            placeholderText: root.newIsFolder ? qsTr("New folder") : qsTr("New playlist")

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
    Skin.Button {
        Layout.fillWidth: true
        Layout.preferredHeight: 42
        activeColor: Theme.accentColor
        font.bold: true
        text: root.folderId > 0 ? qsTr("MOVE INTO %1").arg(root.folderOf(root.folderId)?.name ?? "") : qsTr("MOVE TO THE TOP LEVEL")
        visible: root.pickFolder

        onClicked: root.folderPicked(root.folderId)
    }
}
