/**
 * [AI-GENERATED DISCLAIMER START]
 * This Mixxx 2.5+ script for OMNIS-DUO was generated autonomously by an AI agent
 * from the official manufacturer MIDI Message List PDF:
 * https://cdn.rekordbox.com/files/20240119175526/OMNIS-DUO_List_of_MIDI_messages_E.pdf
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * License: GPL-2.0-or-later
 */

var OmnisDuo = {};

OmnisDuo.kNumDecks = 2;
OmnisDuo.kJogTicksPerRev = 1500;
OmnisDuo.kVinylRpm = 33 + 1 / 3;
OmnisDuo.kAlpha = 1.0 / 8;
OmnisDuo.kBeta = OmnisDuo.kAlpha / 32;
OmnisDuo.kDisplayCh = 11;
OmnisDuo.kLoadLedBase = null;
OmnisDuo.kJogDisplayMode = "none";
OmnisDuo.kHasVuMeter = true;
OmnisDuo.kHasSlipLed = false;

OmnisDuo.vinylMode = {1: true, 2: true, 3: true, 4: true};
OmnisDuo.shiftPressed = {1: false, 2: false, 3: false, 4: false};
OmnisDuo.tempoRanges = [0.06, 0.10, 0.16, 1.0];
OmnisDuo.tempoRangeIdx = {1: 1, 2: 1, 3: 1, 4: 1};
OmnisDuo.activeDeck = 1;
OmnisDuo.connections = [];

OmnisDuo.sendDeckLed = function(deck, note, isOn) {
    if (deck < 1 || deck > OmnisDuo.kNumDecks) {
        return;
    }
    midi.sendShortMsg(0x90 + (deck - 1), note, isOn ? 0x7F : 0x00);
};

OmnisDuo.syncAllLeds = function() {
    for (var d = 1; d <= OmnisDuo.kNumDecks; d++) {
        var ch = "[Channel" + d + "]";
        OmnisDuo.sendDeckLed(d, 0x00, engine.getValue(ch, "play_indicator") > 0);
        OmnisDuo.sendDeckLed(d, 0x01, engine.getValue(ch, "cue_indicator") > 0);
        OmnisDuo.sendDeckLed(d, 0x11, engine.getValue(ch, "keylock") > 0);
        OmnisDuo.sendDeckLed(d, 0x1F, engine.getValue(ch, "sync_enabled") > 0);
        OmnisDuo.sendDeckLed(d, 0x1E, engine.getValue(ch, "sync_leader") > 0);
        OmnisDuo.sendDeckLed(d, 0x12, OmnisDuo.vinylMode[d]);
        if (OmnisDuo.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + OmnisDuo.kDisplayCh, OmnisDuo.kLoadLedBase + (d - 1), engine.getValue(ch, "track_loaded") > 0 ? 0x7F : 0x00);
        }
        midi.sendShortMsg(0xB4, 0x45 + d, engine.getValue(ch, "pfl") > 0 ? 0x7F : 0x00);
    }
    midi.sendShortMsg(0xB4, 0x72, engine.getValue("[EffectRack1_EffectUnit1]", "enabled") > 0 ? 0x7F : 0x00);
};

