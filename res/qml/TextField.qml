import QtQuick 2.12
import QtQuick.Controls 2.12
import "Theme"

TextField {
    color: Theme.textColor
    // Android keyboards hold the word being typed as preedit text, outside
    // `text`, so buttons enabled by the text stayed off until a space
    inputMethodHints: Qt.ImhNoPredictiveText

    background: Rectangle {
        anchors.fill: parent
        color: Theme.embeddedBackgroundColor
        radius: 5
    }
}
