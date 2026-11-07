pragma Singleton
import QtQuick 2.12

QtObject {
    // ---- DJ Sugar theme, OLED-dark ----
    // Original DJ Sugar accent (#623857 / #D85486) kept, backgrounds pushed to true black
    property color accentColor: "#623857"          // DJ Sugar purple
    property color backgroundColor: "#000000"      // True OLED black
    property color blue: "#D85486"                 // DJ Sugar pink (used for blue roles)
    property color bpmSliderBarColor: accentColor
    property color buttonActiveColor: "#FFFFFF"
    property color buttonDisableColor: "#444444"
    property int buttonFontPixelSize: 10
    property color buttonNormalColor: "#666666"
    property color crossfaderBarColor: accentColor
    property color crossfaderOrientationColor: "#555555"
    property color darkGray: "#000000"
    property color darkGray2: "#111111"
    property color darkGray3: "#1A1A1A"
    property color darkGray4: "#0D0D0D"
    property color deckActiveColor: "#FFFFFF"
    property color deckBackgroundColor: "#000000"
    property color deckBeatjumpBackgroundColor: "#1A1A1A"
    property color deckBeatjumpLabelColor: "#444444"
    property color deckEmptyCoverArt: "#1A1A1A"
    property color deckInfoBarBackgroundColor: '#000000'
    property color deckLineColor: "#111111"
    property color deckLoopBackgroundColor: "#1A1A1A"
    property color deckLoopLabelColor: "#444444"
    property color deckTextColor: "#BBBBBB"
    property color effectColor: "#D85486"
    property color effectUnitColor: "#D85486"
    property color embeddedBackgroundColor: "#E0000000"
    property color eqFxColor: "#D85486"
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
    property color knobBackgroundColor: "#0D0D0D"
    property color libraryPanelSplitterBackground: "#000000"
    property color libraryPanelSplitterHandle: "#333333"
    property color libraryPanelSplitterHandleActive: "#555555"
    property color lightGray: "#666666"
    property color lightGray2: "#999999"
    property color lightGray3: "#777777"
    property color midGray: "#555555"
    property color midGray2: "#444444"
    property color midGray3: "#333333"
    property color panelSplitterBackground: backgroundColor
    property color panelSplitterHandle: midGray
    property color panelSplitterHandleActive: lightGray2
    property color pflActiveButtonColor: accentColor
    property color red: "#D85486"
    property color samplerColor: "#D85486"
    property color sunkenBackgroundColor: "#000000"
    property color textColor: "#BBBBBB"
    property int textFontPixelSize: 14
    property color toolbarActiveColor: "#FFFFFF"
    property color toolbarBackgroundColor: "#111111"
    property color volumeSliderBarColor: accentColor
    property color warningColor: "#CC3300"
    property color waveformBeatColor: "#333333"
    property color waveformCursorColor: "#FFFFFF"
    property color waveformMarkerDefault: '#D85486'
    property color waveformMarkerIntroOutroColor: '#E878A6'
    property color waveformMarkerLabel: Qt.rgba(255, 255, 255, 0.9)
    property color waveformMarkerLoopColor: '#00E676'
    property color waveformMarkerLoopColorDisabled: '#FFFFFF'
    property color waveformPostrollColor: "#222222"
    property color waveformPrerollColor: "#222222"
    property color white: "#DDDDDD"
    property color yellow: "#E878A6"

    // ---- Android port: library colors (DJ Sugar, OLED) ----
    property color libraryScrollHandle: "#333333"
    property color libraryScrollHandleHovered: "#555555"
    property color libraryScrollHandlePressed: accentColor
    property color libraryScrollTrack: "#000000"
    property color libraryActionBackground: "#111111"
    property color libraryActionDisabledBackground: "#0D0D0D"
    property color libraryActionText: "#FFFFFF"
    property color libraryActionDisabledText: "#555555"
    property color libraryDeck1Background: accentColor
    property color libraryDeck1Pressed: "#4A2D42"
    property color libraryDeck2Background: "#4A2D42"
    property color libraryDeck2Pressed: "#D85486"
    property color libraryConfirmBackground: accentColor
    property color librarySuccessColor: "#00E676"
    property color librarySourceBarBackground: "#000000"
    property color librarySourceBarBorder: "#111111"
    property color librarySourceTabBackground: "#0D0D0D"
    property color librarySourceTabText: "#888888"
    property color libraryLocalTabColor: accentColor
    property color libraryLocalTabBackground: "#1A0D15"
    property color libraryStreamingTabColor: "#D85486"
    property color libraryStreamingTabText: "#E878A6"
    property color libraryStreamingTabBackground: "#1A0D15"
    property color libraryKeyMatchColor: "#00E676"
    property color libraryKeyMatchBackground: "#00E67633"
    property color libraryKeyHarmonicColor: "#D85486"
    property color libraryKeyHarmonicText: "#E878A6"
    property color libraryKeyHarmonicBackground: "#D8548622"
}
