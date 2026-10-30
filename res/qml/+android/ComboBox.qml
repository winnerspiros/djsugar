import "." as Skin
import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Shapes
import Qt5Compat.GraphicalEffects
import Mixxx 1.0 as Mixxx
import "Theme"

ComboBox {
    id: root

    property bool clip: false
    property list<var> footerItems: []
    property int popupMaxItem: 6
    property alias popupWidth: popupItem.width

    signal activateFooter(int index)

    // Models made of qsTr() texts are rebuilt when the language changes,
    // which resets the selection: restore it afterwards.
    property int indexBeforeRetranslation: -1

    Connections {
        target: Mixxx.Language

        function onLocaleAboutToChange() {
            root.indexBeforeRetranslation = root.currentIndex;
        }
        function onLocaleChanged() {
            if (root.indexBeforeRetranslation >= 0 && root.indexBeforeRetranslation < root.count) {
                root.currentIndex = root.indexBeforeRetranslation;
            }
            root.indexBeforeRetranslation = -1;
        }
    }

    indicator.width: 20

    background: Item {
        Rectangle {
            id: background

            anchors.fill: parent
            anchors.margins: 4
            border.color: '#000000'
            border.width: 1
            color: '#232323'
            radius: 4
        }
        InnerShadow {
            id: bottomInnerEffect

            anchors.fill: parent
            color: "#40000000"
            horizontalOffset: -2
            radius: 4
            samples: 16
            source: background
            spread: 0.3
            verticalOffset: -2
        }
        InnerShadow {
            id: topInnerEffect

            anchors.fill: parent
            color: "#40000000"
            horizontalOffset: 2
            radius: 4
            samples: 16
            source: bottomInnerEffect
            spread: 0.3
            verticalOffset: 2
        }
        DropShadow {
            anchors.fill: parent
            color: "#40000000"
            horizontalOffset: 0
            radius: 4.0
            source: topInnerEffect
            verticalOffset: 0
        }
    }
    contentItem: Text {
        clip: root.clip
        color: Theme.deckTextColor
        elide: root.clip ? Text.ElideNone : Text.ElideRight
        font: root.font
        leftPadding: 5
        text: root.displayText
        verticalAlignment: Text.AlignVCenter
    }
    readonly property real itemHeight: Math.max(38, root.contentItem.height + 16)

    delegate: ItemDelegate {
        id: itemDlgt

        required property int index

        highlighted: root.highlightedIndex === this.index
        implicitHeight: root.itemHeight
        width: ListView.view ? ListView.view.width : root.popup.width

        background: Rectangle {
            border.color: itemDlgt.highlighted ? "#00e5ff" : "transparent"
            border.width: 1
            color: itemDlgt.down ? "#37474f" : (itemDlgt.highlighted ? "#263238" : "transparent")
            radius: 4
        }
        contentItem: Text {
            color: itemDlgt.highlighted ? "#ffffff" : Theme.deckTextColor
            elide: Text.ElideRight
            font.pixelSize: Math.max(12, root.font.pixelSize)
            font.bold: itemDlgt.highlighted
            text: root.textAt(itemDlgt.index)
            verticalAlignment: Text.AlignVCenter
            leftPadding: 10
            rightPadding: 10
        }

        onClicked: {
            root.currentIndex = itemDlgt.index;
            root.activated(itemDlgt.index);
            root.popup.close();
        }
    }
    popup: Popup {
        id: popupItem

        modal: true
        dim: false
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        focus: true

        width: root.width
        height: Math.min(
            root.itemHeight * Math.min(root.popupMaxItem, Math.max(root.count, 1)) + 16 + (root.footerItems.length > 0 ? root.itemHeight * root.footerItems.length : 0),
            root.Window.window ? root.Window.window.height * 0.75 : 450
        )
        padding: 0

        x: {
            if (!root.Window.window) return (root.width - width) / 2;
            const globalPos = root.mapToItem(null, 0, 0);
            if (!globalPos) return (root.width - width) / 2;
            if (globalPos.x + (root.width - width) / 2 < 8) {
                return -globalPos.x + 8;
            }
            const winWidth = root.Window.window.width;
            if (globalPos.x + (root.width + width) / 2 > winWidth - 8) {
                return winWidth - 8 - globalPos.x - width;
            }
            return (root.width - width) / 2;
        }

        y: {
            if (!root.Window.window) return root.height + 2;
            const globalPos = root.mapToItem(null, 0, root.height);
            if (!globalPos) return root.height + 2;
            const winHeight = root.Window.window.height;
            if (globalPos.y + height > winHeight - 10 && globalPos.y - height - root.height >= 10) {
                return -height - 2;
            }
            return root.height + 2;
        }

        onOpened: Qt.callLater(() => popupListView.positionViewAtIndex(Math.max(0, root.currentIndex), ListView.Contain))

        Overlay.modal: MouseArea {
            anchors.fill: parent
            preventStealing: true
            onPressed: popupItem.close()
        }

        background: Rectangle {
            color: "#181a20"
            border.color: "#37474f"
            border.width: 1
            radius: 8

            MouseArea {
                anchors.fill: parent
                preventStealing: true
                onPressed: (mouse) => mouse.accepted = true
            }
        }
        contentItem: Item {
            id: content

            anchors.fill: parent
            anchors.margins: 4

            ListView {
                id: popupListView
                anchors.fill: parent
                clip: true
                // Not bound to highlightedIndex: touching an item while
                // flicking would make the list jump to it.
                highlightFollowsCurrentItem: false
                model: root.popup.visible ? root.delegateModel : null
                interactive: true
                boundsBehavior: Flickable.StopAtBounds
                pixelAligned: true
                pressDelay: 60
                flickDeceleration: 2500
                maximumFlickVelocity: 6000

                ScrollBar.vertical: ScrollBar {
                    id: popupScrollBar
                    policy: popupListView.contentHeight > popupListView.height ? ScrollBar.AlwaysOn : ScrollBar.AsNeeded
                    width: 8
                    contentItem: Rectangle {
                        color: popupScrollBar.pressed ? "#00e5ff" : "#546e7a"
                        radius: 4
                    }
                }

                footer: Item {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: childrenRect.height

                    Repeater {
                        model: root.footerItems

                        Rectangle {
                            y: index * height
                            anchors.left: parent.left
                            anchors.right: parent.right
                            color: "transparent"
                            height: root.itemHeight

                            Item {
                                anchors.fill: parent
                                anchors.margins: 6

                                Text {
                                    anchors.bottom: parent.bottom
                                    anchors.top: parent.top
                                    color: Theme.deckTextColor
                                    elide: Text.ElideRight
                                    font: root.font
                                    text: modelData.text
                                    verticalAlignment: Text.AlignVCenter
                                    leftPadding: 8
                                }
                                Text {
                                    anchors.bottom: parent.bottom
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    color: Theme.deckTextColor
                                    font.pixelSize: 16
                                    text: modelData?.suffix || ''
                                    rightPadding: 8
                                }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                preventStealing: true

                                onPressed: {
                                    root.activateFooter(index);
                                    root.popup.close();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
