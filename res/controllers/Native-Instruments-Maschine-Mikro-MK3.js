// Native Instruments Maschine Mikro MK3: DJ Sugar's Groove Box on the pads.
//
// The Mikro is a plain USB HID device (17cc:1700); no NI software or MIDI
// mode is needed. Report layout as documented by r00tman's MIT-licensed
// maschine-mikro-mk3-driver and flokapi's pymikro:
//
//   in  0x01  buttons: 41 bits from byte 1, encoder 0-15 in byte 7,
//             touch strip in byte 10 (0 = not touched, 1-200)
//   in  0x02  pads: triples (pad, event | value >> 8, value & 0xff), pad
//             0-15 left to right from the top row, value 0-4095. The pads
//             are printed 1-16 left to right from the bottom row, and
//             [GrooveN],padK follows the print (see padNumber).
//   out 0x80  80 bytes of lights: buttons 0-38, pads 39-54 (colour * 4 +
//             brightness 1-3), touch strip 55-79
//   out 0xe0  the 128 x 32 screen, one bit a pixel (clear = lit), in two
//             halves of 256 bytes
//
// What the pads do is the engine's (see crates/engine/src/groove_pads.rs):
// this script passes presses to [GrooveN],padK and shows
// [GrooveN],padK_color.
//
//   STEP / PAD MODE / PATTERN   pads edit steps / play the sounds / pick
//                               grooves, kits and the genre's grooves
//   LEFT / RIGHT                row before / after (SELECT + pad 1-8 too)
//   SHIFT + LEFT / RIGHT        the Groove Box on another deck
//   PLAY / STOP / RESTART       start / stop / start again from the bar
//   REC                         record what the pads play (Play mode)
//   ERASE                       empty the row
//   MUTE / SOLO                 the row; held + pad: that pad's row
//   VOLUME / PITCH / MOD        the encoder and strip set the row's level /
//   SWING / TEMPO               pitch / decay, the swing / own tempo
//   PERFORM                     the strip plays REMIX while touched (16
//                               figures along it); again: back to the knob
//   NOTE REPEAT (held)          FILL: a fill now
//   AUTO                        Auto fill: off, Auto 1, Auto 2

var MikroMK3 = {};

// Button numbers, as the bits of report 0x01 (and the lights' bytes).
MikroMK3.B = {
    MASCHINE: 0, STAR: 1, BROWSE: 2, VOLUME: 3, SWING: 4, TEMPO: 5, PLUGIN: 6,
    SAMPLING: 7, LEFT: 8, RIGHT: 9, PITCH: 10, MOD: 11, PERFORM: 12, NOTES: 13,
    GROUP: 14, AUTO: 15, LOCK: 16, NOTE_REPEAT: 17, RESTART: 18, ERASE: 19,
    TAP: 20, FOLLOW: 21, PLAY: 22, REC: 23, STOP: 24, SHIFT: 25, FIXED_VEL: 26,
    PAD_MODE: 27, KEYBOARD: 28, CHORDS: 29, STEP: 30, SCENE: 31, PATTERN: 32,
    EVENTS: 33, VARIATION: 34, DUPLICATE: 35, SELECT: 36, SOLO: 37, MUTE: 38,
    ENCODER_PRESS: 39, ENCODER_TOUCH: 40,
};
MikroMK3.BUTTON_LIGHTS = 39;

