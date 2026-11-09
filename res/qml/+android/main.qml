import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window 2.12
import "Theme"

ApplicationWindow {
    id: root

    readonly property bool isMobile: Qt.platform.os === "android" || Qt.platform.os === "ios"
    // The main window of the skin, for the settings that depend on it
    readonly property var skinItem: content.item
    readonly property int designWidth: 1792
    readonly property int designHeight: 1008

    // Mobile: the window covers the whole display, camera cutout included.
    // Qt would pad the content by the safe area; the skins draw edge to edge
    // and keep their controls clear of the cutout themselves (CutoutInsets).
    bottomPadding: 0
    color: Theme.backgroundColor
    flags: isMobile ? (Qt.Window | Qt.FramelessWindowHint | Qt.MaximizeUsingFullscreenGeometryHint | Qt.ExpandedClientAreaHint) : Qt.Window
    height: isMobile ? Screen.height : designHeight
    leftPadding: 0
    menuBar: content.item ? content.item.menuBar : null
    minimumHeight: isMobile ? 0 : 300
    minimumWidth: isMobile ? 0 : 680
    rightPadding: 0
    topPadding: 0
    visibility: isMobile ? Window.FullScreen : Window.Windowed
    visible: true
    width: isMobile ? Screen.width : designWidth

    // Back (also swiping in from the edge of the screen) closes the window
    // when nothing on it takes the key, which quits Mixxx in the middle of a
    // set: on mobile it asks first.
    property bool quitConfirmed: false

    onClosing: close => {
        if (root.isMobile && !root.quitConfirmed) {
            close.accepted = false;
            quitPopup.open();
        }
    }

    // From Qt 6.12 a Back that nothing takes moves the app to the background
    // instead of closing the window, so the quit question never came: Back
    // asks it here. Open popups (the quit question too) take Back themselves.
    Shortcut {
        context: Qt.ApplicationShortcut
        enabled: root.isMobile && root.Overlay.overlay.children.length === 0
        sequences: ["Back"]
        onActivated: quitPopup.open()
    }

    function quit() {
        root.quitConfirmed = true;
        Qt.quit();
    }

    function updateVisibility() {
        if (!Mixxx.Core.ready) {
            return;
        }
        root.visibility = Mixxx.Config.configStartInFullscreenKey || isMobile
                ? Window.FullScreen
                : Window.Windowed;
    }

    function openQuickLoad(group) {
        if (content.item && typeof content.item.openQuickLoad === "function") {
            content.item.openQuickLoad(group);
        }
    }

    // The main window of the selected skin: "Classic" is MainWindow.qml
    // next to this file, the others are in Skins/<name>/MainWindow.qml
    function skinSource(skin) {
        if (skin === "Classic" || Mixxx.MobileApp.skinsIn(Qt.resolvedUrl("Skins/")).indexOf(skin) < 0) {
            return Qt.resolvedUrl("MainWindow.qml");
        }
        return Qt.resolvedUrl("Skins/" + skin + "/MainWindow.qml");
    }
    // The values of the Theme singleton before any skin changed them
    property var themeDefaults: null

    // A skin can change the shared colors (Theme) through a skinTheme
    // property of its main window, e.g. {"accentColor": "#1e5fb4"}. The
    // values of the previous skin are undone first.
    function applySkinTheme(skinTheme) {
        if (root.themeDefaults === null) {
            const defaults = {};
            for (const key in Theme) {
                if (key !== "objectName" && typeof Theme[key] !== "function") {
                    defaults[key] = Theme[key];
                }
            }
            root.themeDefaults = defaults;
        }
        for (const key in root.themeDefaults) {
            Theme[key] = root.themeDefaults[key];
        }
        for (const key in (skinTheme || {})) {
            if (key in root.themeDefaults) {
                Theme[key] = skinTheme[key];
            } else {
                console.warn("Unknown Theme property in the skin:", key);
            }
        }
    }
    function loadSkin() {
        if (!Mixxx.Core.ready) {
            return;
        }
        content.setSource(root.skinSource(Mixxx.MobileApp.skin), {
            "applicationWindow": root
        });
    }

    Connections {
        target: Mixxx.Core
        function onReadyChanged() {
            root.updateVisibility();
            root.loadSkin();
        }
    }
    // Switched at once; the decks keep playing. The old skin is unloaded
    // first: both skins create the same [Skin] controls (e.g. SHIFT + LOAD of
    // a controller), which the new one can only create once the old one is
    // gone, and the Loader deletes the old one later.
    Connections {
        target: Mixxx.MobileApp
        function onSkinChanged() {
            content.source = "";
            skinSwitchTimer.restart();
        }
    }
    Timer {
        id: skinSwitchTimer

        interval: 150
        onTriggered: root.loadSkin()
    }

    Component.onCompleted: {
        root.updateVisibility();
        root.loadSkin();
    }

    Timer {
        id: activateContentTimer
        interval: 45
        repeat: false
        running: Mixxx.Core.ready && !content.active
        onTriggered: {
            content.active = true;
        }
    }

    Loader {
        id: content

        anchors.fill: parent

        active: false
        asynchronous: false
        onStatusChanged: {
            if (status === Loader.Ready) {
                root.applySkinTheme(item.skinTheme);
                splash.progress = 1.0;
                loadProgressTimer.stop();
                dismissTimer.start();
            }
            if (status === Loader.Error) {
                if (Mixxx.MobileApp.skin !== "Classic") {
                    // A broken skin must not lock the user out
                    console.error("Failed to load the skin", Mixxx.MobileApp.skin, "- using Classic");
                    Mixxx.MobileApp.skin = "Classic";
                    return;
                }
                console.error("Failed to load the Mixxx main window")
                root.quit()
            }
        }
    }
    // Updates found by the check at startup (see Mixxx.AppUpdate). Created
    // once the core is ready, like the rest of the UI that uses Mixxx.
    Loader {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        // Below the top bar of the skins, so that their buttons stay usable
        anchors.topMargin: 84
        z: 1000
        active: Mixxx.Core.ready
        sourceComponent: Skin.AppUpdateBanner {
            onInstallRequested: updateInstallPopup.open()
        }
    }

    Skin.AppUpdateInstallPopup {
        id: updateInstallPopup
    }

    Popup {
        id: quitPopup

        anchors.centerIn: Overlay.overlay
        width: Math.min(460, root.width - 40)
        modal: true
        dim: true
        padding: 18

        background: Rectangle {
            color: "#181b24"
            radius: 10
            border.color: "#ff5252"
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 12

            Text {
                Layout.fillWidth: true
                text: qsTr("Quit DJ Sugar?")
                font.bold: true
                font.pixelSize: 15
                color: Theme.white
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("The music stops. The loaded tracks and the settings are saved.")
                font.pixelSize: 12
                color: "#b0bec5"
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Skin.Button {
                    Layout.preferredHeight: 44
                    Layout.preferredWidth: 140
                    text: qsTr("Cancel")
                    onClicked: quitPopup.close()
                }

                Item {
                    Layout.fillWidth: true
                }

                Skin.Button {
                    Layout.preferredHeight: 44
                    Layout.preferredWidth: 140
                    activeColor: "#ff5252"
                    backgroundColor: "#4a1c1c"
                    normalColor: "#ff8a80"
                    font.bold: true
                    text: qsTr("QUIT")
                    onClicked: root.quit()
                }
            }
        }
    }

    Rectangle {
        id: splash
        anchors.fill: parent
        color: "#0e1117"
        z: 99999
        visible: opacity > 0

        property real progress: Mixxx.Core.ready ? 0.65 : 0.25
        property bool fadingOut: false

        opacity: fadingOut ? 0.0 : 1.0
        Behavior on opacity {
            NumberAnimation { duration: 180; easing.type: Easing.OutQuad }
        }

        Timer {
            id: loadProgressTimer
            interval: 60
            running: !splash.fadingOut && splash.visible
            repeat: true
            onTriggered: {
                var isReady = (content.status === Loader.Ready && content.item !== null);
                if (!isReady) {
                    if (splash.progress < 0.85) {
                        splash.progress = Math.min(0.85, splash.progress + 0.06);
                    }
                } else {
                    splash.progress = 1.0;
                    loadProgressTimer.stop();
                    dismissTimer.start();
                }
            }
        }

        Timer {
            id: dismissTimer
            interval: 40
            repeat: false
            onTriggered: {
                splash.fadingOut = true;
            }
        }

        Column {
            id: splashContent
            anchors.centerIn: parent
            spacing: 12

            // Clean Vinyl Donut Turntable (rotating vinyl donut disc + stationary tonearm)
            Item {
                width: 92
                height: 86
                anchors.horizontalCenter: parent.horizontalCenter

                // Spinning Vinyl Donut Platter
                Item {
                    id: vinylDonutDisc
                    width: 82
                    height: 82
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter

                    // Metallic platter rim & dark vinyl outer edge
                    Rectangle {
                        anchors.fill: parent
                        radius: width / 2
                        color: "#15171c"
                        border.color: "#383d47"
                        border.width: 2
                    }

                    // Vinyl groove ring
                    Rectangle {
                        anchors.centerIn: parent
                        width: 74
                        height: 74
                        radius: 37
                        color: "transparent"
                        border.color: "#252932"
                        border.width: 1
                    }

                    // Golden donut dough ring
                    Rectangle {
                        anchors.centerIn: parent
                        width: 66
                        height: 66
                        radius: 33
                        color: "#d5944a"
                        border.color: "#b3722d"
                        border.width: 2
                    }

                    // Pink sugar frosting glaze
                    Rectangle {
                        anchors.centerIn: parent
                        width: 56
                        height: 56
                        radius: 28
                        color: "#e63380"
                        border.color: "#ff99ca"
                        border.width: 1.5

                        // Subtle groove ring in the glaze
                        Rectangle {
                            anchors.centerIn: parent
                            width: 44
                            height: 44
                            radius: 22
                            color: "transparent"
                            border.color: "#ba1b5c"
                            border.width: 1
                        }

                        // Rainbow sprinkles embedded on the frosting ring (spin with disc)
                        Rectangle { x: 24; y: 5;  width: 8; height: 3; radius: 1.5; rotation: -22; color: "#0091ff" }
                        Rectangle { x: 41; y: 15; width: 8; height: 3; radius: 1.5; rotation: 35;  color: "#ffffff" }
                        Rectangle { x: 42; y: 34; width: 8; height: 3; radius: 1.5; rotation: -40; color: "#ffb300" }
                        Rectangle { x: 25; y: 46; width: 8; height: 3; radius: 1.5; rotation: 15;  color: "#ff6b57" }
                        Rectangle { x: 8;  y: 35; width: 8; height: 3; radius: 1.5; rotation: 48;  color: "#7cfc00" }
                        Rectangle { x: 9;  y: 15; width: 8; height: 3; radius: 1.5; rotation: -32; color: "#ffffff" }
                    }

                    // Dark vinyl center label & donut hole
                    Rectangle {
                        anchors.centerIn: parent
                        width: 26
                        height: 26
                        radius: 13
                        color: "#1b1c20"
                        border.color: "#2c2e34"
                        border.width: 1.5

                        Rectangle {
                            anchors.centerIn: parent
                            width: 16
                            height: 16
                            radius: 8
                            color: "transparent"
                            border.color: "#383b42"
                            border.width: 1
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: 6
                            height: 6
                            radius: 3
                            color: "#0e1117"
                            border.color: "#cfd4dc"
                            border.width: 1.5
                        }
                    }

                    RotationAnimator on rotation {
                        running: splash.visible
                        from: 0
                        to: 360
                        duration: 2800
                        loops: Animation.Infinite
                    }
                }

                // Stationary Turntable Tonearm (top-right corner, resting on the groove)
                Item {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    width: 30
                    height: 54

                    // Pivot base
                    Rectangle {
                        anchors.right: parent.right
                        anchors.top: parent.top
                        width: 16
                        height: 16
                        radius: 8
                        color: "#252830"
                        border.color: "#6c7380"
                        border.width: 1.5

                        Rectangle {
                            anchors.centerIn: parent
                            width: 6
                            height: 6
                            radius: 3
                            color: "#d0d5dd"
                        }
                    }

                    // Tonearm shaft + pink cartridge headshell
                    Item {
                        x: 18
                        y: 8
                        rotation: 28
                        transformOrigin: Item.Top

                        Rectangle {
                            x: -1.5
                            y: 0
                            width: 3
                            height: 36
                            radius: 1.5
                            color: "#d0d5dd"
                        }

                        Rectangle {
                            x: -3.5
                            y: 33
                            width: 7
                            height: 10
                            radius: 2
                            color: "#252830"
                            border.color: "#ff99ca"
                            border.width: 1
                        }
                    }
                }
            }

            // Clean DJ SUGAR wordmark
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 8

                Text {
                    text: "DJ"
                    font.pixelSize: 24
                    font.bold: true
                    font.letterSpacing: 3
                    color: "#ffffff"
                }
                Text {
                    text: "SUGAR"
                    font.pixelSize: 24
                    font.bold: true
                    font.letterSpacing: 3
                    color: "#ff99ca"
                }
            }

            // Progress bar
            Item {
                id: progressBarContainer
                anchors.horizontalCenter: parent.horizontalCenter
                width: 240
                height: 28

                Rectangle {
                    id: progressTrack
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: parent.top
                    width: parent.width
                    height: 6
                    radius: 3
                    color: "#161b24"
                    border.color: "#283444"
                    border.width: 1
                    clip: true

                    Rectangle {
                        id: progressFill
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: Math.max(8, parent.width * splash.progress)
                        radius: 3
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0.0; color: "#e63380" }
                            GradientStop { position: 0.55; color: "#ff99ca" }
                            GradientStop { position: 1.0; color: "#D85486" }
                        }

                        Behavior on width {
                            NumberAnimation { duration: 100; easing.type: Easing.OutCubic }
                        }
                    }
                }

                Text {
                    anchors.top: progressTrack.bottom
                    anchors.topMargin: 6
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: splash.progress >= 1.0 ? qsTr("Ready to mix!") : (splash.progress > 0.5 ? qsTr("Loading library and decks...") : qsTr("Starting audio engine..."))
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    color: "#8fa1b8"
                }
            }
        }
    }
}
