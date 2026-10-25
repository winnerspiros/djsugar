import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls
import QtQuick.Window 2.12
import "Theme"

ApplicationWindow {
    id: root

    readonly property bool isMobile: Qt.platform.os === "android" || Qt.platform.os === "ios"
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

    Connections {
        target: Mixxx.Core
        function onReadyChanged() {
            root.updateVisibility();
            root.loadSkin();
        }
    }

    // Qt cascades a new window near the top-left of its screen and, on a
    // multi-monitor desktop, can leave the title bar above the screen edge.
    // Place the window deterministically: centred on its screen, then clamped
    // so the title bar is always reachable. Skipped on mobile (fullscreen).
    function clampOntoScreen() {
        if (isMobile || visibility !== Window.Windowed)
            return ;

        let s = root.screen;
        if (!s) {
            const all = Qt.application.screens;
            for (let i = 0; i < all.length; ++i) {
                if (all[i].virtualX === 0 && all[i].virtualY === 0) {
                    s = all[i];
                    break;
                }
            }
            if (!s && all.length > 0)
                s = all[0];
        }
        if (!s)
            return ;

        // NOTE: Screen.desktopAvailableWidth/Height is the VIRTUAL desktop,
        // not this screen. Screen.width/height is this screen only.
        const sw = s.width;
        const sh = s.height;

        root.width = Math.min(root.width, sw);
        root.height = Math.min(root.height, sh);

        root.x = s.virtualX + Math.round((sw - root.width) / 2);
        root.y = s.virtualY + Math.round((sh - root.height) / 2);

        root.x = Math.min(Math.max(root.x, s.virtualX), s.virtualX + sw - root.width);
        root.y = Math.min(Math.max(root.y, s.virtualY), s.virtualY + sh - root.height);
    }

    onScreenChanged: root.clampOntoScreen()

    Timer {
        id: clampTimer

        property int passes: 0

        interval: 400
        repeat: true
        onTriggered: {
            root.clampOntoScreen();
            if (++passes >= 3)
                stop();
        }
    }

    // The main window of the skin: "Classic" is MainWindow.qml next to
    // this file, the others are in Skins/<name>/MainWindow.qml. Switched at
    // once through Settings -> Interface; the decks keep playing. The old
    // skin is unloaded first: both skins create the same [Skin] controls,
    // which the new one can only create once the old one is gone.
    // (Skin switching via Mixxx.MobileApp, from JaviLabarumDJ/mixxx-android.)
    readonly property var skinItem: content.item
    // The values of the Theme singleton before any skin changed them
    property var themeDefaults: null

    // A skin can change the shared colors (Theme) through a skinTheme
    // property of its main window. The values of the previous skin are
    // undone first.
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

    function skinSource(skin) {
        if (skin === "Classic" || Mixxx.MobileApp.skinsIn(Qt.resolvedUrl("Skins/")).indexOf(skin) < 0) {
            return Qt.resolvedUrl("MainWindow.qml");
        }
        return Qt.resolvedUrl("Skins/" + skin + "/MainWindow.qml");
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

    Component.onCompleted: { root.updateVisibility(); root.loadSkin(); clampTimer.start() }
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
        visible: opacity > 0
        color: Theme.backgroundColor
        anchors.fill: parent

        property bool ready: false

        Component.onCompleted: {
            ready = true
        }

        states: [
            State {
                when: splash.ready && content.status != Loader.Ready

                PropertyChanges {
                    text.opacity: 1
                    logo.opacity: 1
                    logo.y: root.height / 2 - logo.height / 2
                }
            },
            State {
                when: content.status == Loader.Ready && content.active

                PropertyChanges {
                    splash.opacity: 0
                }
            }
        ]
        Image {
            id: logo
            anchors.horizontalCenter: parent.horizontalCenter
            source: "qrc:/images/mixxx-icon-logo-symbolic.svg"
            // height: 64
            opacity: 0
            y: root.height / 2

            Behavior on opacity {
                NumberAnimation { duration: 1500; easing.type: Easing.InOutQuad }
            }

            Behavior on y {
                NumberAnimation { duration: 1500; easing.type: Easing.InOutQuad }
            }
        }
        Text {
            id: text
            opacity: 0
            y: logo.y + logo.height*2
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.topMargin: 20
            font.pixelSize: 12
            color: Theme.lightGray3
            text: "DJ your way"
            Behavior on opacity {
                SequentialAnimation {
                    PauseAnimation { duration: 1000 }
                    NumberAnimation { duration: 500; easing.type: Easing.InOutQuad }
                }
            }
        }

        Behavior on opacity {
            NumberAnimation { duration: 500; easing.type: Easing.InOutQuad }
        }
    }
}
