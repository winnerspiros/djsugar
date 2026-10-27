import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import Mixxx 1.0 as Mixxx
import "." as Skin
import "Theme"

Rectangle {
    id: root

    property string targetDeck: "[Channel1]"
    property bool holdMode: false
    property real currentX: 0.5 // 0.0 (LPF) ... 0.5 (Neutral) ... 1.0 (HPF)
    property real currentY: 0.0 // 0.0 (Dry) ... 1.0 (Wet)
    property bool isTouching: false
    // Shown as a page of its own instead of over the decks
    property bool closable: true

    // The look, for the skins
    property string title: qsTr("⚡ XY TOUCH PAD")
    property color accentColor: "#00e5ff"
    property color deck1Color: "#00b0ff"
    property color deck2Color: "#ff6d00"
    property color wetColor: "#e040fb"
    property color holdColor: "#ab47bc"
    property color buttonColor: "#1a222d"
    property color buttonBorderColor: "#303e4f"
    property color buttonTextColor: "#90a4ae"
    property color surfaceColor: "#080b10"
    property color surfaceBorderColor: "#1f2937"
    property int controlRadius: 4
    property int surfaceRadius: 8

    signal closeRequested()

    color: "#e60a0d14"
    radius: 12
    border.color: Qt.alpha(root.accentColor, 0.4)
    border.width: 1

    Mixxx.ControlProxy {
        id: filterProxy
        group: "[QuickEffectRack1_" + root.targetDeck + "]"
        key: "super1"
    }
    Mixxx.ControlProxy {
        id: filterDeck1Proxy
        group: "[QuickEffectRack1_[Channel1]]"
        key: "super1"
    }
    Mixxx.ControlProxy {
        id: filterDeck2Proxy
        group: "[QuickEffectRack1_[Channel2]]"
        key: "super1"
    }

    Mixxx.ControlProxy {
        id: fxMixProxy
        group: "[EffectRack1_EffectUnit1]"
        key: "mix"
    }

    // One proxy per deck, so the routing of the previous deck can be
    // restored after the target deck changed
    Mixxx.ControlProxy {
        id: routingDeck1Proxy
        group: "[EffectRack1_EffectUnit1]"
        key: "group_[Channel1]_enable"
    }
    Mixxx.ControlProxy {
        id: routingDeck2Proxy
        group: "[EffectRack1_EffectUnit1]"
        key: "group_[Channel2]_enable"
    }

    function filterProxyFor(deck) {
        return deck === "[Channel2]" ? filterDeck2Proxy : filterDeck1Proxy;
    }

    function routingProxyFor(deck) {
        return deck === "[Channel2]" ? routingDeck2Proxy : routingDeck1Proxy;
    }

    Mixxx.ControlProxy {
        id: slot1EnabledProxy
        group: "[EffectRack1_EffectUnit1_Effect1]"
        key: "enabled"
    }
    Mixxx.ControlProxy {
        id: slot2EnabledProxy
        group: "[EffectRack1_EffectUnit1_Effect2]"
        key: "enabled"
    }
    Mixxx.ControlProxy {
        id: slot3EnabledProxy
        group: "[EffectRack1_EffectUnit1_Effect3]"
        key: "enabled"
    }

    // State before the pad was touched, restored in SNAP mode
    property bool hasSavedState: false
    property real savedFilter: 0.5
    property real savedMix: 0.0
    property real savedRouting: 0
    property string savedDeck: ""
    property bool enabledSlotForPad: false

    function saveState() {
        if (root.hasSavedState) {
            return;
        }
        root.savedFilter = filterProxy.value;
        root.savedMix = fxMixProxy.value;
        root.savedDeck = root.targetDeck;
        root.savedRouting = routingProxyFor(root.savedDeck).value;
        // The pad must be audible: route the effect unit to the deck and
        // switch on its first effect if none is on
        routingProxyFor(root.savedDeck).value = 1;
        root.enabledSlotForPad = !slot1EnabledProxy.value && !slot2EnabledProxy.value && !slot3EnabledProxy.value;
        if (root.enabledSlotForPad) {
            slot1EnabledProxy.value = 1;
        }
        root.hasSavedState = true;
    }

    function restoreState() {
        if (!root.hasSavedState) {
            return;
        }
        filterProxyFor(root.savedDeck).value = root.savedFilter;
        fxMixProxy.value = root.savedMix;
        routingProxyFor(root.savedDeck).value = root.savedRouting;
        if (root.enabledSlotForPad) {
            slot1EnabledProxy.value = 0;
        }
        root.hasSavedState = false;
    }

    function applyValues(nx, ny) {
        root.currentX = Math.max(0.0, Math.min(1.0, nx));
        root.currentY = Math.max(0.0, Math.min(1.0, ny));

        filterProxy.value = root.currentX;
        // Only the dry/wet mix: super1 drives the linked effect parameters
        // and the controller's own knobs
        fxMixProxy.value = root.currentY;
    }

    function resetNeutral() {
        if (root.hasSavedState) {
            restoreState();
        } else {
            // Neutral filter, effect off
            filterProxy.value = 0.5;
            fxMixProxy.value = 0.0;
        }
        root.currentX = filterProxy.value;
        root.currentY = fxMixProxy.value;
    }

    onTargetDeckChanged: {
        // The saved state belongs to the previous deck
        restoreState();
    }

    onVisibleChanged: {
        if (visible) {
            root.currentX = filterProxy.value;
            root.currentY = fxMixProxy.value;
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 10
        spacing: 8

        // ================= HEADER =================
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                text: root.title
                font.bold: true
                font.pixelSize: 13
                font.letterSpacing: 1
                color: root.accentColor
            }

            Item { Layout.fillWidth: true }

            // Deck 1 / Deck 2 Selector
            Rectangle {
                Layout.preferredHeight: 28
                Layout.preferredWidth: 64
                radius: root.controlRadius
                color: root.targetDeck === "[Channel1]" ? root.deck1Color : root.buttonColor
                border.color: root.targetDeck === "[Channel1]" ? "#ffffff" : root.buttonBorderColor
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: "DECK 1"
                    font.bold: true
                    font.pixelSize: 10
                    color: root.targetDeck === "[Channel1]" ? "#001018" : root.buttonTextColor
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: root.targetDeck = "[Channel1]"
                }
            }

            Rectangle {
                Layout.preferredHeight: 28
                Layout.preferredWidth: 64
                radius: root.controlRadius
                color: root.targetDeck === "[Channel2]" ? root.deck2Color : root.buttonColor
                border.color: root.targetDeck === "[Channel2]" ? "#ffffff" : root.buttonBorderColor
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: "DECK 2"
                    font.bold: true
                    font.pixelSize: 10
                    color: root.targetDeck === "[Channel2]" ? "#000000" : root.buttonTextColor
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: root.targetDeck = "[Channel2]"
                }
            }

            // Hold Toggle Button
            Rectangle {
                Layout.preferredHeight: 28
                Layout.preferredWidth: 62
                radius: root.controlRadius
                color: root.holdMode ? root.holdColor : root.buttonColor
                border.color: root.holdMode ? "#ffffff" : root.buttonBorderColor
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: root.holdMode ? "🔒 HOLD" : "SNAP"
                    font.bold: true
                    font.pixelSize: 10
                    color: root.holdMode ? "#ffffff" : root.buttonTextColor
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: root.holdMode = !root.holdMode
                }
            }

            // Reset Button
            Rectangle {
                Layout.preferredHeight: 28
                Layout.preferredWidth: 54
                radius: root.controlRadius
                color: resetMouse.pressed ? "#455a64" : "#263238"
                border.color: "#455a64"
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: qsTr("RESET")
                    font.bold: true
                    font.pixelSize: 10
                    color: "#cfd8dc"
                }

                MouseArea {
                    id: resetMouse
                    anchors.fill: parent
                    onClicked: root.resetNeutral()
                }
            }

            // Close Button
            Rectangle {
                visible: root.closable
                Layout.preferredHeight: 28
                Layout.preferredWidth: 28
                radius: 14
                color: closeMouse.pressed ? "#b71c1c" : "#374151"

                Text {
                    anchors.centerIn: parent
                    text: "✕"
                    font.bold: true
                    font.pixelSize: 12
                    color: "#ffffff"
                }

                MouseArea {
                    id: closeMouse
                    anchors.fill: parent
                    onClicked: {
                        if (!root.holdMode) {
                            root.resetNeutral();
                        }
                        root.closeRequested();
                    }
                }
            }
        }

        // ================= 2D TOUCH PAD SURFACE =================
        Rectangle {
            id: touchSurface
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: root.surfaceRadius
            color: root.surfaceColor
            border.color: root.isTouching ? root.accentColor : root.surfaceBorderColor
            border.width: 1
            clip: true

            // Grid lines
            Rectangle {
                // Vertical Center (Neutral Filter)
                x: touchSurface.width * 0.5 - 0.5
                y: 0
                width: 1
                height: touchSurface.height
                color: Qt.alpha(root.accentColor, 0.2)
            }

            Rectangle {
                // Horizontal Center
                x: 0
                y: touchSurface.height * 0.5 - 0.5
                width: touchSurface.width
                height: 1
                color: Qt.rgba(1, 1, 1, 0.08)
            }

            // Subtle quadrant cross lines
            Repeater {
                model: 3
                Rectangle {
                    x: touchSurface.width * ((index + 1) * 0.25)
                    y: 0
                    width: 1
                    height: touchSurface.height
                    color: Qt.rgba(1, 1, 1, 0.03)
                    visible: index !== 1
                }
            }

            // Axis Labels
            Skin.ArrowText {
                arrowLeft: true
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("LOW PASS")
                font.bold: true
                font.pixelSize: 11
                color: Qt.alpha(root.deck1Color, 0.4)
            }

            Skin.ArrowText {
                arrowRight: true
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("HIGH PASS")
                font.bold: true
                font.pixelSize: 11
                color: Qt.alpha(root.deck2Color, 0.4)
            }

            // The arrow is drawn: Android's fonts lack ▲
            Row {
                anchors.top: parent.top
                anchors.topMargin: 8
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 4

                Skin.Triangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 7
                    height: 8
                    rotation: -90
                    color: Qt.alpha(root.wetColor, 0.53)
                }
                Text {
                    text: "FX WET (100%)"
                    font.bold: true
                    font.pixelSize: 10
                    color: Qt.alpha(root.wetColor, 0.53)
                }
            }

            // The arrow is drawn: Android's fonts lack ▼
            Row {
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 8
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 4

                Skin.Triangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 7
                    height: 8
                    rotation: 90
                    color: Qt.alpha(root.buttonTextColor, 0.4)
                }
                Text {
                    text: "FX DRY (0%)"
                    font.bold: true
                    font.pixelSize: 10
                    color: Qt.alpha(root.buttonTextColor, 0.4)
                }
            }

            // Current Coordinates Crosshair lines
            Rectangle {
                x: Math.round(root.currentX * touchSurface.width) - 0.5
                y: 0
                width: 1
                height: touchSurface.height
                color: root.isTouching ? root.accentColor : Qt.alpha(root.accentColor, 0.4)
            }

            Rectangle {
                x: 0
                y: Math.round((1.0 - root.currentY) * touchSurface.height) - 0.5
                width: touchSurface.width
                height: 1
                color: root.isTouching ? root.wetColor : Qt.alpha(root.wetColor, 0.4)
            }

            // Glowing Touch Puck
            Rectangle {
                id: puckGlow
                x: (root.currentX * touchSurface.width) - (width / 2)
                y: ((1.0 - root.currentY) * touchSurface.height) - (height / 2)
                width: root.isTouching ? 44 : 32
                height: width
                radius: width / 2
                color: Qt.alpha(root.accentColor, 0.2)
                border.color: root.accentColor
                border.width: 2

                Behavior on width {
                    NumberAnimation { duration: 80 }
                }

                Rectangle {
                    anchors.centerIn: parent
                    width: 12
                    height: 12
                    radius: 6
                    color: root.isTouching ? "#ffffff" : root.accentColor
                }
            }

            // Touch Handler
            MultiPointTouchArea {
                anchors.fill: parent
                mouseEnabled: true

                onPressed: touchPoints => {
                    root.isTouching = true;
                    snapBackAnim.stop();
                    root.saveState();
                    if (touchPoints.length > 0) {
                        var pt = touchPoints[0];
                        var nx = pt.x / touchSurface.width;
                        var ny = 1.0 - (pt.y / touchSurface.height);
                        root.applyValues(nx, ny);
                    }
                }

                onTouchUpdated: touchPoints => {
                    if (touchPoints.length > 0) {
                        var pt = touchPoints[0];
                        var nx = pt.x / touchSurface.width;
                        var ny = 1.0 - (pt.y / touchSurface.height);
                        root.applyValues(nx, ny);
                    }
                }

                onReleased: touchPoints => {
                    if (touchPoints.length > 0) {
                        return;
                    }
                    root.isTouching = false;
                    if (!root.holdMode) {
                        // Animate the cursor back, then restore exactly the
                        // values from before the touch
                        snapBackAnimX.to = root.hasSavedState ? root.savedFilter : 0.5;
                        snapBackAnimY.to = root.hasSavedState ? root.savedMix : 0.0;
                        snapBackAnim.restart();
                    }
                }

                onCanceled: {
                    root.isTouching = false;
                    if (!root.holdMode) {
                        root.restoreState();
                    }
                }
            }

            ParallelAnimation {
                id: snapBackAnim
                NumberAnimation {
                    id: snapBackAnimX
                    target: root
                    property: "currentX"
                    duration: 180
                    easing.type: Easing.OutQuad
                }
                NumberAnimation {
                    id: snapBackAnimY
                    target: root
                    property: "currentY"
                    duration: 180
                    easing.type: Easing.OutQuad
                }
                onFinished: {
                    root.restoreState();
                }
            }
        }

        // ================= BOTTOM STATUS READOUT =================
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Text {
                text: {
                    var f = (root.currentX - 0.5) * 200;
                    if (Math.abs(f) < 2) return qsTr("FILTER: NEUTRAL");
                    if (f < 0) return qsTr("FILTER: LPF %1%").arg(Math.round(Math.abs(f)));
                    return qsTr("FILTER: HPF %1%").arg(Math.round(f));
                }
                font.bold: true
                font.pixelSize: 11
                color: root.currentX < 0.48 ? root.deck1Color : (root.currentX > 0.52 ? root.deck2Color : root.buttonTextColor)
            }

            Item { Layout.fillWidth: true }

            Text {
                text: qsTr("FX DEPTH: %1%").arg(Math.round(root.currentY * 100))
                font.bold: true
                font.pixelSize: 11
                color: root.currentY > 0.05 ? root.wetColor : root.buttonTextColor
            }
        }
    }
}
