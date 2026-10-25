import "../../Library" as LibraryComponent
import Mixxx 1.0 as Mixxx
import QtQuick

// The library sources of the booth skin: the same sources as the shared
// library (Library/SourceTree.qml), with fewer columns and rows drawn like
// the track list of a player. The cells of the selected row tell the
// browser about the track (infoTarget), for its info panel.
Mixxx.LibrarySourceTree {
    id: root

    // The BoothBrowse that shows these sources
    property var infoTarget: null

    readonly property Mixxx.TrackListColumn positionColumn: Mixxx.TrackListColumn {
        columnIdx: Mixxx.TrackListColumn.SQLColumns.Position
        label: "#"
        preferredWidth: 40

        delegate: TextCell {
            alignRight: true
            dim: true
        }
    }

    // The default columns with the position after the cover
    function playlistColumns() {
        const columns = [];
        for (let i = 0; i < root.defaultColumns.length; ++i) {
            columns.push(root.defaultColumns[i]);
            if (i === 0) {
                columns.push(root.positionColumn);
            }
        }
        return columns;
    }

    defaultColumns: [
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Album
            preferredWidth: 40
            role: Mixxx.TrackListColumn.Role.Cover

            delegate: RowCell {
                id: coverCell

                Image {
                    anchors.fill: parent
                    anchors.margins: 3
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                    source: cover_art
                }
                Binding {
                    property: "cover"
                    restoreMode: Binding.RestoreNone
                    target: root.infoTarget
                    value: cover_art
                    when: selected && !!root.infoTarget
                }
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Title
            fillSpan: 3
            label: qsTr("TRACK")
            role: Mixxx.TrackListColumn.Role.Title

            delegate: TextCell {
                infoKey: "title"
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 560
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Artist
            fillSpan: 2
            label: qsTr("ARTIST")
            role: Mixxx.TrackListColumn.Role.Artist

            delegate: TextCell {
                dim: true
                infoKey: "artist"
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Bpm
            label: "BPM"
            preferredWidth: 66

            delegate: TextCell {
                alignRight: true
                infoKey: "bpm"
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Key
            label: qsTr("KEY")
            preferredWidth: 58

            delegate: TextCell {
                alignRight: true
                infoKey: "key"
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 760
            columnIdx: Mixxx.TrackListColumn.SQLColumns.DateAdded
            label: qsTr("ADDED")
            preferredWidth: 96

            delegate: TextCell {
                alignRight: true
                dim: true
                infoKey: "added"
            }
        }
    ]

    Mixxx.LibraryAllTrackSource {
        columns: root.defaultColumns
        label: qsTr("All...")
    }
    Mixxx.LibraryFeatureSource {
        columns: root.playlistColumns()
        feature: Mixxx.LibraryFeatureSource.Feature.Playlists
    }
    Mixxx.LibraryFeatureSource {
        columns: root.defaultColumns
        feature: Mixxx.LibraryFeatureSource.Feature.Crates
    }
    Mixxx.LibraryFeatureSource {
        columns: root.playlistColumns()
        feature: Mixxx.LibraryFeatureSource.Feature.History
    }
    Mixxx.LibraryFeatureSource {
        columns: root.defaultColumns
        feature: Mixxx.LibraryFeatureSource.Feature.Browse
    }
    Mixxx.LibraryFeatureSource {
        columns: root.playlistColumns()
        feature: Mixxx.LibraryFeatureSource.Feature.Rekordbox
    }
    Mixxx.LibraryFeatureSource {
        columns: root.defaultColumns
        feature: Mixxx.LibraryFeatureSource.Feature.EngineDJ
    }
    Mixxx.LibraryFeatureSource {
        columns: root.defaultColumns
        feature: Mixxx.LibraryFeatureSource.Feature.Recordings
    }

    // A cell of a track row: dark rows, the selected one lit in blue. Tap
    // selects, double tap loads (see BoothBrowse.loadRow), long press opens
    // the track menu.
    component RowCell: Rectangle {
        id: rowCell

        readonly property var caps: capabilities

        color: selected ? "#1e5fb4" : (row % 2 === 0 ? "#0c0d0f" : "#16181b")
        implicitHeight: 40

        LibraryComponent.Track {
            anchors.fill: parent
            capabilities: rowCell.caps

            tap.onDoubleTapped: {
                tableView.selectionModel.selectRow(row);
                if (root.infoTarget) {
                    root.infoTarget.loadSelected(tableView);
                } else {
                    tableView.loadSelectedTrackIntoNextAvailableDeck(false);
                }
            }
            tap.onTapped: (eventPoint, button) => {
                tableView.selectionModel.selectRow(row);
            }
        }
        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: "#000000"
            height: 1
        }
    }
    component TextCell: RowCell {
        id: textCell

        property bool alignRight: false
        property bool dim: false
        // The info panel field this column fills, if any
        property string infoKey: ""

        Text {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            color: selected ? "#ffffff" : (textCell.dim ? "#9a9ea6" : "#e6e8eb")
            elide: Text.ElideRight
            font.pixelSize: 15
            horizontalAlignment: textCell.alignRight ? Text.AlignRight : Text.AlignLeft
            text: display ?? ""
            verticalAlignment: Text.AlignVCenter
        }
        Binding {
            property: textCell.infoKey
            restoreMode: Binding.RestoreNone
            target: root.infoTarget
            value: display ?? ""
            when: selected && !!root.infoTarget && textCell.infoKey.length > 0
        }
    }
}
