import QtQuick
import QtQuick.Controls

// An X that empties a search field; Escape does the same. It sits inside a
// TextField (its parent), or beside a field with `field` set.
AbstractButton {
    id: root

    property Item field: parent
    // Off where the field handles Escape itself
    property bool clearOnEscape: true
    // The field keeps its typed text clear of the X: by padding (else the
    // field is narrowed)
    property bool padField: field === parent

    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    anchors.rightMargin: 4
    width: 28
    height: 28
    focusPolicy: Qt.NoFocus
    visible: opacity > 0
    // displayText: the word the keyboard is still composing counts too
    opacity: root.field.displayText.length > 0 ? 1 : 0
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

    // Drawn, not a font glyph: Android fonts have no \u2715
    contentItem: Item {
        Repeater {
            model: [45, -45]

            Rectangle {
                required property int modelData

                anchors.centerIn: parent
                width: 14
                height: 2
                radius: 1
                rotation: modelData
                antialiasing: true
                color: root.hovered || root.pressed ? "#ffffff" : "#c8c8c8"
            }
        }
    }
    background: Rectangle {
        radius: width / 2
        color: root.pressed ? "#40ffffff" : root.hovered ? "#20ffffff" : "transparent"
    }

    // Clears only: the keyboard stays as it was (closed stays closed)
    onClicked: root.clearField()

    // Emptied first: the keyboard then restarts on the empty field instead of
    // committing its composing word back into it
    function clearField() {
        root.field.clear();
        Qt.inputMethod.reset();
        if (root.field.displayText.length > 0) {
            root.field.clear();
        }
    }

    Component.onCompleted: {
        // Typed text stays clear of the X
        if (root.padField) {
            root.field.rightPadding = Math.max(root.field.rightPadding, root.width + 8);
        } else {
            root.field.anchors.rightMargin = root.width + 8;
        }
    }

    Shortcut {
        sequences: [StandardKey.Cancel]
        enabled: root.clearOnEscape && root.field.activeFocus && root.field.displayText.length > 0
        onActivated: root.clearField()
    }
}
