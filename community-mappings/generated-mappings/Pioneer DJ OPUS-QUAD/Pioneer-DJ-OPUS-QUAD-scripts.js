/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for OPUS-QUAD was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://cdn.rekordbox.com/files/20230308164348/OPUS-QUAD_List_of_MIDI_Messages_en.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var OpusQuad = {};

OpusQuad.kNumDecks = 4;
OpusQuad.kJogTicksPerRev = 2048;
OpusQuad.kVinylRpm = 33 + 1 / 3;
OpusQuad.kAlpha = 1.0 / 8;
OpusQuad.kBeta = OpusQuad.kAlpha / 32;
OpusQuad.kDisplayCh = 11;
OpusQuad.kLoadLedBase = 4;
OpusQuad.kJogDisplayMode = "7bit_72_cue";
OpusQuad.kHasVuMeter = false;
OpusQuad.kHasSlipLed = false;

OpusQuad.vinylMode = {1: true, 2: true, 3: true, 4: true};
OpusQuad.shiftPressed = {1: false, 2: false, 3: false, 4: false};
OpusQuad.tempoRanges = [0.06, 0.10, 0.16, 1.0];
OpusQuad.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
OpusQuad.activeDeck = 1;
OpusQuad.connections = [];

OpusQuad.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > OpusQuad.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

OpusQuad.syncAllLeds = function() {
    for (var d = 1; d <= OpusQuad.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        OpusQuad.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        OpusQuad.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        OpusQuad.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        OpusQuad.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        OpusQuad.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        OpusQuad.sendDeckLed(d, 0x12, OpusQuad.vinylMode[d]);
        if (OpusQuad.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + OpusQuad.kDisplayCh, OpusQuad.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

OpusQuad.init = function(_id, _debugging) {
    for (var d = 1; d <= OpusQuad.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            OpusQuad.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                OpusQuad.sendDeckLed(deck, 0x00, v > 0);
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                OpusQuad.sendDeckLed(deck, 0x01, v > 0);
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                OpusQuad.sendDeckLed(deck, 0x11, v > 0);
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                OpusQuad.sendDeckLed(deck, 0x1F, v > 0);
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                OpusQuad.sendDeckLed(deck, 0x1E, v > 0);
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (OpusQuad.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + OpusQuad.kDisplayCh, OpusQuad.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            OpusQuad.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (OpusQuad.kHasSlipLed) {
                OpusQuad.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + OpusQuad.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (OpusQuad.kHasVuMeter) {
                OpusQuad.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (OpusQuad.kJogDisplayMode !== "none") {
                OpusQuad.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + OpusQuad.kDisplayCh, deck - 1, 0x00);
                        if (OpusQuad.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + OpusQuad.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (OpusQuad.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (OpusQuad.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + OpusQuad.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + OpusQuad.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + OpusQuad.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    OpusQuad.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        OpusQuad.syncAllLeds();
    }, true);
};

OpusQuad.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

OpusQuad.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

OpusQuad.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", OpusQuad.relativeVal(value));
};

OpusQuad.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", OpusQuad.relativeVal(value) * 5);
};

OpusQuad.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

OpusQuad.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

OpusQuad.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

OpusQuad.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

OpusQuad.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= OpusQuad.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

OpusQuad.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    OpusQuad.shiftPressed[deck] = (value > 0);
};

OpusQuad.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        OpusQuad.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

OpusQuad.cueButton = function(_ch, _ctrl, value, _st, group) {
    OpusQuad.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

OpusQuad.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

OpusQuad.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

OpusQuad.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

OpusQuad.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

OpusQuad.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

OpusQuad.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

OpusQuad.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

OpusQuad.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

OpusQuad.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

OpusQuad.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        OpusQuad.vinylMode[deck] = !OpusQuad.vinylMode[deck];
        OpusQuad.sendDeckLed(deck, 0x12, OpusQuad.vinylMode[deck]);
    }
};

OpusQuad.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    OpusQuad.activeDeck = deck;
    if (OpusQuad.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, OpusQuad.kJogTicksPerRev, OpusQuad.kVinylRpm, OpusQuad.kAlpha, OpusQuad.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

OpusQuad.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = OpusQuad.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

OpusQuad.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OpusQuad.pioneerRelVal(value) * 2.0);
};

OpusQuad.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OpusQuad.pioneerRelVal(value) * 0.25);
};

OpusQuad.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OpusQuad.pioneerRelVal(value) * 1.5);
};

OpusQuad.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

OpusQuad.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

OpusQuad.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

OpusQuad.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        OpusQuad.tempoRangeIdx[d] = (OpusQuad.tempoRangeIdx[d] + 1) % OpusQuad.tempoRanges.length;
        engine.setValue(group, "rateRange", OpusQuad.tempoRanges[OpusQuad.tempoRangeIdx[d]]);
    }
};

OpusQuad.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

OpusQuad.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = OpusQuad.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

OpusQuad.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OpusQuad.activeDeck = (status & 0x0F) + 1;
    }
};

OpusQuad.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OpusQuad.activeDeck = (status & 0x0F) + 1;
    }
};

OpusQuad.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OpusQuad.activeDeck = (status & 0x0F) + 1;
        OpusQuad.syncAllLeds();
    }
};

OpusQuad.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        OpusQuad.syncAllLeds();
    }
};

OpusQuad.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

OpusQuad.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

OpusQuad.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= OpusQuad.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

OpusQuad.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

OpusQuad.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + OpusQuad.activeDeck + "]", "bpm_tap", 1);
    }
};

OpusQuad.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = OpusQuad.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

OpusQuad.shutdown = function() {
    for (var i = 0; i < OpusQuad.connections.length; i++) {
        OpusQuad.connections[i].disconnect();
    }
    OpusQuad.connections = [];
    for (var d = 1; d <= OpusQuad.kNumDecks; d++) {
        OpusQuad.sendDeckLed(d, 0x00, false);
        OpusQuad.sendDeckLed(d, 0x01, false);
        if (OpusQuad.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + OpusQuad.kDisplayCh, OpusQuad.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