OmnisDuo.init = function(_id, _debugging) {
    for (var d = 1; d <= OmnisDuo.kNumDecks; d++) {
        (function(deck) {
            var ch = "[Channel" + deck + "]";
            engine.softTakeover(ch, "rate", true);
            engine.softTakeover(ch, "volume", true);
            engine.softTakeover(ch, "pregain", true);
            OmnisDuo.connections.push(engine.makeConnection(ch, "play_indicator", function(v) {
                OmnisDuo.sendDeckLed(deck, 0x00, v > 0);
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "cue_indicator", function(v) {
                OmnisDuo.sendDeckLed(deck, 0x01, v > 0);
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "keylock", function(v) {
                OmnisDuo.sendDeckLed(deck, 0x11, v > 0);
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "sync_enabled", function(v) {
                OmnisDuo.sendDeckLed(deck, 0x1F, v > 0);
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "sync_leader", function(v) {
                OmnisDuo.sendDeckLed(deck, 0x1E, v > 0);
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "track_loaded", function(v) {
                if (OmnisDuo.kLoadLedBase !== null) {
                    midi.sendShortMsg(0x90 + OmnisDuo.kDisplayCh, OmnisDuo.kLoadLedBase + (deck - 1), v > 0 ? 0x7F : 0x00);
                }
            }));
            OmnisDuo.connections.push(engine.makeConnection(ch, "pfl", function(v) {
                midi.sendShortMsg(0xB4, 0x45 + deck, v > 0 ? 0x7F : 0x00);
            }));
            if (OmnisDuo.kHasSlipLed) {
                OmnisDuo.connections.push(engine.makeConnection(ch, "slip_enabled", function(v) {
                    midi.sendShortMsg(0x90 + OmnisDuo.kDisplayCh, 0x24 + (deck - 1), v > 0 ? 0x7F : 0x00);
                }));
            }
            if (OmnisDuo.kHasVuMeter) {
                OmnisDuo.connections.push(engine.makeConnection(ch, "vu_meter", function(v) {
                    midi.sendShortMsg(0xB0 + (deck - 1), 0x02, Math.round(Math.max(0, Math.min(1, v)) * 127));
                }));
            }
            if (OmnisDuo.kJogDisplayMode !== "none") {
                OmnisDuo.connections.push(engine.makeConnection(ch, "playposition", function(v) {
                    var dur = engine.getValue(ch, "duration");
                    if (dur <= 0) {
                        midi.sendShortMsg(0xB0 + OmnisDuo.kDisplayCh, deck - 1, 0x00);
                        if (OmnisDuo.kJogDisplayMode === "14bit_360") {
                            midi.sendShortMsg(0xB0 + OmnisDuo.kDisplayCh, 0x20 + (deck - 1), 0x00);
                        }
                        return;
                    }
                    var rev = (v * dur) * (OmnisDuo.kVinylRpm / 60.0);
                    var frac = rev - Math.floor(rev);
                    if (OmnisDuo.kJogDisplayMode === "14bit_360") {
                        var deg = Math.round(frac * 359) % 360;
                        midi.sendShortMsg(0xB0 + OmnisDuo.kDisplayCh, deck - 1, (deg >> 7) & 0x7F);
                        midi.sendShortMsg(0xB0 + OmnisDuo.kDisplayCh, 0x20 + (deck - 1), deg & 0x7F);
                    } else {
                        var step = 1 + (Math.round(frac * 71) % 72);
                        midi.sendShortMsg(0xB0 + OmnisDuo.kDisplayCh, deck - 1, step);
                    }
                }));
            }
        })(d);
    }

    OmnisDuo.connections.push(engine.makeConnection("[EffectRack1_EffectUnit1]", "enabled", function(v) {
        midi.sendShortMsg(0xB4, 0x72, v > 0 ? 0x7F : 0x00);
    }));

    // Defer initial hardware LED sync until Mixxx finishes opening the MIDI output port.
    engine.beginTimer(20, function() {
        OmnisDuo.syncAllLeds();
    }, true);
};

OmnisDuo.relativeVal = function(value) {
    return (value < 0x40) ? value : (value - 0x80);
};

OmnisDuo.pioneerRelVal = function(value) {
    // Pioneer jog encoders center around 0x40: CW 0x41..0x7F (+1..+63), CCW 0x3F..0x00 (-1..-64)
    return value - 0x40;
};

OmnisDuo.browserScroll = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", OmnisDuo.relativeVal(value));
};

OmnisDuo.browserScrollFast = function(_ch, _ctrl, value, _st, _g) {
    engine.setValue("[Library]", "MoveVertical", OmnisDuo.relativeVal(value) * 5);
};

OmnisDuo.browserPress = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "GoToItem", 1);
    }
};

OmnisDuo.browserBack = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveFocusBackward", 1);
    }
};

OmnisDuo.browserTiltUp = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", -1);
    }
};

OmnisDuo.browserTiltDown = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Library]", "MoveVertical", 1);
    }
};

OmnisDuo.globalQuantize = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        var q = engine.getValue("[Channel1]", "quantize") ? 0 : 1;
        for (var d = 1; d <= OmnisDuo.kNumDecks; d++) {
            engine.setValue("[Channel" + d + "]", "quantize", q);
        }
    }
};

OmnisDuo.shiftButton = function(_ch, _ctrl, value, status, _g) {
    var deck = (status & 0x0F) + 1;
    OmnisDuo.shiftPressed[deck] = (value > 0);
};

OmnisDuo.playButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        OmnisDuo.activeDeck = script.deckFromGroup(group);
        script.toggleControl(group, "play");
    }
};

OmnisDuo.cueButton = function(_ch, _ctrl, value, _st, group) {
    OmnisDuo.activeDeck = script.deckFromGroup(group);
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

OmnisDuo.syncButton = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

OmnisDuo.syncLong = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_leader");
    }
};

OmnisDuo.searchFwd = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "fwd", value > 0 ? 1 : 0);
};

OmnisDuo.searchRev = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "back", value > 0 ? 1 : 0);
};

OmnisDuo.trackSearchFwd = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "end", 1);
    }
};

OmnisDuo.trackSearchRev = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "start", 1);
    }
};

OmnisDuo.loopIn = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_in", value > 0 ? 1 : 0);
};

OmnisDuo.loopOut = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "loop_out", value > 0 ? 1 : 0);
};

OmnisDuo.reloopExit = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "reloop_toggle", 1);
    }
};

OmnisDuo.jogModeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var deck = script.deckFromGroup(group);
        OmnisDuo.vinylMode[deck] = !OmnisDuo.vinylMode[deck];
        OmnisDuo.sendDeckLed(deck, 0x12, OmnisDuo.vinylMode[deck]);
    }
};

