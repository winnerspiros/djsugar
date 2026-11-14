import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import "../.." as Skin
import ".." as LibraryComponent

// Controller input for the streaming hub. BROWSE/LOAD act on the open
// service; BROWSE toggle switches between service tabs and results.
Item {
    id: root

    // true while BROWSE turns through the service tabs
    readonly property bool tabsFocused: focusedWidgetControl.value == Skin.FocusedWidgetControl.WidgetKind.Sidebar

    signal moveResult(int offset)
    signal moveTab(int offset)
    signal loadResult(string group, bool play)

    function move(offset) {
        if (!root.enabled || offset == 0)
            return;
        if (root.tabsFocused)
            root.moveTab(offset > 0 ? 1 : -1);
        else
            root.moveResult(offset);
    }

    function showResults() {
        focusedWidgetControl.value = Skin.FocusedWidgetControl.WidgetKind.LibraryView;
    }

    Skin.FocusedWidgetControl {
        id: focusedWidgetControl
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveVertical"
        onValueChanged: (value) => root.move(value)
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveUp"
        onValueChanged: (value) => {
            if (value != 0)
                root.move(-1);
        }
    }

    Mixxx.ControlProxy {
        group: "[Library]"
        key: "MoveDown"
        onValueChanged: (value) => {
            if (value != 0)
                root.move(1);
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "SelectTrackKnob"
        onValueChanged: (value) => {
            if (!root.enabled || value == 0)
                return;
            root.showResults();
            root.moveResult(value);
        }
    }

    // Pressing BROWSE on the results opens the tabs and the other way round;
    // GoToItem (and LoadSelectedIntoFirstStopped) on a result loads it on
    // the first stopped deck.
    Mixxx.ControlProxy {
        group: "[Library]"
        key: "GoToItem"
        onValueChanged: (value) => {
            if (!root.enabled || value == 0)
                return;
            if (root.tabsFocused)
                root.showResults();
            else
                root.loadResult("", false);
        }
    }

    Mixxx.ControlProxy {
        group: "[Playlist]"
        key: "LoadSelectedIntoFirstStopped"
        onValueChanged: (value) => {
            if (root.enabled && value != 0)
                root.loadResult("", false);
        }
    }

    Mixxx.ControlProxy {
        id: numDecksControl

        group: "[App]"
        key: "num_decks"
    }

    Instantiator {
        model: numDecksControl.value

        // the library's own handler, as Library/Control.qml uses it (an
        // inline component here was created with an unset index first and
        // logged "[Channel0] LoadSelectedTrack does not exist")
        delegate: LibraryComponent.ControlLoadSelectedTrackHandler {
            required property int index

            group: "[Channel" + (index + 1) + "]"
            active: root.enabled
            onLoadTrackRequested: (play) => root.loadResult(this.group, play)
        }
    }
}
