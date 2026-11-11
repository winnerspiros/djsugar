// ReloopMixTourPro-script.js
// Mixxx controller mapping script for the Reloop Mixtour Pro (4-deck).
// See docs/superpowers/specs/2026-06-02-reloop-mixtour-pro-mapping-design.md

var ReloopMixTourPro = {};

// ---- Config (tune these live; see spec §11) -------------------------------
ReloopMixTourPro.MTP = {
    decks: 4,
    // Status base per message kind; channel offset = deckIndex (0..3)
    NOTE_BASE: 0x90,   // 0x90..0x93 buttons
    CC_BASE:   0xB0,   // 0xB0..0xB3 CCs
    PAD_BASE:  0x94,   // 0x94..0x97 pads/load
    GLOBAL_NOTE: 0x9F, // shift / split / library / fx -/+
    GLOBAL_CC:   0xBF, // crossfader / master / browse / fx wet-dry

    OFF: 0x00,
    // Button LEDs are bi-color red/blue (value = hue + brightness; higher = brighter).
    // Captured: blue dim=2 bright=6, red dim=1 bright=5 (see docs/.../led-palette-capture.md)
    BTN_BLUE_DIM: 2, BTN_BLUE: 6,
    BTN_RED_DIM:  1, BTN_RED:  5,
    // Pad LEDs are full RGB; value = hue, +64 = same hue brighter (captured live).
    COLORS: {
        off:     { ambient: 0,  bright: 0  },
        blue:    { ambient: 3,  bright: 67 },
        green:   { ambient: 4,  bright: 68 },
        cyan:    { ambient: 5,  bright: 69 },
        red:     { ambient: 16, bright: 80 },
        magenta: { ambient: 17, bright: 81 },
        violet:  { ambient: 19, bright: 83 },
        orange:  { ambient: 20, bright: 84 },
        pink:    { ambient: 37, bright: 101 },
        yellow:  { ambient: 24, bright: 88 },
        white:   { ambient: 25, bright: 89 },
    },

    // Blink periods in ms (tune live)
    BLINK_SLOW: 700, BLINK_FAST: 300,

    // Notes
    N_PLAY: 0x00, N_CUE: 0x01, N_SYNC: 0x02, N_INOUT: 0x03,
    N_PFL: 0x1B, N_SELECTOR: 0x08, N_LOAD: 0x0A,
    N_PAD: [0x14, 0x15, 0x16, 0x17], // 4 rows; column = pad channel
    N_FILTER_LED: 0x24,
    N_SHIFT: 0x00, N_SPLIT: 0x09,      // on GLOBAL_NOTE (0x9F)
    PADDLE_BASE: 0x98, N_PADDLE: 0x05, // FX paddles: 0x98-0x9B note 0x05
    N_VU: 0x1F,                        // VU meter CC on 0xB0-0xB3 (from djay capture)
    FILTER_EPS: 0.02,                  // center deadzone for filter-engaged LED
    INSTANT_FX_SUPER: 0.5,             // QuickEffect super1 level set when an Instant FX pad is held (0=dry .. 1=full); tune to taste
};

ReloopMixTourPro.state = {
    shift: false,
    split: false,
    activeDeck: 0,                      // 0-3: pad target when not split (from mode tap)
    sideDeck: { left: 0, right: 1 },    // virtual deck per side (left∈{0,2}, right∈{1,3})
    // padMode index = controller's mode note: 0=HotCue 1=BounceLoop 2=PitchCue
    // 3=InstantFX 4=AutoLoop 5=Sampler 6=SavedLoops 7=NeuralMix
    padMode: [0, 0, 0, 0],
    loopState: [0, 0, 0, 0],            // per deck: 0 none, 1 armed, 2 looping
    instantFx: [-1, -1, -1, -1],        // per deck: held InstantFX slot (-1 = none)
    blinkSlow: false,
    blinkFast: false,
};

// deckIndex (0-based) -> Mixxx group
ReloopMixTourPro.groupForDeck = function(deckIndex) {
    return "[Channel" + (deckIndex + 1) + "]";
};

// Send a 3-byte MIDI message to a deck's button/pad LED — only when the value
// actually changed (per-LED cache keeps the wire quiet at idle, like djay).
// statusBase: MTP.NOTE_BASE or MTP.PAD_BASE; value: brightness or color byte.
ReloopMixTourPro.ledCache = {};
ReloopMixTourPro.sendLED = function(statusBase, deckIndex, note, value) {
    var status = statusBase + deckIndex;
    var key = (status << 8) | note;
    if (ReloopMixTourPro.ledCache[key] === value) { return; }
    ReloopMixTourPro.ledCache[key] = value;
    midi.sendShortMsg(status, note, value);
};

// Deck button color (bi-color): blue for decks 1/2 (index 0,1), red for decks 3/4.
ReloopMixTourPro.deckBtnColor = function(deckIndex, bright) {
    var m = ReloopMixTourPro.MTP;
    if (deckIndex < 2) { return bright ? m.BTN_BLUE : m.BTN_BLUE_DIM; }
    return bright ? m.BTN_RED : m.BTN_RED_DIM;
};

// ---- Input handlers -------------------------------------------------------

