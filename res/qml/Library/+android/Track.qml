import Mixxx 1.0 as Mixxx
import QtQuick
import "." as LibraryComponent

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
    function openContextMenu() {
        trackDrag.openMenu();
    }

    LibraryComponent.TrackDrag {
        id: trackDrag

        analyze: () => root.library.analyze(root.currentTrack())
        label: decodeURIComponent(file_url.toString().split("/").pop())
        trackUrl: file_url
    }
    DragHandler {
        id: dragHandler

        enabled: false // The TrackDrag above drags on touch screens
    }
    TapHandler {
        id: tapHandler

        acceptedButtons: Qt.LeftButton | Qt.RightButton
        gesturePolicy: TapHandler.DragThreshold

        onTapped: (eventPoint, button) => {
            if (button === Qt.RightButton) {
                root.openContextMenu();
            }
        }
    }
}
