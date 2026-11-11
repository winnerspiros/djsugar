// Pioneer players in USB MIDI mode (CDJ-400, XDJ-700, XDJ-1000): the MIDI
// messages of their manuals, the same for these models. The deck of a unit
// is its MIDI channel (channel 1 = deck 1), or the "Deck" setting, so that
// two units can play two decks even when both send on channel 1.
//
// Jog: the platter and the ring send their speed (64 = still), not ticks.
// With the jog mode on vinyl, touching the platter scratches.

// eslint-disable-next-line no-unused-vars
function PioneerPlayer(model) {
    this.model = model;
    this.vinyl = {};
    this.scratching = {};
    // Notes of the buttons: [control, how]
    this.buttons = {
        0x00: ["play", "toggle"],
        0x01: ["cue_default", "hold"],
        0x02: ["fwd", "hold"],
        0x03: ["back", "hold"],
        0x04: ["next", "browse"],
        0x05: ["previous", "browse"],
        0x06: ["loop_in", "hold"],
        0x07: ["loop_out", "hold"],
        0x08: ["reloop_toggle", "press"],
        0x10: ["tempoRange", "special"],
        0x11: ["keylock", "toggle"],
        0x12: ["jogMode", "special"],
        0x20: ["jogTouch", "special"],
        0x21: ["reverse", "toggle"],
        0x2F: ["eject", "press"],
        0x32: ["browseBack", "special"],
        0x33: ["LoadSelectedTrack", "press"],
    };
    if (model === "cdj400") {
        this.buttons[0x16] = ["loop_double", "press"];
        this.buttons[0x17] = ["loop_halve", "press"];
        // TRANS/ROLL: a roll of a quarter of a beat while held
        this.buttons[0x24] = ["beatlooproll_0.25_activate", "hold"];
    } else {
        this.buttons[0x09] = ["quantize", "toggle"];
        this.buttons[0x18] = ["hotcue_1_activate", "hold"];
        this.buttons[0x19] = ["hotcue_2_activate", "hold"];
        this.buttons[0x1A] = ["hotcue_3_activate", "hold"];
        this.buttons[0x1E] = ["sync_leader", "toggle"];
        this.buttons[0x1F] = ["sync_enabled", "toggle"];
        this.buttons[0x22] = ["vinylBrake", "special"];
        this.buttons[0x27] = ["beatloop_0.5_toggle", "press"];
        this.buttons[0x28] = ["beatloop_1_toggle", "press"];
        this.buttons[0x29] = ["beatloop_2_toggle", "press"];
        this.buttons[0x2A] = ["beatloop_4_toggle", "press"];
        this.buttons[0x2B] = ["beatloop_8_toggle", "press"];
        this.buttons[0x2C] = ["beatloop_16_toggle", "press"];
        this.buttons[0x2D] = ["slip_enabled", "toggle"];
        this.buttons[0x38] = ["beatjump_1_forward", "press"];
        this.buttons[0x39] = ["beatjump_2_forward", "press"];
        this.buttons[0x3A] = ["beatjump_4_forward", "press"];
        this.buttons[0x3B] = ["beatjump_1_backward", "press"];
        this.buttons[0x3C] = ["beatjump_2_backward", "press"];
        this.buttons[0x3D] = ["beatjump_4_backward", "press"];
    }
}

PioneerPlayer.prototype.init = function() {
    for (var deck = 1; deck <= 4; deck++) {
        this.vinyl[deck] = engine.getSetting("jogMode") !== "cdj";
        this.scratching[deck] = false;
    }
};

PioneerPlayer.prototype.shutdown = function() {};

/// The deck of a message: its MIDI channel, or the "Deck" setting
PioneerPlayer.prototype.deckOf = function(channel) {
    var setting = engine.getSetting("deck");
    if (setting && setting !== "auto") {
        return parseInt(setting, 10);
    }
    return Math.min(4, channel + 1);
};

PioneerPlayer.prototype.button = function(channel, control, value) {
    var entry = this.buttons[control];
    if (!entry) {
        return;
    }
    var deck = this.deckOf(channel);
    var group = "[Channel" + deck + "]";
    var pressed = value > 0;
    var key = entry[0];
    switch (entry[1]) {
    case "toggle":
        if (pressed) {
            script.toggleControl(group, key);
        }
        break;
    case "hold":
        engine.setValue(group, key, pressed ? 1 : 0);
        break;
    case "press":
        if (pressed) {
            engine.setValue(group, key, 1);
        }
        break;
    case "browse":
        if (pressed) {
            engine.setValue("[Library]", "MoveVertical", key === "next" ? 1 : -1);
        }
        break;
    case "special":
        this.special(key, deck, group, pressed);
        break;
    }
};

PioneerPlayer.prototype.special = function(key, deck, group, pressed) {
    switch (key) {
    case "tempoRange":
        if (pressed) {
            // 6, 10, 16 % and wide, like the players
            var ranges = [0.06, 0.10, 0.16, 1.0];
            var current = engine.getValue(group, "rateRange");
            var next = ranges[0];
            for (var i = 0; i < ranges.length; i++) {
                if (ranges[i] > current + 0.001) {
                    next = ranges[i];
                    break;
                }
            }
            engine.setValue(group, "rateRange", next);
        }
        break;
    case "jogMode":
        if (pressed) {
            this.vinyl[deck] = !this.vinyl[deck];
        }
        break;
    case "jogTouch":
        if (pressed && this.vinyl[deck]) {
            engine.scratchEnable(deck, 512, 33 + 1 / 3, 1 / 8, 1 / 8 / 32, true);
            this.scratching[deck] = true;
        } else if (!pressed && this.scratching[deck]) {
            engine.scratchDisable(deck, true);
            this.scratching[deck] = false;
        }
        break;
    case "vinylBrake":
        if (pressed && engine.getValue(group, "play")) {
            engine.brake(deck, true);
        }
        break;
    case "browseBack":
        if (pressed) {
            engine.setValue("[Library]", "MoveFocusBackward", 1);
        }
        break;
    }
};

/// The platter (CC 0x10) and the ring (CC 0x30): their speed
PioneerPlayer.prototype.jog = function(channel, control, value) {
    var deck = this.deckOf(channel);
    var speed = value - 64;
    if (this.scratching[deck] && control === 0x10) {
        engine.scratchTick(deck, speed);
    } else {
        engine.setValue("[Channel" + deck + "]", "jog", speed / (control === 0x10 ? 2 : 4));
    }
};

/// The tempo slider (CC 0x1D): 0 is the minus end
PioneerPlayer.prototype.tempo = function(channel, control, value) {
    var group = "[Channel" + this.deckOf(channel) + "]";
    engine.setValue(group, "rate", Math.max(-1, Math.min(1, (value - 64) / 63)));
};

/// The browse encoder (CC 0x4F)
PioneerPlayer.prototype.browse = function(channel, control, value) {
    var steps;
    if (this.model === "cdj400") {
        // 64 plus the change
        steps = value - 64;
    } else {
        // 1 to 30 clockwise, 98 to 127 counter-clockwise
        steps = value < 64 ? value : value - 128;
    }
    if (steps !== 0) {
        engine.setValue("[Library]", "MoveVertical", steps);
    }
};

/// The needle search strip of the XDJ (CC 0x1C): 0 is not touched
PioneerPlayer.prototype.needle = function(channel, control, value) {
    if (value > 0) {
        var group = "[Channel" + this.deckOf(channel) + "]";
        engine.setValue(group, "playposition", (value - 1) / 126);
    }
};
