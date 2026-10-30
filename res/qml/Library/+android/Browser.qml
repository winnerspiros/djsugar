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

    // Reading the databases on USB drives needs more than the music permission
    signal storageAccessNeeded()

    function checkStorageAccess(index) {
        // SidebarModel::IconNameRole
        const icon = root.model.data(index, Qt.UserRole + 1);
        if (icon !== "rekordbox" && icon !== "enginedj") {
            return;
        }
        if (!Mixxx.MobileLibrary.hasFullStorageAccess()) {
            root.storageAccessNeeded();
        }
        // Opened on purpose: asks again for the USB permission of sticks
        // whose earlier request was dismissed
        Mixxx.MobileLibrary.allowUsbPermissionRequests();
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

                        Rectangle {
                            id: background

                            anchors.fill: parent
                            color: depth == 0 ? Theme.midGray2 : 'transparent'

                            MouseArea {
                                id: rowMouseArea

                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                anchors.fill: parent
                                hoverEnabled: true

                                onClicked: event => {
                                    treeView.selectionModel.setCurrentIndex(treeView.index(row, 0), ItemSelectionModel.ClearAndSelect | ItemSelectionModel.Rows);
                                    root.checkStorageAccess(index);
                                    root.activate(index);
                                    if (isTreeNode && hasChildren) {
                                        treeView.toggleExpanded(row);
                                    }
                                    event.accepted = true;
                                }
                            }
                            Rectangle {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.leftMargin: 10 + 15 * depth
                                anchors.right: parent.right
                                anchors.top: parent.top
                                color: current ? Theme.midGray : 'transparent'
                                width: 25

                                // The features give the name of their icon
                                Image {
                                    id: lineIcon

                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    height: 22
                                    source: depth == 0 && icon ? "qrc:/images/library/ic_library_" + icon + ".svg" : ""
                                    sourceSize.height: 22
                                    sourceSize.width: 22
                                    visible: depth == 0 && !!icon
                                    width: 22
                                }
                                Label {
                                    id: indicator

                                    color: Theme.textColor
                                    // "▶" is drawn as an emoji on Android
                                    font.bold: true
                                    font.pixelSize: 22
                                    text: "›"
                                    visible: isTreeNode && hasChildren

                                    anchors {
                                        right: parent.right
                                        rightMargin: 12
                                        verticalCenter: parent.verticalCenter
                                    }
                                }
                                Label {
                                    id: labelItem

                                    anchors.left: parent.left
                                    anchors.leftMargin: lineIcon.visible ? 32 : 8
                                    anchors.right: parent.right
                                    anchors.rightMargin: 30
                                    anchors.verticalCenter: parent.verticalCenter
                                    clip: true
                                    elide: Text.ElideRight
                                    color: Theme.textColor
                                    font.pixelSize: 14
                                    font.weight: depth == 0 ? Font.Bold : Font.Medium
                                    text: label
                                }
                                Item {
                                    id: newItem

                                    height: parent.height
                                    visible: rowMouseArea.containsMouse && isTreeNode && hasChildren

                                    anchors {
                                        right: parent.right
                                        rightMargin: 10
                                        verticalCenter: parent.verticalCenter
                                    }
                                    Rectangle {
                                        anchors.centerIn: parent
                                        height: parent.height
                                        width: 30

                                        gradient: Gradient {
                                            orientation: Gradient.Horizontal

                                            GradientStop {
                                                color: Theme.sunkenBackgroundColor
                                                position: 1
                                            }
                                            GradientStop {
                                                color: 'transparent'
                                                position: 0
                                            }
                                        }
                                    }
                                    Rectangle {
                                        anchors.right: parent.right
                                        anchors.rightMargin: 5
                                        anchors.verticalCenter: parent.verticalCenter
                                        border.color: Theme.white
                                        border.width: 2
                                        color: 'transparent'
                                        height: 20
                                        radius: 20
                                        width: 20

                                        Shape {
                                            anchors.fill: parent
                                            anchors.margins: 4

                                            ShapePath {
                                                capStyle: ShapePath.RoundCap
                                                fillColor: Theme.white
                                                startX: 6
                                                startY: 0
                                                strokeWidth: 2

                                                PathLine {
                                                    x: 6
                                                    y: 12
                                                }
                                                PathLine {
                                                    x: 6
                                                    y: 8
                                                }
                                            }
                                            ShapePath {
                                                capStyle: ShapePath.RoundCap
                                                fillColor: Theme.white
                                                startX: 0
                                                startY: 6
                                                strokeWidth: 2

                                                PathLine {
                                                    x: 12
                                                    y: 6
                                                }
                                                PathLine {
                                                    x: 8
                                                    y: 6
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
