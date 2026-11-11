/**
 * [AI-GENERATED DISCLAIMER START]
 * This mapping file was generated autonomously by an AI agent from the
 * verified Hercules DJControl Inpulse 200 MK2 (USB 06f8:b130) protocol
 * documentation and Mixxx's GPL-2.0+ Inpulse 200 mapping conventions.
 * It has NOT been tested on physical hardware.
 * [AI-GENERATED DISCLAIMER END]
 *
 * Mixxx controller script for Hercules DJControl Inpulse 200 MK2.
 * License: GPL-2.0-or-later
 */

var DJCi200MK2 = {};

DJCi200MK2.kVinylSpeedRpm = 33 + 1 / 3;
DJCi200MK2.kAlpha = 1.0 / 8;
DJCi200MK2.kBeta = DJCi200MK2.kAlpha / 32;
DJCi200MK2.kIntervalsPerRev = 248;

DJCi200MK2.vinylMode = {1: true, 2: true};
DJCi200MK2.stemConnections = [];

DJCi200MK2.useStemsOnMode3 = function() {
    if (typeof engine.getSetting === "function") {
        var val = engine.getSetting("stemsModeOnPadBank3");
        if (val !== undefined) {
            return Boolean(val);
        }
    }
    return true;
};

DJCi200MK2.updateMode3Leds = function() {
    var stems = DJCi200MK2.useStemsOnMode3();
    for (var deck = 1; deck <= 2; deck++) {
        var status = 0x95 + deck; // 0x96 Deck 1, 0x97 Deck 2
        for (var p = 0; p < 4; p++) {
            var ledOn = 0x00;
            if (stems) {
                var stemGroup = "[Channel" + deck + "_Stem" + (p + 1) + "]";
                // Lit when stem is active (unmuted); dim/off when muted.
                ledOn = engine.getValue(stemGroup, "mute") ? 0x00 : 0x7F;
            } else {
                if (p < 3) {
                    var fxGroup = "[EffectRack1_EffectUnit" + deck + "_Effect" + (p + 1) + "]";
                    ledOn = engine.getValue(fxGroup, "enabled") ? 0x7F : 0x00;
                } else {
                    var unitGroup = "[EffectRack1_EffectUnit" + deck + "]";
                    ledOn = engine.getValue(unitGroup, "group_[Channel" + deck + "]_enable") ? 0x7F : 0x00;
                }
            }
            midi.sendShortMsg(status, 0x20 + p, ledOn);

            var shiftOn = 0x00;
            if (p < 3) {
                var sfxGroup = "[EffectRack1_EffectUnit" + deck + "_Effect" + (p + 1) + "]";
                shiftOn = engine.getValue(sfxGroup, "enabled") ? 0x7F : 0x00;
            } else {
                var sunitGroup = "[EffectRack1_EffectUnit" + deck + "]";
                shiftOn = engine.getValue(sunitGroup, "group_[Channel" + deck + "]_enable") ? 0x7F : 0x00;
            }
            midi.sendShortMsg(status, 0x28 + p, shiftOn);
        }
    }
};

DJCi200MK2.padMode3 = function(_channel, control, value, status, _group) {
    if (value === 0) {
        return;
    }
    var deck = (status & 0x0F) - 5; // 0x96 -> 1, 0x97 -> 2
    if (deck < 1 || deck > 2) {
        return;
    }
    var padIndex = control - 0x20;
    if (padIndex < 0 || padIndex > 3) {
        return;
    }
    if (DJCi200MK2.useStemsOnMode3()) {
        var stemGroup = "[Channel" + deck + "_Stem" + (padIndex + 1) + "]";
        var muted = engine.getValue(stemGroup, "mute");
        engine.setValue(stemGroup, "mute", muted ? 0 : 1);
    } else if (padIndex < 3) {
        var fxGroup = "[EffectRack1_EffectUnit" + deck + "_Effect" + (padIndex + 1) + "]";
        engine.setValue(fxGroup, "enabled", engine.getValue(fxGroup, "enabled") ? 0 : 1);
    } else {
        var unitGroup = "[EffectRack1_EffectUnit" + deck + "]";
        var key = "group_[Channel" + deck + "]_enable";
        engine.setValue(unitGroup, key, engine.getValue(unitGroup, key) ? 0 : 1);
    }
    DJCi200MK2.updateMode3Leds();
};

DJCi200MK2.padMode3Shift = function(_channel, control, value, status, _group) {
    if (value === 0) {
        return;
    }
    var deck = (status & 0x0F) - 5; // 0x96 -> 1, 0x97 -> 2
    if (deck < 1 || deck > 2) {
        return;
    }
    var padIndex = control - 0x28;
    if (padIndex < 0 || padIndex > 3) {
        return;
    }
    if (padIndex < 3) {
        var fxGroup = "[EffectRack1_EffectUnit" + deck + "_Effect" + (padIndex + 1) + "]";
        engine.setValue(fxGroup, "enabled", engine.getValue(fxGroup, "enabled") ? 0 : 1);
    } else {
        var unitGroup = "[EffectRack1_EffectUnit" + deck + "]";
        var key = "group_[Channel" + deck + "]_enable";
        engine.setValue(unitGroup, key, engine.getValue(unitGroup, key) ? 0 : 1);
    }
    DJCi200MK2.updateMode3Leds();
};

