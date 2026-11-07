import ".." as Skin
import "." as LibraryComponent
import Mixxx 1.0 as Mixxx
import QtQuick 2.12

Item {
    id: root

    // Several library views can exist at the same time (e.g. the LIB view
    // and the one of the LOAD drawers). The parent disables the ones that
    // aren't visible, so a controller button is handled only once.
    property alias focusWidget: focusedWidgetControl.value

    readonly property bool sidebarFocused: root.focusWidget == Skin.FocusedWidgetControl.WidgetKind.Sidebar
    readonly property bool libraryFocused: root.focusWidget == Skin.FocusedWidgetControl.WidgetKind.LibraryView || root.sidebarFocused

    signal moveVertical(int offset)
    signal moveSidebar(int offset)
    // Opens the source selected in the sidebar
    signal activateSidebarItem()
    // Closes the source selected in the sidebar or moves to its parent
    signal collapseSidebarItem()
    signal loadSelectedTrack(string group, bool play)
    signal loadSelectedTrackIntoNextAvailableDeck(bool play)

    function moveFocus(forward) {
        if (root.sidebarFocused) {
            if (forward) {
                root.activateSidebarItem();
            } else {
                root.collapseSidebarItem();
            }
        } else {
            root.focusWidget = Skin.FocusedWidgetControl.WidgetKind.Sidebar;
        }
    }
    function move(offset) {
        if (root.sidebarFocused) {
            root.moveSidebar(offset);
        } else if (root.focusWidget == Skin.FocusedWidgetControl.WidgetKind.LibraryView) {
            root.moveVertical(offset);
        }
    }

    Skin.FocusedWidgetControl {
        id: focusedWidgetControl

        Component.onCompleted: this.value = Skin.FocusedWidgetControl.WidgetKind.LibraryView
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "GoToItem"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value == 0)
                return;
            if (root.sidebarFocused)
                root.activateSidebarItem();
            else if (root.focusWidget == Skin.FocusedWidgetControl.WidgetKind.LibraryView)
                root.loadSelectedTrackIntoNextAvailableDeck(false);
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "LoadSelectedIntoFirstStopped"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0 && root.libraryFocused)
                root.loadSelectedTrackIntoNextAvailableDeck(false);
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "SelectTrackKnob"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0) {
                root.focusWidget = Skin.FocusedWidgetControl.WidgetKind.LibraryView;
                root.moveVertical(value);
            }
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "SelectPrevTrack"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0) {
                root.focusWidget = Skin.FocusedWidgetControl.WidgetKind.LibraryView;
                root.moveVertical(-1);
            }
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "SelectNextTrack"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0) {
                root.focusWidget = Skin.FocusedWidgetControl.WidgetKind.LibraryView;
                root.moveVertical(1);
            }
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveVertical"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0)
                root.move(value);
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveUp"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0)
                root.move(-1);
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveDown"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0)
                root.move(1);
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveFocusForward"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0)
                root.moveFocus(true);
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveFocusBackward"
        onValueChanged: (value) => {
            if (!root.enabled)
                return;
            if (value != 0)
                root.moveFocus(false);
        }
    }

    Mixxx.ControlProxy {
        id: numDecksControl

        group: "[App]"
        key: "num_decks"
    }

    Instantiator {
        model: numDecksControl.value

        delegate: LibraryComponent.ControlLoadSelectedTrackHandler {
            required property int index

            group: "[Channel" + (index + 1) + "]"
            enabled: root.enabled && root.libraryFocused
            onLoadTrackRequested: (play) => {
                root.loadSelectedTrack(this.group, play);
            }
        }
    }

    Mixxx.ControlProxy {
        id: numPreviewDecksControl

        group: "[App]"
        key: "num_preview_decks"
    }

    Instantiator {
        model: numPreviewDecksControl.value

        delegate: LibraryComponent.ControlLoadSelectedTrackHandler {
            required property int index

            group: "[PreviewDeck" + (index + 1) + "]"
            enabled: root.enabled && root.libraryFocused
            onLoadTrackRequested: (play) => {
                root.loadSelectedTrack(this.group, play);
            }
        }
    }

    Mixxx.ControlProxy {
        id: numSamplersControl

        group: "[App]"
        key: "num_samplers"
    }

    Instantiator {
        model: numSamplersControl.value

        delegate: LibraryComponent.ControlLoadSelectedTrackHandler {
            required property int index

            group: "[Sampler" + (index + 1) + "]"
            enabled: root.enabled && root.libraryFocused
            onLoadTrackRequested: (play) => {
                root.loadSelectedTrack(this.group, play);
            }
        }
    }
}