// SHIFT — 9F 00
ReloopMixTourPro.shiftButton = function(_ch, _ctrl, value) {
    ReloopMixTourPro.state.shift = (value > 0);
};

// Side selector held — 94 08 (left) / 95 08 (right). Press selects that side
// (split off) and marks it held (MODE modifier). Release clears held.
// Mode-button tap (radio): 94/95/96/97 08 -> active deck for the side. Note=0x08.
ReloopMixTourPro.selector = function(_ch, _ctrl, value, status) {
    if (value === 0) { return; }   // radio: act on activate only
    var deckIndex = status - ReloopMixTourPro.MTP.PAD_BASE; // 94-97 -> 0-3
    var side = (deckIndex % 2 === 0) ? "left" : "right";
    ReloopMixTourPro.state.activeDeck = deckIndex;
    ReloopMixTourPro.state.sideDeck[side] = deckIndex;
    ReloopMixTourPro.state.split = false;
    console.log("active deck = " + (deckIndex + 1));
    ReloopMixTourPro.renderAllPads();
};

// Mode hold + pad: 94/95/96/97 0N -> set that deck's pad mode (N = mode note 0-7).
ReloopMixTourPro.padModeSelect = function(_ch, control, value, status) {
    if (value === 0) { return; }
    var deckIndex = status - ReloopMixTourPro.MTP.PAD_BASE; // 94-97 -> 0-3
    ReloopMixTourPro.state.padMode[deckIndex] = control;    // note 0-7 = mode
    console.log("padMode deck" + (deckIndex + 1) + " = " + control);
    ReloopMixTourPro.renderAllPads();
    ReloopMixTourPro.renderModePicker();                    // keep picker highlight in sync
};

// Mode hold + transport: 90-93 27/28/2A/2B (channel=deck). SHIFT adds the alt action.
// The controller encodes mode (and shift) into the note itself, so map notes directly.
ReloopMixTourPro.modeTransport = function(_ch, control, value, _status, group) {
    if (value === 0) { return; }
    switch (control) {
        case 0x2A: engine.setValue(group, "loop_halve", 1); break;   // mode + IN-OUT
        case 0x2B: engine.setValue(group, "loop_double", 1); break;  // mode + SYNC
        case 0x2C: engine.setValue(group, "loop_in", 1); break;      // shift + mode + IN-OUT
        case 0x2E: engine.setValue(group, "loop_out", 1); break;     // shift + mode + SYNC
        case 0x2F: ReloopMixTourPro.tempoNudge(group, -0.1); break;  // shift + mode + CUE
        case 0x30: ReloopMixTourPro.tempoNudge(group, 0.1); break;   // shift + mode + PLAY
        case 0x27: engine.setValue(group, "pitch_down", 1); break;   // mode + CUE: key down
        case 0x28: engine.setValue(group, "pitch_up", 1); break;     // mode + PLAY: key up
        default: break;  // 0x2D goto-start: unused
    }
};

// Shift-layer transport (the controller encodes shift into these notes, per the manual):
// 0x0B shift+PLAY = bend up, 0x0C shift+CUE = bend down, 0x29 shift+SYNC = key match,
// 0x40 shift+IN-OUT = auto loop on/off.
ReloopMixTourPro.shiftTransport = function(_ch, control, value, _status, group) {
    switch (control) {
        case 0x0B: engine.setValue(group, "rate_temp_up", value > 0 ? 1 : 0); break;
        case 0x0C: engine.setValue(group, "rate_temp_down", value > 0 ? 1 : 0); break;
        case 0x29: if (value > 0) { engine.setValue(group, "sync_key", 1); } break; // shift+SYNC: harmonic key match
        case 0x40:
            if (value > 0) {
                if (engine.getValue(group, "loop_enabled") > 0) { engine.setValue(group, "reloop_toggle", 1); }
                else { engine.setValue(group, "beatloop_activate", 1); }
            }
            break;
        default: break;
    }
};

// SPLIT — 9F 09
ReloopMixTourPro.splitButton = function(_ch, _ctrl, value) {
    if (value > 0) {
        ReloopMixTourPro.state.split = true;
        console.log("split on");
        ReloopMixTourPro.renderAllPads();
    }
};

// Deck-switch notification — 90/91/92/93 08 (value 7F = this deck now active on its side)
ReloopMixTourPro.deckSwitch = function(_ch, _ctrl, value, status) {
    if (value === 0) { return; }
    var deckIndex = status - ReloopMixTourPro.MTP.NOTE_BASE; // 0..3
    var side = (deckIndex % 2 === 0) ? "left" : "right";     // 0,2 left ; 1,3 right
    ReloopMixTourPro.state.sideDeck[side] = deckIndex;
    console.log("deckSwitch side=" + side + " deck=" + (deckIndex + 1));
    ReloopMixTourPro.renderAllPads();
};

// ---- Blink timer subsystem -----------------------------------------------

ReloopMixTourPro.blinkTimers = [];

ReloopMixTourPro.startBlink = function() {
    var self = ReloopMixTourPro;
    self.blinkTimers.push(engine.beginTimer(self.MTP.BLINK_SLOW, function() {
        self.state.blinkSlow = !self.state.blinkSlow;
        self.renderAllTransportLEDs();
        self.renderAllPads();                  // animates saved-loop "blink while playing"
    }));
    self.blinkTimers.push(engine.beginTimer(self.MTP.BLINK_FAST, function() {
        self.state.blinkFast = !self.state.blinkFast;
        self.renderAllTransportLEDs();
    }));
};

