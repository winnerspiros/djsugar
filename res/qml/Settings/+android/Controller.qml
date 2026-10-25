import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Dialogs
import Qt.labs.qmlmodels
import Qt5Compat.GraphicalEffects
import Mixxx 1.0 as Mixxx
import ".." as Skin
import "../Theme"

Category {
    id: root

    property bool dirty: false
    property var editedControllers: new Set()
    property var manager: Mixxx.ControllerManager
    property bool processing: false
    property var selectedController: null
    property bool isRescanning: false
    property bool showGuide: true
    // MIDI controllers carry no product information, so they are never
    // "known" devices. Show every device with a mapping as a card and only
    // list the remaining ones as unmapped.
    readonly property var activeDevices: Array.from(manager.knownDevices).concat(
        Array.from(manager.unknownDevices).filter(device => !!device.mapping))
    // Many controllers have several USB interfaces, e.g. the DDJ-FLX4 shows
    // up as "DDJ-FLX4" (MIDI) and "DDJ-FLX4 USB_5" (HID). Once one of them
    // works with a mapping, the others are not listed as unmapped devices.
    readonly property var unmappedDevices: Array.from(manager.unknownDevices).filter(device => !device.mapping && !root.activeDevices.some(active => active !== device && active.name.length > 0 && device.name.startsWith(active.name)))
    signal requestNavigateToAudio()

    function load() {
        for (let controller of root.editedControllers) {
            controller.clear();
        }
        root.editedControllers.clear();
        root.dirty = false;
        errorMessage.text = "";
    }
    function save() {
        for (let controller of root.editedControllers) {
            controller.save(Mixxx.Config);
        }
        root.selectedController = null;
        root.load();
    }

    Timer {
        id: rescanFeedbackTimer
        interval: 1000
        repeat: false
        onTriggered: {
            root.isRescanning = false;
        }
    }

    label: qsTr("Controllers")

    Component.onCompleted: {
        load();
    }

    ScrollView {
        id: scrollView

        anchors.bottom: controllerAction.top
        anchors.bottomMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            spacing: 16
            width: scrollView.availableWidth

            // ================= TOP ACTION & STATUS BAR =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: topBarCol.implicitHeight + 20
                color: "#1c202a"
                radius: 12
                border.color: "#2a364f"
                border.width: 1
                visible: root.selectedController == null

                ColumnLayout {
                    id: topBarCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            width: 38
                            height: 38
                            radius: 19
                            color: "#0d47a1"
                            border.color: "#1e88e5"
                            border.width: 1
                            Text {
                                anchors.centerIn: parent
                                text: "USB"
                                font.bold: true
                                font.pixelSize: 11
                                font.letterSpacing: 1
                                color: "#64b5f6"
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Text {
                                text: qsTr("DJ controllers & USB / MIDI devices")
                                font.pixelSize: 16
                                font.bold: true
                                color: "#ffffff"
                            }
                            Text {
                                text: root.activeDevices.length > 0
                                      ? qsTr("✓ %1 controller(s) with a mapping").arg(root.activeDevices.length)
                                        + (root.unmappedDevices.length > 0 ? qsTr(" · %1 without a mapping").arg(root.unmappedDevices.length) : "")
                                      : (root.unmappedDevices.length > 0
                                         ? qsTr("%1 device(s) detected without a mapping").arg(root.unmappedDevices.length)
                                         : qsTr("No USB controller is currently detected"))
                                font.pixelSize: 12
                                color: root.activeDevices.length > 0 ? "#4caf50" : (root.unmappedDevices.length > 0 ? "#ffb74d" : "#9e9e9e")
                            }
                        }

                        // Rescan Button
                        Rectangle {
                            Layout.preferredWidth: 230
                            Layout.preferredHeight: 38
                            radius: 8
                            color: rescanMouse.pressed ? "#1565c0" : (rescanMouse.containsMouse ? "#1e88e5" : "#0d47a1")
                            border.color: "#64b5f6"
                            border.width: 1

                            RowLayout {
                                anchors.centerIn: parent
                                spacing: 8

                                Item {
                                    width: 16
                                    height: 16
                                    Layout.alignment: Qt.AlignVCenter

                                    Canvas {
                                        id: refreshCanvas
                                        anchors.fill: parent
                                        onPaint: {
                                            var ctx = getContext("2d");
                                            ctx.reset();
                                            ctx.strokeStyle = "#ffffff";
                                            ctx.lineWidth = 2.2;
                                            ctx.lineCap = "round";
                                            ctx.beginPath();
                                            ctx.arc(8, 8, 5.5, -0.2 * Math.PI, 1.4 * Math.PI, false);
                                            ctx.stroke();
                                            // Arrowhead
                                            ctx.fillStyle = "#ffffff";
                                            ctx.beginPath();
                                            ctx.moveTo(11, 2);
                                            ctx.lineTo(15, 5);
                                            ctx.lineTo(11, 8);
                                            ctx.closePath();
                                            ctx.fill();
                                        }
                                        Component.onCompleted: requestPaint()
                                    }

                                    RotationAnimation on rotation {
                                        running: root.isRescanning
                                        from: 0
                                        to: 360
                                        duration: 800
                                        loops: Animation.Infinite
                                    }
                                }

                                Text {
                                    text: root.isRescanning ? qsTr("Searching...") : qsTr("Scan for USB devices")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#ffffff"
                                }
                            }

                            MouseArea {
                                id: rescanMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.isRescanning = true;
                                    root.manager.rescan();
                                    rescanFeedbackTimer.restart();
                                }
                            }
                        }
                    }
                }
            }

            // ================= GUÍA PASO A PASO DE CONEXIÓN =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: guideContent.implicitHeight + 24
                color: "#181f26"
                radius: 12
                border.color: "#273a4b"
                border.width: 1
                visible: root.selectedController == null

                ColumnLayout {
                    id: guideContent
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 12

                    // Header row with toggle
                    RowLayout {
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("💡 How do I connect my DJ controller to Android properly?")
                            font.pixelSize: 14
                            font.bold: true
                            color: "#90caf9"
                        }

                        Item { Layout.fillWidth: true }

                        Rectangle {
                            width: 80
                            height: 24
                            radius: 12
                            color: guideToggleArea.containsMouse ? "#263238" : "#1c252d"
                            border.color: "#37474f"
                            border.width: 1

                            Text {
                                anchors.centerIn: parent
                                text: root.showGuide ? qsTr("Hide") : qsTr("Show guide")
                                font.pixelSize: 11
                                color: "#b0bec5"
                            }

                            MouseArea {
                                id: guideToggleArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.showGuide = !root.showGuide
                            }
                        }
                    }

                    // Steps container
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        visible: root.showGuide

                        // Step 1: Cable OTG
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                width: 24
                                height: 24
                                radius: 12
                                color: "#0288d1"
                                Text {
                                    anchors.centerIn: parent
                                    text: "1"
                                    color: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text {
                                    text: qsTr("USB OTG Type-C adapter")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#ffffff"
                                }
                                Text {
                                    text: qsTr("Use an OTG-capable USB-C to USB-B/A adapter or cable directly to the phone.")
                                    font.pixelSize: 11
                                    color: "#b0bec5"
                                }
                            }
                        }

                        // Step 2: Alimentación eléctrica y HUB
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                width: 24
                                height: 24
                                radius: 12
                                color: "#f57c00"
                                Text {
                                    anchors.centerIn: parent
                                    text: "2"
                                    color: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    text: qsTr("⚡ Power supply: connect through a USB-C hub")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#ffa726"
                                }
                                Text {
                                    text: qsTr("Tablets only supply ~500mA. To keep your controller (Pioneer DDJ-400, etc.) from flickering or disconnecting, connect the tablet charger to the USB-C PD port of the hub, the controller to a USB port of the hub, and the hub cable to the tablet.")
                                    font.pixelSize: 11
                                    color: "#cfd8dc"
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        // Step 3: Permiso USB
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                width: 24
                                height: 24
                                radius: 12
                                color: "#388e3c"
                                Text {
                                    anchors.centerIn: parent
                                    text: "3"
                                    color: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    text: qsTr("USB permission on Android")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#ffffff"
                                }
                                Text {
                                    text: qsTr("MIDI controllers (like the DDJ-400) need no permission. If Android asks 'Allow Mixxx to access the USB device?' (HID controllers), tick 'Always use for this device' and press OK.")
                                    font.pixelSize: 11
                                    color: "#b0bec5"
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        // Step 4: Salida de Audio
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                width: 24
                                height: 24
                                radius: 12
                                color: "#7b1fa2"
                                Text {
                                    anchors.centerIn: parent
                                    text: "4"
                                    color: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 3
                                Text {
                                    text: qsTr("Enable audio on the controller (main + CUE headphones)")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#ffffff"
                                }
                                Text {
                                    text: qsTr("• <b>Full DJ booth</b>: speakers to the RCA outputs of the controller and headphones to the front jack. Select the 'DJ controller' profile.<br>• <b>Hybrid mode</b>: if you have no external speakers, select 'Tablet + controller headphones' so the music plays through the tablet and the pre-listening through your headphones.")
                                    font.pixelSize: 11
                                    color: "#b0bec5"
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }

                                Rectangle {
                                    Layout.preferredWidth: 260
                                    Layout.preferredHeight: 30
                                    radius: 6
                                    color: audioBtnMouse.pressed ? "#6a1b9a" : (audioBtnMouse.containsMouse ? "#8e24aa" : "#7b1fa2")
                                    Text {
                                        anchors.centerIn: parent
                                        text: qsTr("🎧 Go to audio outputs and profiles")
                                        font.bold: true
                                        font.pixelSize: 11
                                        color: "#ffffff"
                                    }
                                    MouseArea {
                                        id: audioBtnMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.requestNavigateToAudio()
                                    }
                                }
                            }
                        }

                        // Step 5: Mapeos Personalizados y de serie
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Rectangle {
                                width: 24
                                height: 24
                                radius: 12
                                color: "#00b0ff"
                                Text {
                                    anchors.centerIn: parent
                                    text: "5"
                                    color: "#ffffff"
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 4
                                Text {
                                    text: qsTr("DJ mappings (.midi.xml / .js)")
                                    font.pixelSize: 13
                                    font.bold: true
                                    color: "#80d8ff"
                                }
                                Text {
                                    text: qsTr("✓ <b>Pioneer DDJ-400</b>, FLX4, Inpulse and 100+ controllers come with an official mapping: it is assigned and enabled automatically the first time they are connected.<br>To add a new or modified mapping, copy it to the public Mixxx folder and press 'Scan for USB devices':")
                                    font.pixelSize: 11
                                    color: "#b0bec5"
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }

                                RowLayout {
                                    spacing: 8
                                    Layout.fillWidth: true

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 28
                                        color: "#10161a"
                                        radius: 4
                                        border.color: "#263238"
                                        border.width: 1

                                        Text {
                                            anchors.verticalCenter: parent.verticalCenter
                                            anchors.left: parent.left
                                            anchors.leftMargin: 8
                                            anchors.right: parent.right
                                            anchors.rightMargin: 8
                                            elide: Text.ElideMiddle
                                            text: Mixxx.MobileControllers.mappingsDirectory || "/storage/emulated/0/Mixxx/controllers/"
                                            font.pixelSize: 11
                                            font.family: "monospace"
                                            color: "#80d8ff"
                                        }
                                    }

                                    Rectangle {
                                        Layout.preferredWidth: 150
                                        Layout.preferredHeight: 28
                                        radius: 4
                                        color: copyMouse.pressed ? "#00838f" : (copyMouse.containsMouse ? "#0097a7" : "#00695c")

                                        Text {
                                            anchors.centerIn: parent
                                            text: copyTimer.running ? qsTr("✓ Copied!") : qsTr("📋 Copy path")
                                            font.pixelSize: 11
                                            font.bold: true
                                            color: "#ffffff"
                                        }

                                        MouseArea {
                                            id: copyMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                Mixxx.MobileControllers.copyToClipboard(Mixxx.MobileControllers.mappingsDirectory || "/storage/emulated/0/Mixxx/controllers/");
                                                copyTimer.restart();
                                            }
                                        }

                                        Timer {
                                            id: copyTimer
                                            interval: 2000
                                            repeat: false
                                        }
                                    }
                                }
                            }
                        }

                        // Chip de modelos soportados
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 32
                            radius: 6
                            color: "#1e272e"

                            Text {
                                anchors.centerIn: parent
                                text: qsTr("✨ Compatible with Pioneer DDJ (FLX4, 400, SB3, 200), Hercules Inpulse, Traktor S2/S4, Numark, Reloop and 100+ controllers.")
                                font.pixelSize: 11
                                color: "#81c784"
                            }
                        }
                    }
                }
            }

            // ================= EMPTY STATE IF NO DEVICES =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 140
                color: "#181818"
                radius: 12
                border.color: "#2c2c2c"
                border.width: 1
                visible: root.selectedController == null && root.activeDevices.length === 0 && root.unmappedDevices.length === 0

                ColumnLayout {
                    anchors.centerIn: parent
                    spacing: 10

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: "🔌"
                        font.pixelSize: 32
                    }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("No connected DJ controller was detected")
                        font.pixelSize: 14
                        font.bold: true
                        color: "#9e9e9e"
                    }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("Connect your controller via USB OTG and press 'Scan for USB devices'")
                        font.pixelSize: 12
                        color: "#616161"
                    }
                }
            }

            // ================= BANNER: DISPOSITIVO EN CONFIGURACIÓN =================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 58
                color: "#0d47a1"
                radius: 10
                border.color: "#42a5f5"
                border.width: 1.5
                visible: root.selectedController != null

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    spacing: 12

                    Text {
                        text: "🎛️"
                        font.pixelSize: 22
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Text {
                            text: qsTr("Configuring: %1").arg(root.selectedController?.name || "")
                            font.bold: true
                            font.pixelSize: 13
                            color: "#ffffff"
                        }
                        Text {
                            text: root.selectedController?.mapping 
                                  ? qsTr("Assigned mapping: %1 (%2)").arg(root.selectedController.mapping.name).arg(root.selectedController.enabled ? qsTr("Enabled") : qsTr("Disabled"))
                                  : qsTr("Select its mapping in the drop-down list and press Save")
                            font.pixelSize: 11
                            color: root.selectedController?.mapping ? "#a7ffeb" : "#ffe082"
                        }
                    }

                    // Botón directo a audio
                    Rectangle {
                        Layout.preferredWidth: 105
                        Layout.preferredHeight: 32
                        radius: 6
                        color: audioNavMouse.pressed ? "#4a148c" : (audioNavMouse.containsMouse ? "#7b1fa2" : "#6a1b9a")
                        border.color: "#ce93d8"
                        border.width: 1

                        Text {
                            anchors.centerIn: parent
                            text: qsTr("🎧 Audio outputs")
                            font.pixelSize: 11
                            font.bold: true
                            color: "#ffffff"
                        }
                        MouseArea {
                            id: audioNavMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.requestNavigateToAudio()
                        }
                    }

                    // Botón Volver a la lista
                    Rectangle {
                        Layout.preferredWidth: 90
                        Layout.preferredHeight: 32
                        radius: 6
                        color: backMouse.pressed ? "#0b3c8a" : (backMouse.containsMouse ? "#1565c0" : "#1976d2")
                        border.color: "#90caf9"
                        border.width: 1

                        Text {
                            anchors.centerIn: parent
                            text: qsTr("Show all")
                            font.pixelSize: 11
                            font.bold: true
                            color: "#ffffff"
                        }
                        MouseArea {
                            id: backMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.selectedController = null;
                            }
                        }
                    }
                }
            }

            // ================= KNOWN CONTROLLERS =================
            GridLayout {
                Layout.fillWidth: true
                columnSpacing: 24
                columns: parseInt(root.width / 1050) + 1

                Repeater {
                    id: controllers

                    model: {
                        if (root.selectedController) {
                            return [root.selectedController];
                        }
                        return root.activeDevices;
                    }

                    delegate: ControllerCard {
                        id: controller

                        focused: root.selectedController == modelData
                        knownDevice: Array.from(manager.knownDevices).indexOf(modelData) > -1
                        opacity: root.selectedController && root.selectedController != modelData ? 0.2 : 1

                        onSelect: {
                            root.selectedController = modelData;
                        }
                        onSettingsChanged: {
                            if (!controller.settings && controller.focused) {
                                root.selectedController = null;
                            }
                        }

                        Connections {
                            function onEditedChanged() {
                                if (controller.modelData.edited) {
                                    root.editedControllers.add(controller.modelData);
                                    root.dirty = true;
                                }
                            }

                            target: controller.modelData
                        }
                    }
                }
                Item {
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    visible: root.selectedController == null || !root.selectedController.mapping?.hasSettings
                }
            }

            // ================= UNKNOWN DEVICES =================
            ListView {
                id: unknownDevicesList

                Layout.fillWidth: true
                currentIndex: -1
                implicitHeight: contentHeight
                model: root.unmappedDevices
                visible: root.selectedController == null

                delegate: Rectangle {
                    required property int index
                    required property var modelData

                    color: index % 2 == 0 ? '#0C0C0C' : '#272727'
                    height: 38
                    width: ListView.view.width

                    MouseArea {
                        id: unknownDeviceRow

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            unknownDevicesList.currentIndex = index;
                            root.selectedController = modelData;
                            root.dirty = true;
                            scrollView.contentItem.contentY = 0;
                        }
                    }
                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 6

                        Rectangle {
                            Layout.preferredHeight: 22
                            Layout.preferredWidth: 44
                            color: '#393939'
                            radius: 4

                            Text {
                                anchors.centerIn: parent
                                color: '#FFFFFF'
                                font.pixelSize: 12
                                font.weight: Font.Medium
                                text: {
                                    switch (modelData.type) {
                                    case Mixxx.ControllerDevice.Type.MIDI:
                                        return "MIDI";
                                    case Mixxx.ControllerDevice.Type.HID:
                                        return "HID";
                                    case Mixxx.ControllerDevice.Type.BULK:
                                        return "BULK";
                                    }
                                    return "UNKN";
                                }
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            color: '#FFFFFF'
                            font.pixelSize: 14
                            font.weight: Font.Medium
                            text: modelData.name
                        }
                        Skin.FormButton {
                            activeColor: "#1e88e5"
                            text: qsTr("Configure mapping")
                            visible: true

                            onPressed: {
                                unknownDevicesList.currentIndex = index;
                                root.selectedController = modelData;
                                root.dirty = true;
                                scrollView.contentItem.contentY = 0;
                            }
                        }
                        TapHandler {
                            onTapped: {
                                unknownDevicesList.currentIndex = index;
                            }
                        }
                    }
                }
                header: Rectangle {
                    color: "#161616"
                    implicitHeight: 36
                    width: ListView.view.width
                    visible: root.unmappedDevices.length > 0

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 20
                        anchors.margins: 10
                        anchors.rightMargin: 20
                        color: '#FFFFFF'
                        font.pixelSize: 15
                        font.weight: Font.DemiBold
                        text: qsTr("Other USB devices detected without a mapping")
                    }
                }
            }
        }
    }

    // ================= BARRA INFERIOR FIJA: ACCIONES Y ESTADO =================
    Rectangle {
        id: controllerAction
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
                color: root.dirty ? "#332200" : (root.activeDevices.length > 0 ? "#0d2b1a" : "#1a2233")
                border.color: root.dirty ? "#ffb300" : (root.activeDevices.length > 0 ? "#00c853" : "#2196f3")
                border.width: 1

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: 8
                    Text {
                        text: root.dirty ? "●" : (root.activeDevices.length > 0 ? "✓" : "🎛️")
                        font.bold: true
                        font.pixelSize: 13
                        color: root.dirty ? "#ffb300" : (root.activeDevices.length > 0 ? "#00e676" : "#64b5f6")
                    }
                    Text {
                        text: root.dirty 
                              ? qsTr("Unsaved changes")
                              : (root.selectedController 
                                 ? qsTr("Editing controller")
                                 : (root.activeDevices.length > 0 ? qsTr("Controller ready") : qsTr("USB MIDI")))
                        font.bold: true
                        font.pixelSize: 12
                        color: root.dirty ? "#ffe082" : (root.activeDevices.length > 0 ? "#b9f6ca" : "#bbdefb")
                    }
                }
            }

            // Error message if any
            Text {
                id: errorMessage
                Layout.alignment: Qt.AlignVCenter
                visible: text.length > 0
                color: "#ff5252"
                font.pixelSize: 12
                text: ""
            }

            Item { Layout.fillWidth: true }

            // Botón Restablecer
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 140
                radius: 6
                visible: !!root.selectedController?.mapping?.hasSettings
                color: resetMouse.pressed ? "#5c2b2b" : (resetMouse.containsMouse ? "#4a2222" : "#371e1e")
                border.color: "#772b2b"
                border.width: 1

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "↺"; color: "#ff8a80"; font.pixelSize: 14 }
                    Text {
                        text: qsTr("Reset")
                        font.bold: true
                        font.pixelSize: 12
                        color: "#ffcdd2"
                    }
                }

                MouseArea {
                    id: resetMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.selectedController?.mapping?.resetSettings(root.selectedController);
                        root.load();
                    }
                }
            }

            // Botón Descartar / Volver
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 130
                radius: 6
                visible: root.dirty || root.selectedController !== null
                color: discardMouse.pressed ? "#424242" : (discardMouse.containsMouse ? "#333333" : "#262626")
                border.color: "#555555"
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: root.selectedController ? qsTr("Back to list") : qsTr("Discard")
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
                        root.selectedController = null;
                        root.load();
                        errorMessage.text = "";
                    }
                }
            }

            // Botón Guardar Cambios
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
                        text: root.dirty ? qsTr("Save changes") : qsTr("Saved")
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
                        errorMessage.text = "";
                        root.save();
                    }
                }
            }
        }
    }
}
