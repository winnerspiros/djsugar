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

    // Search, when there are many
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 38
        color: Theme.embeddedBackgroundColor
        radius: 8
        visible: root.playlists.length > 6

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.lightGray
            font.pixelSize: 15
            text: "⌕"
        }
        TextField {
            anchors.fill: parent
            anchors.leftMargin: 30
            background: null
            color: Theme.textColor
            font.pixelSize: 14
            inputMethodHints: Qt.ImhNoPredictiveText
            placeholderText: qsTr("Search playlists")
            placeholderTextColor: Theme.lightGray

            onTextChanged: root.filter = text
        }
    }
    // The playlists, on the sunken background of the library lists
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: list.wantedHeight + 8
        color: Theme.sunkenBackgroundColor
        radius: 10

        ListView {
            id: list

            readonly property real wantedHeight: Math.max(48, Math.min(contentHeight, 5 * 50))

            anchors.fill: parent
            anchors.margins: 4
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            model: root.entries
            spacing: 2

            ScrollIndicator.vertical: ScrollIndicator {
            }
            delegate: Rectangle {
                id: entry

                required property var modelData
                readonly property bool checked: root.checkedIds.includes(modelData.id)

                color: entryArea.pressed ? Theme.darkGray3 : checked ? Qt.alpha(Theme.accentColor, 0.22) : "transparent"
                height: 48
                opacity: modelData.locked ? 0.45 : 1
                radius: 8
                width: ListView.view.width

                Behavior on color {
                    ColorAnimation {
                        duration: 120
                    }
                }

                // In the playlist: an accent bar, as the sidebar's selection
                Rectangle {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.accentColor
                    height: parent.height - 16
                    radius: 2
                    visible: entry.checked
                    width: 3
                }
                Image {
                    id: entryIcon

                    anchors.left: parent.left
                    anchors.leftMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    height: 20
                    source: "qrc:/images/library/ic_library_playlist.svg"
                    sourceSize: Qt.size(20, 20)
                    visible: false
                    width: 20
                }
                ColorOverlay {
                    anchors.fill: entryIcon
                    color: entry.checked ? Theme.accentColor : Theme.textColor
                    source: entryIcon
                }
                Column {
                    anchors.left: entryIcon.right
                    anchors.leftMargin: 12
                    anchors.right: parent.right
                    anchors.rightMargin: 44
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 1

                    Text {
                        color: entry.checked ? Theme.white : Theme.textColor
                        elide: Text.ElideRight
                        font.pixelSize: 14
                        font.weight: entry.checked ? Font.DemiBold : Font.Normal
                        text: (entry.modelData.locked ? "🔒 " : "") + entry.modelData.name
                        width: parent.width
                    }
                    Text {
                        color: Theme.lightGray
                        font.pixelSize: 11
                        text: entry.modelData.count === 1 ? qsTr("1 track") : qsTr("%1 tracks").arg(entry.modelData.count ?? 0)
                        visible: entry.modelData.count !== undefined
                    }
                }
                // Check box
                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    border.color: entry.checked ? Theme.accentColor : Theme.midGray
                    border.width: 2
                    color: entry.checked ? Theme.accentColor : "transparent"
                    height: 22
                    radius: 11
                    visible: root.checkable
                    width: 22

                    Behavior on color {
                        ColorAnimation {
                            duration: 120
                        }
                    }

                    Text {
                        anchors.centerIn: parent
                        color: Theme.white
                        font.bold: true
                        font.pixelSize: 13
                        scale: entry.checked ? 1 : 0
                        text: "✓"

                        Behavior on scale {
                            NumberAnimation {
                                duration: 140
                                easing.type: Easing.OutBack
                            }
                        }
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
                color: Theme.lightGray
                font.pixelSize: 13
                text: root.filter.length > 0 ? qsTr("Nothing found") : qsTr("No playlists yet")
                visible: list.count === 0
            }
        }
    }
    // A new playlist, which gets the track at once
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 44
        border.color: newName.activeFocus ? Theme.accentColor : "transparent"
        border.width: 1
        color: Theme.embeddedBackgroundColor
        radius: 8

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 14
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.accentColor
            font.pixelSize: 20
            text: "+"
        }
        TextField {
            id: newName

            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.leftMargin: 34
            anchors.right: addButton.left
            anchors.top: parent.top
            background: null
            color: Theme.textColor
            font.pixelSize: 14
            inputMethodHints: Qt.ImhNoPredictiveText
            placeholderText: qsTr("New playlist")
            placeholderTextColor: Theme.lightGray

            onAccepted: root.create()
        }
        Skin.Button {
            id: addButton

            activeColor: Theme.accentColor
            anchors.right: parent.right
            anchors.rightMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            enabled: newName.displayText.trim().length > 0
            height: 36
            text: qsTr("CREATE")
            visible: newName.displayText.trim().length > 0

            onClicked: root.create()
        }
    }
}
