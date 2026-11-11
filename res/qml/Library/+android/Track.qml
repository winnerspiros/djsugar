import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls 2.15
import "../Theme"
import ".." as Skin

Item {
    id: root

    required property var capabilities
    property alias drag: dragHandler
    readonly property var library: Mixxx.Library
    property alias tap: tapHandler

    function hasCapabilities(caps) {
        return (root.capabilities & caps) == caps;
    }
    // Loading the track is expensive, only do it when it's needed
    function currentTrack() {
        return tableView.model.getTrack(row);
    }
    function loadToDeck(group) {
        Mixxx.PlayerManager.getPlayer(group).loadTrackFromLocationUrl(file_url, false);
    }
    // Every cell has a menu, it's only created when it's opened
    function openContextMenu() {
        contextMenuLoader.active = true;
        contextMenuLoader.item.popup();
    }

    // Hold opens the menu; hold and move drags the track onto a deck, a
    // sampler or a playlist. Before the hold, moves scroll the list.
    MouseArea {
        id: dragHandler

        property point holdAt
        property Item ghost: null
        readonly property bool active: ghost !== null

        acceptedButtons: Qt.LeftButton | Qt.RightButton
        anchors.fill: parent

        onClicked: mouse => tapHandler.tapped(mouse, mouse.button)
        onDoubleClicked: tapHandler.doubleTapped()
        onPressAndHold: mouse => {
            if (mouse.button === Qt.RightButton) {
                return;
            }
            preventStealing = true;
            holdAt = Qt.point(mouse.x, mouse.y);
            ghost = ghostComponent.createObject(root.Window.contentItem);
            moveGhost(mouse);
        }
        onPositionChanged: mouse => {
            if (!ghost) {
                return;
            }
            moveGhost(mouse);
            ghost.Drag.active = ghost.Drag.active || Math.hypot(mouse.x - holdAt.x, mouse.y - holdAt.y) > 12;
        }
        onReleased: finish(true)
        onCanceled: finish(false)

        function moveGhost(mouse) {
            const p = mapToItem(ghost.parent, mouse.x, mouse.y);
            ghost.x = p.x - ghost.width / 2;
            ghost.y = p.y - ghost.height - 24; // above the finger
        }
        function finish(released) {
            preventStealing = false;
            if (!ghost) {
                return;
            }
            const dragged = ghost.Drag.active;
            if (dragged && released) {
                ghost.Drag.drop();
            }
            ghost.destroy();
            ghost = null;
            if (!dragged && released) {
                root.openContextMenu();
            }
        }
    }
    QtObject {
        id: tapHandler

        signal doubleTapped
        signal tapped(var eventPoint, int button)

        onTapped: (eventPoint, button) => {
            if (button === Qt.RightButton) {
                root.openContextMenu();
            }
        }
    }
    Component {
        id: ghostComponent

        // The dragged track: the drop targets read trackUrl
        Rectangle {
            readonly property url trackUrl: file_url

            Drag.hotSpot.x: width / 2
            Drag.hotSpot.y: height + 24
            Drag.keys: ["mixxx/track"]
            color: Theme.darkGray2
            border.color: Theme.accentColor
            border.width: 2
            height: 40
            radius: 20
            width: Math.min(260, label.implicitWidth + 32)
            z: 1000

            Text {
                id: label

                anchors.centerIn: parent
                width: Math.min(implicitWidth, 228)
                color: Theme.white
                elide: Text.ElideRight
                font.pixelSize: 14
                text: "♪ " + decodeURIComponent(file_url.toString().split("/").pop())
            }
        }
    }
    Loader {
        id: contextMenuLoader

        active: false

        sourceComponent: Component {
            Menu {
                id: contextMenu

                title: qsTr("File")

                MenuItem {
                    id: loadDeck1Item

                    text: qsTr("Load Deck 1")
                    onTriggered: root.loadToDeck("[Channel1]")

                    // The arrow drawn, as Android draws ◀ ▶ as emoji
                    contentItem: Skin.ArrowText {
                        arrowLeft: true
                        color: loadDeck1Item.highlighted ? loadDeck1Item.palette.highlightedText : loadDeck1Item.palette.windowText
                        font: loadDeck1Item.font
                        text: loadDeck1Item.text
                    }
                }
                MenuItem {
                    id: loadDeck2Item

                    text: qsTr("Load Deck 2")
                    onTriggered: root.loadToDeck("[Channel2]")

                    // The arrow drawn, as Android draws ◀ ▶ as emoji
                    contentItem: Skin.ArrowText {
                        arrowRight: true
                        color: loadDeck2Item.highlighted ? loadDeck2Item.palette.highlightedText : loadDeck2Item.palette.windowText
                        font: loadDeck2Item.font
                        text: loadDeck2Item.text
                    }
                }
                MenuSeparator {
                }

                Menu {
                    enabled: {
                        hasCapabilities(Mixxx.LibraryTrackListModel.Capability.LoadToDeck) || hasCapabilities(Mixxx.LibraryTrackListModel.Capability.LoadToSampler) || hasCapabilities(Mixxx.LibraryTrackListModel.Capability.LoadToPreviewDeck);
                    }
                    title: qsTr("Load to")

                    Menu {
                        id: loadToDeckMenu

                        enabled: hasCapabilities(Mixxx.LibraryTrackListModel.Capability.LoadToDeck)
                        title: qsTr("Deck")

                        Instantiator {
                            model: 4

                            delegate: MenuItem {
                                text: qsTr("Deck %1").arg(modelData + 1)

                                onTriggered: root.loadToDeck(`[Channel${modelData + 1}]`)
                            }

                            onObjectAdded: (index, object) => loadToDeckMenu.insertItem(index, object)
                            onObjectRemoved: (index, object) => loadToDeckMenu.removeItem(object)
                        }
                    }
                    Menu {
                        enabled: hasCapabilities(Mixxx.LibraryTrackListModel.Capability.LoadToSampler)
                        title: qsTr("Sampler")
                    }

                    // Instantiator {
                    //     id: recentFilesInstantiator
                    //     model: settings.recentFiles
                    //     delegate: MenuItem {
                    //         text: settings.displayableFilePath(modelData)
                    //         onTriggered: loadFile(modelData)
                    //     }

                    //     onObjectAdded: (index, object) => recentFilesMenu.insertItem(index, object)
                    //     onObjectRemoved: (index, object) => recentFilesMenu.removeItem(object)
                    // }
                }
                Menu {
                    id: addToPlaylistMenu

                    enabled: {
                        hasCapabilities(Mixxx.LibraryTrackListModel.Capability.AddToTrackSet);
                    }
                    title: qsTr("Add to playlists")

                    MenuSeparator {
                    }
                    MenuItem {
                        enabled: false // TODO implement
                        text: qsTr("Create New Playlist")
                    }
                }
                Menu {
                    id: addToCrateMenu

                    enabled: {
                        hasCapabilities(Mixxx.LibraryTrackListModel.Capability.AddToTrackSet);
                    }
                    title: qsTr("Crates")

                    MenuSeparator {
                    }
                    MenuItem {
                        enabled: false // TODO implement
                        text: qsTr("Create New Crate")
                    }
                }
                Menu {
                    id: analyzeMenu

                    enabled: {
                        hasCapabilities(Mixxx.LibraryTrackListModel.Capability.EditMetadata) || hasCapabilities(Mixxx.LibraryTrackListModel.Capability.Analyze);
                    }
                    title: qsTr("Analyze")

                    MenuItem {
                        text: qsTr("Analyze")

                        onTriggered: {
                            library.analyze(root.currentTrack());
                        }
                    }
                    MenuItem {
                        enabled: false // TODO implement
                        text: qsTr("Reanalyze")
                    }
                    MenuItem {
                        enabled: false // TODO implement
                        text: qsTr("Reanalyze (constant BPM)")
                    }
                    MenuItem {
                        enabled: false // TODO implement
                        text: qsTr("Reanalyze (variable BPM)")
                    }
                }
            }
        }
    }
}