// The Groove Box's kits, as crates/engine/src/kits.rs lists them: every
// kit has the same nine rows (kits::ROLES), then its own extra sounds.
MikroMK3.KITS = [
    {name: "Analog", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Low tom", "High tom"]},
    {name: "Synthetic", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Low tom", "High tom"]},
    {name: "Boom", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Tom"]},
    {name: "Punch", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Tom"]},
    {name: "Tin", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Shaker", "Tom"]},
    {name: "Dub", sounds: ["Kick", "Snare", "Closed hat", "Open hat", "Clap", "Rim", "Cowbell", "Crash", "Ride", "Chord", "Shaker", "Tom"]},
];
// Rows to pick from, and the rows the pads play in Play mode (twice over).
MikroMK3.ROWS = 8;
MikroMK3.PLAY_ROWS = 8;
MikroMK3.DECKS = 4;

// Mikro pad colours (0 off, then red ... white).
MikroMK3.C = {OFF: 0, RED: 1, ORANGE: 2, YELLOW: 5, GREEN: 7, CYAN: 9, BLUE: 11,
    VIOLET: 13, PURPLE: 14, MAGENTA: 15, FUCHSIA: 16, WHITE: 17};
// [GrooveN],padK_color (groove_pads::light) -> [colour, brightness 1-3].
MikroMK3.ROW_COLORS = [MikroMK3.C.RED, MikroMK3.C.ORANGE, MikroMK3.C.YELLOW, MikroMK3.C.GREEN,
    MikroMK3.C.CYAN, MikroMK3.C.BLUE, MikroMK3.C.PURPLE, MikroMK3.C.MAGENTA];
MikroMK3.padLight = function(v) {
    const C = MikroMK3.C;
    if (v >= 1 && v <= 8) {
        return [MikroMK3.ROW_COLORS[v - 1], 3];
    }
    if (v >= 13 && v <= 20) {
        return [MikroMK3.ROW_COLORS[v - 13], 1];
    }
    switch (v) {
    case 9: return [C.WHITE, 3];
    case 10: return [C.VIOLET, 3];
    case 11: return [C.WHITE, 1];
    case 12: return [C.VIOLET, 1];
    default: return [C.OFF, 0];
    }
};

// Button brightness.
MikroMK3.OFF = 0x00;
MikroMK3.DIM = 0x7c;
MikroMK3.ON = 0x7e;
MikroMK3.BRIGHT = 0x7f;

// What the encoder and strip set: [group kind, key, step, label, format].
MikroMK3.TARGETS = {
    level: {row: true, key: "level", step: 0.02, label: "LEVEL", show: v => Math.round(v * 100) + "%"},
    pitch: {row: true, key: "pitch", step: 1, label: "PITCH", show: v => (v > 0 ? "+" : "") + Math.round(v)},
    decay: {row: true, key: "decay", step: 0.02, label: "DECAY", show: v => Math.round(v * 100) + "%"},
    swing: {row: false, key: "swing", step: 0.025, label: "SWING", show: v => Math.round(v * 100) + "%"},
    // Changing the tempo stops it following the mix, as on screen.
    bpm: {row: false, key: "bpm", step: 1, label: "TEMPO", show: v => (Math.round(v * 10) / 10) + " BPM", unfollow: true},
};
MikroMK3.TARGET_BUTTONS = [
    ["level", MikroMK3.B.VOLUME], ["pitch", MikroMK3.B.PITCH], ["decay", MikroMK3.B.MOD],
    ["swing", MikroMK3.B.SWING], ["bpm", MikroMK3.B.TEMPO],
];

MikroMK3.init = function(_id) {
    this.deck = 0;
    this.buttons = [];
    this.pressed = new Array(16).fill(false);
    // Pads held with MUTE, SOLO or SELECT: their release stays here.
    this.captured = new Array(16).fill(false);
    this.modifierUsed = false;
    this.encoder = null;
    this.target = "level";
    this.stripRemix = false;
    this.lastStrip = 0;
    this.message = "";
    this.messageUntil = 0;
    this.lastLights = "";
    this.lastScreen = "";
    this.pickDeck(0);
    this.timer = engine.beginTimer(25, () => this.refresh());
    this.refresh();
};

MikroMK3.shutdown = function() {
    engine.stopTimer(this.timer);
    controller.sendOutputReport(0x80, new Array(80).fill(0), true);
    this.sendScreen(new Array(512).fill(0xff));
};

MikroMK3.group = function() {
    return "[Groove" + (this.deck + 1) + "]";
};

MikroMK3.channel = function() {
    return "[Channel" + (this.deck + 1) + "]";
};

MikroMK3.isGroove = function(d) {
    return engine.getValue("[Groove" + (d + 1) + "]", "enabled") > 0;
};

// The Groove Box `step` decks along from the current one (0: the current
// one if it still is a Groove Box, else the first).
MikroMK3.pickDeck = function(step) {
    if (step === 0) {
        if (!this.isGroove(this.deck)) {
            for (let d = 0; d < this.DECKS; d++) {
                if (this.isGroove(d)) {
                    this.deck = d;
                    break;
                }
            }
        }
        return;
    }
    for (let i = 1; i < this.DECKS; i++) {
        const d = (this.deck + step * i + this.DECKS * i) % this.DECKS;
        if (this.isGroove(d)) {
            this.deck = d;
            return;
        }
    }
};

MikroMK3.row = function() {
    return Math.max(1, Math.min(this.ROWS, Math.round(engine.getValue(this.group(), "row_selected"))));
};

MikroMK3.held = function(b) {
    return !!this.buttons[b];
};

MikroMK3.say = function(text) {
    this.message = text;
    this.messageUntil = Date.now() + 1500;
};

// The printed pad (0-15: 1-16, left to right from the bottom row) for a
// pad in the reports (0-15, left to right from the top row), and the other
// way: flipping the rows twice gives back what it started with.
MikroMK3.padNumber = function(raw) {
    return (3 - (raw >> 2)) * 4 + (raw & 3);
};

MikroMK3.incomingData = function(data, _length) {
    if (data[0] === 0x01) {
        this.onButtons(data);
    } else if (data[0] === 0x02) {
        this.onPads(data);
    }
};

MikroMK3.onButtons = function(data) {
    const now = [];
    for (let b = 0; b <= this.B.ENCODER_TOUCH; b++) {
        now[b] = (data[1 + (b >> 3)] & (1 << (b & 7))) !== 0;
    }
    for (let b = 0; b <= this.B.ENCODER_TOUCH; b++) {
        if (now[b] !== this.held(b)) {
            this.buttons[b] = now[b];
            if (now[b]) {
                this.press(b);
            } else {
                this.release(b);
            }
        }
    }
    const enc = data[7] & 0x0f;
    if (this.encoder !== null && enc !== this.encoder) {
        let d = enc - this.encoder;
        if (d > 8) {
            d -= 16;
        } else if (d < -8) {
            d += 16;
        }
        this.turn(d);
    }
    this.encoder = enc;
    const strip = data[10];
    if (this.stripRemix) {
        // Remix while touched; let go and the groove goes back.
        if (strip > 0) {
            engine.setValue(this.group(), "remix", Math.max(0.001, Math.min(1, (strip - 1) / 199)));
        } else if (this.lastStrip > 0) {
            engine.setValue(this.group(), "remix", 0);
        }
    } else if (strip > 0) {
        this.slide(Math.min(1, (strip - 1) / 199));
    }
    this.lastStrip = strip;
};

MikroMK3.press = function(b) {
    const B = this.B;
    const g = this.group();
    const shift = this.held(B.SHIFT);
    for (const [name, button] of this.TARGET_BUTTONS) {
        if (b === button) {
            this.target = name;
            this.say(this.TARGETS[name].label);
            return;
        }
    }
    switch (b) {
    case B.STEP:
        engine.setValue(g, "pad_mode", 0);
        this.say("STEPS");
        break;
    case B.PAD_MODE:
    case B.KEYBOARD:
        engine.setValue(g, "pad_mode", 1);
        this.say("PLAY THE PADS");
        break;
    case B.PATTERN:
    case B.SCENE:
        engine.setValue(g, "pad_mode", 2);
        this.say("GROOVES AND KITS");
        break;
    case B.LEFT:
    case B.RIGHT:
        if (shift) {
            this.pickDeck(b === B.LEFT ? -1 : 1);
            this.say("DECK " + (this.deck + 1));
        } else {
            engine.setValue(g, "row_selected", this.row() + (b === B.LEFT ? -1 : 1));
        }
        break;
    case B.PLAY:
        engine.setValue(this.channel(), "play", engine.getValue(this.channel(), "play") > 0 ? 0 : 1);
        break;
    case B.STOP:
        engine.setValue(this.channel(), "play", 0);
        break;
    case B.RESTART:
        engine.setValue(this.channel(), "play", 0);
        engine.beginTimer(30, () => engine.setValue(this.channel(), "play", 1), true);
        break;
    case B.REC: {
        const on = engine.getValue(g, "record") > 0 ? 0 : 1;
        engine.setValue(g, "record", on);
        if (on) {
            engine.setValue(g, "pad_mode", 1);
        }
        this.say(on ? "RECORDING THE PADS" : "RECORD OFF");
        break;
    }
    case B.PERFORM:
        this.stripRemix = !this.stripRemix;
        engine.setValue(g, "remix", 0);
        this.say(this.stripRemix ? "STRIP: REMIX" : "STRIP: " + this.TARGETS[this.target].label);
        break;
    case B.NOTE_REPEAT:
        engine.setValue(g, "fill", 1);
        this.say("FILL");
        break;
    case B.AUTO: {
        const auto = (Math.round(engine.getValue(g, "auto_fill")) + 1) % 3;
        engine.setValue(g, "auto_fill", auto);
        this.say(auto === 0 ? "NO FILLS" : "AUTO " + auto);
        break;
    }
    case B.ERASE:
        engine.setValue(g, "row_clear", 1);
        this.say("ROW " + this.row() + " EMPTY");
        break;
    case B.MUTE:
    case B.SOLO:
    case B.SELECT:
        this.modifierUsed = false;
        break;
    case B.ENCODER_PRESS:
        this.target = "level";
        this.say("LEVEL");
        break;
    default:
        break;
    }
};

MikroMK3.release = function(b) {
    const B = this.B;
    if (b === B.NOTE_REPEAT) {
        engine.setValue(this.group(), "fill", 0);
    }
    if ((b === B.MUTE || b === B.SOLO) && !this.modifierUsed) {
        const key = "row" + this.row() + (b === B.MUTE ? "_mute" : "_solo");
        engine.setValue(this.group(), key, engine.getValue(this.group(), key) > 0 ? 0 : 1);
    }
};

MikroMK3.onPads = function(data) {
    for (let i = 1; i + 2 < data.length; i += 3) {
        const raw = data[i];
        const event = data[i + 1] & 0xf0;
        const value = ((data[i + 1] & 0x0f) << 8) | data[i + 2];
        if (i > 1 && raw === 0 && event === 0 && value === 0) {
            break;
        }
        if (raw > 15) {
            continue;
        }
        const pad = this.padNumber(raw);
        // 0x00 / 0x10 press, 0x20 / 0x30 release, 0x40 pressure.
        if (event === 0x00 || event === 0x10) {
            if (!this.pressed[pad]) {
                this.pressed[pad] = true;
                this.padDown(pad, value);
            }
        } else if (event === 0x20 || event === 0x30 || (event === 0x40 && value === 0)) {
            if (this.pressed[pad]) {
                this.pressed[pad] = false;
                this.padUp(pad);
            }
        }
    }
};

MikroMK3.padDown = function(pad, value) {
    const B = this.B;
    const g = this.group();
    const row = (pad % this.PLAY_ROWS) + 1;
    if (this.held(B.MUTE) || this.held(B.SOLO)) {
        const key = "row" + row + (this.held(B.MUTE) ? "_mute" : "_solo");
        engine.setValue(g, key, engine.getValue(g, key) > 0 ? 0 : 1);
        this.modifierUsed = true;
        this.captured[pad] = true;
        return;
    }
    if (this.held(B.SELECT) && pad < this.ROWS) {
        engine.setValue(g, "row_selected", pad + 1);
        this.modifierUsed = true;
        this.captured[pad] = true;
        return;
    }
    // Soft hits read about a quarter of the way; hard ones reach the top.
    const velocity = Math.max(0.05, Math.min(1, value / 3000));
    engine.setValue(g, "pad" + (pad + 1), this.held(B.FIXED_VEL) ? 1 : velocity);
};

MikroMK3.padUp = function(pad) {
    if (this.captured[pad]) {
        this.captured[pad] = false;
        return;
    }
    engine.setValue(this.group(), "pad" + (pad + 1), 0);
};

MikroMK3.targetGroupKey = function() {
    const t = this.TARGETS[this.target];
    return [this.group(), t.row ? "row" + this.row() + "_" + t.key : t.key];
};

MikroMK3.turn = function(delta) {
    const t = this.TARGETS[this.target];
    const [group, key] = this.targetGroupKey();
    const step = this.held(this.B.SHIFT) ? t.step / 10 : t.step;
    if (t.unfollow) {
        engine.setValue(group, "follow", 0);
    }
    engine.setValue(group, key, engine.getValue(group, key) + delta * step);
    this.say(t.label + " " + t.show(engine.getValue(group, key)));
};

MikroMK3.slide = function(position) {
    const t = this.TARGETS[this.target];
    const [group, key] = this.targetGroupKey();
    if (t.unfollow) {
        engine.setValue(group, "follow", 0);
    }
    engine.setParameter(group, key, position);
    this.say(t.label + " " + t.show(engine.getValue(group, key)));
};

// Every 25 ms: the deck still a Groove Box, then the lights and screen,
// each sent only when it changed.
MikroMK3.refresh = function() {
    this.pickDeck(0);
    this.sendLights();
    this.drawScreen();
};

MikroMK3.sendLights = function() {
    const B = this.B;
    const out = new Array(80).fill(0);
    const on = this.isGroove(this.deck);
    const g = this.group();
    const lit = (b, v) => {
        out[b] = v ? this.ON : this.DIM;
    };
    if (on) {
        for (let k = 0; k < 16; k++) {
            const [color, bright] = this.padLight(Math.round(engine.getValue(g, "pad" + (k + 1) + "_color")));
            out[this.BUTTON_LIGHTS + this.padNumber(k)] = color === 0 ? 0 : color * 4 + bright;
        }
        const mode = Math.round(engine.getValue(g, "pad_mode"));
        lit(B.STEP, mode === 0);
        lit(B.PAD_MODE, mode === 1);
        lit(B.PATTERN, mode === 2);
        lit(B.PLAY, engine.getValue(this.channel(), "play") > 0);
        out[B.STOP] = this.DIM;
        out[B.RESTART] = this.DIM;
        out[B.REC] = engine.getValue(g, "record") > 0 ? this.BRIGHT : this.DIM;
        out[B.ERASE] = this.DIM;
        lit(B.MUTE, engine.getValue(g, "row" + this.row() + "_mute") > 0);
        lit(B.SOLO, engine.getValue(g, "row" + this.row() + "_solo") > 0);
        lit(B.LEFT, this.row() > 1);
        lit(B.RIGHT, this.row() < this.ROWS);
        for (const [name, button] of this.TARGET_BUTTONS) {
            lit(button, this.target === name);
        }
        out[B.SELECT] = this.held(B.SELECT) ? this.ON : this.DIM;
        out[B.FIXED_VEL] = this.held(B.FIXED_VEL) ? this.ON : this.OFF;
        out[B.SHIFT] = this.held(B.SHIFT) ? this.ON : this.DIM;
        lit(B.PERFORM, this.stripRemix);
        lit(B.AUTO, engine.getValue(g, "auto_fill") > 0);
        out[B.NOTE_REPEAT] = engine.getValue(g, "fill_playing") > 0 ? this.BRIGHT : this.DIM;
        // The strip shows the encoder's value, or the Remix figure.
        const [group, key] = this.stripRemix ? [g, "remix"] : this.targetGroupKey();
        const n = Math.round(engine.getParameter(group, key) * 25);
        for (let i = 0; i < 25; i++) {
            out[55 + i] = i < n ? this.C.VIOLET * 4 + (i === n - 1 ? 3 : 1) : 0;
        }
    }
    const key = out.join(",");
    if (key !== this.lastLights) {
        this.lastLights = key;
        controller.sendOutputReport(0x80, out);
    }
};

MikroMK3.lines = function() {
    if (!this.isGroove(this.deck)) {
        return ["NO GROOVE BOX", "PRESS GROOVE BOX", "ON A STOPPED DECK", "IN DJSUGAR"];
    }
    const g = this.group();
    const kit = this.KITS[Math.round(engine.getValue(g, "kit"))] || this.KITS[0];
    const playing = engine.getValue(this.channel(), "play") > 0;
    const groove = Math.round(engine.getValue(g, "groove"));
    const queued = Math.round(engine.getValue(g, "groove_queued"));
    const row = this.row();
    const sound = kit.sounds[Math.round(engine.getValue(g, "row" + row + "_sound"))] || "";
    const muted = engine.getValue(g, "row" + row + "_mute") > 0;
    const solo = engine.getValue(g, "row" + row + "_solo") > 0;
    const bpm = Math.round(engine.getValue(g, "bpm") * 10) / 10;
    const letters = "ABCD";
    const modes = ["STEPS", "PLAY", "GROOVES"];
    const mode = modes[Math.round(engine.getValue(g, "pad_mode"))] || modes[0];
    const rec = engine.getValue(g, "record") > 0 ? "  REC" : "";
    const bottom = Date.now() < this.messageUntil ? this.message : mode + rec;
    return [
        "DECK " + (this.deck + 1) + " " + kit.name + (playing ? "  >" : ""),
        "GROOVE " + letters[groove] + (queued !== groove ? ">" + letters[queued] : "") + "  " + bpm,
        "ROW " + row + " " + sound + (muted ? " M" : "") + (solo ? " S" : ""),
        bottom,
    ];
};

MikroMK3.drawScreen = function() {
    const lines = this.lines();
    const key = lines.join("\n");
    if (key === this.lastScreen) {
        return;
    }
    this.lastScreen = key;
    const buf = new Array(512).fill(0xff);
    for (let l = 0; l < 4; l++) {
        const text = String(lines[l] || "").toUpperCase().slice(0, 21);
        for (let c = 0; c < text.length; c++) {
            const glyph = this.glyph(text[c]);
            for (let x = 0; x < 5; x++) {
                buf[l * 128 + c * 6 + x] = 0xff ^ glyph[x];
            }
        }
    }
    this.sendScreen(buf);
};

MikroMK3.sendScreen = function(buf) {
    controller.sendOutputReport(0xe0, [0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x02, 0x00].concat(buf.slice(0, 256)), true);
    controller.sendOutputReport(0xe0, [0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x02, 0x00].concat(buf.slice(256, 512)), true);
};

// A 5 x 7 font: five columns a character, bit 0 the top row.
MikroMK3.FONT = {
    " ": [0x00, 0x00, 0x00, 0x00, 0x00], "!": [0x00, 0x00, 0x5f, 0x00, 0x00],
    "#": [0x14, 0x7f, 0x14, 0x7f, 0x14], "%": [0x23, 0x13, 0x08, 0x64, 0x62],
    "+": [0x08, 0x08, 0x3e, 0x08, 0x08], "-": [0x08, 0x08, 0x08, 0x08, 0x08],
    ".": [0x00, 0x60, 0x60, 0x00, 0x00], "/": [0x20, 0x10, 0x08, 0x04, 0x02],
    ":": [0x00, 0x36, 0x36, 0x00, 0x00], "<": [0x08, 0x14, 0x22, 0x41, 0x00],
    ">": [0x41, 0x22, 0x14, 0x08, 0x00], "?": [0x02, 0x01, 0x51, 0x09, 0x06],
    "0": [0x3e, 0x51, 0x49, 0x45, 0x3e], "1": [0x00, 0x42, 0x7f, 0x40, 0x00],
    "2": [0x42, 0x61, 0x51, 0x49, 0x46], "3": [0x21, 0x41, 0x45, 0x4b, 0x31],
    "4": [0x18, 0x14, 0x12, 0x7f, 0x10], "5": [0x27, 0x45, 0x45, 0x45, 0x39],
    "6": [0x3c, 0x4a, 0x49, 0x49, 0x30], "7": [0x01, 0x71, 0x09, 0x05, 0x03],
    "8": [0x36, 0x49, 0x49, 0x49, 0x36], "9": [0x06, 0x49, 0x49, 0x29, 0x1e],
    "A": [0x7e, 0x11, 0x11, 0x11, 0x7e], "B": [0x7f, 0x49, 0x49, 0x49, 0x36],
    "C": [0x3e, 0x41, 0x41, 0x41, 0x22], "D": [0x7f, 0x41, 0x41, 0x22, 0x1c],
    "E": [0x7f, 0x49, 0x49, 0x49, 0x41], "F": [0x7f, 0x09, 0x09, 0x01, 0x01],
    "G": [0x3e, 0x41, 0x41, 0x51, 0x32], "H": [0x7f, 0x08, 0x08, 0x08, 0x7f],
    "I": [0x00, 0x41, 0x7f, 0x41, 0x00], "J": [0x20, 0x40, 0x41, 0x3f, 0x01],
    "K": [0x7f, 0x08, 0x14, 0x22, 0x41], "L": [0x7f, 0x40, 0x40, 0x40, 0x40],
    "M": [0x7f, 0x02, 0x04, 0x02, 0x7f], "N": [0x7f, 0x04, 0x08, 0x10, 0x7f],
    "O": [0x3e, 0x41, 0x41, 0x41, 0x3e], "P": [0x7f, 0x09, 0x09, 0x09, 0x06],
    "Q": [0x3e, 0x41, 0x51, 0x21, 0x5e], "R": [0x7f, 0x09, 0x19, 0x29, 0x46],
    "S": [0x46, 0x49, 0x49, 0x49, 0x31], "T": [0x01, 0x01, 0x7f, 0x01, 0x01],
    "U": [0x3f, 0x40, 0x40, 0x40, 0x3f], "V": [0x1f, 0x20, 0x40, 0x20, 0x1f],
    "W": [0x7f, 0x20, 0x18, 0x20, 0x7f], "X": [0x63, 0x14, 0x08, 0x14, 0x63],
    "Y": [0x03, 0x04, 0x78, 0x04, 0x03], "Z": [0x61, 0x51, 0x49, 0x45, 0x43],
};

MikroMK3.glyph = function(ch) {
    return this.FONT[ch] || this.FONT["?"];
};
