/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for Pioneer XDJ-RR was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-rr/xdj-rr_list_of_midi_message_e.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var XdjRr = {};

XdjRr.kNumDecks = 2;
XdjRr.kJogTicksPerRev = 1500;
XdjRr.kVinylRpm = 33 + 1 / 3;
XdjRr.kAlpha = 1.0 / 8;
XdjRr.kBeta = XdjRr.kAlpha / 32;
XdjRr.kDisplayCh = 11;
XdjRr.kLoadLedBase = 0;
XdjRr.kJogDisplayMode = "none";
XdjRr.kHasVuMeter = true;
XdjRr.kHasSlipLed = false;

XdjRr.vinylMode = {1: true, 2: true, 3: true, 4: true};
XdjRr.shiftPressed = {1: false, 2: false, 3: false, 4: false};
XdjRr.tempoRanges = [0.06, 0.10, 0.16, 1.0];
XdjRr.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
XdjRr.activeDeck = 1;
XdjRr.connections = [];

XdjRr.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > XdjRr.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

XdjRr.syncAllLeds = function() {
    for (var d = 1; d <= XdjRr.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        XdjRr.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        XdjRr.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        XdjRr.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        XdjRr.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        XdjRr.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        XdjRr.sendDeckLed(d, 0x12, XdjRr.vinylMode[d]);
        if (XdjRr.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjRr.kDisplayCh, XdjRr.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

XdjRr.init = function(_id, _debugging) {
    for (var d = 1; d <= XdjRr.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            XdjRr.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                XdjRr.sendDeckLed(deck, 0x00, v > 0);
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                XdjRr.sendDeckLed(deck, 0x01, v > 0);
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                XdjRr.sendDeckLed(deck, 0x11, v > 0);
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                XdjRr.sendDeckLed(deck, 0x1F, v > 0);
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                XdjRr.sendDeckLed(deck, 0x1E, v > 0);
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (XdjRr.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + XdjRr.kDisplayCh, XdjRr.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            XdjRr.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (XdjRr.kHasSlipLed) {
                XdjRr.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + XdjRr.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (XdjRr.kHasVuMeter) {
                XdjRr.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (XdjRr.kJogDisplayMode !== "none") {
                XdjRr.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + XdjRr.kDisplayCh, deck - 1, 0x00);
                        if (XdjRr.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + XdjRr.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (XdjRr.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (XdjRr.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + XdjRr.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + XdjRr.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + XdjRr.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    XdjRr.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        XdjRr.syncAllLeds();
    }, true);
};

XdjRr.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

XdjRr.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

XdjRr.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjRr.relativeVal(value));
};

XdjRr.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", XdjRr.relativeVal(value) * 5);
};

XdjRr.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

XdjRr.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

XdjRr.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

XdjRr.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

XdjRr.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= XdjRr.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

XdjRr.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    XdjRr.shiftPressed[deck] = (value > 0);
};

XdjRr.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        XdjRr.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

XdjRr.cueButton = function(_ch, _ctrl, value, _st, group) {
    XdjRr.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

XdjRr.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

XdjRr.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

XdjRr.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

XdjRr.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

XdjRr.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

XdjRr.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

XdjRr.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

XdjRr.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

XdjRr.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

XdjRr.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        XdjRr.vinylMode[deck] = !XdjRr.vinylMode[deck];
        XdjRr.sendDeckLed(deck, 0x12, XdjRr.vinylMode[deck]);
    }
};

XdjRr.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    XdjRr.activeDeck = deck;
    if (XdjRr.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, XdjRr.kJogTicksPerRev, XdjRr.kVinylRpm, XdjRr.kAlpha, XdjRr.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

XdjRr.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = XdjRr.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

XdjRr.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRr.pioneerRelVal(value) * 2.0);
};

XdjRr.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRr.pioneerRelVal(value) * 0.25);
};

XdjRr.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", XdjRr.pioneerRelVal(value) * 1.5);
};

XdjRr.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

XdjRr.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

XdjRr.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

XdjRr.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        XdjRr.tempoRangeIdx[d] = (XdjRr.tempoRangeIdx[d] + 1) % XdjRr.tempoRanges.length;
        engine.setValue(group, "rateRange", XdjRr.tempoRanges[XdjRr.tempoRangeIdx[d]]);
    }
};

XdjRr.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

XdjRr.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = XdjRr.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

XdjRr.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRr.activeDeck = (status & 0x0F) + 1;
    }
};

XdjRr.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRr.activeDeck = (status & 0x0F) + 1;
    }
};

XdjRr.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        XdjRr.activeDeck = (status & 0x0F) + 1;
        XdjRr.syncAllLeds();
    }
};

XdjRr.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        XdjRr.syncAllLeds();
    }
};

XdjRr.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

XdjRr.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

XdjRr.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= XdjRr.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

XdjRr.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

XdjRr.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + XdjRr.activeDeck + "]", "bpm_tap", 1);
    }
};

XdjRr.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = XdjRr.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

XdjRr.shutdown = function() {
    for (var i = 0; i < XdjRr.connections.length; i++) {
        XdjRr.connections[i].disconnect();
    }
    XdjRr.connections = [];
    for (var d = 1; d <= XdjRr.kNumDecks; d++) {
        XdjRr.sendDeckLed(d, 0x00, false);
        XdjRr.sendDeckLed(d, 0x01, false);
        if (XdjRr.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + XdjRr.kDisplayCh, XdjRr.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