OmnisDuo.jogTouch = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    OmnisDuo.activeDeck = deck;
    if (OmnisDuo.vinylMode[deck] && value > 0) {
        engine.scratchEnable(deck, OmnisDuo.kJogTicksPerRev, OmnisDuo.kVinylRpm, OmnisDuo.kAlpha, OmnisDuo.kBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

OmnisDuo.jogTop = function(_ch, _ctrl, value, _st, group) {
    var deck = script.deckFromGroup(group);
    var delta = OmnisDuo.pioneerRelVal(value);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * 0.25);
    }
};

OmnisDuo.jogTopShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OmnisDuo.pioneerRelVal(value) * 2.0);
};

OmnisDuo.jogSide = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OmnisDuo.pioneerRelVal(value) * 0.25);
};

OmnisDuo.jogSideShift = function(_ch, _ctrl, value, _st, group) {
    engine.setValue(group, "jog", OmnisDuo.pioneerRelVal(value) * 1.5);
};

OmnisDuo.needlePosition = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "playposition", value / 127.0);
    }
};

OmnisDuo.vinylSpeedAdjust = function(_ch, _ctrl, _value, _st, _group) {
    // Hardware-side brake adjustment; Mixxx scratch ramp constants are fixed per deck.
};

OmnisDuo.tempoReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "rate", 0);
    }
};

OmnisDuo.tempoRangeToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        var d = script.deckFromGroup(group);
        OmnisDuo.tempoRangeIdx[d] = (OmnisDuo.tempoRangeIdx[d] + 1) % OmnisDuo.tempoRanges.length;
        engine.setValue(group, "rateRange", OmnisDuo.tempoRanges[OmnisDuo.tempoRangeIdx[d]]);
    }
};

OmnisDuo.tempoRangeFixed = function(_ch, control, value, _st, group) {
    if (value > 0) {
        var map = {0x14: 0.06, 0x15: 0.10, 0x16: 0.16, 0x17: 1.0};
        if (map[control] !== undefined) {
            engine.setValue(group, "rateRange", map[control]);
        }
    }
};

OmnisDuo.beatJumpRotary = function(_ch, _ctrl, value, _st, group) {
    var delta = OmnisDuo.relativeVal(value);
    if (delta > 0) {
        engine.setValue(group, "beatjump_forward", 1);
    } else if (delta < 0) {
        engine.setValue(group, "beatjump_backward", 1);
    }
};

OmnisDuo.deckSelect = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OmnisDuo.activeDeck = (status & 0x0F) + 1;
    }
};

OmnisDuo.deckSelectButton = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OmnisDuo.activeDeck = (status & 0x0F) + 1;
    }
};

OmnisDuo.pcControlMode = function(_ch, _ctrl, value, status, _group) {
    if (value > 0) {
        OmnisDuo.activeDeck = (status & 0x0F) + 1;
        OmnisDuo.syncAllLeds();
    }
};

OmnisDuo.mixerModeState = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        OmnisDuo.syncAllLeds();
    }
};

OmnisDuo.stemMuteToggle = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        script.toggleControl(group, "mute");
    }
};

OmnisDuo.stemMuteReset = function(_ch, _ctrl, value, _st, group) {
    if (value > 0) {
        engine.setValue(group, "mute", 0);
    }
};

OmnisDuo.colorFxSelect = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        for (var d = 1; d <= OmnisDuo.kNumDecks; d++) {
            engine.setValue("[QuickEffectRack1_[Channel" + d + "]]", "next_chain_preset", 1);
        }
    }
};

OmnisDuo.colorFxParameter = function(_ch, _ctrl, value, _st, _g) {
    // Hardware sends global ColorFX Parameter; adjust Mixxx QuickEffect super1 only via per-channel knobs.
    void value;
};

OmnisDuo.beatFxTap = function(_ch, _ctrl, value, _st, _g) {
    if (value > 0) {
        engine.setValue("[Channel" + OmnisDuo.activeDeck + "]", "bpm_tap", 1);
    }
};

OmnisDuo.beatFxTime = function(_ch, _ctrl, value, _st, _g) {
    var delta = OmnisDuo.relativeVal(value);
    var cur = engine.getValue("[EffectRack1_EffectUnit1]", "super1");
    engine.setValue("[EffectRack1_EffectUnit1]", "super1", Math.max(0, Math.min(1, cur + delta * 0.02)));
};

OmnisDuo.shutdown = function() {
    for (var i = 0; i < OmnisDuo.connections.length; i++) {
        OmnisDuo.connections[i].disconnect();
    }
    OmnisDuo.connections = [];
    for (var d = 1; d <= OmnisDuo.kNumDecks; d++) {
        OmnisDuo.sendDeckLed(d, 0x00, false);
        OmnisDuo.sendDeckLed(d, 0x01, false);
        if (OmnisDuo.kLoadLedBase !== null) {
            midi.sendShortMsg(0x90 + OmnisDuo.kDisplayCh, OmnisDuo.kLoadLedBase + (d - 1), 0x00);
        }
    }
};
