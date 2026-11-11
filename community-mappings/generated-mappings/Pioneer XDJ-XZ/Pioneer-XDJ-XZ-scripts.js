/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for Pioneer XDJ-XZ was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-xz/xdj-xz_midi_message_list_e.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var XdjXz = {};

XdjXz.kNumDecks = 4;
XdjXz.kJogTicksPerRev = 2048;
XdjXz.kVinylRpm = 33 + 1 / 3;
XdjXz.kAlpha = 1.0 / 8;
XdjXz.kBeta = XdjXz.kAlpha / 32;
XdjXz.kDisplayCh = 11;
XdjXz.kLoadLedBase = 0;
XdjXz.kJogDisplayMode = "14bit_360";
XdjXz.kHasVuMeter = true;
XdjXz.kHasSlipLed = false;

XdjXz.vinylMode = {1: true, 2: true, 3: true, 4: true};
XdjXz.shiftPressed = {1: false, 2: false, 3: false, 4: false};
XdjXz.tempoRanges = [0.06, 0.10, 0.16, 1.0];
XdjXz.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
XdjXz.activeDeck = 1;
XdjXz.connections = [];

XdjXz.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > XdjXz.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

XdjXz.syncAllLeds = function() {
    for (var d = 1; d <= XdjXz.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        XdjXz.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        XdjXz.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        XdjXz.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        XdjXz.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        XdjXz.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        XdjXz.sendDeckLed(d, 0x12, XdjXz.vinylMode[d]);
        if (XdjXz.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjXz.kDisplayCh, XdjXz.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

XdjXz.init = function(_id, _debugging) {
    for (var d = 1; d <= XdjXz.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            XdjXz.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                XdjXz.sendDeckLed(deck, 0x00, v > 0);
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                XdjXz.sendDeckLed(deck, 0x01, v > 0);
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                XdjXz.sendDeckLed(deck, 0x11, v > 0);
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                XdjXz.sendDeckLed(deck, 0x1F, v > 0);
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                XdjXz.sendDeckLed(deck, 0x1E, v > 0);
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (XdjXz.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + XdjXz.kDisplayCh, XdjXz.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            XdjXz.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (XdjXz.kHasSlipLed) {
                XdjXz.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + XdjXz.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (XdjXz.kHasVuMeter) {
                XdjXz.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (XdjXz.kJogDisplayMode !== "none") {
                XdjXz.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + XdjXz.kDisplayCh, deck - 1, 0x00);
                        if (XdjXz.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + XdjXz.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (XdjXz.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (XdjXz.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + XdjXz.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + XdjXz.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + XdjXz.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    XdjXz.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        XdjXz.syncAllLeds();
    }, true);
};

XdjXz.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

XdjXz.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

XdjXz.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjXz.relativeVal(value));
};

XdjXz.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjXz.relativeVal(value) * 5);
};

XdjXz.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

XdjXz.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

XdjXz.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

XdjXz.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

XdjXz.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= XdjXz.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

XdjXz.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    XdjXz.shiftPressed[deck] = (value > 0);
};

XdjXz.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        XdjXz.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

XdjXz.cueButton = function(_ch, _ctrl, value, _st, group) {
    XdjXz.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

XdjXz.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

XdjXz.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

XdjXz.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

XdjXz.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

XdjXz.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

XdjXz.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

XdjXz.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

XdjXz.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

XdjXz.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

XdjXz.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        XdjXz.vinylMode[deck] = !XdjXz.vinylMode[deck];
        XdjXz.sendDeckLed(deck, 0x12, XdjXz.vinylMode[deck]);
    }
};

XdjXz.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    XdjXz.activeDeck = deck;
    if (XdjXz.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, XdjXz.kJogTicksPerRev, XdjXz.kVinylRpm, XdjXz.kAlpha, XdjXz.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

XdjXz.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = XdjXz.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

XdjXz.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjXz.pioneerRelVal(value) * 2.0);
};

XdjXz.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjXz.pioneerRelVal(value) * 0.25);
};

XdjXz.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjXz.pioneerRelVal(value) * 1.5);
};

XdjXz.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

XdjXz.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

XdjXz.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

XdjXz.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        XdjXz.tempoRangeIdx[d] = (XdjXz.tempoRangeIdx[d] + 1) % XdjXz.tempoRanges.length;
        engine.setValue(group, "rateRange", XdjXz.tempoRanges[XdjXz.tempoRangeIdx[d]]);
    }
};

XdjXz.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

XdjXz.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = XdjXz.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

XdjXz.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjXz.activeDeck = (status & 0x0F) + 1;
    }
};

XdjXz.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjXz.activeDeck = (status & 0x0F) + 1;
    }
};

XdjXz.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjXz.activeDeck = (status & 0x0F) + 1;
        XdjXz.syncAllLeds();
    }
};

XdjXz.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        XdjXz.syncAllLeds();
    }
};

XdjXz.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

XdjXz.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

XdjXz.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= XdjXz.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

XdjXz.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

XdjXz.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + XdjXz.activeDeck + "]", "bpm_tap", 1);
    }
};

XdjXz.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = XdjXz.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

XdjXz.shutdown = function() {
    for (var i = 0; i < XdjXz.connections.length; i++) {
        XdjXz.connections[i].disconnect();
    }
    XdjXz.connections = [];
    for (var d = 1; d <= XdjXz.kNumDecks; d++) {
        XdjXz.sendDeckLed(d, 0x00, false);
        XdjXz.sendDeckLed(d, 0x01, false);
        if (XdjXz.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjXz.kDisplayCh, XdjXz.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
