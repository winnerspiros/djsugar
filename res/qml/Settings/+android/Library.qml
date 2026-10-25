import QtQuick
import QtQuick.Layouts
import QtQuick.Controls 2.15
import Mixxx 1.0 as Mixxx
import "." as SettingComponents
import ".." as Skin
import "../Theme"

Category {
    id: root

    property bool dirty: false
    property var sourcesList: []
    property bool showFolderBrowser: false
    property string currentBrowserPath: "/storage/emulated/0/Music"
    property var subDirectoriesList: []
    property var backupList: []
    property string backupMessage: ""
    property bool backupError: false

    function refreshBackups() {
        root.backupList = Mixxx.MobileLibrary.backups();
    }

    // "2026-10-22_14-05-09" -> "2026-10-22  14:05:09"
    function backupLabel(name) {
        return name.replace("_", "  ").replace(/-(\d\d)-(\d\d)$/, ":$1:$2");
    }
    readonly property bool isScanning: Mixxx.Library ? (Mixxx.Library.libraryScanActive || (Mixxx.Library.scanner && Mixxx.Library.scanner.running)) : false

    function refreshSources() {
        if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.getSourcesList) {
            root.sourcesList = Mixxx.MobileLibrary.getSourcesList();
        }
    }

    function updateSubDirs() {
        if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.getSubDirectories) {
            root.subDirectoriesList = Mixxx.MobileLibrary.getSubDirectories(root.currentBrowserPath);
        }
    }

    function goUpDir() {
        var p = root.currentBrowserPath;
        var lastSlash = p.lastIndexOf("/");
        if (lastSlash > 0) {
            root.currentBrowserPath = p.substring(0, lastSlash);
        } else {
            root.currentBrowserPath = "/storage/emulated/0";
        }
        root.updateSubDirs();
    }

    function selectSubDir(dirName) {
        var base = root.currentBrowserPath;
        if (!base.endsWith("/")) {
            base += "/";
        }
        root.currentBrowserPath = base + dirName;
        root.updateSubDirs();
    }

    function addQuickFolder(path) {
        if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.addSourcePath) {
            Mixxx.MobileLibrary.addSourcePath(path);
            Mixxx.Library.rescanLibrary();
            root.refreshSources();
        }
    }

    function removeFolder(path) {
        if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.removeSourcePath) {
            Mixxx.MobileLibrary.removeSourcePath(path);
            Mixxx.Library.rescanLibrary();
            root.refreshSources();
        }
    }

    function isFolderAdded(path) {
        if (!root.sourcesList) return false;
        for (var i = 0; i < root.sourcesList.length; i++) {
            if (root.sourcesList[i].path === path) return true;
        }
        return false;
    }

    function load() {
        root.refreshSources();
        root.refreshBackups();
        if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.defaultMusicDirectory) {
            root.currentBrowserPath = Mixxx.MobileLibrary.defaultMusicDirectory();
        }
        if (Mixxx.StreamingManager) {
            Mixxx.StreamingManager.refreshCacheSize();
        }
    }

    function save() {
        root.dirty = false;
    }

    label: qsTr("Music Library")

    Component.onCompleted: {
        root.load();
    }

    Connections {
        target: Mixxx.MobileLibrary

        function onSourcesChanged() {
            root.refreshSources();
        }
    }

    Connections {
        target: Mixxx.Library

        function onLibraryScanActiveChanged() {
            root.refreshSources();
        }
    }

    ScrollView {
        id: scrollView
        anchors.bottom: libraryAction.top
        anchors.bottomMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            width: scrollView.availableWidth
            spacing: 14

            // ================= 1. CARPETAS DE MÚSICA LOCAL =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: localCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                ColumnLayout {
                    id: localCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 12

                    // Header row
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Text {
                            text: qsTr("📁 Local music folders")
                            font.bold: true
                            font.pixelSize: 15
                            color: Theme.white
                            Layout.fillWidth: true
                        }

                        Rectangle {
                            Layout.preferredHeight: 24
                            Layout.preferredWidth: scannerStatusText.implicitWidth + 16
                            radius: 12
                            color: root.isScanning ? "#ff910022" : "#00e67622"
                            border.color: root.isScanning ? "#ff9100" : "#00e676"
                            border.width: 1

                            Text {
                                id: scannerStatusText
                                anchors.centerIn: parent
                                font.pixelSize: 11
                                font.bold: true
                                color: root.isScanning ? "#ff9100" : "#00e676"
                                text: root.isScanning ? qsTr("🔄 Scanning...") : qsTr("✔ Collection ready")
                            }
                        }
                    }

                    Text {
                        text: qsTr("Choose the folders on your device where you keep your music. Mixxx will watch them and index the tracks it finds automatically.")
                        font.pixelSize: 11
                        color: "#90a4ae"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    // --- LISTA DE CARPETAS MONITORIZADAS ---
                    Text {
                        text: qsTr("Folders currently added:")
                        font.bold: true
                        font.pixelSize: 12
                        color: "#b0bec5"
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Repeater {
                            model: root.sourcesList

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 48
                                color: "#161b24"
                                radius: 6
                                border.color: "#283344"
                                border.width: 1

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 10
                                    spacing: 8

                                    Text {
                                        text: "📁"
                                        font.pixelSize: 16
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2

                                        Text {
                                            text: modelData.path
                                            font.bold: true
                                            font.pixelSize: 12
                                            color: "#ffffff"
                                            elide: Text.ElideMiddle
                                            Layout.fillWidth: true
                                        }

                                        Text {
                                            text: qsTr("%1 tracks indexed").arg(modelData.trackCount || 0) +
                                                  (modelData.totalSecond > 0 ? " • " + qsTr("%1 min").arg(Math.round(modelData.totalSecond / 60)) : "")
                                            font.pixelSize: 10
                                            color: "#00e5ff"
                                        }
                                    }

                                    Skin.Button {
                                        Layout.preferredHeight: 30
                                        Layout.preferredWidth: 80
                                        activeColor: "#ff5252"
                                        backgroundColor: "#421e24"
                                        normalColor: "#ff8a80"
                                        font.bold: true
                                        font.pixelSize: 10
                                        text: qsTr("🗑 Remove")
                                        onClicked: {
                                            root.removeFolder(modelData.path);
                                        }
                                    }
                                }
                            }
                        }

                        // Empty State if no folder configured
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 42
                            color: "#1c212c"
                            radius: 6
                            visible: !root.sourcesList || root.sourcesList.length === 0

                            RowLayout {
                                anchors.centerIn: parent
                                spacing: 8
                                Text {
                                    text: qsTr("⚠️ No folders added yet. Use the buttons below to add one.")
                                    font.pixelSize: 11
                                    color: "#ffb74d"
                                }
                            }
                        }
                    }

                    // --- ACCESOS RÁPIDOS ANDROID ---
                    Text {
                        text: qsTr("Shortcuts (standard Android locations):")
                        font.bold: true
                        font.pixelSize: 12
                        color: "#b0bec5"
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: 8

                        Skin.Button {
                            height: 32
                            width: 170
                            activeColor: "#00e5ff"
                            backgroundColor: root.isFolderAdded("/storage/emulated/0/Music") ? "#004d40" : "#1a2533"
                            normalColor: root.isFolderAdded("/storage/emulated/0/Music") ? "#00e676" : "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: (root.isFolderAdded("/storage/emulated/0/Music") ? "✔ " : "➕ ") + qsTr("🎵 Music (/sdcard)")
                            onClicked: {
                                root.addQuickFolder("/storage/emulated/0/Music");
                            }
                        }

                        Skin.Button {
                            height: 32
                            width: 180
                            activeColor: "#00e5ff"
                            backgroundColor: root.isFolderAdded("/storage/emulated/0/Download") ? "#004d40" : "#1a2533"
                            normalColor: root.isFolderAdded("/storage/emulated/0/Download") ? "#00e676" : "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: (root.isFolderAdded("/storage/emulated/0/Download") ? "✔ " : "➕ ") + qsTr("📥 Downloads (/Download)")
                            onClicked: {
                                root.addQuickFolder("/storage/emulated/0/Download");
                            }
                        }

                        Skin.Button {
                            height: 32
                            width: 160
                            activeColor: "#00e5ff"
                            backgroundColor: root.isFolderAdded("/storage/emulated/0/Mixxx") ? "#004d40" : "#1a2533"
                            normalColor: root.isFolderAdded("/storage/emulated/0/Mixxx") ? "#00e676" : "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: (root.isFolderAdded("/storage/emulated/0/Mixxx") ? "✔ " : "➕ ") + "💽 Mixxx DJ (/Mixxx)"
                            onClicked: {
                                root.addQuickFolder("/storage/emulated/0/Mixxx");
                            }
                        }
                    }

                    // --- BOTÓN EXPLORAR / AÑADIR OTRA CARPETA ---
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Skin.Button {
                            Layout.preferredHeight: 36
                            Layout.preferredWidth: 220
                            activeColor: "#00ffff"
                            backgroundColor: root.showFolderBrowser ? "#00363a" : "#263238"
                            normalColor: "#00e5ff"
                            font.bold: true
                            font.pixelSize: 11
                            text: root.showFolderBrowser ? qsTr("▲ Hide browser") : qsTr("📂 Browse / other folder...")
                            onClicked: {
                                root.showFolderBrowser = !root.showFolderBrowser;
                                if (root.showFolderBrowser) {
                                    root.updateSubDirs();
                                }
                            }
                        }

                        Skin.Button {
                            Layout.preferredHeight: 36
                            Layout.preferredWidth: 190
                            activeColor: "#00e5ff"
                            backgroundColor: "#1565c0"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: qsTr("🔄 Rescan music")
                            enabled: !root.isScanning
                            onClicked: {
                                if (Mixxx.Library && Mixxx.Library.rescanLibrary) {
                                    Mixxx.Library.rescanLibrary();
                                }
                            }
                        }
                    }

                    // --- PANEL EXPANDIBLE DE EXPLORADOR DE CARPETAS ---
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: browserCol.implicitHeight + 20
                        color: "#111620"
                        radius: 6
                        border.color: "#00e5ff44"
                        border.width: 1
                        visible: root.showFolderBrowser

                        ColumnLayout {
                            id: browserCol
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 8

                            Text {
                                text: qsTr("🔍 Device folder browser:")
                                font.bold: true
                                font.pixelSize: 12
                                color: "#00e5ff"
                            }

                            // Current path display with Back/Up button
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6

                                Skin.Button {
                                    Layout.preferredHeight: 32
                                    Layout.preferredWidth: 80
                                    activeColor: "#00ffff"
                                    backgroundColor: "#1c2a38"
                                    normalColor: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 11
                                    text: qsTr("⬆ Up")
                                    onClicked: {
                                        root.goUpDir();
                                    }
                                }

                                TextField {
                                    id: pathInputField
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 32
                                    text: root.currentBrowserPath
                                    color: "#ffffff"
                                    font.pixelSize: 11
                                    background: Rectangle {
                                        color: "#18222d"
                                        radius: 4
                                        border.color: pathInputField.activeFocus ? "#00ffff" : "#37474f"
                                        border.width: 1
                                    }
                                    onAccepted: {
                                        root.currentBrowserPath = text.trim();
                                        root.updateSubDirs();
                                    }
                                }
                            }

                            // Subdirectories flow
                            Text {
                                text: qsTr("Subfolders in this location (tap to open):")
                                font.pixelSize: 10
                                color: "#90a4ae"
                            }

                            Flow {
                                Layout.fillWidth: true
                                spacing: 6

                                Repeater {
                                    model: root.subDirectoriesList

                                    Rectangle {
                                        height: 28
                                        width: dirLabel.implicitWidth + 20
                                        radius: 4
                                        color: "#1e293b"
                                        border.color: "#334155"
                                        border.width: 1

                                        RowLayout {
                                            anchors.centerIn: parent
                                            spacing: 4
                                            Text { text: "📁"; font.pixelSize: 11 }
                                            Text {
                                                id: dirLabel
                                                text: modelData
                                                color: "#e2e8f0"
                                                font.pixelSize: 11
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            onClicked: {
                                                root.selectSubDir(modelData);
                                            }
                                        }
                                    }
                                }

                                Text {
                                    visible: !root.subDirectoriesList || root.subDirectoriesList.length === 0
                                    text: qsTr("(No more subfolders)")
                                    font.pixelSize: 10
                                    color: "#64748b"
                                }
                            }

                            // Confirm Add Button
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                Skin.Button {
                                    Layout.preferredHeight: 34
                                    Layout.fillWidth: true
                                    activeColor: "#00e676"
                                    backgroundColor: "#004d40"
                                    normalColor: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 11
                                    text: qsTr("✔ Add '%1' to the library").arg(root.currentBrowserPath)
                                    onClicked: {
                                        root.addQuickFolder(root.currentBrowserPath);
                                        root.showFolderBrowser = false;
                                    }
                                }

                                Skin.Button {
                                    Layout.preferredHeight: 34
                                    Layout.preferredWidth: 90
                                    activeColor: "#ff5252"
                                    backgroundColor: "#2c3440"
                                    normalColor: "#b0bec5"
                                    font.pixelSize: 11
                                    text: qsTr("Close")
                                    onClicked: {
                                        root.showFolderBrowser = false;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ================= 2. CACHÉ DE STREAMING (NAVIDROME / AUDIUS / TIDAL) =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: streamingCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                ColumnLayout {
                    id: streamingCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("☁️ Streaming storage & cache")
                        font.bold: true
                        font.pixelSize: 14
                        color: "#00e5ff"
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Tracks played from streaming platforms (Navidrome, Audius, SoundCloud, Tidal) are stored locally so playback continues without dropouts even if the internet connection is lost.")
                        font.pixelSize: 11
                        color: "#90a4ae"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            Layout.preferredHeight: 40
                            Layout.fillWidth: true
                            color: "#181b24"
                            radius: 6
                            border.color: "#292f3d"
                            border.width: 1

                            RowLayout {
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 8

                                Text {
                                    text: qsTr("💾 Space used:")
                                    font.pixelSize: 12
                                    color: "#b0bec5"
                                }

                                Text {
                                    text: {
                                        var bytes = Mixxx.StreamingManager ? Mixxx.StreamingManager.cacheSizeBytes : 0;
                                        if (bytes <= 0) return qsTr("0 MB (cache empty)");
                                        var mb = bytes / (1024 * 1024);
                                        if (mb > 1024) return (mb / 1024).toFixed(2) + " GB";
                                        return mb.toFixed(1) + " MB";
                                    }
                                    font.bold: true
                                    font.pixelSize: 13
                                    color: "#00e676"
                                }

                                // Shortened to the space left next to the size
                                Text {
                                    text: qsTr("Path: %1").arg(Mixxx.StreamingManager ? Mixxx.StreamingManager.cacheDirectory : "")
                                    font.pixelSize: 10
                                    color: "#607d8b"
                                    elide: Text.ElideMiddle
                                    horizontalAlignment: Text.AlignRight
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 150
                            activeColor: "#ff5252"
                            backgroundColor: "#421e24"
                            normalColor: "#ff8a80"
                            font.bold: true
                            text: qsTr("🗑️ Clear cache")
                            onClicked: {
                                if (Mixxx.StreamingManager) {
                                    Mixxx.StreamingManager.clearCache();
                                }
                            }
                        }
                    }
                }
            }

            // ================= LIBRARY BACKUP =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: backupCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                ColumnLayout {
                    id: backupCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("💾 Library backup")
                        font.bold: true
                        font.pixelSize: 14
                        color: "#00e5ff"
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Saves the library (tracks, BPM, key, cues, playlists, crates and history) and the settings to %1. That folder is kept when Mixxx is uninstalled or its data is cleared, so a backup can be restored afterwards.").arg(Mixxx.MobileLibrary.backupDirectory)
                        font.pixelSize: 11
                        color: "#90a4ae"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 170
                            activeColor: "#00e676"
                            backgroundColor: "#1b3a2a"
                            normalColor: "#69f0ae"
                            font.bold: true
                            text: qsTr("💾 Back up now")
                            onClicked: {
                                const result = Mixxx.MobileLibrary.createBackup();
                                root.backupError = !result.ok;
                                root.backupMessage = result.message;
                                root.refreshBackups();
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: root.backupMessage
                            font.pixelSize: 11
                            color: root.backupError ? "#ff8a80" : "#69f0ae"
                            wrapMode: Text.WordWrap
                        }
                    }

                    Text {
                        visible: root.backupList.length === 0
                        text: qsTr("No backups yet.")
                        font.pixelSize: 11
                        color: "#607d8b"
                    }

                    Repeater {
                        model: root.backupList

                        RowLayout {
                            id: backupRow

                            required property string modelData

                            Layout.fillWidth: true
                            spacing: 12

                            Text {
                                Layout.fillWidth: true
                                text: root.backupLabel(backupRow.modelData)
                                font.pixelSize: 13
                                color: Theme.white
                            }

                            Skin.Button {
                                Layout.preferredHeight: 36
                                Layout.preferredWidth: 130
                                activeColor: "#ffab40"
                                backgroundColor: "#3a2a14"
                                normalColor: "#ffd180"
                                font.bold: true
                                text: qsTr("Restore")
                                onClicked: {
                                    restorePopup.backupName = backupRow.modelData;
                                    restorePopup.open();
                                }
                            }
                        }
                    }
                }
            }

            // ================= REKORDBOX STICK BACKUP =================
            // Copies the Rekordbox export of a USB stick to the device, see
            // library/rekordbox/rekordboxdrivebackup.h
            Rectangle {
                id: driveBackupCard

                property string message: ""
                property bool messageIsError: false

                Layout.fillWidth: true
                Layout.preferredHeight: driveBackupCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                Connections {
                    target: Mixxx.MobileLibrary

                    function onDriveBackupFinished(message, error) {
                        driveBackupCard.message = message;
                        driveBackupCard.messageIsError = error;
                    }
                }

                ColumnLayout {
                    id: driveBackupCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("🎛 Rekordbox stick backup")
                        font.bold: true
                        font.pixelSize: 14
                        color: "#00e5ff"
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Copies the Rekordbox export of a USB stick (playlists, analysis and tracks) to %1/<name>. The copy appears under Rekordbox in the library like the stick itself, also when the stick is not plugged in. Backing up the stick again only copies what changed, and deletes what was removed from the stick.").arg("/storage/emulated/0/Music/Mixxx/Rekordbox")
                        font.pixelSize: 11
                        color: "#90a4ae"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 170
                            enabled: !Mixxx.MobileLibrary.rekordboxDrivesLoading
                            activeColor: "#00e5ff"
                            backgroundColor: "#14303a"
                            normalColor: "#80deea"
                            font.bold: true
                            text: Mixxx.MobileLibrary.rekordboxDrivesLoading ? qsTr("Looking…") : qsTr("🔍 Find sticks")
                            onClicked: {
                                driveBackupCard.message = "";
                                Mixxx.MobileLibrary.refreshRekordboxDrives();
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: !Mixxx.MobileLibrary.rekordboxDrivesLoading && Mixxx.MobileLibrary.rekordboxDrives.length === 0
                            text: qsTr("Plug in a stick and tap Find sticks.")
                            font.pixelSize: 11
                            color: "#607d8b"
                            wrapMode: Text.WordWrap
                        }
                    }

                    Repeater {
                        model: Mixxx.MobileLibrary.rekordboxDrives

                        RowLayout {
                            id: driveRow

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 12

                            Skin.TextField {
                                id: driveBackupName

                                Layout.fillWidth: true
                                Layout.preferredHeight: 40
                                enabled: !Mixxx.MobileLibrary.driveBackupRunning
                                font.pixelSize: 13
                                placeholderText: qsTr("Backup name")
                                text: driveRow.modelData.name
                            }

                            Skin.Button {
                                Layout.preferredHeight: 40
                                Layout.preferredWidth: 130
                                enabled: !Mixxx.MobileLibrary.driveBackupRunning && driveBackupName.text.trim().length > 0
                                activeColor: "#00e676"
                                backgroundColor: "#1b3a2a"
                                normalColor: "#69f0ae"
                                font.bold: true
                                text: qsTr("BACK UP")
                                onClicked: {
                                    const error = Mixxx.MobileLibrary.backUpRekordboxDrive(driveRow.modelData.path, driveBackupName.text.trim());
                                    driveBackupCard.messageIsError = error.length > 0;
                                    driveBackupCard.message = error;
                                }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        visible: Mixxx.MobileLibrary.driveBackupRunning
                        spacing: 12

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            ProgressBar {
                                Layout.fillWidth: true
                                from: 0
                                to: 1
                                value: Mixxx.MobileLibrary.driveBackupProgress
                            }
                            Text {
                                Layout.fillWidth: true
                                text: Mixxx.MobileLibrary.driveBackupFilesTotal > 0 ? qsTr("%1 of %2 files").arg(Mixxx.MobileLibrary.driveBackupFilesDone).arg(Mixxx.MobileLibrary.driveBackupFilesTotal) : qsTr("Reading the stick…")
                                font.pixelSize: 11
                                color: Theme.white
                            }
                        }

                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 130
                            activeColor: "#ff8a80"
                            backgroundColor: "#3a1b1b"
                            normalColor: "#ff8a80"
                            font.bold: true
                            text: qsTr("STOP")
                            onClicked: Mixxx.MobileLibrary.cancelDriveBackup()
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: driveBackupCard.message.length > 0
                        text: driveBackupCard.message
                        font.pixelSize: 11
                        color: driveBackupCard.messageIsError ? "#ff8a80" : "#69f0ae"
                        wrapMode: Text.WordWrap
                    }
                }
            }

            // ================= 3. OPCIONES DE BÚSQUEDA Y METADATOS =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: searchCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                ColumnLayout {
                    id: searchCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("🔍 Search & organization")
                        font.bold: true
                        font.pixelSize: 14
                        color: Theme.white
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Text {
                            text: qsTr("Autocomplete searches while typing")
                            font.pixelSize: 12
                            color: Theme.white
                            Layout.fillWidth: true
                        }

                        Switch {
                            checked: Mixxx.Config.librarySearchCompletionsEnable
                            onToggled: {
                                Mixxx.Config.librarySearchCompletionsEnable = checked;
                                root.dirty = true;
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: "#282c37"
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Text {
                            text: qsTr("Sync metadata and ID3 tags when editing")
                            font.pixelSize: 12
                            color: Theme.white
                            Layout.fillWidth: true
                        }

                        Switch {
                            checked: Mixxx.Config.librarySyncTrackMetadataExport
                            onToggled: {
                                Mixxx.Config.librarySyncTrackMetadataExport = checked;
                                root.dirty = true;
                            }
                        }
                    }
                }
            }

            // ================= 4. PRE-ANÁLISIS DE TONALIDAD & BPM =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: analysisCol.implicitHeight + 24
                color: Theme.darkGray3
                radius: 8
                border.color: "#303440"
                border.width: 1

                ColumnLayout {
                    id: analysisCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    Text {
                        text: qsTr("⚡ Collection pre-analysis (BPM & Camelot key)")
                        font.bold: true
                        font.pixelSize: 14
                        color: "#00e5ff"
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Calculates the BPM, beatgrid and musical key of your tracks in the background. Pre-analyzed tracks load instantly and the harmonic assistant can highlight compatible tracks.")
                        font.pixelSize: 11
                        color: "#90a4ae"
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        // Both halves of the row
                        Skin.Button {
                            Layout.preferredHeight: 38
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            activeColor: "#00e676"
                            backgroundColor: "#004d40"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: qsTr("⚡ Analyze tracks without BPM/key")
                            onClicked: {
                                if (Mixxx.MobileLibrary && Mixxx.MobileLibrary.analyzeUnanalyzedTracks) {
                                    Mixxx.MobileLibrary.analyzeUnanalyzedTracks();
                                }
                            }
                        }

                        Skin.Button {
                            Layout.preferredHeight: 38
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            activeColor: "#ff9100"
                            backgroundColor: "#263238"
                            normalColor: "#ffe082"
                            font.bold: true
                            font.pixelSize: 11
                            text: qsTr("🔄 Re-analyze the whole collection")
                            onClicked: reanalyzePopup.open()
                        }
                    }
                }
            }
        }
    }

    // ================= BARRA INFERIOR FIJA: ACCIONES Y ESTADO =================
    Rectangle {
        id: libraryAction
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: 58
        color: "#181b24"
        border.color: "#282d3c"
        border.width: 1
        radius: 8

        RowLayout {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 12

            // Status indicator pill
            Rectangle {
                Layout.preferredHeight: 34
                Layout.preferredWidth: statusRow.implicitWidth + 24
                radius: 17
                color: root.isScanning ? "#332200" : (root.dirty ? "#332200" : "#0d2b1a")
                border.color: root.isScanning ? "#ffb300" : (root.dirty ? "#ffb300" : "#00c853")
                border.width: 1

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: 8
                    Text {
                        text: root.isScanning ? "⏳" : (root.dirty ? "●" : "✓")
                        font.bold: true
                        font.pixelSize: 13
                        color: (root.isScanning || root.dirty) ? "#ffb300" : "#00e676"
                    }
                    Text {
                        text: root.isScanning ? qsTr("Scanning...") : (root.dirty ? qsTr("Unsaved changes") : qsTr("Collection ready"))
                        font.bold: true
                        font.pixelSize: 12
                        color: (root.isScanning || root.dirty) ? "#ffe082" : "#b9f6ca"
                    }
                }
            }

            Item { Layout.fillWidth: true }

            // Botón Re-escanear Pistas
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 160
                radius: 6
                color: rescanMouse.pressed ? "#1565c0" : (rescanMouse.containsMouse ? "#1e88e5" : "#0d47a1")
                border.color: "#64b5f6"
                border.width: 1

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "🔄"; font.pixelSize: 12 }
                    Text {
                        text: root.isScanning ? qsTr("Scanning...") : qsTr("Scan tracks")
                        font.bold: true
                        font.pixelSize: 12
                        color: "#ffffff"
                    }
                }

                MouseArea {
                    id: rescanMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    enabled: !root.isScanning
                    onClicked: {
                        if (Mixxx.Library && Mixxx.Library.rescanLibrary) {
                            Mixxx.Library.rescanLibrary();
                        }
                    }
                }
            }

            // Botón Descartar (visible si hay cambios)
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 110
                radius: 6
                visible: root.dirty
                color: discardMouse.pressed ? "#424242" : (discardMouse.containsMouse ? "#333333" : "#262626")
                border.color: "#555555"
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: qsTr("Discard")
                    font.bold: true
                    font.pixelSize: 13
                    color: "#e0e0e0"
                }

                MouseArea {
                    id: discardMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.load();
                        root.dirty = false;
                    }
                }
            }

            // Botón Guardar Preferencias
            Rectangle {
                Layout.preferredHeight: 40
                Layout.preferredWidth: 220
                radius: 6
                color: root.dirty
                       ? (saveMouse.pressed ? "#009624" : (saveMouse.containsMouse ? "#00e676" : "#00c853"))
                       : (saveMouse.pressed ? "#0d47a1" : (saveMouse.containsMouse ? "#1e88e5" : "#1976d2"))
                border.color: root.dirty ? "#69f0ae" : "#64b5f6"
                border.width: 1.5

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        text: root.dirty ? "💾" : "✓"
                        font.pixelSize: 14
                        color: "#ffffff"
                    }

                    Text {
                        text: root.dirty ? qsTr("Save library") : qsTr("Saved")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#ffffff"
                    }
                }

                MouseArea {
                    id: saveMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.save();
                    }
                }
            }
        }
    }

    // Re-analyzing the whole collection may replace the beatgrids corrected
    // by hand, so it asks first
    Popup {
        id: reanalyzePopup

        anchors.centerIn: Overlay.overlay
        width: Math.min(560, root.width - 40)
        modal: true
        dim: true
        padding: 18

        background: Rectangle {
            color: "#181b24"
            radius: 10
            border.color: "#ffab40"
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 12

            Text {
                Layout.fillWidth: true
                text: qsTr("Re-analyze the whole collection?")
                font.bold: true
                font.pixelSize: 15
                color: Theme.white
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("The waveforms, keys and the rest of the analysis are done again in the background. You can keep the beatgrids or analyze them again too.")
                font.pixelSize: 12
                color: "#b0bec5"
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("⚠ Analyzing the beatgrids again replaces the ones you corrected by hand, unless you locked them. Locked beatgrids and those of Rekordbox are always kept.")
                font.pixelSize: 12
                color: "#ffcc80"
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Skin.Button {
                    Layout.preferredHeight: 40
                    Layout.preferredWidth: 110
                    text: qsTr("Cancel")
                    onClicked: reanalyzePopup.close()
                }

                Item {
                    Layout.fillWidth: true
                }

                Skin.Button {
                    Layout.preferredHeight: 40
                    Layout.preferredWidth: 170
                    activeColor: "#ffffff"
                    backgroundColor: "#1e5f3a"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("KEEP BEATGRIDS")
                    onClicked: {
                        Mixxx.MobileLibrary.analyzeAllTracks(false);
                        reanalyzePopup.close();
                    }
                }

                Skin.Button {
                    Layout.preferredHeight: 40
                    Layout.preferredWidth: 170
                    activeColor: "#ffab40"
                    backgroundColor: "#3a2a14"
                    normalColor: "#ffd180"
                    font.bold: true
                    text: qsTr("ALSO BEATGRIDS")
                    onClicked: {
                        Mixxx.MobileLibrary.analyzeAllTracks(true);
                        reanalyzePopup.close();
                    }
                }
            }
        }
    }

    Popup {
        id: restorePopup

        property string backupName: ""
        property bool restoring: false

        anchors.centerIn: Overlay.overlay
        width: Math.min(520, root.width - 40)
        modal: true
        dim: true
        closePolicy: restoring ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)
        padding: 18

        onAboutToShow: {
            restoring = false;
            restoreMessage.text = "";
        }

        background: Rectangle {
            color: "#181b24"
            radius: 10
            border.color: "#ffab40"
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 12

            Text {
                Layout.fillWidth: true
                text: qsTr("Restore the backup of %1?").arg(root.backupLabel(restorePopup.backupName))
                font.bold: true
                font.pixelSize: 15
                color: Theme.white
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Your current library and settings are replaced by the backup (a copy of them is kept). Mixxx closes; open it again to finish.")
                font.pixelSize: 12
                color: "#b0bec5"
                wrapMode: Text.WordWrap
            }

            Text {
                id: restoreMessage

                Layout.fillWidth: true
                visible: text.length > 0
                font.pixelSize: 12
                color: restorePopup.restoring ? "#69f0ae" : "#ff8a80"
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                visible: !restorePopup.restoring

                Item {
                    Layout.fillWidth: true
                }

                Skin.Button {
                    Layout.preferredHeight: 40
                    Layout.preferredWidth: 130
                    text: qsTr("Cancel")
                    onClicked: restorePopup.close()
                }

                Skin.Button {
                    Layout.preferredHeight: 40
                    Layout.preferredWidth: 150
                    activeColor: "#ffab40"
                    backgroundColor: "#3a2a14"
                    normalColor: "#ffd180"
                    font.bold: true
                    text: qsTr("Restore")
                    onClicked: {
                        const result = Mixxx.MobileLibrary.restoreBackup(restorePopup.backupName);
                        restorePopup.restoring = result.ok;
                        restoreMessage.text = result.message;
                    }
                }
            }
        }
    }
}
