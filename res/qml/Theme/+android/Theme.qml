pragma Singleton
import QtQuick 2.12

QtObject {
    // ---- Rekordbox-inspired color scheme ----
    // Dark professional grays with orange accent
    property color accentColor: "#FF6600"          // Rekordbox orange
    property color backgroundColor: "#1A1A1A"      // Near black
    property color blue: "#FF6600"                 // Use accent for blue roles
    property color bpmSliderBarColor: accentColor
    property color buttonActiveColor: "#FFFFFF"
    property color buttonDisableColor: "#555555"
    property int buttonFontPixelSize: 10
    property color buttonNormalColor: "#888888"
    property color crossfaderBarColor: accentColor
    property color crossfaderOrientationColor: "#666666"
    property color darkGray: "#121212"
    property color darkGray2: "#2A2A2A"
    property color darkGray3: "#333333"
    property color darkGray4: "#1E1E1E"
    property color deckActiveColor: "#FFFFFF"
    property color deckBackgroundColor: "#1A1A1A"
    property color deckBeatjumpBackgroundColor: "#333333"
    property color deckBeatjumpLabelColor: "#555555"
    property color deckEmptyCoverArt: "#333333"
    property color deckInfoBarBackgroundColor: '#0D0D0D'
    property color deckLineColor: "#2A2A2A"
    property color deckLoopBackgroundColor: "#333333"
    property color deckLoopLabelColor: "#555555"
    property color deckTextColor: "#CCCCCC"
    property color effectColor: "#FF6600"
    property color effectUnitColor: "#FF6600"
    property color embeddedBackgroundColor: "#E0000000"
    property color eqFxColor: "#FF6600"
    property color eqHighColor: "#FFFFFF"
    property color eqLowColor: "#FFFFFF"
    property color eqMidColor: "#FFFFFF"
    property string fontFamily: "Open Sans"
    property color gainKnobColor: accentColor
    property color green: "#00E676"
    property string imgBpmSliderBackground: "images/slider_bpm.svg"
    property string imgButton: "images/button.svg"
    property string imgButtonPressed: "images/button_pressed.svg"
    property string imgCrossfaderBackground: "images/slider_crossfader.svg"
    property string imgCrossfaderHandle: "images/slider_handle_crossfader.svg"
    property string imgKnob: "images/knob.svg"
    property string imgKnobMini: "images/miniknob.svg"
    property string imgKnobMiniShadow: "images/miniknob_shadow.svg"
    property string imgKnobShadow: "images/knob_shadow.svg"
    property string imgMicDuckingSlider: "images/slider_micducking.svg"
    property string imgMicDuckingSliderHandle: "images/slider_handle_micducking.svg"
    property string imgPopupBackground: imgButton
    property string imgSectionBackground: "images/section.svg"
    property string imgSliderHandle: "images/slider_handle.svg"
    property string imgVolumeSliderBackground: "images/slider_volume.svg"
    property color knobBackgroundColor: "#1E1E1E"
    property color libraryPanelSplitterBackground: "#1A1A1A"
    property color libraryPanelSplitterHandle: "#444444"
    property color libraryPanelSplitterHandleActive: "#666666"
    property color lightGray: "#777777"
    property color lightGray2: "#AAAAAA"
    property color lightGray3: "#888888"
    property color midGray: "#666666"
    property color midGray2: "#555555"
    property color midGray3: "#444444"
    property color panelSplitterBackground: backgroundColor
    property color panelSplitterHandle: midGray
    property color panelSplitterHandleActive: lightGray2
    property color pflActiveButtonColor: accentColor
    property color red: "#FF3300"
    property color samplerColor: "#FF9500"
    property color sunkenBackgroundColor: "#0F0F0F"
    property color textColor: "#CCCCCC"
    property int textFontPixelSize: 14
    property color toolbarActiveColor: "#FFFFFF"
    property color toolbarBackgroundColor: "#2A2A2A"
    property color volumeSliderBarColor: accentColor
    property color warningColor: "#CC3300"
    property color waveformBeatColor: "#555555"
    property color waveformCursorColor: "#FFFFFF"
    property color waveformMarkerDefault: '#FF6600'
    property color waveformMarkerIntroOutroColor: '#FF9500'
    property color waveformMarkerLabel: Qt.rgba(255, 255, 255, 0.9)
    property color waveformMarkerLoopColor: '#00E676'
    property color waveformMarkerLoopColorDisabled: '#FFFFFF'
    property color waveformPostrollColor: "#444444"
    property color waveformPrerollColor: "#444444"
    property color white: "#EDEDED"
    property color yellow: "#FFB340"

    // ---- Android port: library colors ----
    property color libraryScrollHandle: "#444444"
    property color libraryScrollHandleHovered: "#666666"
    property color libraryScrollHandlePressed: accentColor
    property color libraryScrollTrack: "#121212"
    property color libraryActionBackground: "#2A2A2A"
    property color libraryActionDisabledBackground: "#1E1E1E"
    property color libraryActionText: "#FFFFFF"
    property color libraryActionDisabledText: "#666666"
    property color libraryDeck1Background: accentColor
    property color libraryDeck1Pressed: "#CC5500"
    property color libraryDeck2Background: "#CC5500"
    property color libraryDeck2Pressed: "#FF9500"
    property color libraryConfirmBackground: accentColor
    property color librarySuccessColor: "#00E676"
    property color librarySourceBarBackground: "#121212"
    property color librarySourceBarBorder: "#2A2A2A"
    property color librarySourceTabBackground: "#1E1E1E"
    property color librarySourceTabText: "#AAAAAA"
    property color libraryLocalTabColor: accentColor
    property color libraryLocalTabBackground: "#2A1A0A"
    property color libraryStreamingTabColor: "#FF9500"
    property color libraryStreamingTabText: "#FF9500"
    property color libraryStreamingTabBackground: "#2A1A0A"
    property color libraryKeyMatchColor: "#00E676"
    property color libraryKeyMatchBackground: "#00E67633"
    property color libraryKeyHarmonicColor: "#FF9500"
    property color libraryKeyHarmonicText: "#FFB340"
    property color libraryKeyHarmonicBackground: "#FF950022"
}
