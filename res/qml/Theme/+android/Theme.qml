pragma Singleton
import QtQuick 2.12

QtObject {
    // ---- DJ Sugar theme, OLED-dark (Sugar Night) ----
    // Deep ink blacks, indigo-tinted surfaces, hot pink (#ff4fa8) accent,
    // electric violet (#7c5cff) active state. Deck 1 = cyan-blue, Deck 2 = orange.
    property color accentColor: "#ff4fa8"          // hot pink — brand accent
    property color activeColor: "#7c5cff"          // electric violet — pressed/active
    property color backgroundColor: "#000000"      // true OLED black
    property color blue: "#ff4fa8"                 // used for "blue" roles (brand pink)
    property color bpmSliderBarColor: accentColor
    property color buttonActiveColor: "#ffffff"
    property color buttonDisableColor: "#444444"
    property int buttonFontPixelSize: 10
    property color buttonNormalColor: "#888888"
    property color crossfaderBarColor: accentColor
    property color crossfaderOrientationColor: "#666666"
    property color darkGray: "#000000"
    property color darkGray2: "#0a0b10"            // indigo ink
    property color darkGray3: "#14161c"            // indigo surface
    property color darkGray4: "#0d0e13"            // indigo deep
    property color deckActiveColor: "#ffffff"
    property color deckBackgroundColor: "#000000"
    property color deckBeatjumpBackgroundColor: "#14161c"
    property color deckBeatjumpLabelColor: "#666666"
    property color deckEmptyCoverArt: "#14161c"
    property color deckInfoBarBackgroundColor: "#000000"
    property color deckLineColor: "#0a0b10"
    property color deckLoopBackgroundColor: "#14161c"
    property color deckLoopLabelColor: "#666666"
    property color deckTextColor: "#e0e0e0"
    property color effectColor: "#ff4fa8"
    property color effectUnitColor: "#ff4fa8"
    property color embeddedBackgroundColor: "#E0000000"
    property color eqFxColor: "#ff4fa8"
    property color eqHighColor: "#ffffff"
    property color eqLowColor: "#ffffff"
    property color eqMidColor: "#ffffff"
    property string fontFamily: "Open Sans"
    property color gainKnobColor: accentColor
    property color green: "#00e676"
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
    property color knobBackgroundColor: "#0d0e13"
    property color libraryPanelSplitterBackground: "#000000"
    property color libraryPanelSplitterHandle: "#333333"
    property color libraryPanelSplitterHandleActive: "#ff4fa8"
    property color lightGray: "#888888"
    property color lightGray2: "#bbbbbb"
    property color lightGray3: "#999999"
    property color midGray: "#555555"
    property color midGray2: "#444444"
    property color midGray3: "#333333"
    property color panelSplitterBackground: backgroundColor
    property color panelSplitterHandle: midGray
    property color panelSplitterHandleActive: lightGray2
    property color pflActiveButtonColor: accentColor
    property color red: "#ff6b6b"                  // warm coral for red roles
    property color samplerColor: "#ff4fa8"
    property color sunkenBackgroundColor: "#000000"
    property color textColor: "#e8e8e8"
    property int textFontPixelSize: 14
    property color toolbarActiveColor: "#ffffff"
    property color toolbarBackgroundColor: "#0a0b10"
    property color volumeSliderBarColor: accentColor
    property color warningColor: "#ff6b6b"
    property color waveformBeatColor: "#222222"
    property color waveformCursorColor: "#ffffff"
    property color waveformMarkerDefault: "#ff4fa8"
    property color waveformMarkerIntroOutroColor: "#ff99ca"
    property color waveformMarkerLabel: "#ffffff"
    property color waveformMarkerLoopColor: "#00e676"
    property color waveformMarkerLoopColorDisabled: "#ffffff"
    property color waveformPostrollColor: "#1a1c22"
    property color waveformPrerollColor: "#1a1c22"
    property color white: "#eeeeee"
    property color yellow: "#ffd54f"               // proper amber for yellow roles

    // ---- Android port: library colors (Sugar Night) ----
    property color libraryScrollHandle: "#333333"
    property color libraryScrollHandleHovered: "#7c5cff"
    property color libraryScrollHandlePressed: "#ff4fa8"
    property color libraryScrollTrack: "#000000"
    property color libraryActionBackground: "#0a0b10"
    property color libraryActionDisabledBackground: "#0d0e13"
    property color libraryActionText: "#eeeeee"
    property color libraryActionDisabledText: "#666666"
    property color libraryDeck1Background: "#1f6fd1"        // Deck 1 cyan-blue (functional)
    property color libraryDeck1Pressed: "#4aa3f5"
    property color libraryDeck2Background: "#e8650f"        // Deck 2 orange (functional)
    property color libraryDeck2Pressed: "#ff9a52"
    property color libraryConfirmBackground: accentColor
    property color librarySuccessColor: "#00e676"
    property color librarySourceBarBackground: "#000000"
    property color librarySourceBarBorder: "#0a0b10"
    property color librarySourceTabBackground: "#0d0e13"
    property color librarySourceTabText: "#888888"
    property color libraryLocalTabColor: accentColor
    property color libraryLocalTabBackground: "#1a1c22"
    property color libraryStreamingTabColor: accentColor
    property color libraryStreamingTabText: "#ff99ca"
    property color libraryStreamingTabBackground: "#1a1c22"
    property color libraryKeyMatchColor: "#00e676"
    property color libraryKeyMatchBackground: "#00e67633"
    property color libraryKeyHarmonicColor: "#7c5cff"
    property color libraryKeyHarmonicText: "#d0c8ff"
    property color libraryKeyHarmonicBackground: "#7c5cff22"
}
