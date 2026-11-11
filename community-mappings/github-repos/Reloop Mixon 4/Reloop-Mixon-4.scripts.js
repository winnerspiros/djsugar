// Reloop Mixon 4 controller mapping for Mixxx 2.5+
//
// Repository: https://github.com/daTobi1/mixxx-integration
// MIDI reference: docs/hardware/midi-map.md (derived from the official
// Reloop-Mixon4_MIDI-Map.pdf)
//
// Layer overview (see docs/architecture.md):
//   1. Mixon4.MIDI   - addresses and channels from the official MIDI map
//   2. Components    - input handling (decks, mixer, master)
//   3. LED engine    - output handling (cached sends, VU meters)
//   4. Mode manager  - shift/deck/pad-mode state (added from phase 3 onwards)

// eslint-disable-next-line no-var
var Mixon4 = {};

// Tunables ------------------------------------------------------------------

Mixon4.OPTIONS = {
    // true = lock the hardware to decks 1/2 (sends the deck restriction
    // SysEx at startup, the deck toggle buttons then do nothing)
    twoDeckMode: false,
};

Mixon4.JOG = {
    // Encoder ticks per full wheel revolution. Not documented by Reloop;
    // adjust if one physical revolution does not equal one turntable
    // revolution while scratching.
    wheelResolution: 720,
    // Scratch filter parameters (Mixxx defaults for 155 mm jogs)
    alpha: 1 / 8,
    beta: 1 / 8 / 32,
    rpm: 33 + 1 / 3,
};

// --------------------------------------------------------------------------
// Layer 1: MIDI specification
// --------------------------------------------------------------------------

Mixon4.MIDI = {
    // Channel layout (0-based MIDI channels). The hardware deck toggle
    // switches each side between channels (left: 4<->6, right: 5<->7), so a
    // static channel-to-deck mapping covers all four decks.
    DECK_CHANNELS: [0x04, 0x05, 0x06, 0x07], // deck 1-4 control surface
    PAD_CHANNELS: [0x00, 0x01, 0x02, 0x03], // deck 1-4 performance pads
    FX_CHANNELS: [0x08, 0x09], // FX unit 1 / 2
    GLOBAL_CHANNEL: 0x0A, // mixer master, navigation, shift

    // Status nibbles
    NOTE_ON: 0x90,
    CC: 0xB0,

    // Notes with a controllable LED on each deck channel (base + shift layer).
    // Mode buttons (hot cue, sampler, ...) have hardware-controlled LEDs and
    // are intentionally absent.
    DECK_LED_NOTES: [
        0x02, // LOAD
        0x03, // CUE (PFL)
        0x08, // AUTO LOOP
        0x0A, 0x0B, // PARAM - / +
        0x0C, 0x0D, 0x0E, // PLAY, CUE, SYNC
        0x0F, 0x10, 0x11, // SLIP, KEY LOCK, KEY SYNC
        0x15, // PITCH LED
        0x4A, 0x4B, // shift: PARAM - / +
        0x4C, 0x4D, 0x4E, // shift: PLAY, CUE, SYNC
        0x50, 0x51, // shift: KEY LOCK, KEY SYNC
    ],
    LOOP_LED_NOTE: 0x09, // deck channels; values 0x00-0x0C
    JOG_LED_CC: 0x06, // deck channels; 0x00 clears the ring
    VU_DECK_CC: 0x1E, // deck channels; 0 clears the channel meter

    // FX unit LEDs (FX channels)
    FX_LED_NOTES: [0x00, 0x01, 0x02, 0x04], // ON 1-3, TAP

    // Global channel LEDs
    GLOBAL_LED_NOTES: [0x01], // BACK
    VU_MASTER_CCS: [0x05, 0x06], // master meter left / right

    // Pads: notes 0x00-0x3F (8 modes x 8 pads), shift layer 0x40-0x7F
    PAD_NOTE_MAX: 0x7F,

    // SysEx
    SYSEX_IDENTIFY_REQUEST: [0xF0, 0x00, 0x20, 0x7F, 0x03, 0x01, 0xF7],
    SYSEX_ENABLE_DECKS_34: [0xF0, 0x0A, 0x01, 0xF7],
    SYSEX_DISABLE_DECKS_34: [0xF0, 0x0A, 0x00, 0xF7],
};

// --------------------------------------------------------------------------
// Layer 3: LED engine
// --------------------------------------------------------------------------

