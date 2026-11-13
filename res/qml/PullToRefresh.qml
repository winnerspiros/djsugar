import QtQuick

// Pull down at the top of a list to refresh it; F5 does the same while
// the list is shown. Declared inside the list with `flickable` set to it and
// `busy` bound to the view's loading state; it sits on the list itself, so it
// does not scroll with the rows.
Item {
    id: root

    required property Flickable flickable
    property bool busy: false
    // Pull distance that triggers a refresh, in pixels
    property real threshold: 80
    readonly property real pull: root.flickable ? Math.max(0, -root.flickable.verticalOvershoot) : 0
    property bool armed: false
    // Set by a pull or F5: background loading (a scan at start, a list
    // opening) never shows the indicator
    property bool refreshing: false

    signal refresh

    function trigger() {
        root.armed = false;
        if (root.busy) {
            return;
        }
        root.refreshing = true;
        root.refresh();
        // The indicator shows at least briefly, even for an instant refresh
        holdTimer.restart();
    }

    parent: root.flickable
    x: root.flickable ? (root.flickable.width - root.width) / 2 : 0
    width: 36
    height: 36
    z: 10

    readonly property bool spinning: root.refreshing && (root.busy || holdTimer.running)
    onSpinningChanged: {
        if (!root.spinning) {
            root.refreshing = false;
        }
    }
    // Follows the finger with resistance, rests at 16 px while spinning
    property real offset: root.spinning ? 16 : Math.min(root.pull * 0.6, root.threshold * 0.9) - root.height
    y: offset
    opacity: root.spinning ? 1 : Math.min(1, root.pull / root.threshold)
    visible: opacity > 0.01
    scale: root.armed || root.spinning ? 1 : 0.7 + 0.3 * Math.min(1, root.pull / root.threshold)

    Behavior on offset {
        enabled: !root.flickable || !root.flickable.dragging
        SpringAnimation {
            spring: 4
            damping: 0.35
        }
    }
    Behavior on opacity {
        enabled: !root.flickable || !root.flickable.dragging
        NumberAnimation {
            duration: 150
        }
    }
    Behavior on scale {
        NumberAnimation {
            duration: 120
            easing.type: Easing.OutBack
        }
    }

    Timer {
        id: holdTimer

        interval: 450
    }

    Connections {
        target: root.flickable

        function onVerticalOvershootChanged() {
            if (root.flickable.dragging) {
                root.armed = root.pull >= root.threshold;
            }
        }
        function onDraggingChanged() {
            if (!root.flickable.dragging && root.armed) {
                root.trigger();
            }
        }
    }

    Shortcut {
        sequences: ["F5"]
        enabled: root.flickable && root.flickable.visible && root.flickable.enabled
        onActivated: root.trigger()
    }

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: "#2a2f38"
        border.color: root.armed || root.spinning ? "#00e5ff" : "#4a505a"
        border.width: 1

        // Arrow while pulling (turns over once far enough), arc while busy
        Text {
            anchors.centerIn: parent
            visible: !root.spinning
            text: "\u2193"
            color: root.armed ? "#00e5ff" : "#c0c0c0"
            font.pixelSize: 18
            rotation: root.armed ? 180 : Math.min(1, root.pull / root.threshold) * 120

            Behavior on rotation {
                NumberAnimation {
                    duration: 120
                }
            }
        }
        Canvas {
            id: arc

            anchors.fill: parent
            anchors.margins: 8
            visible: root.spinning
            onPaint: {
                const ctx = getContext("2d");
                ctx.reset();
                ctx.strokeStyle = "#00e5ff";
                ctx.lineWidth = 2.5;
                ctx.lineCap = "round";
                ctx.beginPath();
                ctx.arc(width / 2, height / 2, width / 2 - 2, 0, Math.PI * 1.5);
                ctx.stroke();
            }

            RotationAnimator on rotation {
                running: arc.visible
                from: 0
                to: 360
                duration: 700
                loops: Animation.Infinite
            }
        }
    }
}
