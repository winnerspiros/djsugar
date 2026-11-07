import QtQuick
import Mixxx 1.0 as Mixxx
import "." as LibraryComponent
import "../Theme"

Mixxx.LibrarySourceTree {
    id: root

    // The position in the playlist, for the sources made of playlists
    readonly property Mixxx.TrackListColumn positionColumn: Mixxx.TrackListColumn {
        columnIdx: Mixxx.TrackListColumn.SQLColumns.Position
        label: qsTr("#")
        preferredWidth: 46

        delegate: DefaultDelegate {
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
            autoHideWidth: 750
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Album
            preferredWidth: 100

            delegate: Rectangle {
                color: decoration
                implicitHeight: 38

                Image {
                    anchors.fill: parent
                    asynchronous: true
                    clip: true
                    fillMode: Image.PreserveAspectCrop
                    source: cover_art
                }
            }
        },
        // FIXME: WaveformOverview is currently disabled due to performance limitation. Like for the legacy UI, a cache likely needs to be implemented to help
        // Mixxx.TrackListColumn {
        //     label: qsTr("Preview")
        //     fillSpan: 3
        //     preferredWidth: 300
        //     columnIdx: Mixxx.TrackListColumn.SQLColumns.Title

        //     delegate: LibraryCell {
        //         // implicitHeight: 30
        //         anchors.fill: parent

        //         readonly property var trackProxy: track

        //         Drag.active: dragArea.drag.active
        //         Drag.dragType: Drag.Automatic
        //         Drag.supportedActions: Qt.CopyAction
        //         Drag.mimeData: {
        //             "text/uri-list": file_url,
        //             "text/plain": file_url
        //         }

        //         LibraryComponent.Track {
        //             id: dragArea
        //             anchors.fill: parent
        //             capabilities: parent.capabilities

        //             onPressed: {
        //                 if (pressedButtons == Qt.LeftButton) {
        //                     tableView.selectionModel.selectRow(row);
        //                     parent.dragImage.grabToImage((result) => {
        //                             parent.Drag.imageSource = result.url;
        //                     });
        //                 } else {
        //                 }
        //             }
        //             onDoubleClicked: {
        //                 tableView.selectionModel.selectRow(row);
        //                 tableView.loadSelectedTrackIntoNextAvailableDeck(false);
        //             }
        //         }

        //         Mixxx.WaveformOverview {
        //             anchors.fill: parent
        //             channels: Mixxx.WaveformOverview.Channels.LeftChannel
        //             renderer: Mixxx.WaveformOverview.Renderer.Filtered
        //             colorHigh: Theme.white
        //             colorMid: Theme.blue
        //             colorLow: Theme.green
        //             track: trackProxy
        //         }
        //         Rectangle {
        //             id: border
        //             color: Theme.darkGray2
        //             width: 1
        //             anchors {
        //                 top: parent.top
        //                 bottom: parent.bottom
        //                 right: parent.right
        //             }
        //         }
        //     }

        // },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Title
            fillSpan: 3
            label: qsTr("Title")

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Artist
            fillSpan: 2
            label: qsTr("Artist")

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 690
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Album
            fillSpan: 1
            label: qsTr("Album")

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 750
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Year
            label: qsTr("Year")
            preferredWidth: 80

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Bpm
            label: qsTr("Bpm")
            preferredWidth: 60

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Key
            label: qsTr("Key")
            preferredWidth: 70

            delegate: KeyDelegate {
            }
        },
        // Sortable, e.g. to build a playlist from the newest tracks of a
        // folder (where it is the time the file was last modified)
        Mixxx.TrackListColumn {
            columnIdx: Mixxx.TrackListColumn.SQLColumns.DateAdded
            label: qsTr("Added")
            preferredWidth: 150

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 900
            columnIdx: Mixxx.TrackListColumn.SQLColumns.FileType
            label: qsTr("File Type")
            preferredWidth: 70

            delegate: DefaultDelegate {
            }
        },
        Mixxx.TrackListColumn {
            autoHideWidth: 1200
            columnIdx: Mixxx.TrackListColumn.SQLColumns.Bitrate
            label: qsTr("Bitrate")
            preferredWidth: 70

            delegate: DefaultDelegate {
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

    component DefaultDelegate: LibraryComponent.Cell {
        id: cell

        readonly property var caps: capabilities

        LibraryComponent.Track {
            id: dragArea

            anchors.fill: parent
            capabilities: cell.caps

            tap.onDoubleTapped: {
                tableView.selectionModel.selectRow(row);
                tableView.loadSelectedTrackIntoNextAvailableDeck(false);
            }
            tap.onTapped: (eventPoint, button) => {
                tableView.selectionModel.selectRow(row);
            }
        }
        Text {
            id: value

            anchors.fill: parent
            anchors.leftMargin: 15
            color: Theme.textColor
            elide: Text.ElideRight
            font.pixelSize: 14
            text: display ?? ""
            verticalAlignment: Text.AlignVCenter
        }
    }

    resources: [
        Mixxx.ControlProxy {
            id: deck1KeyProxy
            group: "[Channel1]"
            key: "visual_key"
        },
        Mixxx.ControlProxy {
            id: deck2KeyProxy
            group: "[Channel2]"
            key: "visual_key"
        }
    ]

    function checkCamelotCompatibility(trackKey) {
        if (!trackKey || trackKey === "") return 0;
        var d1 = (deck1KeyProxy.value || "").toString().trim().toUpperCase();
        var d2 = (deck2KeyProxy.value || "").toString().trim().toUpperCase();
        var tk = trackKey.toString().trim().toUpperCase();

        function matchOne(deckKey, tKey) {
            if (!deckKey || deckKey === "") return 0;
            if (deckKey === tKey) return 2; // Perfect exact match

            var m1 = deckKey.match(/^(\d{1,2})([AB])$/);
            var m2 = tKey.match(/^(\d{1,2})([AB])$/);
            if (m1 && m2) {
                var num1 = parseInt(m1[1]);
                var let1 = m1[2];
                var num2 = parseInt(m2[1]);
                var let2 = m2[2];

                if (let1 === let2) {
                    var diff = Math.abs(num1 - num2);
                    if (diff === 1 || diff === 11) return 1; // Harmonic match (+/- 1 step)
                }
                if (num1 === num2 && let1 !== let2) return 1; // Relative major/minor
            }
            return 0;
        }

        var res1 = matchOne(d1, tk);
        var res2 = matchOne(d2, tk);
        return Math.max(res1, res2);
    }

    component KeyDelegate: LibraryComponent.Cell {
        id: keyCell

        readonly property var caps: capabilities
        readonly property int compat: root.checkCamelotCompatibility(display)

        LibraryComponent.Track {
            id: keyDragArea
            anchors.fill: parent
            capabilities: keyCell.caps

            tap.onDoubleTapped: {
                tableView.selectionModel.selectRow(row);
                tableView.loadSelectedTrackIntoNextAvailableDeck(false);
            }
            tap.onTapped: (eventPoint, button) => {
                tableView.selectionModel.selectRow(row);
            }
        }

        Rectangle {
            anchors.centerIn: parent
            width: Math.min(parent.width - 6, keyText.implicitWidth + (keyCell.compat === 2 ? 22 : 14))
            height: 24
            radius: 4
            color: keyCell.compat === 2 ? Theme.libraryKeyMatchBackground : (keyCell.compat === 1 ? Theme.libraryKeyHarmonicBackground : "transparent")
            border.color: keyCell.compat === 2 ? Theme.libraryKeyMatchColor : (keyCell.compat === 1 ? Theme.libraryKeyHarmonicColor : "transparent")
            border.width: keyCell.compat > 0 ? 1 : 0

            Row {
                anchors.centerIn: parent
                spacing: 3

                Text {
                    visible: keyCell.compat === 2
                    text: "★"
                    font.pixelSize: 10
                    color: Theme.libraryKeyMatchColor
                }

                Text {
                    id: keyText
                    font.bold: keyCell.compat > 0
                    font.pixelSize: 12
                    color: keyCell.compat === 2 ? Theme.libraryKeyMatchColor : (keyCell.compat === 1 ? Theme.libraryKeyHarmonicText : Theme.textColor)
                    text: display ?? ""
                }
            }
        }
    }
}