// Pad RGB palette: 7-bit values, bit layout 00rrggbb with 2 bits per color
// channel (0/33/66/100 %); adding 0x40 selects the dimmed variant.
Mixon4.COLORS = {
    off: 0x00,
    loopRoll: 0x03, // blue, bright (roll active)
    loopRollDim: 0x43, // blue, dimmed (idle)
    samplerLoaded: 0x7F, // white, dimmed
    samplerPlaying: 0x0C, // green, bright
    beatjump: 0x38, // orange, bright (while jumping)
    beatjumpDim: 0x78, // orange, dimmed (idle)
    pitchDown: 0x03, // blue (negative semitones)
    pitchZero: 0x3F, // white (original key)
    pitchUp: 0x30, // red (positive semitones)
    fixedLoop: 0x0F, // cyan, bright (loop of this size active)
    fixedLoopDim: 0x4F, // cyan, dimmed (idle)
};

// Maps Mixxx 24-bit colors (e.g. hotcue colors) to the nearest of the 64
// bright palette colors of the controller.
Mixon4.colorMapper = (function() {
    const levels = [0x00, 0x55, 0xAA, 0xFF];
    const palette = {};
    for (let r = 0; r < 4; r++) {
        for (let g = 0; g < 4; g++) {
            for (let b = 0; b < 4; b++) {
                palette[(levels[r] << 16) | (levels[g] << 8) | levels[b]] =
                    (r << 4) | (g << 2) | b;
            }
        }
    }
    return new ColorMapper(palette);
})();

// Cache of the last value sent per LED address so repeated values (e.g. from
// VU meter connections firing at rendering rate) never flood the MIDI bus.
Mixon4.ledCache = {};

Mixon4.sendLed = function(status, midino, value) {
    const key = (status << 8) | midino;
    if (Mixon4.ledCache[key] === value) {
        return;
    }
    Mixon4.ledCache[key] = value;
    midi.sendShortMsg(status, midino, value);
};

Mixon4.sendNote = function(channel, note, value) {
    Mixon4.sendLed(Mixon4.MIDI.NOTE_ON | channel, note, value);
};

Mixon4.sendCc = function(channel, cc, value) {
    Mixon4.sendLed(Mixon4.MIDI.CC | channel, cc, value);
};

Mixon4.allLedsOff = function() {
    const spec = Mixon4.MIDI;

    // Components and connections send LEDs directly, so the cache may be
    // stale; reset it to force every message out.
    Mixon4.ledCache = {};

    spec.DECK_CHANNELS.forEach(function(channel) {
        spec.DECK_LED_NOTES.forEach(function(note) {
            Mixon4.sendNote(channel, note, 0x00);
        });
        Mixon4.sendNote(channel, spec.LOOP_LED_NOTE, 0x00);
        Mixon4.sendCc(channel, spec.JOG_LED_CC, 0x00);
        Mixon4.sendCc(channel, spec.VU_DECK_CC, 0x00);
    });

    spec.PAD_CHANNELS.forEach(function(channel) {
        for (let note = 0x00; note <= spec.PAD_NOTE_MAX; note++) {
            Mixon4.sendNote(channel, note, 0x00);
        }
    });

    spec.FX_CHANNELS.forEach(function(channel) {
        spec.FX_LED_NOTES.forEach(function(note) {
            Mixon4.sendNote(channel, note, 0x00);
        });
    });

    spec.GLOBAL_LED_NOTES.forEach(function(note) {
        Mixon4.sendNote(spec.GLOBAL_CHANNEL, note, 0x00);
    });
    spec.VU_MASTER_CCS.forEach(function(cc) {
        Mixon4.sendCc(spec.GLOBAL_CHANNEL, cc, 0x00);
    });
};

// Jog LED ring: 17-step track progress display (behaviour 1 of the hardware,
// CC values 0x00 = off, 0x01-0x10 = segments 1..16 lit).
Mixon4.connectJogLeds = function() {
    const spec = Mixon4.MIDI;
    Mixon4.jogLedConnections = [];

    spec.DECK_CHANNELS.forEach(function(channel, index) {
        const group = `[Channel${index + 1}]`;
        const update = function() {
            let segments = 0x00;
            if (engine.getValue(group, "track_loaded") > 0) {
                const position = engine.getValue(group, "playposition");
                segments = Math.max(1, Math.min(16,
                    1 + Math.floor(position * 16)));
            }
            Mixon4.sendCc(channel, spec.JOG_LED_CC, segments);
        };
        Mixon4.jogLedConnections.push(
            engine.makeConnection(group, "playposition", update));
        Mixon4.jogLedConnections.push(
            engine.makeConnection(group, "track_loaded", update));
    });
};