ReloopMixTourPro.renderAllTransportLEDs = function() {
    for (var d = 0; d < ReloopMixTourPro.MTP.decks; d++) {
        ReloopMixTourPro.renderPlayLED(d);
        ReloopMixTourPro.renderCueLED(d);
        ReloopMixTourPro.renderSyncLED(d);
        ReloopMixTourPro.renderLoopLED(d);
    }
};

ReloopMixTourPro.deckIndexForGroup = function(group) {
    return parseInt(group.charAt(8), 10) - 1; // "[ChannelN]" -> N-1
};

// Nudge effective tempo by deltaBpm (e.g. +0.1) by adjusting `rate`.
ReloopMixTourPro.tempoNudge = function(group, deltaBpm) {
    var fileBpm = engine.getValue(group, "file_bpm");
    var rateRange = engine.getValue(group, "rateRange");
    var rateDir = engine.getValue(group, "rate_dir");
    if (fileBpm <= 0 || rateRange <= 0 || rateDir === 0) { return; }
    var dRate = deltaBpm / (fileBpm * rateRange * rateDir);
    engine.setValue(group, "rate", engine.getValue(group, "rate") + dRate);
};

ReloopMixTourPro.loopStep = function(deckIndex, group) {
    var st = ReloopMixTourPro.state;
    var s = st.loopState[deckIndex];
    if (s === 0) {
        engine.setValue(group, "loop_in", 1);
        st.loopState[deckIndex] = 1;
    } else if (s === 1) {
        engine.setValue(group, "loop_out", 1);
        st.loopState[deckIndex] = 2;
    } else {
        if (engine.getValue(group, "loop_enabled") > 0) {
            engine.setValue(group, "reloop_toggle", 1);
        }
        st.loopState[deckIndex] = 0;
    }
    ReloopMixTourPro.renderLoopLED(deckIndex);
};

ReloopMixTourPro.renderLoopLED = function(deckIndex) {
    var m = ReloopMixTourPro.MTP, st = ReloopMixTourPro.state;
    var dim = ReloopMixTourPro.deckBtnColor(deckIndex, false);
    var bright = ReloopMixTourPro.deckBtnColor(deckIndex, true);
    var s = st.loopState[deckIndex];
    var v;
    if (s === 0) { v = dim; }
    else if (s === 1) { v = st.blinkFast ? bright : dim; }
    else { v = bright; }
    ReloopMixTourPro.sendLED(m.NOTE_BASE, deckIndex, m.N_INOUT, v);
};

// ---- Transport handlers --------------------------------------------------

ReloopMixTourPro.playButton = function(_ch, _ctrl, value, _status, group) {
    if (value === 0) { return; }
    script.toggleControl(group, "play");
};

