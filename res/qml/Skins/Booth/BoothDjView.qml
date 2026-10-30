import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts

// The DJ view of the booth skin, for playing without a controller: the two
// decks with the mixer between them, like the PERFORMANCE layout of
// rekordbox, and below them the headphones, the crossfader and the master
// level.
Item {
    id: root

    property bool compact: false
    readonly property int gap: 3

    ColumnLayout {
        anchors.fill: parent
        spacing: root.gap

        RowLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: root.gap

            BoothPerformanceDeck {
                Layout.fillHeight: true
                Layout.fillWidth: true
                compact: root.compact
                deckNumber: 1
                group: "[Channel1]"
            }
            BoothMixer {
                Layout.fillHeight: true
                Layout.preferredWidth: Math.round(Math.min(root.width * 0.24, root.height * 0.9))
            }
            BoothPerformanceDeck {
                Layout.fillHeight: true
                Layout.fillWidth: true
                compact: root.compact
                deckNumber: 2
                group: "[Channel2]"
                mirrored: true
            }
        }

        // ---- Headphones, crossfader and master ----
        Rectangle {
            id: bottomBar

            readonly property real knobSize: height - 4

            Layout.fillWidth: true
            Layout.preferredHeight: root.compact ? 30 : 40
            border.color: "#2a2c30"
            border.width: 1
            color: "#0c0d0f"

            Row {
                anchors.left: parent.left
                anchors.leftMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#9a9ea6"
                    font.pixelSize: Math.round(bottomBar.height * 0.45)
                    text: "🎧"
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#9a9ea6"
                    font.pixelSize: Math.round(bottomBar.height * 0.28)
                    text: qsTr("MIXING")
                }
                BoothKnob {
                    group: "[Master]"
                    height: bottomBar.knobSize
                    key: "headMix"
                    width: bottomBar.knobSize
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#9a9ea6"
                    font.pixelSize: Math.round(bottomBar.height * 0.28)
                    text: qsTr("LEVEL")
                }
                BoothKnob {
                    bipolar: false
                    group: "[Master]"
                    height: bottomBar.knobSize
                    key: "headGain"
                    width: bottomBar.knobSize
                }
            }
            BoothFader {
                anchors.centerIn: parent
                centerMark: true
                group: "[Master]"
                height: bottomBar.height - 6
                horizontal: true
                key: "crossfader"
                width: Math.round(Math.min(parent.width * 0.3, 320))
            }
            Row {
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: "#9a9ea6"
                    font.pixelSize: Math.round(bottomBar.height * 0.28)
                    text: qsTr("MASTER")
                }
                BoothKnob {
                    bipolar: false
                    group: "[Master]"
                    height: bottomBar.knobSize
                    key: "gain"
                    width: bottomBar.knobSize
                }
            }
        }
    }
}
