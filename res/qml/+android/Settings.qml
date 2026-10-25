import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes
import "Theme"
import "Settings" as Settings

Popup {
    id: root

    property var activeCategory: null
    property alias activeCategoryIndex: categoryList.currentIndex
    readonly property var manager: managerItem
    property alias sections: managerItem.data
    property var categoryItems: []
    // Skins can give the window their own title and colors
    property string title: qsTr("⚙️ Settings")
    property color headerColor: Theme.darkGray3
    property color backgroundColor: Theme.darkGray2

    function updateActiveCategory() {
        root.activeCategory?.deactivated();
        if (root.categoryItems && categoryList.currentIndex >= 0 && categoryList.currentIndex < root.categoryItems.length) {
            root.activeCategory = root.categoryItems[categoryList.currentIndex];
        } else {
            root.activeCategory = null;
        }
        root.activeCategory?.activated();
    }

    function resetSearch() {
        searchSetting.active = false;
        searchInput.text = "";
        // Assigning text doesn't emit textEdited, so reset the filter too
        root.manager.search("");
        root.forceActiveFocus();
    }

    onOpened: {
        resetSearch();
    }

    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    focus: true
    horizontalPadding: 8
    verticalPadding: 8

    Keys.onBackPressed: root.close()
    Keys.onEscapePressed: root.close()

    background: Rectangle {
        anchors.fill: parent
        color: root.backgroundColor
        opacity: parent.radius < 0 ? Math.max(0.1, 1 + parent.radius / 8) : 1
        radius: 8
        border.color: "#383c48"
        border.width: 1
    }

    contentItem: ColumnLayout {
        anchors.fill: parent
        spacing: 6

        // Top navigation bar with Back / Title / Close button
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            color: root.headerColor
            radius: 6

            RowLayout {
                anchors.fill: parent
                anchors.margins: 4

                Skin.Button {
                    id: backButton
                    Layout.preferredHeight: 32
                    Layout.preferredWidth: 105
                    activeColor: Theme.accentColor
                    font.bold: true
                    text: qsTr("← Back")
                    onClicked: root.close()
                }

                Text {
                    Layout.fillWidth: true
                    color: Theme.white
                    font.bold: true
                    font.pixelSize: 15
                    horizontalAlignment: Text.AlignHCenter
                    text: root.title
                }

                Skin.Button {
                    id: closeButton
                    Layout.preferredHeight: 32
                    Layout.preferredWidth: 42
                    activeColor: Theme.accentColor
                    text: "✕"
                    onClicked: root.close()
                }
            }
        }

        RowLayout {
            Layout.fillHeight: true
            Layout.fillWidth: parent.width
            spacing: 6

            // Left Sidebar: Categories
            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: Math.min(230, Math.max(160, Math.floor(parent.width * 0.22)))
                border.color: "#30343f"
                border.width: 1
                color: Theme.darkGray
                radius: 6

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 4

                    Rectangle {
                        id: searchSetting

                        property bool active: false
                        property alias input: searchInput

                        Layout.fillWidth: true
                        color: Theme.midGray
                        height: 32
                        radius: 4

                        Text {
                            id: searchInputPlaceholder
                            anchors.left: parent.left
                            anchors.leftMargin: 8
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#9e9e9e"
                            font.pixelSize: 12
                            text: qsTr("🔍 Search settings...")
                            visible: !parent.active && searchInput.text.length === 0
                        }

                        TextInput {
                            id: searchInput
                            focus: false
                            activeFocusOnTab: false
                            anchors.left: parent.left
                            anchors.right: clearSearchBtn.visible ? clearSearchBtn.left : parent.right
                            anchors.leftMargin: 8
                            anchors.rightMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.white
                            font.pixelSize: 12
                            visible: parent.active || text.length > 0

                            onTextEdited: {
                                root.manager.search(text);
                            }
                        }

                        Text {
                            id: clearSearchBtn
                            anchors.right: parent.right
                            anchors.rightMargin: 8
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#bbdefb"
                            font.pixelSize: 12
                            font.bold: true
                            text: "✕"
                            visible: parent.active || searchInput.text.length > 0
                            MouseArea {
                                anchors.fill: parent
                                anchors.margins: -6
                                onClicked: {
                                    root.resetSearch();
                                }
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            visible: !searchSetting.active && searchInput.text.length === 0
                            onClicked: {
                                searchSetting.active = true;
                                searchInput.forceActiveFocus();
                            }
                        }
                    }

                    ListView {
                        id: categoryList

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        clip: true
                        currentIndex: 0
                        focus: true
                        model: sectionProperties
                        visible: !searchSetting.active
                        spacing: 4

                        delegate: Rectangle {
                            id: categoryDelegate
                            required property int index
                            required property string label
                            required property string iconGlyph
                            required property string subtext

                            readonly property bool isSelected: ListView.isCurrentItem

                            color: isSelected ? Theme.accentColor : (tapArea.pressed ? "#2a2d36" : "transparent")
                            height: 48
                            radius: 6
                            width: ListView.view.width

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 8
                                spacing: 10

                                Text {
                                    color: categoryDelegate.isSelected ? "#ffffff" : Theme.accentColor
                                    font.pixelSize: 18
                                    text: iconGlyph
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1

                                    Text {
                                        Layout.fillWidth: true
                                        color: categoryDelegate.isSelected ? "#ffffff" : "#e0e0e0"
                                        elide: Text.ElideRight
                                        font.bold: categoryDelegate.isSelected
                                        font.pixelSize: 13
                                        text: qsTr(label)
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        color: categoryDelegate.isSelected ? "#d0e4ff" : "#808595"
                                        elide: Text.ElideRight
                                        font.pixelSize: 10
                                        text: qsTr(subtext)
                                    }
                                }
                            }

                            TapHandler {
                                onTapped: {
                                    categoryList.currentIndex = index;
                                }
                            }

                            MouseArea {
                                id: tapArea
                                anchors.fill: parent
                                onClicked: {
                                    categoryList.currentIndex = index;
                                }
                            }
                        }
                    }

                    ListView {
                        id: settingResultList

                        Layout.fillHeight: true
                        Layout.fillWidth: true
                        clip: true
                        focus: true
                        model: root.manager.model
                        visible: searchSetting.active
                        spacing: 2

                        delegate: Rectangle {
                            required property var display
                            required property int index
                            required property var toolTip
                            required property var whatsThis

                            color: Theme.darkGray2
                            height: 42
                            radius: 4
                            width: ListView.view.width

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 4
                                spacing: 1

                                Text {
                                    Layout.fillWidth: true
                                    color: Theme.white
                                    font.pixelSize: 12
                                    font.bold: true
                                    text: searchSetting.input.text ? display.replace(searchSetting.input.text, `<b>${searchSetting.input.text}</b>`) : display
                                    textFormat: Text.RichText
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    color: Theme.midGray
                                    font.pixelSize: 10
                                    text: searchSetting.input.text ? whatsThis.replace(searchSetting.input.text, `<b>${searchSetting.input.text}</b>`) : whatsThis
                                    textFormat: Text.RichText
                                    elide: Text.ElideRight
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                onClicked: {
                                    for (let setting of toolTip) {
                                        setting.activated();
                                    }
                                    searchSetting.active = false;
                                }
                            }
                        }
                    }
                }
            }

            // Right Content Area: Tabs + Category Page
            ColumnLayout {
                Layout.fillHeight: true
                Layout.fillWidth: true
                spacing: 4

                Rectangle {
                    id: tabBar

                    readonly property int selectedIndex: root.activeCategory?.selectedIndex ?? 0
                    readonly property var tabs: root.activeCategory?.tabs ?? []

                    Layout.fillWidth: true
                    Layout.preferredHeight: 34
                    color: Theme.darkGray3
                    radius: 4
                    visible: tabs && tabs.length > 0

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 3
                        spacing: 6

                        Repeater {
                            model: tabBar.tabs

                            Skin.Button {
                                required property int index
                                required property string modelData

                                Layout.alignment: Qt.AlignHCenter
                                Layout.preferredHeight: 28
                                Layout.preferredWidth: Math.max(100, Math.floor(parent.width / (tabBar.tabs.length + 0.5)))
                                activeColor: Theme.accentColor
                                checked: tabBar.selectedIndex === index
                                text: modelData

                                onClicked: {
                                    if (root.activeCategory?.selectedIndex !== undefined) {
                                        root.activeCategory.selectedIndex = index;
                                    }
                                }
                            }
                        }
                    }
                }

                Mixxx.SettingParameterManager {
                    id: managerItem

                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    Layout.leftMargin: 6

                    Settings.SoundHardware {
                        id: soundHardwareSection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 0
                        enabled: visible
                    }

                    Settings.Controller {
                        id: controllerSection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 1
                        enabled: visible
                    }

                    Settings.Library {
                        id: librarySection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 2
                        enabled: visible
                    }

                    Settings.Waveform {
                        id: waveformSection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 3
                        enabled: visible
                    }

                    Settings.Interface {
                        id: interfaceSection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 4
                        enabled: visible
                    }

                    Settings.About {
                        id: aboutSection
                        anchors.fill: parent
                        visible: categoryList.currentIndex === 5
                        enabled: visible
                    }

                    Component.onCompleted: {
                        root.categoryItems = [
                            soundHardwareSection,
                            controllerSection,
                            librarySection,
                            waveformSection,
                            interfaceSection,
                            aboutSection
                        ];
                        soundHardwareSection.activated.connect(function() { categoryList.currentIndex = 0; });
                        controllerSection.activated.connect(function() { categoryList.currentIndex = 1; });
                        controllerSection.requestNavigateToAudio.connect(function() { categoryList.currentIndex = 0; });
                        librarySection.activated.connect(function() { categoryList.currentIndex = 2; });
                        waveformSection.activated.connect(function() { categoryList.currentIndex = 3; });
                        interfaceSection.activated.connect(function() { categoryList.currentIndex = 4; });
                        aboutSection.activated.connect(function() { categoryList.currentIndex = 5; });
                        root.updateActiveCategory();
                    }
                }
            }
        }
    }

    onActiveCategoryIndexChanged: updateActiveCategory()
    onSectionsChanged: updateActiveCategory()

    ListModel {
        id: sectionProperties

        ListElement {
            label: QT_TR_NOOP("Audio & Outputs")
            iconGlyph: "🎧"
            subtext: QT_TR_NOOP("Main, Headphones, Latency")
        }
        ListElement {
            label: QT_TR_NOOP("DJ Controllers")
            iconGlyph: "🎛️"
            subtext: "MIDI / HID USB"
        }
        ListElement {
            label: QT_TR_NOOP("Music Library")
            iconGlyph: "📁"
            subtext: QT_TR_NOOP("Folders, Track scanning")
        }
        ListElement {
            label: QT_TR_NOOP("Waveforms")
            iconGlyph: "〰️"
            subtext: QT_TR_NOOP("FPS, RGB/HSV colors, Zoom")
        }
        ListElement {
            label: QT_TR_NOOP("Interface")
            iconGlyph: "📱"
            subtext: QT_TR_NOOP("Theme, Decks, Display")
        }
        ListElement {
            label: QT_TR_NOOP("About")
            iconGlyph: "✨"
            subtext: qsTr("JaviLabarum, Socials & Info")
        }
    }
}
