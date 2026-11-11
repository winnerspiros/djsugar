/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for Pioneer XDJ-RX2 was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-rx2/xdj-rx2_list_of_midi_message_e3.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var XdjRx2 = {};

XdjRx2.kNumDecks = 2;
XdjRx2.kJogTicksPerRev = 2048;
XdjRx2.kVinylRpm = 33 + 1 / 3;
XdjRx2.kAlpha = 1.0 / 8;
XdjRx2.kBeta = XdjRx2.kAlpha / 32;
XdjRx2.kDisplayCh = 11;
XdjRx2.kLoadLedBase = 0;
XdjRx2.kJogDisplayMode = "7bit_72_pos_only";
XdjRx2.kHasVuMeter = true;
XdjRx2.kHasSlipLed = true;

XdjRx2.vinylMode = {1: true, 2: true, 3: true, 4: true};
XdjRx2.shiftPressed = {1: false, 2: false, 3: false, 4: false};
XdjRx2.tempoRanges = [0.06, 0.10, 0.16, 1.0];
XdjRx2.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
XdjRx2.activeDeck = 1;
XdjRx2.connections = [];

XdjRx2.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > XdjRx2.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

XdjRx2.syncAllLeds = function() {
    for (var d = 1; d <= XdjRx2.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        XdjRx2.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        XdjRx2.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        XdjRx2.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        XdjRx2.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        XdjRx2.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        XdjRx2.sendDeckLed(d, 0x12, XdjRx2.vinylMode[d]);
        if (XdjRx2.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjRx2.kDisplayCh, XdjRx2.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

XdjRx2.init = function(_id, _debugging) {
    for (var d = 1; d <= XdjRx2.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            XdjRx2.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                XdjRx2.sendDeckLed(deck, 0x00, v > 0);
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                XdjRx2.sendDeckLed(deck, 0x01, v > 0);
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                XdjRx2.sendDeckLed(deck, 0x11, v > 0);
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                XdjRx2.sendDeckLed(deck, 0x1F, v > 0);
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                XdjRx2.sendDeckLed(deck, 0x1E, v > 0);
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (XdjRx2.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + XdjRx2.kDisplayCh, XdjRx2.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            XdjRx2.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (XdjRx2.kHasSlipLed) {
                XdjRx2.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + XdjRx2.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (XdjRx2.kHasVuMeter) {
                XdjRx2.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (XdjRx2.kJogDisplayMode !== "none") {
                XdjRx2.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + XdjRx2.kDisplayCh, deck - 1, 0x00);
                        if (XdjRx2.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + XdjRx2.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (XdjRx2.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (XdjRx2.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + XdjRx2.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + XdjRx2.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + XdjRx2.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    XdjRx2.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        XdjRx2.syncAllLeds();
    }, true);
};

XdjRx2.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

XdjRx2.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

XdjRx2.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjRx2.relativeVal(value));
};

XdjRx2.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjRx2.relativeVal(value) * 5);
};

XdjRx2.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

XdjRx2.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

XdjRx2.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

XdjRx2.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

XdjRx2.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= XdjRx2.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

XdjRx2.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    XdjRx2.shiftPressed[deck] = (value > 0);
};

XdjRx2.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        XdjRx2.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

XdjRx2.cueButton = function(_ch, _ctrl, value, _st, group) {
    XdjRx2.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

XdjRx2.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

XdjRx2.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

XdjRx2.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

XdjRx2.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

XdjRx2.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

XdjRx2.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

XdjRx2.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

XdjRx2.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

XdjRx2.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

XdjRx2.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        XdjRx2.vinylMode[deck] = !XdjRx2.vinylMode[deck];
        XdjRx2.sendDeckLed(deck, 0x12, XdjRx2.vinylMode[deck]);
    }
};

XdjRx2.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    XdjRx2.activeDeck = deck;
    if (XdjRx2.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, XdjRx2.kJogTicksPerRev, XdjRx2.kVinylRpm, XdjRx2.kAlpha, XdjRx2.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

XdjRx2.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = XdjRx2.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

XdjRx2.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRx2.pioneerRelVal(value) * 2.0);
};

XdjRx2.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRx2.pioneerRelVal(value) * 0.25);
};

XdjRx2.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRx2.pioneerRelVal(value) * 1.5);
};

XdjRx2.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

XdjRx2.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

XdjRx2.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

XdjRx2.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        XdjRx2.tempoRangeIdx[d] = (XdjRx2.tempoRangeIdx[d] + 1) % XdjRx2.tempoRanges.length;
        engine.setValue(group, "rateRange", XdjRx2.tempoRanges[XdjRx2.tempoRangeIdx[d]]);
    }
};

XdjRx2.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

XdjRx2.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = XdjRx2.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

XdjRx2.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRx2.activeDeck = (status & 0x0F) + 1;
    }
};

XdjRx2.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRx2.activeDeck = (status & 0x0F) + 1;
    }
};

XdjRx2.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRx2.activeDeck = (status & 0x0F) + 1;
        XdjRx2.syncAllLeds();
    }
};

XdjRx2.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        XdjRx2.syncAllLeds();
    }
};

XdjRx2.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

XdjRx2.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

XdjRx2.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= XdjRx2.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

XdjRx2.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

XdjRx2.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + XdjRx2.activeDeck + "]", "bpm_tap", 1);
    }
};

XdjRx2.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = XdjRx2.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

XdjRx2.shutdown = function() {
    for (var i = 0; i < XdjRx2.connections.length; i++) {
        XdjRx2.connections[i].disconnect();
    }
    XdjRx2.connections = [];
    for (var d = 1; d <= XdjRx2.kNumDecks; d++) {
        XdjRx2.sendDeckLed(d, 0x00, false);
        XdjRx2.sendDeckLed(d, 0x01, false);
        if (XdjRx2.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjRx2.kDisplayCh, XdjRx2.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