ReloopMixTourPro.cueButton = function(_ch, _ctrl, value, _status, group) {
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

ReloopMixTourPro.syncButton = function(_ch, _ctrl, value, _status, group) {
    if (value === 0) { return; }
    script.toggleControl(group, "sync_enabled");
};

// ---- Play LED render + cue/sync LED connections --------------------------

ReloopMixTourPro.renderPlayLED = function(deckIndex) {
    var g = ReloopMixTourPro.groupForDeck(deckIndex);
    var m = ReloopMixTourPro.MTP, st = ReloopMixTourPro.state;
    var loaded = engine.getValue(g, "track_loaded") > 0;
    var playing = engine.getValue(g, "play") > 0;
    var bright = ReloopMixTourPro.deckBtnColor(deckIndex, true);
    var dim = ReloopMixTourPro.deckBtnColor(deckIndex, false);
    var v = dim;                                            // idle: dim backlight
    if (playing) { v = bright; }
    else if (loaded) { v = st.blinkSlow ? bright : dim; }   // paused: pulse dim<->bright
    ReloopMixTourPro.sendLED(m.NOTE_BASE, deckIndex, m.N_PLAY, v);
};

// Cue LED: bright when cue_indicator on (blinks per Mixxx when paused off-cue), dim at rest.
ReloopMixTourPro.renderCueLED = function(deckIndex) {
    var m = ReloopMixTourPro.MTP, g = ReloopMixTourPro.groupForDeck(deckIndex);
    var on = engine.getValue(g, "cue_indicator") > 0;
    ReloopMixTourPro.sendLED(m.NOTE_BASE, deckIndex, m.N_CUE, ReloopMixTourPro.deckBtnColor(deckIndex, on));
};

// Sync LED: bright when sync enabled, dim at rest.
ReloopMixTourPro.renderSyncLED = function(deckIndex) {
    var m = ReloopMixTourPro.MTP, g = ReloopMixTourPro.groupForDeck(deckIndex);
    var on = engine.getValue(g, "sync_enabled") > 0;
    ReloopMixTourPro.sendLED(m.NOTE_BASE, deckIndex, m.N_SYNC, ReloopMixTourPro.deckBtnColor(deckIndex, on));
};

ReloopMixTourPro.connectTransportLEDs = function() {
    var self = ReloopMixTourPro, m = self.MTP;
    for (var d = 0; d < m.decks; d++) {
        (function(deckIndex) {
            var g = self.groupForDeck(deckIndex);
            engine.makeConnection(g, "play", function() { self.renderPlayLED(deckIndex); });
            engine.makeConnection(g, "track_loaded", function() { self.renderPlayLED(deckIndex); });
            engine.makeConnection(g, "cue_indicator", function() { self.renderCueLED(deckIndex); });
            engine.makeConnection(g, "sync_enabled", function() { self.renderSyncLED(deckIndex); });
        })(d);
    }
};

ReloopMixTourPro.connectLoopLEDs = function() {
    var self = ReloopMixTourPro, m = self.MTP;
    for (var d = 0; d < m.decks; d++) {
        (function(deckIndex) {
            engine.makeConnection(self.groupForDeck(deckIndex), "loop_enabled", function(v) {
                var st = self.state;
                if (v > 0) { st.loopState[deckIndex] = 2; }
                else if (st.loopState[deckIndex] === 2) { st.loopState[deckIndex] = 0; }
                self.renderLoopLED(deckIndex);
            });
        })(d);
    }
};

ReloopMixTourPro.inOutButton = function(_ch, _ctrl, value, _status, group) {
    if (value === 0) { return; }
    ReloopMixTourPro.loopStep(ReloopMixTourPro.deckIndexForGroup(group), group);
};

// ---- Pad system ----------------------------------------------------------

// Auto Loop: pads 1-8 (slots 0-7) increasing size. Pads 1-4 = left col, 5-8 = right col.
ReloopMixTourPro.BEATLOOP_SIZES = [0.25, 0.5, 1, 2, 4, 8, 16, 32];
// Bounce Loop (beatjump): 4 sizes per row; left column jumps back, right column forward.
ReloopMixTourPro.BEATJUMP_SIZES = [4, 8, 16, 32];
// Pitch Cue: semitone each pad sets (slot 0 = reset to original key). Tunable.
ReloopMixTourPro.PITCH_SEMITONES = [0, 1, 2, 3, 4, 5, 6, 7];

// Standard hues for mapping a Mixxx hotcue RGB color -> nearest controller color name.
ReloopMixTourPro.PALETTE_RGB = {
    red: 0xFF0000, green: 0x00FF00, blue: 0x0000FF, cyan: 0x00FFFF,
    yellow: 0xFFFF00, magenta: 0xFF00FF, orange: 0xFF8000, pink: 0xFF60B0,
    white: 0xFFFFFF, violet: 0x8000FF
};
ReloopMixTourPro.nearestColorName = function(rgb) {
    var r = (rgb >> 16) & 0xFF, g = (rgb >> 8) & 0xFF, b = rgb & 0xFF;
    if (Math.max(r, g, b) - Math.min(r, g, b) < 12) { return "white"; } // near-gray only
    var best = "red", bestD = 1e12, P = ReloopMixTourPro.PALETTE_RGB;
    for (var name in P) {
        if (name === "white") { continue; }  // colored input -> a saturated hue, not white
        var pr = (P[name] >> 16) & 0xFF, pg = (P[name] >> 8) & 0xFF, pb = P[name] & 0xFF;
        var dd = (r - pr) * (r - pr) + (g - pg) * (g - pg) + (b - pb) * (b - pb);
        if (dd < bestD) { bestD = dd; best = name; }
    }
    return best;
};

// Light a pad LED on its physical channel (PAD_BASE+physDeck), note, color name + bright.
ReloopMixTourPro.sendPad = function(physDeck, note, colorName, bright) {
    var c = ReloopMixTourPro.MTP.COLORS[colorName] || ReloopMixTourPro.MTP.COLORS.off;
    ReloopMixTourPro.sendLED(ReloopMixTourPro.MTP.PAD_BASE, physDeck, note, bright ? c.bright : c.ambient);
};

// Flash a pad bright in colorName, then restore on a one-shot timer (momentary feedback).
ReloopMixTourPro.flashPad = function(deckIndex, slot, colorName) {
    ReloopMixTourPro.sendPad(deckIndex, 0x14 + slot, colorName, true);
    ReloopMixTourPro.sendPad(deckIndex, 0x1C + slot, colorName, true);
    engine.beginTimer(120, function() { ReloopMixTourPro.renderAllPads(); }, true);
};

// Performance pad. The controller addresses pads by channel = deck, note = slot:
// non-split sends all 8 on the active deck's channel (notes 0x14-0x1B); split sends 4 on
// each side's channel (0x14-0x17). SHIFT adds +0x08 to the note (0x1C-0x23).
ReloopMixTourPro.padPress = function(_ch, control, value, status) {
    var shifted = control >= 0x1C;
    var slot = (shifted ? control - 0x1C : control - 0x14); // 0-7
    var deckIndex = status - ReloopMixTourPro.MTP.PAD_BASE;
    ReloopMixTourPro.padAction(ReloopMixTourPro.groupForDeck(deckIndex), deckIndex, slot, value, shifted);
};

ReloopMixTourPro.padAction = function(group, deckIndex, slot, value, shifted) {
    switch (ReloopMixTourPro.state.padMode[deckIndex]) {
        case 0: ReloopMixTourPro.padHotcue(group, slot, value, shifted); break;   // HotCue
        case 1: ReloopMixTourPro.padBeatjump(group, deckIndex, slot, value, shifted); break; // BounceLoop
        case 2: ReloopMixTourPro.padPitchCue(group, slot, value, shifted); break; // PitchCue
        case 3: ReloopMixTourPro.padInstantFX(group, deckIndex, slot, value); break; // InstantFX
        case 4: ReloopMixTourPro.padBeatloop(group, slot, value); break;          // AutoLoop
        case 5: ReloopMixTourPro.padSampler(slot, value, shifted); break;         // Sampler
        case 6: ReloopMixTourPro.padSavedLoop(group, slot, value, shifted); break; // SavedLoops
        default: break; // 7 NeuralMix (stub)
    }
};

// Instant FX: hold pad N = switch this deck's QuickEffect to chain preset N+2 (instant
// effect while held); release = back to preset 1 (the normal filter). The 8 effects are
// the QuickEffect chain presets 2-9 (Preferences -> Effects).
ReloopMixTourPro.instantFxPrevSuper = [0.5, 0.5, 0.5, 0.5];
ReloopMixTourPro.instantFxPrevPreset = [1, 1, 1, 1];

ReloopMixTourPro.padInstantFX = function(group, deckIndex, slot, value) {
    var qe = "[QuickEffectRack1_" + group + "]";
    var st = ReloopMixTourPro.state;
    if (value > 0) {
        if (st.instantFx[deckIndex] === -1) {  // don't overwrite while rolling between pads
            ReloopMixTourPro.instantFxPrevSuper[deckIndex] = engine.getValue(qe, "super1");
            ReloopMixTourPro.instantFxPrevPreset[deckIndex] = engine.getValue(qe, "loaded_chain_preset");
        }
        engine.setValue(qe, "loaded_chain_preset", slot + 2);
        engine.setValue(qe, "super1", ReloopMixTourPro.MTP.INSTANT_FX_SUPER); // intensity (tunable)
        engine.setValue(qe, "enabled", 1);
        st.instantFx[deckIndex] = slot;
    } else {
        engine.setValue(qe, "loaded_chain_preset", ReloopMixTourPro.instantFxPrevPreset[deckIndex] || 1);
        engine.setValue(qe, "super1", ReloopMixTourPro.instantFxPrevSuper[deckIndex]);
        st.instantFx[deckIndex] = -1;
    }
    ReloopMixTourPro.renderAllPads();
};

ReloopMixTourPro.padHotcue = function(group, slot, value, shifted) {
    var n = slot + 1;
    if (value > 0) {
        engine.setValue(group, shifted ? ("hotcue_" + n + "_clear") : ("hotcue_" + n + "_activate"), 1);
    } else {
        engine.setValue(group, "hotcue_" + n + "_activate", 0);
    }
};

ReloopMixTourPro.padBeatloop = function(group, slot, value) {
    if (value === 0) { return; }
    engine.setValue(group, "beatloop_" + ReloopMixTourPro.BEATLOOP_SIZES[slot] + "_toggle", 1);
};

ReloopMixTourPro.padBeatjump = function(group, deckIndex, slot, value, shifted) {
    if (value === 0) { return; }
    var backward = (slot < 4);              // left column = backward, right column = forward
    if (shifted) { backward = !backward; }  // shift inverts direction
    var size = ReloopMixTourPro.BEATJUMP_SIZES[slot % 4];
    engine.setValue(group, "beatjump_" + size + "_" + (backward ? "backward" : "forward"), 1);
    ReloopMixTourPro.flashPad(deckIndex, slot, slot < 4 ? "orange" : "green");
};

ReloopMixTourPro.padSampler = function(slot, value, shifted) {
    if (value === 0) { return; }
    var sg = "[Sampler" + (slot + 1) + "]";
    if (shifted) {
        if (engine.getValue(sg, "play") > 0) {
            engine.setValue(sg, "cue_gotoandstop", 1);      // playing: stop + back to start
        } else if (engine.getValue(sg, "track_loaded") > 0) {
            engine.setValue(sg, "eject", 1);                // stopped: eject the sample
            engine.setValue(sg, "eject", 0);
        }
        return;
    }
    if (engine.getValue(sg, "track_loaded") > 0) { engine.setValue(sg, "cue_gotoandplay", 1); }
    else { engine.setValue(sg, "LoadSelectedTrack", 1); }
};

// Pitch Cue: set the musical key (pitch in semitones); pad 1 resets to original.
// Shift + pad jumps to hot cue 1-8 (switch between cue points).
ReloopMixTourPro.padPitchCue = function(group, slot, value, shifted) {
    if (value === 0) { return; }
    if (shifted) {
        engine.setValue(group, "hotcue_" + (slot + 1) + "_activate", 1);
    } else {
        engine.setValue(group, "pitch", ReloopMixTourPro.PITCH_SEMITONES[slot]);
    }
};

// Saved Loops: a bank of 8 hot loops (loop-type hotcues). Pad saves the current loop to
// an empty slot, or recalls (jumps to + enables) a saved one. Shift + pad clears the slot.
ReloopMixTourPro.padSavedLoop = function(group, slot, value, shifted) {
    if (value === 0) { return; }
    var n = slot + 1;
    if (shifted) {
        engine.setValue(group, "hotcue_" + n + "_clear", 1);
    } else if (engine.getValue(group, "hotcue_" + n + "_status") > 0) {
        engine.setValue(group, "hotcue_" + n + "_activate", 1);   // recall saved loop
    } else {
        engine.setValue(group, "hotcue_" + n + "_setloop", 1);    // save current loop
    }
};

// Color for one logical slot (0-7) of a deck under its current pad mode.
ReloopMixTourPro.padColorForSlot = function(deckIndex, slot) {
    var g = ReloopMixTourPro.groupForDeck(deckIndex);
    var mode = ReloopMixTourPro.state.padMode[deckIndex];
    if (mode === 0) {                                   // HotCue: cue color if set, else off
        var n = slot + 1;
        if (engine.getValue(g, "hotcue_" + n + "_status") > 0) {
            var rgb = engine.getValue(g, "hotcue_" + n + "_color");
            return { color: ReloopMixTourPro.nearestColorName(rgb), bright: true };
        }
        return { color: "off", bright: false };
    }
    if (mode === 4) {                                   // AutoLoop: ambient blue, bright if active
        var on = engine.getValue(g, "beatloop_" + ReloopMixTourPro.BEATLOOP_SIZES[slot] + "_enabled") > 0;
        return { color: "blue", bright: on };
    }
    if (mode === 1) { return { color: (slot < 4 ? "orange" : "green"), bright: false }; } // BounceLoop: back=orange, fwd=green
    if (mode === 5) {                                   // Sampler: pink (bright while playing)
        var sg = "[Sampler" + (slot + 1) + "]";
        if (engine.getValue(sg, "track_loaded") <= 0) { return { color: "off", bright: false }; }
        return { color: "pink", bright: engine.getValue(sg, "play") > 0 };
    }
    if (mode === 2) {                                   // PitchCue: pad 0 = reset (white), rest red
        if (slot === 0) { return { color: "white", bright: true }; }
        var cur = Math.round(engine.getValue(g, "pitch"));
        return { color: "red", bright: (cur === ReloopMixTourPro.PITCH_SEMITONES[slot]) };
    }
    if (mode === 3) { return { color: "green", bright: ReloopMixTourPro.state.instantFx[deckIndex] === slot }; } // InstantFX
    if (mode === 6) {                                   // SavedLoops: ambient if empty, bright if saved, blink while active
        var hn = slot + 1;
        if (engine.getValue(g, "hotcue_" + hn + "_status") <= 0) { return { color: "blue", bright: false }; }
        if (engine.getValue(g, "hotcue_" + hn + "_enabled") > 0) { return { color: "blue", bright: ReloopMixTourPro.state.blinkSlow }; }
        return { color: "blue", bright: true };
    }
    if (mode === 7) { return { color: (slot < 4 ? "yellow" : "white"), bright: (slot < 4) }; } // NeuralMix (stub)
    return { color: "off", bright: false };
};

// Light `count` pads of a deck on its own channel (notes 0x14..0x14+count-1).
ReloopMixTourPro.renderPadGroup = function(deckIndex, count) {
    for (var slot = 0; slot < count; slot++) {
        var c = ReloopMixTourPro.padColorForSlot(deckIndex, slot);
        ReloopMixTourPro.sendPad(deckIndex, 0x14 + slot, c.color, c.bright);  // normal layer
        ReloopMixTourPro.sendPad(deckIndex, 0x1C + slot, c.color, c.bright);  // shift layer (stays lit while SHIFT held)
    }
};

// Non-split: 8 pads on the active deck's channel. Split: 4 on each side's channel.
ReloopMixTourPro.renderAllPads = function() {
    var st = ReloopMixTourPro.state;
    if (st.split) {
        ReloopMixTourPro.renderPadGroup(st.sideDeck.left, 4);   // left side: slots 0-3
        ReloopMixTourPro.renderPadGroup(st.sideDeck.right, 4);  // right side: slots 0-3
    } else {
        ReloopMixTourPro.renderPadGroup(st.activeDeck, 8);      // active deck: slots 0-7
    }
};

// Mode-hold transport layer LEDs (as djay does): combos dim violet, loop in/out bright.
ReloopMixTourPro.renderModeLayerLEDs = function() {
    var m = ReloopMixTourPro.MTP;
    for (var d = 0; d < m.decks; d++) {
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x2A, 3); // mode+IN-OUT  (loop halve)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x2B, 3); // mode+SYNC    (loop double)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x27, 3); // mode+CUE
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x28, 3); // mode+PLAY
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x2C, 7); // shift+mode+IN-OUT (loop in)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x2E, 7); // shift+mode+SYNC   (loop out)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x2F, 3); // shift+mode+CUE  (tempo -)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x30, 3); // shift+mode+PLAY (tempo +)
        var dim = ReloopMixTourPro.deckBtnColor(d, false);
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x0B, dim); // shift+PLAY (bend up)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x0C, dim); // shift+CUE  (bend down)
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, 0x40, dim); // shift+IN-OUT (auto loop)
        ReloopMixTourPro.sendLED(m.PAD_BASE, d, 0x24, 1);    // Neural Mix button idle (dim, as djay)
    }
};

