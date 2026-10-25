import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Mixxx 1.0 as Mixxx
import ".." as Skin
import "." as Setting
import "../Theme"

Category {
    id: root

    property bool dirty: false
    label: qsTr("Waveforms")
    readonly property string graphicsApiName: {
        switch (GraphicsInfo.api) {
        case GraphicsInfo.OpenGL:
            return "OpenGL";
        case GraphicsInfo.Metal:
            return "Metal";
        case GraphicsInfo.Direct3D11:
            return "Direct3D 11";
        case GraphicsInfo.Direct3D12:
            return "Direct3D 12";
        case GraphicsInfo.Vulkan:
            return "Vulkan";
        case GraphicsInfo.Software:
            return "Software";
        case GraphicsInfo.Null:
            return "Null";
        default:
            return qsTr("Unknown");
        }
    }
    readonly property list<int> waveformTypes: [Mixxx.WaveformDisplay.Type.Simple, Mixxx.WaveformDisplay.Type.Filtered, Mixxx.WaveformDisplay.Type.HSV, Mixxx.WaveformDisplay.Type.RGB, Mixxx.WaveformDisplay.Type.Stacked]

    function load() {
        overviewTypeInput.currentIndex = Math.max(0, Math.min(2, Mixxx.Config.waveformOverviewType));
        overviewStereoInput.selected = Mixxx.Config.waveformOverviewStereo ? "on" : "off";
        overviewScalingInput.selected = Mixxx.Config.waveformOverviewNormalized ? "normalize" : "global gain";
        waveformEnabledInput.selected = Mixxx.Config.waveformEnabled ? "on" : "off";
        frameRateInput.value = Mixxx.Config.waveformFrameRate;
        if (frameRateInput.value <= 60) frameRateRatio.selected = "60 fps";
        else if (frameRateInput.value <= 90) frameRateRatio.selected = "90 fps";
        else frameRateRatio.selected = "120 fps ★";
        waveformTypeInput.currentIndex = root.waveformTypes.indexOf(Mixxx.Config.waveformType);
        if (waveformTypeInput.currentIndex < 0)
            waveformTypeInput.currentIndex = 3;
        zoomSynchronizationInput.selected = Mixxx.Config.waveformZoomSynchronization ? "on" : "off";
        var options = Mixxx.Config.waveformOptions;
        splitStereoSignalInput.selected = (options & Mixxx.WaveformDisplay.Option.SplitStereoSignal) ? "on" : "off";
        highDetailInput.selected = (options & Mixxx.WaveformDisplay.Option.HighDetail) ? "on" : "off";
        defaultZoomInput.value = Mixxx.Config.waveformDefaultZoom;
        endOfTrackWarningInput.value = Mixxx.Config.waveformEndOfTrackWarningTime;
        beatGridAlphaInput.value = Mixxx.Config.waveformBeatGridAlpha;
        playMarkerPositionInput.value = Mixxx.Config.waveformPlayMarkerPosition * 100;
        visualGainAllInput.value = Mixxx.Config.waveformVisualGainAll;
        visualGainLowInput.value = Mixxx.Config.waveformVisualGainLow;
        visualGainMidInput.value = Mixxx.Config.waveformVisualGainMedium;
        visualGainHighInput.value = Mixxx.Config.waveformVisualGainHigh;
        untilMarkShowBeatsInput.selected = Mixxx.Config.waveformUntilMarkShowBeats ? "on" : "off";
        untilMarkShowTimeInput.selected = Mixxx.Config.waveformUntilMarkShowTime ? "on" : "off";
        untilMarkAlignInput.currentIndex = Math.max(0, Math.min(2, Mixxx.Config.waveformUntilMarkAlign));
        untilMarkTextSizeInput.value = Mixxx.Config.waveformUntilMarkTextPointSize;
        stemOpacityInput.value = Mixxx.Config.waveformStemOpacity;
        stemOutlineOpacityInput.value = Mixxx.Config.waveformStemOutlineOpacity;
        stemReorderInput.selected = Mixxx.Config.waveformStemReorderOnChange ? "on" : "off";
        stemDisplayModeInput.selected = Mixxx.Config.waveformStemSplitTracks ? "stacked" : "overlap";
        cachingInput.selected = Mixxx.Config.waveformCachingEnabled ? "on" : "off";
        generateWithAnalysisInput.selected = Mixxx.Config.waveformGenerationWithAnalysisEnabled ? "on" : "off";
        root.dirty = false;
    }
    function markDirty() {
        if (!root.dirty)
            root.dirty = true;
    }
    function reset() {
        overviewTypeInput.currentIndex = 2;
        overviewStereoInput.selected = "on";
        overviewMinuteMarkersInput.selected = "on";
        overviewScalingInput.selected = "global gain";
        waveformTypeInput.currentIndex = 3;
        zoomSynchronizationInput.selected = "on";
        waveformEnabledInput.selected = "on";
        frameRateInput.value = 60;
        splitStereoSignalInput.selected = "off";
        highDetailInput.selected = "off";
        defaultZoomInput.value = 3;
        endOfTrackWarningInput.value = 30;
        beatGridAlphaInput.value = 90;
        playMarkerPositionInput.value = 50;
        visualGainAllInput.value = 2;
        visualGainLowInput.value = 1;
        visualGainMidInput.value = 1;
        visualGainHighInput.value = 1;
        untilMarkShowBeatsInput.selected = "off";
        untilMarkShowTimeInput.selected = "off";
        untilMarkAlignInput.currentIndex = 1;
        untilMarkTextSizeInput.value = 24;
        stemOpacityInput.value = 0.75;
        stemOutlineOpacityInput.value = 0.15;
        stemReorderInput.selected = "on";
        stemDisplayModeInput.selected = "overlap";
        cachingInput.selected = "on";
        generateWithAnalysisInput.selected = "off";
        root.dirty = true;
    }
    function save() {
        Mixxx.Config.waveformOverviewType = overviewTypeInput.currentIndex;
        Mixxx.Config.waveformOverviewStereo = overviewStereoInput.selected === "on";
        Mixxx.Config.waveformOverviewMinuteMarkers = overviewMinuteMarkersInput.selected === "on";
        Mixxx.Config.waveformOverviewNormalized = overviewScalingInput.selected === "normalize";
        Mixxx.Config.waveformType = root.waveformTypes[waveformTypeInput.currentIndex];
        Mixxx.Config.waveformEnabled = waveformEnabledInput.selected === "on";
        Mixxx.Config.waveformZoomSynchronization = zoomSynchronizationInput.selected === "on";
        Mixxx.Config.waveformFrameRate = frameRateInput.value;
        var options = 0;
        if (waveformTypeInput.currentIndex === 3 && splitStereoSignalInput.selected === "on")
            options |= Mixxx.WaveformDisplay.Option.SplitStereoSignal;
        if (highDetailInput.selected === "on")
            options |= Mixxx.WaveformDisplay.Option.HighDetail;
        Mixxx.Config.waveformOptions = options;
        Mixxx.Config.waveformDefaultZoom = defaultZoomInput.value;
        Mixxx.Config.waveformEndOfTrackWarningTime = endOfTrackWarningInput.value;
        Mixxx.Config.waveformBeatGridAlpha = beatGridAlphaInput.value;
        Mixxx.Config.waveformPlayMarkerPosition = playMarkerPositionInput.value / 100;
        Mixxx.Config.waveformVisualGainAll = visualGainAllInput.value;
        Mixxx.Config.waveformVisualGainLow = visualGainLowInput.value;
        Mixxx.Config.waveformVisualGainMedium = visualGainMidInput.value;
        Mixxx.Config.waveformVisualGainHigh = visualGainHighInput.value;
        Mixxx.Config.waveformUntilMarkShowBeats = untilMarkShowBeatsInput.selected === "on";
        Mixxx.Config.waveformUntilMarkShowTime = untilMarkShowTimeInput.selected === "on";
        Mixxx.Config.waveformUntilMarkAlign = untilMarkAlignInput.currentIndex;
        Mixxx.Config.waveformUntilMarkTextPointSize = untilMarkTextSizeInput.value;
        Mixxx.Config.waveformStemOpacity = stemOpacityInput.value;
        Mixxx.Config.waveformStemOutlineOpacity = stemOutlineOpacityInput.value;
        Mixxx.Config.waveformStemReorderOnChange = stemReorderInput.selected === "on";
        Mixxx.Config.waveformStemSplitTracks = stemDisplayModeInput.selected === "stacked";
        Mixxx.Config.waveformCachingEnabled = cachingInput.selected === "on";
        Mixxx.Config.waveformGenerationWithAnalysisEnabled = generateWithAnalysisInput.selected === "on";
        root.load();
    }

    Component.onCompleted: {
        Mixxx.Library.refreshWaveformCacheDiskUsage();
        root.load();
    }

    ScrollView {
        id: scrollView

        anchors.bottom: buttonActions.top
        anchors.bottomMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            id: content

            width: scrollView.availableWidth
            spacing: 10

            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.pixelSize: 14
                font.weight: Font.DemiBold
                text: qsTr("Overview Waveforms")
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Type")
                }
                Skin.ComboBox {
                    id: overviewTypeInput

                    Layout.preferredWidth: 180
                    model: [qsTr("Filtered"), "HSV", "RGB"]

                    onCurrentIndexChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Stereo channels")
                }
                RatioChoice {
                    id: overviewStereoInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Minute markers")
                }
                RatioChoice {
                    id: overviewMinuteMarkersInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Gain scaling")
                }
                RatioChoice {
                    id: overviewScalingInput

                    options: ["normalize", "global gain"]

                    onSelectedChanged: root.markDirty()
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.darkGray3
            }
            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.pixelSize: 14
                font.weight: Font.DemiBold
                text: qsTr("Scrolling Waveforms")
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Type")
                }
                Skin.ComboBox {
                    id: waveformTypeInput

                    Layout.preferredWidth: 180
                    model: [qsTr("Simple"), qsTr("Filtered"), "HSV", "RGB", qsTr("Stacked")]

                    onCurrentIndexChanged: root.markDirty()
                }
            }
            Text {
                Layout.fillWidth: true
                color: Theme.midGray
                text: qsTr("Simple, Filtered, HSV, RGB and Stacked are supported by the QML renderer.")
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Enabled")
                }
                RatioChoice {
                    id: waveformEnabledInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true
                WaveformSettingParameter {
                    label: qsTr("Waveform smoothness (FPS)")
                }
                RatioChoice {
                    id: frameRateRatio
                    options: ["60 fps", "90 fps", "120 fps ★"]
                    onSelectedChanged: {
                        if (selected === "60 fps") frameRateInput.value = 60;
                        else if (selected === "90 fps") frameRateInput.value = 90;
                        else frameRateInput.value = 120;
                        root.markDirty();
                    }
                }
            }
            WaveformSliderRow {
                id: frameRateInput

                decimals: 0
                label: qsTr("Frame rate")
                max: 240
                min: 10
                suffix: " fps"
                visible: false

                onValueChanged: root.markDirty()
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Sync deck zoom")
                }
                RatioChoice {
                    id: zoomSynchronizationInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            WaveformSliderRow {
                id: defaultZoomInput

                decimals: 0
                label: qsTr("Default zoom")
                max: 10
                min: 1
                suffix: "x"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: endOfTrackWarningInput

                decimals: 0
                label: qsTr("End-of-track warning")
                max: 120
                min: 0
                suffix: "s"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: beatGridAlphaInput

                decimals: 0
                label: qsTr("Beat-grid opacity")
                max: 100
                min: 0
                suffix: "%"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: playMarkerPositionInput

                decimals: 0
                label: qsTr("Play-marker position")
                max: 100
                min: 0
                suffix: "%"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: visualGainAllInput

                decimals: 2
                label: qsTr("Visual gain (all)")
                max: 3
                min: 0
                suffix: "x"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: visualGainLowInput

                decimals: 2
                label: qsTr("Visual gain (low)")
                max: 3
                min: 0
                suffix: "x"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: visualGainMidInput

                decimals: 2
                label: qsTr("Visual gain (mid)")
                max: 3
                min: 0
                suffix: "x"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: visualGainHighInput

                decimals: 2
                label: qsTr("Visual gain (high)")
                max: 3
                min: 0
                suffix: "x"

                onValueChanged: root.markDirty()
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Split stereo signal")
                }
                RatioChoice {
                    id: splitStereoSignalInput

                    enabled: waveformTypeInput.currentIndex === 3
                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            Text {
                Layout.fillWidth: true
                color: Theme.midGray
                text: qsTr("Split stereo signal is used by the RGB renderer.")
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("High detail (legacy)")
                }
                RatioChoice {
                    id: highDetailInput

                    enabled: false
                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            Text {
                Layout.fillWidth: true
                color: Theme.midGray
                text: qsTr("High detail is retained for compatibility but is unavailable in the QML scene-graph renderer.")
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Show beats until marker")
                }
                RatioChoice {
                    id: untilMarkShowBeatsInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Show time until marker")
                }
                RatioChoice {
                    id: untilMarkShowTimeInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Marker alignment")
                }
                Skin.ComboBox {
                    id: untilMarkAlignInput

                    Layout.preferredWidth: 180
                    model: [qsTr("Top"), qsTr("Center"), qsTr("Bottom")]

                    onCurrentIndexChanged: root.markDirty()
                }
            }
            WaveformSliderRow {
                id: untilMarkTextSizeInput

                decimals: 0
                label: qsTr("Marker text size")
                max: 48
                min: 6
                suffix: "px"

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: stemOpacityInput

                decimals: 2
                label: qsTr("Stem channel opacity")
                max: 1
                min: 0

                onValueChanged: root.markDirty()
            }
            WaveformSliderRow {
                id: stemOutlineOpacityInput

                decimals: 2
                label: qsTr("Stem outline opacity")
                max: 1
                min: 0

                onValueChanged: root.markDirty()
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Reorder stems when volume changes")
                }
                RatioChoice {
                    id: stemReorderInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Stem display mode")
                }
                RatioChoice {
                    id: stemDisplayModeInput

                    options: ["overlap", "stacked"]

                    onSelectedChanged: root.markDirty()
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.darkGray3
            }
            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.pixelSize: 14
                font.weight: Font.DemiBold
                text: qsTr("Caching")
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Enable waveform caching")
                }
                RatioChoice {
                    id: cachingInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                WaveformSettingParameter {
                    label: qsTr("Generate while analyzing")
                }
                RatioChoice {
                    id: generateWithAnalysisInput

                    options: ["on", "off"]

                    onSelectedChanged: root.markDirty()
                }
            }
            RowLayout {
                Layout.fillWidth: true

                Text {
                    Layout.fillWidth: true
                    color: Theme.white
                    text: qsTr("Cached waveforms occupy %1 on disk.").arg(Mixxx.Library.waveformCacheDiskUsage || qsTr("unknown"))
                }
                Skin.Button {
                    text: qsTr("Clear cached waveforms")

                    onPressed: Mixxx.Library.clearCachedWaveforms()
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.darkGray3
            }
            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.pixelSize: 14
                font.weight: Font.DemiBold
                text: qsTr("Graphics API Status")
            }
            Text {
                Layout.fillWidth: true
                color: Theme.white
                text: qsTr("Qt Quick renderer: %1").arg(root.graphicsApiName)
            }
            Text {
                Layout.fillWidth: true
                color: Theme.white
                text: Mixxx.Config.waveformAverageFrameRate > 0
                        ? qsTr("Average frame rate: %1 fps").arg(Number(Mixxx.Config.waveformAverageFrameRate).toFixed(2))
                        : qsTr("Average frame rate: Measuring…")
            }
            Text {
                Layout.fillWidth: true
                color: Theme.midGray
                text: GraphicsInfo.api === GraphicsInfo.OpenGL ? "OpenGL " + GraphicsInfo.majorVersion + "." + GraphicsInfo.minorVersion : qsTr("Runtime API reported by Qt Quick")
            }
        }
    }
    // ================= BARRA INFERIOR FIJA: ACCIONES Y ESTADO =================
    Rectangle {
        id: buttonActions
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
                color: root.dirty ? "#332200" : "#0d2b1a"
                border.color: root.dirty ? "#ffb300" : "#00c853"
                border.width: 1

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: 8
                    Text {
                        text: root.dirty ? "●" : "✓"
                        font.bold: true
                        font.pixelSize: 13
                        color: root.dirty ? "#ffb300" : "#00e676"
                    }
                    Text {
                        text: root.dirty ? qsTr("Unsaved changes") : qsTr("Waveforms saved")
                        font.bold: true
                        font.pixelSize: 12
                        color: root.dirty ? "#ffe082" : "#b9f6ca"
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

            // Botón Restablecer valores
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 140
                radius: 6
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
                        root.reset();
                    }
                }
            }

            // Botón Descartar
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
                        errorMessage.text = "";
                    }
                }
            }

            // Botón Guardar Formas de Onda
            Rectangle {
                Layout.preferredHeight: 40
                Layout.preferredWidth: 230
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
                        text: root.dirty ? qsTr("Save waveforms") : qsTr("Saved")
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

    component WaveformSettingParameter: Mixxx.SettingParameter {
        Layout.fillWidth: true
        implicitHeight: 30

        Text {
            anchors.fill: parent
            color: Theme.white
            font.pixelSize: 14
            text: parent.label
            verticalAlignment: Text.AlignVCenter
        }
    }
    component WaveformSliderRow: RowLayout {
        property alias decimals: slider.decimals
        required property string label
        property alias max: slider.max
        property alias min: slider.min
        property alias suffix: slider.suffix
        property alias value: slider.value

        Layout.fillWidth: true

        WaveformSettingParameter {
            label: parent.label
        }
        Setting.Slider {
            id: slider

            Layout.preferredWidth: 260
        }
    }
}
