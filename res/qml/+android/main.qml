import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls
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
    height: isMobile ? Screen.height : designHeight
    menuBar: content.item ? content.item.menuBar : null
    minimumHeight: isMobile ? 0 : 300
    minimumWidth: isMobile ? 0 : 680
    visible: true
    width: isMobile ? Screen.width : designWidth

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
                Qt.quit()
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
            NumberAnimation { duration: 500; easing.type: Easing.InOutQuad }
        }

        Timer {
            id: loadProgressTimer
            interval: 35
            running: !splash.fadingOut && splash.visible
            repeat: true
            property int ticks: 0
            onTriggered: {
                ticks++;
                var isReady = (content.status === Loader.Ready && content.item !== null);

                // Give at least 40 ticks * 35ms = 1400ms so user clearly sees startup progress
                if (!isReady || ticks < 40) {
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
            interval: 320
            repeat: false
            onTriggered: {
                splash.fadingOut = true;
            }
        }

        Column {
            id: splashContent
            anchors.centerIn: parent
            spacing: 14
            opacity: 1

            // Mixxx Logo (Icon and MIXXX together)
            Image {
                id: logo
                anchors.horizontalCenter: parent.horizontalCenter
                source: "qrc:/images/mixxx-icon-logo-symbolic.svg"
                fillMode: Image.PreserveAspectFit
                width: 280
                height: 60
                smooth: true
                antialiasing: true
            }

            // Subtitle
            Text {
                id: subtitleText
                anchors.horizontalCenter: parent.horizontalCenter
                color: "#ff9100"
                font.pixelSize: 15
                font.bold: true
                font.letterSpacing: 0.8
                text: qsTr("By JaviLabarumDJ on Android")
            }

            Item {
                // Spacer
                height: 8
                width: 1
            }

            // Progress bar
            Item {
                id: progressBarContainer
                anchors.horizontalCenter: parent.horizontalCenter
                width: 260
                height: 32

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
                        width: Math.max(6, parent.width * splash.progress)
                        radius: 3
                        color: "#00e5ff"

                        Behavior on width {
                            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
                        }
                    }
                }

                Text {
                    anchors.top: progressTrack.bottom
                    anchors.topMargin: 7
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: splash.progress >= 1.0 ? qsTr("Ready to mix!") : (splash.progress > 0.7 ? qsTr("Optimizing DJ controls...") : (splash.progress > 0.35 ? qsTr("Loading library and decks...") : qsTr("Starting audio engine...")))
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    color: "#8fa1b8"
                }
            }
        }
    }
}