// Mode-picker highlight (shown by the controller while a mode button is held):
// active mode note = 0x7F, inactive = 0x3F (djay's values), per deck pad channel.
ReloopMixTourPro.renderModePicker = function() {
    var m = ReloopMixTourPro.MTP, st = ReloopMixTourPro.state;
    for (var d = 0; d < m.decks; d++) {
        for (var mode = 0; mode < 8; mode++) {
            ReloopMixTourPro.sendLED(m.PAD_BASE, d, mode, (st.padMode[d] === mode) ? 0x7F : 0x3F);
        }
    }
};

ReloopMixTourPro.connectPadLEDs = function() {
    var self = ReloopMixTourPro;
    var rerender = function() { self.renderAllPads(); };
    for (var d = 0; d < self.MTP.decks; d++) {
        var g = self.groupForDeck(d);
        for (var n = 1; n <= 8; n++) {
            engine.makeConnection(g, "hotcue_" + n + "_status", rerender);
            engine.makeConnection(g, "hotcue_" + n + "_color", rerender);
            engine.makeConnection(g, "hotcue_" + n + "_enabled", rerender); // saved-loop active state
        }
        engine.makeConnection(g, "loop_enabled", rerender);
        engine.makeConnection(g, "pitch", rerender);                       // Pitch Cue active pad
        for (var bi = 0; bi < self.BEATLOOP_SIZES.length; bi++) {
            engine.makeConnection(g, "beatloop_" + self.BEATLOOP_SIZES[bi] + "_enabled", rerender);
        }
    }
    for (var s = 1; s <= 8; s++) {
        engine.makeConnection("[Sampler" + s + "]", "play", rerender);
        engine.makeConnection("[Sampler" + s + "]", "track_loaded", rerender);
    }
};

