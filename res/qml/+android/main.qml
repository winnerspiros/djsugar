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

    color: Theme.backgroundColor
    flags: isMobile ? (Qt.Window | Qt.FramelessWindowHint | Qt.MaximizeUsingFullscreenGeometryHint) : Qt.Window
    height: isMobile ? Screen.height : designHeight
    menuBar: content.item ? content.item.menuBar : null
    minimumHeight: isMobile ? 0 : 300
    minimumWidth: isMobile ? 0 : 680
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

    Loader {
        id: content

        anchors.fill: parent

        active: Mixxx.Core.ready
        asynchronous: true
        onStatusChanged: {
            if (status === Loader.Ready) {
                root.applySkinTheme(item.skinTheme);
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

        property real progress: 0.0
        property bool fadingOut: false

        opacity: fadingOut ? 0.0 : 1.0
        Behavior on opacity {
            NumberAnimation { duration: 250; easing.type: Easing.InOutQuad }
        }

        Timer {
            id: loadProgressTimer
            interval: 35
            running: !splash.fadingOut && splash.visible
            repeat: true
            onTriggered: {
                var isReady = (content.status === Loader.Ready && content.item !== null);

                // Gone as soon as the skin is loaded: it used to stay at
                // least 1.4 s, plus 0.3 s full and a 0.5 s fade
                if (!isReady) {
                    if (splash.progress < 0.35) {
                        splash.progress = Math.min(0.35, splash.progress + 0.045);
                    } else if (splash.progress < 0.75) {
                        splash.progress = Math.min(0.75, splash.progress + 0.025);
                    } else if (splash.progress < 0.94) {
                        splash.progress = Math.min(0.94, splash.progress + 0.012);
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
            interval: 100
            repeat: false
            onTriggered: {
                splash.fadingOut = true;
            }
        }

        gradient: Gradient {
            GradientStop { position: 0.0; color: "#0b0610" }
            GradientStop { position: 0.48; color: "#190c22" }
            GradientStop { position: 1.0; color: "#0c0712" }
        }

        // Animated frosting pink & golden glaze ambient glow orb
        Rectangle {
            width: Math.min(parent.width, parent.height) * 0.85
            height: width
            radius: width / 2
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -24
            color: "#22e63380"
            SequentialAnimation on scale {
                running: splash.visible
                loops: Animation.Infinite
                NumberAnimation { from: 0.92; to: 1.12; duration: 1600; easing.type: Easing.InOutSine }
                NumberAnimation { from: 1.12; to: 0.92; duration: 1600; easing.type: Easing.InOutSine }
            }
        }

        // Floating animated sugar sprinkles matching the Vinyl Donut logo palette
        Repeater {
            model: [
                { "dx": -145, "dy": -88, "rot": -28, "col": "#0091ff", "w": 18, "dur": 1700 },
                { "dx": 148,  "dy": -78, "rot": 32,  "col": "#ffb300", "w": 20, "dur": 1950 },
                { "dx": -162, "dy": 22,  "rot": 18,  "col": "#ff6b57", "w": 16, "dur": 1800 },
                { "dx": 165,  "dy": 28,  "rot": -24, "col": "#7cfc00", "w": 17, "dur": 2100 },
                { "dx": -112, "dy": 96,  "rot": 40,  "col": "#ffffff", "w": 15, "dur": 1650 },
                { "dx": 118,  "dy": 92,  "rot": -36, "col": "#ff99ca", "w": 19, "dur": 1850 }
            ]
            delegate: Rectangle {
                required property var modelData
                width: modelData.w
                height: 5
                radius: 2.5
                color: modelData.col
                opacity: 0.72
                rotation: modelData.rot
                x: splash.width / 2 + modelData.dx - width / 2
                y: splash.height / 2 + modelData.dy

                SequentialAnimation on y {
                    running: splash.visible
                    loops: Animation.Infinite
                    NumberAnimation {
                        from: splash.height / 2 + modelData.dy - 7
                        to: splash.height / 2 + modelData.dy + 7
                        duration: modelData.dur
                        easing.type: Easing.InOutSine
                    }
                    NumberAnimation {
                        from: splash.height / 2 + modelData.dy + 7
                        to: splash.height / 2 + modelData.dy - 7
                        duration: modelData.dur
                        easing.type: Easing.InOutSine
                    }
                }
            }
        }

        Column {
            id: splashContent
            anchors.centerIn: parent
            spacing: 12
            opacity: 1

            // Vinyl Donut Turntable Platter
            Item {
                width: 142
                height: 142
                anchors.horizontalCenter: parent.horizontalCenter

                // Pulsing outer sugar-glaze aura ring
                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: "transparent"
                    border.width: 2
                    border.color: "#66ff99ca"

                    SequentialAnimation on scale {
                        running: splash.visible
                        loops: Animation.Infinite
                        NumberAnimation { from: 0.96; to: 1.08; duration: 1100; easing.type: Easing.InOutQuad }
                        NumberAnimation { from: 1.08; to: 0.96; duration: 1100; easing.type: Easing.InOutQuad }
                    }
                }

                // Sprinkle-strobe platter rim
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 5
                    radius: width / 2
                    color: "#140b1c"
                    border.width: 3
                    border.color: "#e63380"

                    Rectangle {
                        width: 9; height: 9; radius: 4.5
                        color: "#0091ff"
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.top
                        anchors.topMargin: -4.5
                    }
                    Rectangle {
                        width: 9; height: 9; radius: 4.5
                        color: "#ffb300"
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: -4.5
                    }
                    Rectangle {
                        width: 8; height: 8; radius: 4
                        color: "#7cfc00"
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: -4
                    }
                    Rectangle {
                        width: 8; height: 8; radius: 4
                        color: "#ff6b57"
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.right: parent.right
                        anchors.rightMargin: -4
                    }

                    RotationAnimator on rotation {
                        running: splash.visible
                        from: 0
                        to: 360
                        duration: 3200
                        loops: Animation.Infinite
                    }
                }

                // The Vinyl Donut Logo spinning on the platter
                Rectangle {
                    anchors.centerIn: parent
                    width: 114
                    height: 114
                    radius: 57
                    color: "#000000"
                    border.color: "#ff99ca"
                    border.width: 2
                    clip: true

                    Image {
                        id: logo
                        anchors.centerIn: parent
                        source: "qrc:/images/mixxx_logo.svg"
                        fillMode: Image.PreserveAspectFit
                        width: 108
                        height: 108
                        smooth: true
                        antialiasing: true

                        RotationAnimator on rotation {
                            running: splash.visible
                            from: 0
                            to: 360
                            duration: 4500
                            loops: Animation.Infinite
                        }
                    }

                    SequentialAnimation on scale {
                        running: splash.visible
                        loops: Animation.Infinite
                        NumberAnimation { from: 1.0; to: 1.045; duration: 520; easing.type: Easing.InOutSine }
                        NumberAnimation { from: 1.045; to: 1.0; duration: 520; easing.type: Easing.InOutSine }
                    }
                }
            }

            // DJ SUGAR brand wordmark + vinyl donut subtitle pill
            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 4

                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 6

                    Text {
                        text: "DJ"
                        font.pixelSize: 23
                        font.bold: true
                        font.letterSpacing: 3
                        color: "#ffffff"
                    }
                    Text {
                        text: "SUGAR"
                        font.pixelSize: 23
                        font.bold: true
                        font.letterSpacing: 3
                        color: "#ff99ca"
                    }
                }

                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    height: 20
                    width: taglineRow.implicitWidth + 18
                    radius: 10
                    color: "#24e63380"
                    border.color: "#55ff99ca"
                    border.width: 1

                    Row {
                        id: taglineRow
                        anchors.centerIn: parent
                        spacing: 6

                        Row {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2
                            Repeater {
                                model: [
                                    { "c": "#e63380", "d": 320 },
                                    { "c": "#ffb300", "d": 260 },
                                    { "c": "#0091ff", "d": 380 },
                                    { "c": "#7cfc00", "d": 290 }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    width: 2.5
                                    height: 10
                                    radius: 1
                                    color: modelData.c
                                    anchors.verticalCenter: parent.verticalCenter
                                    SequentialAnimation on height {
                                        running: splash.visible
                                        loops: Animation.Infinite
                                        NumberAnimation { from: 3; to: 11; duration: modelData.d; easing.type: Easing.InOutSine }
                                        NumberAnimation { from: 11; to: 3; duration: modelData.d; easing.type: Easing.InOutSine }
                                    }
                                }
                            }
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            font.pixelSize: 10
                            font.bold: true
                            font.letterSpacing: 1.8
                            color: "#ffd6eb"
                            text: "SWEET MIXING · PRO DJ STUDIO"
                        }
                    }
                }
            }

            // Modern frosted progress card with sugar-glaze gradient bar
            Rectangle {
                id: progressBarContainer
                anchors.horizontalCenter: parent.horizontalCenter
                width: 280
                height: 44
                radius: 12
                color: "#181022"
                border.color: "#38ff99ca"
                border.width: 1

                Column {
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 6

                    Row {
                        width: parent.width
                        Text {
                            text: splash.progress >= 1.0
                                ? qsTr("Ready to drop the needle!")
                                : (splash.progress > 0.7
                                    ? qsTr("Glazing decks & controllers…")
                                    : (splash.progress > 0.35
                                        ? qsTr("Spinning up library & waveforms…")
                                        : qsTr("Warming up audio engine…")))
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            color: "#f3d9e8"
                        }
                    }

                    Rectangle {
                        id: progressTrack
                        width: parent.width
                        height: 7
                        radius: 3.5
                        color: "#281836"
                        clip: true

                        Rectangle {
                            id: progressFill
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: Math.max(14, parent.width * splash.progress)
                            radius: 3.5
                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop { position: 0.0; color: "#e63380" }
                                GradientStop { position: 0.45; color: "#ff99ca" }
                                GradientStop { position: 0.78; color: "#ffb300" }
                                GradientStop { position: 1.0; color: "#0091ff" }
                            }

                            Behavior on width {
                                NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
                            }
                        }
                    }
                }
            }
        }
    }
}
