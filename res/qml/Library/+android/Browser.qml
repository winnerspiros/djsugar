import ".." as Skin
import Mixxx 1.0 as Mixxx
import Qt.labs.qmlmodels
import QtQml
import QtQuick
import QtQml.Models
import QtQuick.Layouts
import QtQuick.Controls 2.15
import QtQuick.Shapes 1.12
import Qt5Compat.GraphicalEffects
import "../Theme"

Rectangle {
    id: root

    readonly property var featureSelection: ItemSelectionModel {
        model: root.model
    }
    required property var model
    // Shows that the controller's browse knob moves through the sources
    property bool highlighted: false
    // The name of the playlist, crate or folder whose tracks are shown
    property string activeLabel: ""
    // Whether the tracks shown are a whole USB drive (Rekordbox or Engine
    // DJ) rather than one of its playlists
    property bool activeIsDrive: false
    // Bumped when a playlist changes here, to re-read lock states
    property int playlistsRevision: 0

    Connections {
        target: Mixxx.MobileLibrary
        function onPlaylistsChanged() {
            root.playlistsRevision++;
        }
    }

    // Reading the databases on USB drives, and browsing folders (FOLDER, e.g.
    // on SD cards), needs more than the music permission
    // (from JaviLabarumDJ/mixxx-android android-stable)
    signal storageAccessNeeded(bool forFolders)

    function checkStorageAccess(index) {
        // SidebarModel::IconNameRole
        const icon = root.model.data(index, Qt.UserRole + 1);
        if (icon !== "rekordbox" && icon !== "enginedj" && icon !== "computer") {
            return;
        }
        if (!Mixxx.MobileLibrary.hasFullStorageAccess()) {
            root.storageAccessNeeded(icon === "computer");
        }
        // Opened on purpose: asks again for the USB permission of sticks
        // whose earlier request was dismissed
        Mixxx.MobileLibrary.allowUsbPermissionRequests();
    }

    // The playlist of a sidebar label, which carries the count and duration
    function playlistFor(label, revision) {
        let best = null;
        for (const p of Mixxx.MobileLibrary.playlists()) {
            if (label.startsWith(p.name) && (!best || p.name.length > best.name.length)) {
                best = p;
            }
        }
        return best;
    }
    function openMenu(playlist, isSection) {
        if (playlist) {
            playlistMenu.playlist = playlist;
            playlistMenu.open();
        } else if (isSection) {
            playlistEditor.openFor(null, null);
        }
    }

    function currentRow() {
        return featureView.rowAtIndex(featureSelection.currentIndex);
    }
    function selectRow(row) {
        featureSelection.setCurrentIndex(featureView.index(row, 0), ItemSelectionModel.ClearAndSelect | ItemSelectionModel.Rows);
        featureView.positionViewAtRow(row, TableView.Contain);
    }
    function moveSelection(offset) {
        if (featureView.rows <= 0) {
            return;
        }
        const row = root.currentRow();
        root.selectRow(row < 0 ? 0 : Math.max(0, Math.min(featureView.rows - 1, row + offset)));
    }
    // Opens the selected source. Returns true if the track list is what the
    // user wants to browse next: for an item without children, or for a
    // folder or playlist whose children are already shown (its own tracks
    // are loaded too). The top level sections only open and close.
    function activate(index) {
        const label = root.model.data(index, Qt.DisplayRole);
        root.activeLabel = label ? label.toString() : "";
        // A drive is a direct child of the Rekordbox or Engine DJ section;
        // SidebarModel::IconNameRole
        const parent = index.parent;
        const sectionIcon = parent.valid && !parent.parent.valid ? root.model.data(parent, Qt.UserRole + 1) : "";
        root.activeIsDrive = sectionIcon === "rekordbox" || sectionIcon === "enginedj";
        root.model.activate(index);
    }
    function activateCurrent() {
        const row = root.currentRow();
        if (row < 0) {
            root.selectRow(0);
            return false;
        }
        const index = featureView.index(row, 0);
        root.checkStorageAccess(index);
        root.activate(index);
        if (!root.model.hasChildren(index)) {
            return true;
        }
        if (!index.parent.valid) {
            featureView.toggleExpanded(row);
            return false;
        }
        if (featureView.isExpanded(row)) {
            return true;
        }
        featureView.expand(row);
        return false;
    }
    // Collapses the selected source or moves to its parent
    function collapseCurrent() {
        const row = root.currentRow();
        if (row < 0) {
            return;
        }
        if (featureView.isExpanded(row)) {
            featureView.collapse(row);
            return;
        }
        const parentIndex = featureView.index(row, 0).parent;
        if (parentIndex.valid) {
            const parentRow = featureView.rowAtIndex(parentIndex);
            featureView.collapse(parentRow);
            root.selectRow(parentRow);
        }
    }

    color: Theme.backgroundColor
    border.color: Theme.accentColor
    border.width: highlighted ? 2 : 0

    Component.onCompleted: {
        root.activate(root.model.index(0, 0));
    }

    Rectangle {
        anchors.bottomMargin: 7
        anchors.fill: parent
        anchors.leftMargin: 7
        anchors.rightMargin: 15
        anchors.topMargin: 7
        color: Theme.sunkenBackgroundColor

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            ScrollView {
                Layout.fillHeight: true
                Layout.fillWidth: true

                TreeView {
                    id: featureView

                    Layout.fillWidth: true
                    clip: true
                    model: root.model
                    selectionModel: featureSelection

                    delegate: FocusScope {
                        required property int column
                        required property bool current
                        required property int depth
                        required property bool expanded
                        required property int hasChildren
                        required property var icon
                        readonly property real indentation: 40
                        // modelIndex() changed its signature after Qt 6.4.2, index() is unambiguous
                        // See https://github.com/mixxxdj/mixxx/pull/14514#issuecomment-2770811094 for further details
                        readonly property var index: treeView.index(row, column)

                        // Rotate indicator when expanded by the user
                        // (requires TreeView to have a selectionModel)
                        property Animation indicatorAnimation: NumberAnimation {
                            duration: 100
                            easing.type: Easing.OutQuart
                            from: expanded ? 0 : 90
                            property: "rotation"
                            target: indicator
                            to: expanded ? 90 : 0
                        }
                        required property bool isTreeNode
                        required property string label
                        readonly property real padding: 5
                        required property int row

                        // Assigned to by TreeView:
                        required property TreeView treeView

                        implicitHeight: depth == 0 ? 42 : 35
                        implicitWidth: treeView.width

                        TableView.onPooled: indicatorAnimation.complete()
                        TableView.onReused: if (current)
                            indicatorAnimation.start()
                        onExpandedChanged: indicator.rotation = expanded ? 90 : 0

                        // A playlist row: {id, name, locked}, else null
                        readonly property var playlist: depth === 1 && root.model.data(index.parent, Qt.UserRole + 1) === "playlist" ? root.playlistFor(label, root.playlistsRevision) : null
                        readonly property bool isPlaylistSection: depth === 0 && icon === "playlist"

                        Rectangle {
                            id: background

                            anchors.fill: parent
                            anchors.leftMargin: 6 + 14 * depth
                            anchors.rightMargin: 6
                            anchors.topMargin: 2
                            anchors.bottomMargin: 2
                            color: rowDrop.containsDrag ? Qt.alpha(Theme.accentColor, 0.25) : current ? Theme.darkGray3 : rowMouseArea.pressed ? Theme.darkGray2 : "transparent"
                            border.color: rowDrop.containsDrag ? Theme.accentColor : "transparent"
                            border.width: 2
                            radius: 8

                            Behavior on color {
                                ColorAnimation {
                                    duration: 120
                                }
                            }

                            // The selected source: an accent bar
                            Rectangle {
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                color: Theme.accentColor
                                height: parent.height - 14
                                radius: 2
                                visible: current
                                width: 3
                            }
                            MouseArea {
                                id: rowMouseArea

                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                anchors.fill: parent
                                pressAndHoldInterval: 500

                                onClicked: event => {
                                    if (event.button === Qt.RightButton) {
                                        root.openMenu(playlist, isPlaylistSection);
                                        return;
                                    }
                                    treeView.selectionModel.setCurrentIndex(treeView.index(row, 0), ItemSelectionModel.ClearAndSelect | ItemSelectionModel.Rows);
                                    root.checkStorageAccess(index);
                                    root.activate(index);
                                    if (isTreeNode && hasChildren) {
                                        treeView.toggleExpanded(row);
                                    }
                                }
                                onPressAndHold: root.openMenu(playlist, isPlaylistSection)
                            }
                            // A dragged track lands in this playlist, or in a
                            // new one on the Playlists section
                            DropArea {
                                id: rowDrop

                                anchors.fill: parent
                                enabled: !!(playlist && !playlist.locked) || isPlaylistSection
                                keys: ["mixxx/track"]

                                onDropped: drop => {
                                    if (playlist) {
                                        drop.source.addTo(playlist.id);
                                    } else {
                                        playlistEditor.openFor(null, drop.source);
                                    }
                                    drop.accept();
                                }
                            }
                            Image {
                                id: lineIcon

                                anchors.left: parent.left
                                anchors.leftMargin: 12
                                anchors.verticalCenter: parent.verticalCenter
                                height: 20
                                source: depth == 0 && icon ? "qrc:/images/library/ic_library_" + icon + ".svg" : ""
                                sourceSize: Qt.size(20, 20)
                                visible: false
                                width: 20
                            }
                            // Icons in one colour: the accent when selected
                            ColorOverlay {
                                anchors.fill: lineIcon
                                color: current ? Theme.accentColor : Theme.textColor
                                source: lineIcon
                                visible: depth == 0 && !!icon
                            }
                            Label {
                                id: labelItem

                                anchors.left: parent.left
                                anchors.leftMargin: depth == 0 && icon ? 42 : 14
                                anchors.right: addButton.visible ? addButton.left : indicator.left
                                anchors.rightMargin: 6
                                anchors.verticalCenter: parent.verticalCenter
                                color: current ? Theme.white : Theme.textColor
                                elide: Text.ElideRight
                                font.pixelSize: depth == 0 ? 14 : 13
                                font.weight: depth == 0 ? Font.DemiBold : Font.Normal
                                text: (playlist && playlist.locked ? "🔒 " : "") + label
                            }
                            // New playlist, always at hand
                            Skin.Button {
                                id: addButton

                                anchors.right: indicator.left
                                anchors.rightMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                activeColor: Theme.accentColor
                                font.pixelSize: 18
                                height: 28
                                text: "+"
                                visible: isPlaylistSection
                                width: 28

                                onClicked: playlistEditor.openFor(null, null)
                            }
                            Label {
                                id: indicator

                                anchors.right: parent.right
                                anchors.rightMargin: 10
                                anchors.verticalCenter: parent.verticalCenter
                                color: Theme.lightGray2
                                // "▶" is drawn as an emoji on Android
                                font.pixelSize: 20
                                opacity: isTreeNode && hasChildren ? 1 : 0
                                text: "›"
                                width: 12
                            }
                        }
                    }
                }
            }
        }
    }

    // Rename, lock or delete a playlist
    Popup {
        id: playlistMenu

        property var playlist: ({})

        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 12
        width: Math.min(320, Overlay.overlay ? Overlay.overlay.width - 24 : 320)

        Overlay.modal: Rectangle {
            color: "#99000000"
        }
        background: Rectangle {
            border.color: Theme.darkGray3
            color: Theme.darkGray2
            radius: 12
        }
        contentItem: ColumnLayout {
            spacing: 8

            Text {
                Layout.fillWidth: true
                color: Theme.white
                elide: Text.ElideRight
                font.bold: true
                font.pixelSize: 15
                text: playlistMenu.playlist.name ?? ""
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 42
                activeColor: Theme.accentColor
                text: qsTr("RENAME")

                onClicked: {
                    playlistMenu.close();
                    playlistEditor.openFor(playlistMenu.playlist, null);
                }
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 42
                activeColor: Theme.accentColor
                text: playlistMenu.playlist.locked ? qsTr("UNLOCK") : qsTr("LOCK")

                onClicked: {
                    Mixxx.MobileLibrary.setPlaylistLocked(playlistMenu.playlist.id, !playlistMenu.playlist.locked);
                    root.playlistsRevision++;
                    playlistMenu.close();
                }
            }
            Skin.Button {
                id: deleteButton

                property bool armed: false

                Layout.fillWidth: true
                Layout.preferredHeight: 42
                activeColor: Theme.red
                enabled: !playlistMenu.playlist.locked
                normalColor: Theme.red
                text: armed ? qsTr("TAP AGAIN TO DELETE") : qsTr("DELETE")

                onClicked: {
                    if (!armed) {
                        armed = true;
                        return;
                    }
                    Mixxx.MobileLibrary.deletePlaylist(playlistMenu.playlist.id);
                    root.playlistsRevision++;
                    playlistMenu.close();
                }
            }
        }

        onClosed: deleteButton.armed = false
    }
    // New playlist (optionally with a dropped track) or rename
    Popup {
        id: playlistEditor

        property var playlist: null
        property Item track: null

        function openFor(playlist, track) {
            playlistEditor.playlist = playlist;
            playlistEditor.track = track;
            nameField.text = playlist ? playlist.name : (track ? track.label : "");
            error.visible = false;
            open();
            nameField.forceActiveFocus();
            nameField.selectAll();
        }
        function save() {
            const name = nameField.text.trim();
            let ok;
            if (playlist) {
                ok = Mixxx.MobileLibrary.renamePlaylist(playlist.id, name);
            } else {
                const id = Mixxx.MobileLibrary.createNamedPlaylist(name);
                ok = id >= 0;
                if (ok && track) {
                    track.addToPlaylist(id);
                }
            }
            error.visible = !ok;
            if (ok) {
                root.playlistsRevision++;
                close();
            }
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
            spacing: 8

            Text {
                color: Theme.white
                font.bold: true
                font.pixelSize: 15
                text: playlistEditor.playlist ? qsTr("Rename playlist") : qsTr("New playlist")
            }
            Skin.TextField {
                id: nameField

                Layout.fillWidth: true
                Layout.preferredHeight: 42
                font.pixelSize: 14
                placeholderText: qsTr("Playlist name")

                onAccepted: playlistEditor.save()
            }
            Text {
                id: error

                color: Theme.red
                font.pixelSize: 12
                text: qsTr("That name is empty or already used")
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 42
                activeColor: Theme.accentColor
                enabled: nameField.text.trim().length > 0
                text: playlistEditor.playlist ? qsTr("RENAME") : qsTr("CREATE")

                onClicked: playlistEditor.save()
            }
        }
    }
}