// ---- FX paddles, filter LED, VU meters (Milestone C) ----------------------

// FX paddle (98-9B 05). 3-position lever: up latches, down is spring-loaded; both send
// 7F on engage / 00 on release, so we just mirror the message. SHIFT + paddle = backspin.
ReloopMixTourPro.spinbackActive = [false, false, false, false];

ReloopMixTourPro.paddle = function(_ch, control, value, status, _group) {
    var deckIndex = status - ReloopMixTourPro.MTP.PADDLE_BASE; // 0-3
    var group = ReloopMixTourPro.groupForDeck(deckIndex);
    if (value > 0) {
        if (control === 0x00) {                                // shift+paddle: backspin
            engine.spinback(deckIndex + 1, true);
            ReloopMixTourPro.spinbackActive[deckIndex] = true;
        } else {                                               // 0x05: FX engage
            engine.setValue("[EffectRack1_EffectUnit1_Effect1]", "enabled", 1);
            engine.setValue("[EffectRack1_EffectUnit1]", "group_" + group + "_enable", 1);
        }
        return;
    }
    // Release — may arrive on the *other* note if shift changed mid-gesture, so end
    // everything: disengage FX and stop any active spinback (prevents a stuck deck).
    engine.setValue("[EffectRack1_EffectUnit1]", "group_" + group + "_enable", 0);
    if (ReloopMixTourPro.spinbackActive[deckIndex]) {
        engine.spinback(deckIndex + 1, false);
        ReloopMixTourPro.spinbackActive[deckIndex] = false;
    }
};

