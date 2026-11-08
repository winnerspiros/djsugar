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

    DragHandler {
        id: dragHandler
        enabled: false // Disabled on touch devices so TableView/Flickable scrolls smoothly
    }
    TapHandler {
        id: tapHandler

        acceptedButtons: Qt.LeftButton | Qt.RightButton
        gesturePolicy: TapHandler.DragThreshold

        onLongPressed: mouse => {
            root.openContextMenu();
        }
        onTapped: (eventPoint, button) => {
            if (button === Qt.RightButton) {
                root.openContextMenu();
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
