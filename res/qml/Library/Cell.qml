import Qt5Compat.GraphicalEffects
import QtQuick
import QtQuick.Layouts
import "../Theme"

Rectangle {
    id: root

    readonly property alias dragImage: dragImageLoader.item

    Drag.dragType: Drag.Automatic
    Drag.mimeData: {
        "text/uri-list": file_url.toString(),
        "text/plain": file_url.toString()
    }
    Drag.supportedActions: Qt.CopyAction
    anchors.fill: parent
    color: selected ? Theme.accentColor : (row % 2 == 0 ? Theme.sunkenBackgroundColor : Theme.backgroundColor)

    Item {
        id: dragImageSource

        height: 85
        visible: false
        width: 190

        Rectangle {
            color: Theme.sunkenBackgroundColor
            radius: 12

            anchors {
                bottom: parent.bottom
                left: parent.left
                margins: 5
                right: parent.right
                top: parent.top
            }
            RowLayout {
                anchors.fill: parent

                Image {
                    id: cover

                    Layout.fillHeight: true
                    Layout.preferredWidth: cover_art ? 75 : 0
                    asynchronous: true
                    clip: true
                    fillMode: Image.PreserveAspectFit
                    source: cover_art
                }
                ColumnLayout {
                    Layout.fillHeight: true
                    Layout.fillWidth: true

                    Text {
                        color: Theme.textColor
                        text: track ? track.title : 'Unknown title'
                    }
                    Text {
                        color: Theme.midGray
                        text: track ? track.artist : 'Unknown artist'
                    }
                }
            }
            Rectangle {
                width: 20

                gradient: Gradient {
                    orientation: Gradient.Horizontal

                    GradientStop {
                        color: Theme.darkGray
                        position: 1
                    }
                    GradientStop {
                        color: 'transparent'
                        position: 0
                    }
                }

                anchors {
                    bottom: parent.bottom
                    right: parent.right
                    top: parent.top
                }
            }
        }
    }
    // Drag image for desktop drag & drop. A DropShadow is several items and
    // shader sources: built per row it made long library lists slow to
    // scroll on phones, where rows are never dragged.
    Loader {
        id: dragImageLoader

        anchors.fill: dragImageSource
        active: Qt.platform.os !== "android"
        visible: false

        sourceComponent: DropShadow {
            color: "#80000000"
            horizontalOffset: 0
            radius: 10.0
            source: dragImageSource
            verticalOffset: 0
        }
    }
    Rectangle {
        id: border

        color: Theme.darkGray2
        width: 1

        anchors {
            bottom: parent.bottom
            right: parent.right
            top: parent.top
        }
    }
}