Mixon4.connectVuMeters = function() {
    const spec = Mixon4.MIDI;
    Mixon4.vuConnections = [];

    spec.DECK_CHANNELS.forEach(function(channel, index) {
        const group = `[Channel${index + 1}]`;
        Mixon4.vuConnections.push(
            engine.makeUnbufferedConnection(group, "vu_meter", function(value) {
                Mixon4.sendCc(channel, spec.VU_DECK_CC, Math.round(value * 127));
            }));
    });

    Mixon4.vuConnections.push(
        engine.makeUnbufferedConnection("[Main]", "vu_meter_left", function(value) {
            Mixon4.sendCc(spec.GLOBAL_CHANNEL, spec.VU_MASTER_CCS[0],
                Math.round(value * 127));
        }));
    Mixon4.vuConnections.push(
        engine.makeUnbufferedConnection("[Main]", "vu_meter_right", function(value) {
            Mixon4.sendCc(spec.GLOBAL_CHANNEL, spec.VU_MASTER_CCS[1],
                Math.round(value * 127));
        }));
};

// --------------------------------------------------------------------------
// Layer 2: Components
// --------------------------------------------------------------------------

Mixon4.Deck = function(deckNumber, midiChannel) {
    components.Deck.call(this, [deckNumber]);
    const theDeck = this;
    const noteStatus = Mixon4.MIDI.NOTE_ON | midiChannel;
    const group = this.currentDeck;

    // Trim and EQ (CC 0x00-0x03)
    this.gain = new components.Pot({group: group, inKey: "pregain"});
    this.eqLow = new components.Pot({
        group: `[EqualizerRack1_${group}_Effect1]`, inKey: "parameter1"});
    this.eqMid = new components.Pot({
        group: `[EqualizerRack1_${group}_Effect1]`, inKey: "parameter2"});
    this.eqHigh = new components.Pot({
        group: `[EqualizerRack1_${group}_Effect1]`, inKey: "parameter3"});

    // Filter knob controls the quick effect super knob (CC 0x04)
    this.filter = new components.Pot({
        group: `[QuickEffectRack1_${group}]`, inKey: "super1"});

    // Channel fader (CC 0x05)
    this.volume = new components.Pot({group: group, inKey: "volume"});

    // LOAD (note 0x02): loads the selected track; LED shows loaded state
    this.load = new components.Button({
        midi: [noteStatus, 0x02],
        group: group,
        inKey: "LoadSelectedTrack",
        outKey: "track_loaded",
    });

    // CUE/PFL (note 0x03): headphone pre-listen toggle with LED
    this.pfl = new components.Button({
        midi: [noteStatus, 0x03],
        group: group,
        key: "pfl",
        type: components.Button.prototype.types.toggle,
    });

    // Transport (notes 0x0C-0x0E). The shift layer arrives on its own
    // addresses (0x4C-0x4E) with their own LEDs, so each function is a
    // separate component and no shift state tracking is needed.
    this.play = new components.PlayButton({midi: [noteStatus, 0x0C]});
    this.reverse = new components.Button({ // shift + play: reverse while held
        midi: [noteStatus, 0x4C],
        key: "reverse",
    });
    this.cue = new components.CueButton({midi: [noteStatus, 0x0D]});
    this.startStop = new components.Button({ // shift + cue: jump to start, stop
        midi: [noteStatus, 0x4D],
        inKey: "start_stop",
        outKey: "cue_indicator",
    });
    this.sync = new components.SyncButton({midi: [noteStatus, 0x0E]});
    this.quantize = new components.Button({ // shift + sync: quantize toggle
        midi: [noteStatus, 0x4E],
        key: "quantize",
        type: components.Button.prototype.types.toggle,
    });

    // Loop section: auto loop (note 0x08, LED = loop_enabled), shift =
    // reactivate last loop; encoder (CC 0x08) halves/doubles the size.
    this.autoLoop = new components.Button({
        midi: [noteStatus, 0x08],
        inKey: "beatloop_activate",
        outKey: "loop_enabled",
    });
    this.reloop = new components.Button({
        midi: [noteStatus, 0x48],
        inKey: "reloop_toggle",
    });
    this.loopEncoder = function(channel, control, value) {
        if (value > 0x40) {
            engine.setValue(group, "loop_double", 1);
        } else if (value < 0x40) {
            engine.setValue(group, "loop_halve", 1);
        }
    };

    // Param buttons: beatjump; with shift they move an active loop
    this.beatjumpBack = new components.Button({
        midi: [noteStatus, 0x0A],
        key: "beatjump_backward",
    });
    this.beatjumpForward = new components.Button({
        midi: [noteStatus, 0x0B],
        key: "beatjump_forward",
    });
    const loopMove = function(direction) {
        return function(channel, control, value) {
            if (value > 0) {
                engine.setValue(group, "loop_move",
                    direction * engine.getValue(group, "beatjump_size"));
            }
        };
    };
    this.loopMoveBack = loopMove(-1);
    this.loopMoveForward = loopMove(1);

    // Loop LED display (note 0x09). The five LEDs carry two printed labels
    // each: blinking = fractional sizes 1/32..1/2, solid = 1..16 beats.
    // (Loop-active state is shown by the auto loop button LED instead.)
    const LOOP_LED_VALUES = {
        0.03125: 0x07, // 1/32, LED 1 blinking
        0.0625: 0x08, // 1/16, LED 2 blinking
        0.125: 0x09, // 1/8, LED 3 blinking
        0.25: 0x0A, // 1/4, LED 4 blinking
        0.5: 0x0B, // 1/2, LED 5 blinking
        1: 0x01, // LED 1 solid
        2: 0x02, // LED 2 solid
        4: 0x03, // LED 3 solid
        8: 0x04, // LED 4 solid
        16: 0x05, // LED 5 solid
    };
    const updateLoopLeds = function() {
        const size = engine.getValue(group, "beatloop_size");
        let value = LOOP_LED_VALUES[size];
        if (value === undefined) {
            // Outside the labelled range: everything on (solid above 16,
            // blinking below 1/32)
            value = size > 16 ? 0x06 : 0x0C;
        }
        Mixon4.sendNote(midiChannel, Mixon4.MIDI.LOOP_LED_NOTE, value);
    };
    this.loopLedConnections = [
        engine.makeConnection(group, "beatloop_size", updateLoopLeds),
    ];
    updateLoopLeds();

    // Pitch fader (14-bit pitch bend on the deck channel). Reloop faders
    // send inverted values, hence the negation - same as the shipped
    // Reloop Beatmix / Terminal Mix mappings.
    this.pitch = function(channel, control, value, status) {
        engine.setValue(group, "rate", -script.midiPitch(control, value, status));
    };
    engine.softTakeover(group, "rate", true);

    // Pitch LED (note 0x15) lights whenever the rate deviates from 0
    this.pitchLedConnection = engine.makeConnection(group, "rate",
        function(value) {
            Mixon4.sendNote(midiChannel, 0x15,
                Math.abs(value) > 0.001 ? 0x7F : 0x00);
        });
    this.pitchLedConnection.trigger();

    // Key handling and slip mode
    this.keylock = new components.Button({
        midi: [noteStatus, 0x10],
        key: "keylock",
        type: components.Button.prototype.types.toggle,
    });
    this.keylockHold = function(channel, control, value) {
        // Holding keylock for 2 s (hardware timer) resets the pitch fader
        // value in Mixxx to 0 without touching the fader.
        if (value > 0) {
            engine.setValue(group, "rate", 0);
        }
    };
    this.keySync = new components.Button({ // match musical key to other deck
        midi: [noteStatus, 0x11],
        key: "sync_key",
    });
    this.keyReset = new components.Button({ // double press: back to track key
        midi: [noteStatus, 0x24],
        key: "reset_key",
    });
    this.slip = new components.Button({
        midi: [noteStatus, 0x0F],
        key: "slip_enabled",
        type: components.Button.prototype.types.toggle,
    });

    // Jog wheel (encoder CC 0x06, touch note 0x07). Touching the platter
    // top enables scratching; wheel motion without touch nudges the tempo.
    this.jog = new components.JogWheelBasic({
        deck: deckNumber,
        group: group,
        wheelResolution: Mixon4.JOG.wheelResolution,
        alpha: Mixon4.JOG.alpha,
        beta: Mixon4.JOG.beta,
        rpm: Mixon4.JOG.rpm,
        // The Mixon 4 sends offset-64 relative values (0x41 = forwards,
        // 0x3F = backwards); the JogWheelBasic default expects
        // two's complement, so override the scaling.
        inValueScale: function(value) {
            return value - 0x40;
        },
    });

    // Crossfader assignment switch (notes 0x04/0x05/0x06). The hardware
    // sends 0x7F on the note of the selected position.
    // Mixxx orientation: 0 = left (A), 1 = center (thru), 2 = right (B).
    const makeAssign = function(orientation) {
        return function(channel, control, value) {
            if (value > 0) {
                engine.setValue(group, "orientation", orientation);
            }
        };
    };
    this.cfAssignA = makeAssign(0);
    this.cfAssignThru = makeAssign(1);
    this.cfAssignB = makeAssign(2);

    // Fader start (shift + channel fader, notes 0x5D/0x5E): moving the
    // fader up starts the deck, pulling it down to zero stops it.
    midi.makeInputHandler(noteStatus, 0x5D, function(channel, control, value) {
        if (value > 0) {
            engine.setValue(group, "play", 1);
        }
    });
    midi.makeInputHandler(noteStatus, 0x5E, function(channel, control, value) {
        if (value > 0) {
            engine.setValue(group, "play", 0);
        }
    });

    // FILTER FX button (note 0x01, LED hardware-controlled): assigns this
    // channel to its FX unit (decks 1/3 -> unit 1, decks 2/4 -> unit 2).
    const fxUnitNumber = (deckNumber % 2 === 1) ? 1 : 2;
    midi.makeInputHandler(noteStatus, 0x01, function(channel, control, value) {
        if (value > 0) {
            script.toggleControl(`[EffectRack1_EffectUnit${fxUnitNumber}]`,
                `group_${group}_enable`);
        }
    });

    // ---- Performance pads -------------------------------------------------
    // Pads live on their own MIDI channel per deck, and every pad mode has
    // its own note range (hot cue 0x00+, loop roll 0x08+, sampler 0x10+,
    // shift layer +0x40). All modes therefore stay connected simultaneously;
    // the hardware routes button presses and LEDs by the active mode itself.
    // Pad inputs are registered via midi.makeInputHandler to keep the XML
    // lean. Velocity (Bn) and aftertouch (An) messages stay unused.
    const padStatus =
        Mixon4.MIDI.NOTE_ON | Mixon4.MIDI.PAD_CHANNELS[deckNumber - 1];
    const bindPad = function(note, target) {
        midi.makeInputHandler(padStatus, note,
            function(channel, control, value, status) {
                target(channel, control, value, status);
            });
    };

    // Hot cues (notes 0x00-0x07): press = set/jump, shift = clear.
    // Pad color follows the hotcue color via the ColorMapper.
    this.hotcues = [];
    for (let i = 1; i <= 8; i++) {
        const note = i - 1;
        const button = new components.HotcueButton({
            midi: [padStatus, note],
            number: i,
            group: group,
            colorMapper: Mixon4.colorMapper,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        bindPad(note + 0x40, function(channel, control, value) {
            engine.setValue(group, `hotcue_${i}_clear`, value > 0 ? 1 : 0);
        });
        this.hotcues[i] = button;
    }

    // Loop rolls (notes 0x08-0x0F): momentary beatloop rolls from 1/32 to
    // 4 beats; pads glow dim blue, bright while rolling.
    const ROLL_SIZES = [1 / 32, 1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4];
    this.loopRolls = [];
    ROLL_SIZES.forEach(function(size, index) {
        const note = 0x08 + index;
        const button = new components.Button({
            midi: [padStatus, note],
            group: group,
            key: `beatlooproll_${size}_activate`,
            on: Mixon4.COLORS.loopRoll,
            off: Mixon4.COLORS.loopRollDim,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        theDeck.loopRolls[index] = button;
    });

    // Samplers (notes 0x10-0x17): press plays sampler 1-8 (or loads the
    // selected track into an empty slot); shift stops a playing sampler or
    // ejects a stopped one. LED: off = empty, dim white = loaded,
    // green = playing.
    this.samplerButtons = [];
    for (let i = 1; i <= 8; i++) {
        const note = 0x10 + i - 1;
        const button = new components.SamplerButton({
            midi: [padStatus, note],
            number: i,
            empty: Mixon4.COLORS.off,
            loaded: Mixon4.COLORS.samplerLoaded,
            playing: Mixon4.COLORS.samplerPlaying,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, button.group);
        });
        bindPad(note + 0x40, function(channel, control, value) {
            const samplerGroup = `[Sampler${i}]`;
            if (value > 0) {
                if (engine.getValue(samplerGroup, "play") === 1) {
                    engine.setValue(samplerGroup, "play", 0);
                } else {
                    engine.setValue(samplerGroup, "eject", 1);
                }
            } else if (engine.getValue(samplerGroup, "play") === 0) {
                engine.setValue(samplerGroup, "eject", 0);
            }
        });
        this.samplerButtons[i] = button;
    }

    // Slicer bank (notes 0x18-0x1F): Mixxx has no slicer feature, so this
    // bank does beatjumps instead - pads 1-4 jump backwards by 8/4/2/1
    // beats, pads 5-8 forwards by 1/2/4/8 beats.
    const BEATJUMPS = [-8, -4, -2, -1, 1, 2, 4, 8];
    this.beatjumpPads = [];
    BEATJUMPS.forEach(function(beats, index) {
        const note = 0x18 + index;
        const button = new components.Button({
            midi: [padStatus, note],
            group: group,
            key: `beatjump_${Math.abs(beats)}_${beats < 0 ? "backward" : "forward"}`,
            on: Mixon4.COLORS.beatjump,
            off: Mixon4.COLORS.beatjumpDim,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        theDeck.beatjumpPads[index] = button;
    });

    // Cue loop bank (notes 0x20-0x27): native hotcue_X_cueloop - enables a
    // loop from hotcue 1-8 (the saved loop if the hotcue is one, otherwise
    // beatloop_size). Pad colors follow the hotcue colors; shift clears.
    this.cueLoopPads = [];
    for (let i = 1; i <= 8; i++) {
        const note = 0x20 + i - 1;
        const button = new components.HotcueButton({
            midi: [padStatus, note],
            number: i,
            group: group,
            colorMapper: Mixon4.colorMapper,
            unshift: function() {
                this.inKey = `hotcue_${this.number}_cueloop`;
            },
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        bindPad(note + 0x40, function(channel, control, value) {
            engine.setValue(group, `hotcue_${i}_clear`, value > 0 ? 1 : 0);
        });
        this.cueLoopPads[i] = button;
    }

    // Saved loops bank (notes 0x28-0x2F) = hotcues 9-16: pressing an empty
    // pad while a loop is active saves that loop as the hotcue; pressing it
    // again re-enables the loop. Shift clears.
    this.savedLoopPads = [];
    for (let i = 9; i <= 16; i++) {
        const note = 0x28 + i - 9;
        const button = new components.HotcueButton({
            midi: [padStatus, note],
            number: i,
            group: group,
            colorMapper: Mixon4.colorMapper,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        bindPad(note + 0x40, function(channel, control, value) {
            engine.setValue(group, `hotcue_${i}_clear`, value > 0 ? 1 : 0);
        });
        this.savedLoopPads[i] = button;
    }

    // Pitch play bank (notes 0x30-0x37): semitone offsets around the
    // original key via the "pitch" control (blue = down, white = original,
    // red = up; the active offset lights bright). Shift resets the key.
    const PITCH_OFFSETS = [-4, -3, -2, -1, 0, 1, 2, 3];
    PITCH_OFFSETS.forEach(function(offset, index) {
        const note = 0x30 + index;
        bindPad(note, function(channel, control, value) {
            if (value > 0) {
                engine.setValue(group, "pitch", offset);
            }
        });
        bindPad(note + 0x40, function(channel, control, value) {
            if (value > 0) {
                engine.setValue(group, "pitch", 0);
            }
        });
    });
    this.pitchLedsConnection = engine.makeConnection(group, "pitch",
        function(value) {
            PITCH_OFFSETS.forEach(function(offset, index) {
                let color;
                if (offset < 0) {
                    color = Mixon4.COLORS.pitchDown;
                } else if (offset === 0) {
                    color = Mixon4.COLORS.pitchZero;
                } else {
                    color = Mixon4.COLORS.pitchUp;
                }
                const active = Math.abs(value - offset) < 0.01;
                Mixon4.sendLed(padStatus, 0x30 + index,
                    active ? color : color + 0x40);
            });
        });
    this.pitchLedsConnection.trigger();

    // Fixed loop bank ("slicer loop" notes 0x38-0x3F): toggles beatloops of
    // fixed sizes 1/8-16; the pad of an active loop size lights bright.
    const FIXED_LOOPS = [1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8, 16];
    this.fixedLoopPads = [];
    FIXED_LOOPS.forEach(function(size, index) {
        const note = 0x38 + index;
        const button = new components.Button({
            midi: [padStatus, note],
            group: group,
            inKey: `beatloop_${size}_toggle`,
            outKey: `beatloop_${size}_enabled`,
            on: Mixon4.COLORS.fixedLoop,
            off: Mixon4.COLORS.fixedLoopDim,
        });
        bindPad(note, function(channel, control, value, status) {
            button.input(channel, control, value, status, group);
        });
        theDeck.fixedLoopPads[index] = button;
    });

    this.reconnectComponents(function(component) {
        if (component.group === undefined) {
            component.group = group;
        }
    });
};
Mixon4.Deck.prototype = Object.create(components.Deck.prototype);

// Browser section on the global channel: TRAX encoder scrolls the library
// (with shift: 10 rows per click), its push activates the highlighted item
// (GoToItem), BACK moves the focus back (e.g. into the sidebar). The global
// shift button state is tracked here because the TRAX encoder is the only
// control without a dedicated shift MIDI address.
Mixon4.setupBrowserSection = function() {
    const spec = Mixon4.MIDI;
    const noteStatus = spec.NOTE_ON | spec.GLOBAL_CHANNEL;
    const ccStatus = spec.CC | spec.GLOBAL_CHANNEL;

    Mixon4.shiftPressed = false;
    midi.makeInputHandler(noteStatus, 0x03, function(channel, control, value) {
        Mixon4.shiftPressed = value > 0;
    });

    midi.makeInputHandler(ccStatus, 0x04, function(channel, control, value) {
        const step = (value > 0x40) ? 1 : -1;
        engine.setValue("[Library]", "MoveVertical",
            Mixon4.shiftPressed ? step * 10 : step);
    });
    midi.makeInputHandler(noteStatus, 0x00, function(channel, control, value) {
        if (value > 0) {
            engine.setValue("[Library]", "GoToItem", 1);
        }
    });
    midi.makeInputHandler(noteStatus, 0x01, function(channel, control, value) {
        engine.setValue("[Library]", "MoveFocusBackward", value > 0 ? 1 : 0);
        Mixon4.sendNote(spec.GLOBAL_CHANNEL, 0x01, value > 0 ? 0x7F : 0x00);
    });

    // Quantize button (hardware fires after a 2 s hold): toggles quantize
    // on all four decks at once.
    midi.makeInputHandler(noteStatus, 0x02, function(channel, control, value) {
        if (value > 0) {
            const target = engine.getValue("[Channel1]", "quantize") > 0 ? 0 : 1;
            for (let i = 1; i <= 4; i++) {
                engine.setValue(`[Channel${i}]`, "quantize", target);
            }
            if (Mixon4.debugging) {
                console.log(`Reloop Mixon 4: quantize toggled -> ${target}`);
            }
        }
    });
};

// FX section: two hardware units on MIDI channels 8/9 mapped to Mixxx
// EffectUnit 1/2 via the components EffectUnit container. Effect knobs
// control the metaknobs (or, while the TAP/focus button is held and an ON
// button focused an effect, its parameters); ON buttons toggle the slots.
// The PARAMETER encoder adjusts the unit's dry/wet mix, its push resets to
// 50 %. Effect selection: hold TAP, press an ON button to focus that slot,
// then the knobs control the focused effect's parameters.
Mixon4.setupEffectUnits = function() {
    [1, 2].forEach(function(unitNumber, index) {
        const fxChannel = Mixon4.MIDI.FX_CHANNELS[index];
        const noteStatus = Mixon4.MIDI.NOTE_ON | fxChannel;
        const ccStatus = Mixon4.MIDI.CC | fxChannel;

        const unit = new components.EffectUnit([unitNumber]);
        for (let n = 1; n <= 3; n++) {
            unit.enableButtons[n].midi = [noteStatus, n - 1];
            unit.knobs[n].midi = [ccStatus, n - 1];
        }
        unit.effectFocusButton.midi = [noteStatus, 0x04]; // TAP
        unit.init();

        for (let n = 1; n <= 3; n++) {
            const button = unit.enableButtons[n];
            midi.makeInputHandler(noteStatus, n - 1,
                function(channel, control, value, status) {
                    button.input(channel, control, value, status, button.group);
                });
            const knob = unit.knobs[n];
            midi.makeInputHandler(ccStatus, n - 1,
                function(channel, control, value, status) {
                    knob.input(channel, control, value, status, knob.group);
                });
        }
        midi.makeInputHandler(noteStatus, 0x04,
            function(channel, control, value, status) {
                unit.effectFocusButton.input(channel, control, value, status,
                    unit.group);
            });
        midi.makeInputHandler(ccStatus, 0x03, function(channel, control, value) {
            const mix = engine.getValue(unit.group, "mix") +
                (value - 0x40) * 0.04;
            engine.setValue(unit.group, "mix", Math.max(0, Math.min(1, mix)));
        });
        midi.makeInputHandler(noteStatus, 0x03, function(channel, control, value) {
            if (value > 0) {
                engine.setValue(unit.group, "mix", 0.5);
            }
        });

        Mixon4[`effectUnit${unitNumber}`] = unit;
    });
};

Mixon4.createMasterSection = function() {
    Mixon4.crossfader = new components.Pot({
        group: "[Master]", inKey: "crossfader"});

    // Headphone section: cue mix (CC 0x03) and cue volume (CC 0x07)
    Mixon4.headMix = new components.Pot({group: "[Master]", inKey: "headMix"});
    Mixon4.headGain = new components.Pot({group: "[Master]", inKey: "headGain"});

    // Crossfader curve knob (CC 0x01): left = smooth fade, right = sharp cut
    Mixon4.cfCurve = function(channel, control, value) {
        script.crossfaderCurve(value, 0, 127);
    };

    // Sampler volume knob (CC 0x02) sets the volume of all samplers
    Mixon4.samplerVolume = function(channel, control, value) {
        const numSamplers = engine.getValue("[App]", "num_samplers");
        for (let i = 1; i <= numSamplers; i++) {
            engine.setParameter(`[Sampler${i}]`, "volume", value / 127);
        }
    };
};

// --------------------------------------------------------------------------
// Lifecycle
// --------------------------------------------------------------------------

Mixon4.init = function(id, debugging) {
    Mixon4.id = id;
    Mixon4.debugging = debugging;

    Mixon4.allLedsOff();

    // Deck availability: all four decks, or 1/2 only in two-deck mode.
    // The hardware handles deck switching itself by changing the MIDI
    // channels (left 4<->6, right 5<->7), so no software toggle is needed.
    const deckSysex = Mixon4.OPTIONS.twoDeckMode ?
        Mixon4.MIDI.SYSEX_DISABLE_DECKS_34 :
        Mixon4.MIDI.SYSEX_ENABLE_DECKS_34;
    midi.sendSysexMsg(deckSysex, deckSysex.length);

    Mixon4.deck1 = new Mixon4.Deck(1, Mixon4.MIDI.DECK_CHANNELS[0]);
    Mixon4.deck2 = new Mixon4.Deck(2, Mixon4.MIDI.DECK_CHANNELS[1]);
    Mixon4.deck3 = new Mixon4.Deck(3, Mixon4.MIDI.DECK_CHANNELS[2]);
    Mixon4.deck4 = new Mixon4.Deck(4, Mixon4.MIDI.DECK_CHANNELS[3]);
    Mixon4.createMasterSection();
    Mixon4.setupBrowserSection();
    Mixon4.setupEffectUnits();
    Mixon4.connectVuMeters();
    Mixon4.connectJogLeds();

    // Ask the controller to report the current position of every control so
    // the mapped pots align Mixxx state right away (soft takeover).
    midi.sendSysexMsg(Mixon4.MIDI.SYSEX_IDENTIFY_REQUEST,
        Mixon4.MIDI.SYSEX_IDENTIFY_REQUEST.length);

    console.log(`Reloop Mixon 4: mapping initialized (id: ${id})`);
};

Mixon4.shutdown = function() {
    Mixon4.allLedsOff();
    console.log("Reloop Mixon 4: mapping shut down, LEDs cleared");
};