// FX param buttons (global 9F 03/04): step the unit's super knob down/up.
ReloopMixTourPro.fxParam = function(_ch, control, value) {
    if (value === 0) { return; }
    var unit = "[EffectRack1_EffectUnit1]";
    var v = engine.getValue(unit, "super1");
    engine.setValue(unit, "super1", control === 0x03 ? Math.max(0, v - 0.05) : Math.min(1, v + 0.05));
};

// shift + FX param buttons arrive as 98-9B 0B/0C: select prev/next effect in slot 1.
ReloopMixTourPro.fxSelect = function(_ch, control, value) {
    if (value === 0) { return; }
    engine.setValue("[EffectRack1_EffectUnit1_Effect1]", "effect_selector", control === 0x0C ? 1 : -1);
};

// Mode + browse-encoder turn (B0-B3 06): beat-snapped seek — 4 beats (one bar) per
// click via beatjump, so the needle stays locked to the beatgrid.
ReloopMixTourPro.modeBrowseSeek = function(_ch, _ctrl, value, _status, group) {
    var delta = value - 64;                  // relative encoder, 64-centered
    if (delta === 0) { return; }
    var dir = delta > 0 ? "forward" : "backward";
    var steps = Math.abs(delta);
    for (var i = 0; i < steps; i++) {
        engine.setValue(group, "beatjump_4_" + dir, 1);
    }
};

// Paddle LED: dim at rest (djay uses 01), bright while the FX unit is enabled for the deck.
ReloopMixTourPro.renderPaddleLED = function(deckIndex) {
    var m = ReloopMixTourPro.MTP;
    var on = engine.getValue("[EffectRack1_EffectUnit1]",
        "group_" + ReloopMixTourPro.groupForDeck(deckIndex) + "_enable") > 0;
    ReloopMixTourPro.sendLED(m.PADDLE_BASE, deckIndex, m.N_PADDLE, on ? 5 : 1);
};

