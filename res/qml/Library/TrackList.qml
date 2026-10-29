import ".." as Skin
import "." as LibraryComponent
import Mixxx 1.0 as Mixxx
import Qt.labs.qmlmodels
import QtQml
import QtQuick
import QtQml.Models
import QtQuick.Layouts
import QtQuick.Controls 2.15
import "../Theme"

Rectangle {
    id: root

    required property var model
    // Booth browse sets this so LOAD 1/2 (and the deck buttons below)
    // target the chosen deck; empty = next available deck.
    property string loadTarget: ""

    function focusView() {
        view.forceActiveFocus();
    }

    color: Theme.darkGray

    LibraryComponent.Control {
        id: libraryControl

        onFocusWidgetChanged: {
            switch (focusWidget) {
            case Skin.FocusedWidgetControl.WidgetKind.LibraryView:
                view.forceActiveFocus();
                break;
            }
        }
        onLoadSelectedTrack: (group, play) => {
            view.loadSelectedTrack(group, play);
        }
        onLoadSelectedTrackIntoNextAvailableDeck: play => {
            view.loadSelectedTrackIntoNextAvailableDeck(play);
        }
        onMoveVertical: offset => {
            view.selectionModel.moveSelectionVertical(offset);
        }
    }
    HorizontalHeaderView {
        id: horizontalHeader

        property int sortingColumn: -1
        property var sortingOrder: Qt.Descending

        anchors.left: parent.left
        anchors.margins: 5
        anchors.right: parent.right
        anchors.top: parent.top
        syncView: view

        delegate: Item {
            id: column

            required property string display
            required property int index

            implicitHeight: columnName.contentHeight + 5
            implicitWidth: columnName.contentWidth + 5

            MouseArea {
                id: columnMouseHandler

                acceptedButtons: Qt.LeftButton
                anchors.fill: parent

                onClicked: {
                    if (horizontalHeader.sortingColumn == index) {
                        horizontalHeader.sortingOrder = horizontalHeader.sortingOrder == Qt.DescendingOrder ? Qt.AscendingOrder : Qt.DescendingOrder;
                    } else {
                        horizontalHeader.sortingColumn = index;
                        horizontalHeader.sortingOrder = Qt.AscendingOrder;
                    }
                    view.model.sort(horizontalHeader.sortingColumn, horizontalHeader.sortingOrder);
                }
            }
            Text {
                id: columnName

                anchors.fill: parent
                anchors.leftMargin: 15
                color: Theme.textColor
                elide: Text.ElideRight
                font.capitalization: Font.Capitalize
                font.family: Theme.fontFamily
                font.pixelSize: 12
                font.weight: Font.Medium
                horizontalAlignment: Text.AlignLeft
                text: display
                verticalAlignment: Text.AlignVCenter
            }
            Item {
                anchors {
                    bottom: parent.bottom
                    left: parent.left
                    leftMargin: 5
                    top: parent.top
                }
                Label {
                    id: sortIndicator

                    anchors.centerIn: parent
                    color: "red"
                    elide: Text.ElideRight
                    font.bold: true
                    font.capitalization: Font.AllUppercase
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.buttonFontPixelSize
                    horizontalAlignment: Text.AlignRight
                    rotation: horizontalHeader.sortingOrder == Qt.AscendingOrder ? 90 : -90
                    text: "▶"
                    verticalAlignment: Text.AlignVCenter
                    visible: horizontalHeader.sortingColumn == index
                }
            }
            Rectangle {
                id: columnResizer

                color: Theme.darkGray2
                width: 1

                anchors {
                    bottom: parent.bottom
                    right: parent.right
                    top: parent.top
                }
                MouseArea {
                    id: columnResizeHandler

                    property int sizeOffset: 0

                    anchors.fill: parent
                    cursorShape: Qt.SizeHorCursor
                    preventStealing: true

                    onMouseXChanged: {
                        if (drag.active) {
                            column.width += mouseX;
                            sizeOffset += mouseX;
                        }
                    }

                    drag {
                        axis: Drag.XAxis
                        target: parent
                        threshold: 2

                        onActiveChanged: {
                            if (!drag.active && columnResizeHandler.sizeOffset !== 0) {
                                view.model.columns[index].preferredWidth = column.width;
                                columnResizeHandler.sizeOffset = 0;
                                view.updateColumnSize();
                                view.forceLayout();
                            }
                        }
                    }
                }
            }
        }
    }
    TableView {
        id: view

        property int dynamicColumnCount: 0
        property int usedWidth: 0

        // Keyboard navigation lives here (not on LibraryComponent.Control,
        // which is a QtObject and cannot carry Keys): view holds focus.
        Keys.onDownPressed: this.selectionModel.moveSelectionVertical(1)
        Keys.onEnterPressed: this.loadSelectedTrackIntoNextAvailableDeck(false)
        Keys.onReturnPressed: this.loadSelectedTrackIntoNextAvailableDeck(false)
        Keys.onUpPressed: this.selectionModel.moveSelectionVertical(-1)

        function loadSelectedTrack(group, play) {
            const urls = this.selectionModel.selectedTrackUrls();
            if (urls.length == 0)
                return;

            Mixxx.PlayerManager.getPlayer(group).loadTrackFromLocationUrl(urls[0], play);
        }
        function loadSelectedTrackIntoNextAvailableDeck(play) {
            const urls = this.selectionModel.selectedTrackUrls();
            if (urls.length == 0)
                return;

            if (root.loadTarget.length > 0) {
                Mixxx.PlayerManager.getPlayer(root.loadTarget).loadTrackFromLocationUrl(urls[0], play);
                return;
            }
            Mixxx.PlayerManager.loadLocationUrlIntoNextAvailableDeck(urls[0], play);
        }
        function updateColumnSize() {
            const oldUsedWidth = usedWidth;
            const oldDynamicColumnCount = dynamicColumnCount;
            usedWidth = 0;
            dynamicColumnCount = 0;
            if (model == null) {
                return;
            }
            for (let c = 0; c < model.columns.length; c++) {
                if (model.columns[c].hidden || model.columns[c].autoHideWidth > view.width) {
                    continue;
                } else if (model.columns[c].preferredWidth > 0) {
                    usedWidth += model.columns[c].preferredWidth;
                } else {
                    dynamicColumnCount += model.columns[c].fillSpan || 1;
                }
            }
            return oldDynamicColumnCount != dynamicColumnCount || oldUsedWidth != usedWidth;
        }

        anchors.bottom: quickLoadBar.top
        anchors.left: parent.left
        anchors.margins: 5
        anchors.right: parent.right
        anchors.top: horizontalHeader.bottom
        clip: true
        flickableDirection: Flickable.VerticalFlick
        boundsBehavior: Flickable.StopAtBounds
        interactive: true
        columnWidthProvider: function (column) {
            const columnDef = view.model.columns[column];
            if (columnDef.hidden) {
                return 0;
            }
            if (columnDef.autoHideWidth > 0 && columnDef.autoHideWidth > view.width) {
                return 0;
            }
            if (columnDef.preferredWidth >= 0) {
                return columnDef.preferredWidth;
            }
            const span = columnDef.fillSpan || 1;
            return span * (view.width - view.usedWidth) / view.dynamicColumnCount;
        }
        keyNavigationEnabled: false
        model: root.model
        pointerNavigationEnabled: false
        reuseItems: true

        ScrollBar.vertical: ScrollBar {
            id: verticalScrollBar

            policy: ScrollBar.AlwaysOn
            width: 16
            interactive: true
            contentItem: Rectangle {
                implicitWidth: 12
                implicitHeight: 48
                radius: 6
                color: verticalScrollBar.pressed ? "#00e5ff" : (verticalScrollBar.hovered ? "#78909c" : "#546e7a")
            }
            background: Rectangle {
                implicitWidth: 16
                color: "#181a20"
                opacity: 0.7
                radius: 4
            }
        }
        delegate: Item {
            id: item

            required property url cover_art
            required property color decoration
            required property var display
            required property string file_url
            required property int row
            required property bool selected
            required property var track

            implicitHeight: 38

            Loader {
                id: loader

                property var capabilities: root.model ? root.model.getCapabilities() : Mixxx.LibraryTrackListModel.Capability.None
                property url cover_art: item.cover_art
                property color decoration: item.decoration
                property var display: item.display
                property url file_url: item.file_url
                property int row: item.row
                property bool selected: item.selected
                property var tableView: view
                property var track: item.track

                anchors.fill: parent
                focus: true
                sourceComponent: delegate
                asynchronous: true
                opacity: loader.status == Loader.Ready

                Behavior on opacity { NumberAnimation { duration: 100; easing.type: Easing.Linear} }
            }
        }
        selectionModel: ItemSelectionModel {
            function moveSelectionVertical(value) {
                if (value == 0)
                    return;

                const selected = this.selectedIndexes;
                const oldRow = (selected.length == 0) ? 0 : selected[0].row;
                this.selectRow(oldRow + value);
            }
            function selectRow(row) {
                const rowCount = this.model.rowCount();
                if (rowCount == 0) {
                    this.clear();
                    return;
                }
                const newRow = Mixxx.MathUtils.positiveModulo(row, rowCount);
                this.select(this.model.index(newRow, 0), ItemSelectionModel.Rows | ItemSelectionModel.Select | ItemSelectionModel.Clear | ItemSelectionModel.Current);
                if (!view.isRowLoaded(newRow)) {
                    view.positionViewAtRow(newRow, TableView.Visible | TableView.AlignVCenter)
                }
            }
            function selectedTrackUrls() {
                return this.selectedIndexes.map(index => {
                    return this.model.getUrl(index.row);
                });
            }

            model: view.model
        }

        // Touch: double-tap loads the selected track (into loadTarget when
        // set by the Booth browser, else the next available deck). Single
        // taps select via the delegates' own handlers (Booth RowCell);
        // the quick-load bar below is the explicit touch load path.
        TapHandler {
            acceptedButtons: Qt.LeftButton
            gesturePolicy: TapHandler.ReleaseWithinBounds

            onDoubleTapped: {
                view.loadSelectedTrackIntoNextAvailableDeck(false);
            }
        }

        Component.onCompleted: this.updateColumnSize()
        onModelChanged: this.updateColumnSize()
        onWidthChanged: {
            if (view.updateColumnSize()) {
                // forceLayout is costly - only invoke if there was a change in the column layouts
                view.forceLayout();
            }
        }
    }

    Rectangle {
        id: quickLoadBar

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.sunkenBackgroundColor
        height: 48
        border.color: Theme.panelSplitterBackground
        border.width: 1

        // Refreshed on every selection change (a plain binding to
        // selectedTrackUrls() would never re-evaluate: it is a JS call).
        property var selectedUrls: []
        readonly property bool hasTrack: selectedUrls.length > 0

        Connections {
            target: view.selectionModel
            function onSelectionChanged() {
                quickLoadBar.selectedUrls = view.selectionModel.selectedTrackUrls();
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.margins: 5
            spacing: 8

            Rectangle {
                Layout.fillHeight: true
                Layout.fillWidth: true
                color: Theme.darkGray3
                radius: 4

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 6

                    Text {
                        Layout.fillWidth: true
                        color: quickLoadBar.hasTrack ? Theme.white : Theme.lightGray3
                        elide: Text.ElideRight
                        font.bold: quickLoadBar.hasTrack
                        font.pixelSize: 13
                        text: {
                            if (!quickLoadBar.hasTrack)
                                return qsTr("Select a track to load");
                            const urlStr = quickLoadBar.selectedUrls[0].toString();
                            return decodeURIComponent(urlStr.substring(urlStr.lastIndexOf('/') + 1));
                        }
                    }
                }
            }

            Skin.Button {
                id: loadDeck1Btn

                Layout.preferredHeight: 38
                Layout.preferredWidth: 115
                activeColor: "#64b5f6"
                backgroundColor: quickLoadBar.hasTrack ? "#1565c0" : "#262930"
                normalColor: quickLoadBar.hasTrack ? "#ffffff" : "#78909c"
                highlight: quickLoadBar.hasTrack
                enabled: quickLoadBar.hasTrack
                font.bold: true
                text: "\u25C0 DECK 1"

                onClicked: {
                    view.loadSelectedTrack(root.loadTarget.length > 0 ? root.loadTarget : "[Channel1]", false);
                }
            }

            Skin.Button {
                id: loadDeck2Btn

                Layout.preferredHeight: 38
                Layout.preferredWidth: 115
                activeColor: "#ffb74d"
                backgroundColor: quickLoadBar.hasTrack ? "#e65100" : "#262930"
                normalColor: quickLoadBar.hasTrack ? "#ffffff" : "#78909c"
                highlight: quickLoadBar.hasTrack
                enabled: quickLoadBar.hasTrack
                font.bold: true
                text: "DECK 2 \u25B6"

                onClicked: {
                    view.loadSelectedTrack(root.loadTarget.length > 0 ? root.loadTarget : "[Channel2]", false);
                }
            }
        }
    }
}
