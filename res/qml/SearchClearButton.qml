import QtQuick
import QtQuick.Controls

// An X that empties a search field; Escape does the same. The field keeps
// the keyboard focus. It sits inside a TextField (its parent), or beside a
// TextInput: then it goes in the field's box with `field` set.
AbstractButton {
    id: root

    property Item field: parent

    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    anchors.rightMargin: 4
    width: 28
    height: 28
    focusPolicy: Qt.NoFocus
    visible: opacity > 0
    opacity: root.field.text.length > 0 ? 1 : 0
    scale: root.pressed ? 0.85 : 1

    Behavior on opacity {
        NumberAnimation {
            duration: 120
        }
    }
    Behavior on scale {
        NumberAnimation {
            duration: 80
        }
    }

    contentItem: Text {
        text: "\u2715"
        color: root.hovered || root.pressed ? "#ffffff" : "#a0a0a0"
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: width / 2
        color: root.pressed ? "#40ffffff" : root.hovered ? "#20ffffff" : "transparent"
    }

    onClicked: {
        root.field.clear();
        root.field.forceActiveFocus();
    }

    Component.onCompleted: {
        // Typed text stays clear of the X
        if (root.field === root.parent) {
            root.field.rightPadding = Math.max(root.field.rightPadding, root.width + 8);
        } else {
            root.field.anchors.rightMargin = root.width + 8;
        }
    }

    Shortcut {
        sequences: [StandardKey.Cancel]
        enabled: root.field.activeFocus && root.field.text.length > 0
        onActivated: root.field.clear()
    }
}