ReloopMixTourPro.connectFX = function() {
    var self = ReloopMixTourPro;
    for (var d = 0; d < self.MTP.decks; d++) {
        (function(deckIndex) {
            engine.makeConnection("[EffectRack1_EffectUnit1]",
                "group_" + self.groupForDeck(deckIndex) + "_enable",
                function() { self.renderPaddleLED(deckIndex); });
        })(d);
    }
};

// Filter-engaged LED: address still unknown — 0x24 turned out to be the Neural Mix
// button (djay toggleUnmixerEQMode), which we now just idle dim. Re-enable this once
// the real filter LED address is captured (spy djay while turning the filter knob).
ReloopMixTourPro.renderFilterLED = function(_deckIndex) {};

ReloopMixTourPro.connectFilterLEDs = function() {
    var self = ReloopMixTourPro;
    for (var d = 0; d < self.MTP.decks; d++) {
        (function(deckIndex) {
            engine.makeConnection("[QuickEffectRack1_" + self.groupForDeck(deckIndex) + "]",
                "super1", function() { self.renderFilterLED(deckIndex); });
        })(d);
    }
};

// VU meters: CC 0x1F per deck channel; 6 segments (5 green + 1 red). Level fills the
// greens (0-5); the red top segment lights only on actual clipping (peak_indicator).
ReloopMixTourPro.connectVU = function() {
    var self = ReloopMixTourPro, m = self.MTP;
    for (var d = 0; d < m.decks; d++) {
        (function(deckIndex) {
            var g = self.groupForDeck(deckIndex);
            var send = function() {
                var peak = engine.getValue(g, "peak_indicator") > 0;
                var v = engine.getValue(g, "vu_meter");
                self.sendLED(m.CC_BASE, deckIndex, m.N_VU, peak ? 6 : Math.round(v * 5));
            };
            engine.makeConnection(g, "vu_meter", send);
            engine.makeConnection(g, "peak_indicator", send);
        })(d);
    }
};

// ---- Lifecycle ------------------------------------------------------------

ReloopMixTourPro.init = function(_id, _debug) {
    console.log("ReloopMixTourPro: init");
    ReloopMixTourPro.connectTransportLEDs();
    ReloopMixTourPro.connectLoopLEDs();
    ReloopMixTourPro.connectPadLEDs();
    ReloopMixTourPro.connectFX();
    ReloopMixTourPro.connectFilterLEDs();
    ReloopMixTourPro.connectVU();
    ReloopMixTourPro.startBlink();
    // Reset every deck's QuickEffect to preset #1 (keep the Filter first in
    // Preferences -> Effects; instant-FX pads use presets 2-9).
    for (var q = 0; q < ReloopMixTourPro.MTP.decks; q++) {
        engine.setValue("[QuickEffectRack1_" + ReloopMixTourPro.groupForDeck(q) + "]", "loaded_chain_preset", 1);
    }
    ReloopMixTourPro.renderAllTransportLEDs();
    ReloopMixTourPro.renderAllPads();
    ReloopMixTourPro.renderModeLayerLEDs();
    for (var d = 0; d < ReloopMixTourPro.MTP.decks; d++) {
        ReloopMixTourPro.renderPaddleLED(d);
        ReloopMixTourPro.renderFilterLED(d);
    }
    // Enable the controller's advanced ("Serato") mode — replicates djay's startup
    // (captured live). Without this, the mode macro / picker / combo notes are inert
    // after a power-cycle.
    midi.sendSysexMsg([0xF0, 0x00, 0x20, 0x7F, 0x00, 0xF7], 6);
    midi.sendSysexMsg([0xF0, 0x00, 0x20, 0x7F, 0x19, 0xF7], 6);
    ReloopMixTourPro.renderModePicker();
};

ReloopMixTourPro.shutdown = function() {
    var m = ReloopMixTourPro.MTP;
    for (var i = 0; i < ReloopMixTourPro.blinkTimers.length; i++) {
        engine.stopTimer(ReloopMixTourPro.blinkTimers[i]);
    }
    for (var d = 0; d < m.decks; d++) {
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, m.N_PLAY, m.OFF);
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, m.N_CUE, m.OFF);
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, m.N_SYNC, m.OFF);
        ReloopMixTourPro.sendLED(m.NOTE_BASE, d, m.N_INOUT, m.OFF);
        for (var r = 0; r < 16; r++) { ReloopMixTourPro.sendPad(d, 0x14 + r, "off", false); }
        var comboNotes = [0x27, 0x28, 0x2A, 0x2B, 0x2C, 0x2E, 0x2F, 0x30];
        for (var c2 = 0; c2 < comboNotes.length; c2++) {
            ReloopMixTourPro.sendLED(m.NOTE_BASE, d, comboNotes[c2], m.OFF);
        }
        for (var mi = 0; mi < 8; mi++) { ReloopMixTourPro.sendLED(m.PAD_BASE, d, mi, 0x00); }
        ReloopMixTourPro.sendLED(m.PADDLE_BASE, d, m.N_PADDLE, m.OFF);
        ReloopMixTourPro.sendLED(m.PAD_BASE, d, m.N_FILTER_LED, m.OFF);
        ReloopMixTourPro.sendLED(m.CC_BASE, d, m.N_VU, 0x00);
    }
};
