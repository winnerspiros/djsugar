import Mixxx 1.0 as Mixxx
import QtQuick
import QtQml.Models
import QtQuick.Controls
import "../Theme"

// Touch drag of a track: hold, then move. A pill with its name follows the
// finger and drops on a deck, a sampler or a playlist, which light up under
// it. Before the hold a move scrolls the list; a hold without a move is
// `held` (the menu). Streaming tracks download first.
Item {
    id: root

    required property string label
    // A file on disk; empty for a streaming track, which then needs:
    property url trackUrl
    // Loads the track to a deck or sampler, with the service's progress
    property var loadTo: null
    // Starts the download of `fetchId`; `fetchSource` then emits fileReady
    // or downloadFinished (id, path), or downloadFailed (id)
    property var fetch: null
    property QtObject fetchSource: null
    property string fetchId
    property Item ghost: null
    property Item list: null

    // A hold without a move: the streaming rows' menu by default
    signal held

    onHeld: {
        if (fetch) {
            menuComponent.createObject(root).popup();
        }
    }

    function loadToGroup(group) {
        if (loadTo) {
            loadTo(group);
        } else {
            deliver({
                "group": group
            });
        }
    }
    function addToPlaylist(playlistId) {
        deliver({
            "playlistId": playlistId
        });
    }
    // The waiter lives in the window: the list row may be gone by the time
    // the download ends
    function deliver(target) {
        const waiter = Qt.createQmlObject(`
            import Mixxx 1.0 as Mixxx
            import QtQuick
            Connections {
                property string trackId
                property string group
                property int playlistId: -1
                function onFileReady(id, path) { finish(id, path); }
                function onDownloadFinished(id, path) { finish(id, path); }
                function onDownloadFailed(id) { if (id === trackId) destroy(); }
                function finish(id, path) {
                    if (id !== trackId || !enabled) return;
                    enabled = false;
                    const url = path.startsWith("/") ? "file://" + path : path;
                    if (playlistId >= 0) Mixxx.MobileLibrary.addTrackToPlaylist(playlistId, url);
                    else Mixxx.PlayerManager.getPlayer(group).loadTrackFromLocationUrl(url, false);
                    destroy();
                }
                ignoreUnknownSignals: true
            }`, root.Window.contentItem);
        Object.assign(waiter, target);
        if (trackUrl.toString() !== "") {
            waiter.trackId = "file";
            waiter.finish("file", trackUrl.toString());
            return;
        }
        waiter.trackId = fetchId;
        waiter.target = fetchSource;
        fetch();
    }
    function begin(position) {
        for (let p = parent; p; p = p.parent) {
            if (p.flickableDirection !== undefined) {
                list = p;
                list.interactive = false;
                break;
            }
        }
        ghost = ghostComponent.createObject(root.Window.contentItem);
        move(position);
    }
    function move(position) {
        const p = mapToItem(ghost.parent, position.x, position.y);
        ghost.x = p.x - ghost.width / 2;
        ghost.y = p.y - ghost.height - 24; // above the finger
        ghost.Drag.active = ghost.Drag.active || Math.hypot(position.x - finger.pressAt.x, position.y - finger.pressAt.y) > 12;
    }
    function end(released) {
        if (list) {
            list.interactive = true;
            list = null;
        }
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
            root.held();
        }
    }

    anchors.fill: parent

    // Passive: buttons and taps on the row keep working
    TapHandler {
        gesturePolicy: TapHandler.DragThreshold
        // The platform long press: 0.4-0.5 s on Android, 0.5 s on iOS
        longPressThreshold: 0.5

        onLongPressed: root.begin(finger.point.position)
    }
    PointHandler {
        id: finger

        property point pressAt

        onActiveChanged: {
            if (active) {
                pressAt = point.position;
            } else {
                root.end(true); // after onCanceled, this finds nothing to do
            }
        }
        onPointChanged: {
            if (root.ghost) {
                root.move(point.position);
            }
        }
        onCanceled: root.end(false)
    }
    Component {
        id: menuComponent

        Menu {
            onClosed: destroy()

            MenuItem {
                text: qsTr("Load Deck 1")
                onTriggered: root.loadToGroup("[Channel1]")
            }
            MenuItem {
                text: qsTr("Load Deck 2")
                onTriggered: root.loadToGroup("[Channel2]")
            }
            MenuItem {
                text: qsTr("Download")
                onTriggered: root.fetch()
            }
            Menu {
                id: playlistMenu

                title: qsTr("Add to playlist")

                Instantiator {
                    model: Mixxx.MobileLibrary.playlists().filter(p => !p.locked)

                    MenuItem {
                        required property var modelData

                        text: modelData.name
                        onTriggered: root.addToPlaylist(modelData.id)
                    }
                    onObjectAdded: (index, item) => playlistMenu.insertItem(index, item)
                    onObjectRemoved: (index, item) => playlistMenu.removeItem(item)
                }
            }
        }
    }
    Component {
        id: ghostComponent

        Rectangle {
            function loadTo(group) {
                root.loadToGroup(group);
            }
            function addTo(playlistId) {
                root.addToPlaylist(playlistId);
            }

            Drag.hotSpot.x: width / 2
            Drag.hotSpot.y: height + 24
            Drag.keys: ["mixxx/track"]
            border.color: Theme.accentColor
            border.width: 2
            color: Theme.darkGray2
            height: 40
            radius: 20
            width: Math.min(260, text.implicitWidth + 32)
            z: 1000
            scale: 0.6

            // Lifts into the finger
            Component.onCompleted: scale = 1
            Behavior on scale {
                NumberAnimation {
                    duration: 120
                    easing.type: Easing.OutBack
                }
            }

            Text {
                id: text

                anchors.centerIn: parent
                color: Theme.white
                elide: Text.ElideRight
                font.pixelSize: 14
                text: "♪ " + root.label
                width: Math.min(implicitWidth, 228)
            }
        }
    }
}
