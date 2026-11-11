/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for AlphaTheta XDJ-AZ was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://cdn.rekordbox.com/files/20241016164551/XDJ-AZ_MIDI_message_list_E.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var XdjAz = {};

XdjAz.kNumDecks = 4;
XdjAz.kJogTicksPerRev = 2048;
XdjAz.kVinylRpm = 33 + 1 / 3;
XdjAz.kAlpha = 1.0 / 8;
XdjAz.kBeta = XdjAz.kAlpha / 32;
XdjAz.kDisplayCh = 13;
XdjAz.kLoadLedBase = 0;
XdjAz.kJogDisplayMode = "14bit_360";
XdjAz.kHasVuMeter = true;
XdjAz.kHasSlipLed = false;

XdjAz.vinylMode = {1: true, 2: true, 3: true, 4: true};
XdjAz.shiftPressed = {1: false, 2: false, 3: false, 4: false};
XdjAz.tempoRanges = [0.06, 0.10, 0.16, 1.0];
XdjAz.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
XdjAz.activeDeck = 1;
XdjAz.connections = [];

XdjAz.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > XdjAz.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

XdjAz.syncAllLeds = function() {
    for (var d = 1; d <= XdjAz.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        XdjAz.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        XdjAz.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        XdjAz.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        XdjAz.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        XdjAz.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        XdjAz.sendDeckLed(d, 0x12, XdjAz.vinylMode[d]);
        if (XdjAz.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjAz.kDisplayCh, XdjAz.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

XdjAz.init = function(_id, _debugging) {
    for (var d = 1; d <= XdjAz.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            XdjAz.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                XdjAz.sendDeckLed(deck, 0x00, v > 0);
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                XdjAz.sendDeckLed(deck, 0x01, v > 0);
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                XdjAz.sendDeckLed(deck, 0x11, v > 0);
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                XdjAz.sendDeckLed(deck, 0x1F, v > 0);
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                XdjAz.sendDeckLed(deck, 0x1E, v > 0);
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (XdjAz.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + XdjAz.kDisplayCh, XdjAz.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            XdjAz.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (XdjAz.kHasSlipLed) {
                XdjAz.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + XdjAz.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (XdjAz.kHasVuMeter) {
                XdjAz.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (XdjAz.kJogDisplayMode !== "none") {
                XdjAz.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + XdjAz.kDisplayCh, deck - 1, 0x00);
                        if (XdjAz.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + XdjAz.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (XdjAz.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (XdjAz.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + XdjAz.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + XdjAz.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + XdjAz.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    XdjAz.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        XdjAz.syncAllLeds();
    }, true);
};

XdjAz.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

XdjAz.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

XdjAz.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjAz.relativeVal(value));
};

XdjAz.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjAz.relativeVal(value) * 5);
};

XdjAz.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

XdjAz.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

XdjAz.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

XdjAz.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

XdjAz.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= XdjAz.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

XdjAz.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    XdjAz.shiftPressed[deck] = (value > 0);
};

XdjAz.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        XdjAz.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

XdjAz.cueButton = function(_ch, _ctrl, value, _st, group) {
    XdjAz.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

XdjAz.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

XdjAz.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

XdjAz.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

XdjAz.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

XdjAz.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

XdjAz.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

XdjAz.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

XdjAz.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

XdjAz.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

XdjAz.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        XdjAz.vinylMode[deck] = !XdjAz.vinylMode[deck];
        XdjAz.sendDeckLed(deck, 0x12, XdjAz.vinylMode[deck]);
    }
};

XdjAz.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    XdjAz.activeDeck = deck;
    if (XdjAz.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, XdjAz.kJogTicksPerRev, XdjAz.kVinylRpm, XdjAz.kAlpha, XdjAz.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

XdjAz.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = XdjAz.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

XdjAz.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjAz.pioneerRelVal(value) * 2.0);
};

XdjAz.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjAz.pioneerRelVal(value) * 0.25);
};

XdjAz.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjAz.pioneerRelVal(value) * 1.5);
};

XdjAz.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

XdjAz.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

XdjAz.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

XdjAz.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        XdjAz.tempoRangeIdx[d] = (XdjAz.tempoRangeIdx[d] + 1) % XdjAz.tempoRanges.length;
        engine.setValue(group, "rateRange", XdjAz.tempoRanges[XdjAz.tempoRangeIdx[d]]);
    }
};

XdjAz.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

XdjAz.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = XdjAz.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

XdjAz.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjAz.activeDeck = (status & 0x0F) + 1;
    }
};

XdjAz.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjAz.activeDeck = (status & 0x0F) + 1;
    }
};

XdjAz.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjAz.activeDeck = (status & 0x0F) + 1;
        XdjAz.syncAllLeds();
    }
};

XdjAz.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        XdjAz.syncAllLeds();
    }
};

XdjAz.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

XdjAz.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

XdjAz.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= XdjAz.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

XdjAz.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

XdjAz.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + XdjAz.activeDeck + "]", "bpm_tap", 1);
    }
};

XdjAz.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = XdjAz.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

XdjAz.shutdown = function() {
    for (var i = 0; i < XdjAz.connections.length; i++) {
        XdjAz.connections[i].disconnect();
    }
    XdjAz.connections = [];
    for (var d = 1; d <= XdjAz.kNumDecks; d++) {
        XdjAz.sendDeckLed(d, 0x00, false);
        XdjAz.sendDeckLed(d, 0x01, false);
        if (XdjAz.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjAz.kDisplayCh, XdjAz.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