DJCi200MK2.init = function(_id, _debugging) {
    DJCi200MK2.vinylMode = {1: true, 2: true};

    for (var deck = 1; deck <= 2; deck++) {
        var chGroup = "[Channel" + deck + "]";
        engine.softTakeover(chGroup, "rate", true);
        engine.softTakeover(chGroup, "volume", true);
        engine.softTakeover(chGroup, "pregain", true);
        engine.softTakeover("[EqualizerRack1_" + chGroup + "_Effect1]", "parameter1", true);
        engine.softTakeover("[EqualizerRack1_" + chGroup + "_Effect1]", "parameter3", true);
        engine.softTakeover("[QuickEffectRack1_" + chGroup + "]", "super1", true);
    }
    engine.softTakeover("[Master]", "crossfader", true);

    for (var d = 1; d <= 2; d++) {
        for (var s = 1; s <= 4; s++) {
            DJCi200MK2.stemConnections.push(
                engine.makeConnection("[Channel" + d + "_Stem" + s + "]", "mute", DJCi200MK2.updateMode3Leds)
            );
        }
        for (var fx = 1; fx <= 3; fx++) {
            DJCi200MK2.stemConnections.push(
                engine.makeConnection("[EffectRack1_EffectUnit" + d + "_Effect" + fx + "]", "enabled", DJCi200MK2.updateMode3Leds)
            );
        }
        DJCi200MK2.stemConnections.push(
            engine.makeConnection("[EffectRack1_EffectUnit" + d + "]", "group_[Channel" + d + "]_enable", DJCi200MK2.updateMode3Leds)
        );
    }

    // Mixxx opens MIDI outputs right after init() completes; defer initial hardware queries & LEDs.
    engine.beginTimer(20, function() {
        midi.sendShortMsg(0x90, 0x04, 0x05); // Browser LED blue default
        midi.sendShortMsg(0x91, 0x03, DJCi200MK2.vinylMode[1] ? 0x7F : 0x00);
        midi.sendShortMsg(0x92, 0x03, DJCi200MK2.vinylMode[2] ? 0x7F : 0x00);
        // Ask the controller to dump initial pot/fader positions on USB.
        midi.sendShortMsg(0xB0, 0x7F, 0x7F);
        DJCi200MK2.updateMode3Leds();
    }, true);
};

DJCi200MK2.vinylButton = function(_channel, _control, value, status, group) {
    if (value === 0) {
        return;
    }
    var deck = script.deckFromGroup(group);
    DJCi200MK2.vinylMode[deck] = !DJCi200MK2.vinylMode[deck];
    var ledStatus = (deck === 1) ? 0x91 : 0x92;
    midi.sendShortMsg(ledStatus, 0x03, DJCi200MK2.vinylMode[deck] ? 0x7F : 0x00);
};

DJCi200MK2.wheelTouch = function(_channel, _control, value, _status, group) {
    var deck = script.deckFromGroup(group);
    if (DJCi200MK2.vinylMode[deck] && value === 0x7F) {
        engine.scratchEnable(
            deck,
            DJCi200MK2.kIntervalsPerRev,
            DJCi200MK2.kVinylSpeedRpm,
            DJCi200MK2.kAlpha,
            DJCi200MK2.kBeta
        );
    } else {
        engine.scratchDisable(deck);
    }
};

DJCi200MK2.wheelTouchShift = function(_channel, _control, value, _status, group) {
    var deck = script.deckFromGroup(group);
    if (value === 0x7F) {
        engine.scratchEnable(
            deck,
            DJCi200MK2.kIntervalsPerRev,
            DJCi200MK2.kVinylSpeedRpm,
            DJCi200MK2.kAlpha,
            DJCi200MK2.kBeta
        );
    } else {
        engine.scratchDisable(deck);
    }
};

DJCi200MK2.scratchWheel = function(_channel, _control, value, _status, group) {
    var newValue = (value < 0x40) ? value : (value - 0x80);
    var deck = script.deckFromGroup(group);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, newValue);
    } else {
        var speed = (newValue > 0) ? Math.pow(newValue, 2) / 10 : -Math.pow(newValue, 2) / 10;
        engine.setValue(group, "jog", speed);
    }
};

DJCi200MK2.bendWheel = function(_channel, _control, value, _status, group) {
    var newValue = (value < 0x40) ? value : (value - 0x80);
    var speed = (newValue > 0) ? Math.pow(newValue, 2) / 10 : -Math.pow(newValue, 2) / 10;
    engine.setValue(group, "jog", speed);
};

DJCi200MK2.scratchPad = function(_channel, _control, value, _status, group) {
    var deck = script.deckFromGroup(group);
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, value === 0x01 ? 1 : -1);
    } else {
        engine.setValue(group, "jog", value);
    }
};

DJCi200MK2.shutdown = function() {
    for (var i = 0; i < DJCi200MK2.stemConnections.length; i++) {
        DJCi200MK2.stemConnections[i].disconnect();
    }
    DJCi200MK2.stemConnections = [];
    for (var deck = 1; deck <= 2; deck++) {
        midi.sendShortMsg(0x90 + deck, 0x03, 0x00);
        for (var p = 0; p < 4; p++) {
            midi.sendShortMsg(0x95 + deck, 0x20 + p, 0x00);
            midi.sendShortMsg(0x95 + deck, 0x28 + p, 0x00);
        }
    }
    midi.sendShortMsg(0x90, 0x04, 0x00);
};
