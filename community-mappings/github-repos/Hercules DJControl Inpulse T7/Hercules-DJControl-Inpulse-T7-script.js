// Hercules DJControl Inpulse T7 mapping script for Mixxx
// ***************************************************************************
// * Authors: Infinite-Grok
// *   Developed with AI assistance (Anthropic Claude, OpenAI Codex), reviewed and
// *   tested on hardware by the author.
// * Based on the behaviour of the Hercules DJControl Inpulse 500 mapping by
// *   Ev3nt1ne, DJ Phatso, resetreboot (crossfader curve switch, channel layout).
// * Controller messages were measured from the hardware by the author; no code from
// *   other controller software is included.
// * Version 0.9.0-beta (October 2026)
// * Wiki: https://github.com/Infinite-Grok/mixxx-hercules-t7
// ***************************************************************************
//
// This file maps everything on the T7 except PLAY and the platters, which belong to
// Hercules-DJControl-Inpulse-T7-platter.js (DJCiT7Platter).
//
// MIDI layout of the T7, as measured on the hardware
// ----------------------------------------------------
// - 0x90 / 0xB0: mixer section (buttons / knobs and faders). Knobs and faders are 14 bit:
//   the control change number of the high seven bits is paired with the low seven bits at
//   control change number + 0x20.
// - 0x91 / 0x92: buttons of deck 1 / deck 2. 0xB1 / 0xB2: knobs, faders and encoders of the deck.
// - 0x94 / 0x95: the same buttons and encoders while SHIFT is held. The T7 itself re-routes
//   the messages, so the SHIFT layer of a control is a different MIDI address and needs no
//   state in this script.
// - 0x96 / 0x97: the eight performance pads of deck 1 / deck 2. The note number is the page
//   base ((page - 1) * 16) plus the pad number (0 to 7), plus 8 for the SHIFT layer.
// - Lamps are driven by sending the button's own status and note number back to the T7:
//   0x7F is on, 0x00 is off. The script lights only the pad notes of the page that is shown.
// - The value of a pad lamp is a colour: the Hercules code 0bRRGGGBB (the palette of the
//   Inpulse 500). Every pad lamp is written at the pad's note and at the same pad's SHIFT note
//   (note + 8), so the pads stay lit while SHIFT is held.
// - Note 0x7F on 0x91 / 0x92 and control changes 0x08 / 0x28 on 0xB1 / 0xB2 are the platter
//   motor commands. Nothing in this file may send them.
//
// Sending 0xB0 0x7F 0x7F makes the T7 report the position of every knob, fader and switch,
// so the mapping starts in sync with the hardware.
//
// Each deck has its own pad page. The four page buttons choose pages 1 to 4, and SHIFT with a
// page button chooses pages 5 to 8:
//   1 hot cues, 2 loops, 3 unused (dark), 4 samplers, 5 beat jump, 6 loop roll, 7 effects, 8 key.
// The sampler pads play like a held cue: a press plays the sampler from its cue point, a release
// stops it there (not the library's SamplerButton, which would load and eject tracks). Pressing pad N raises the number of samplers in Mixxx to N if it is lower;
// nothing is written to it when the mapping loads.

var DJCiT7 = {}; // eslint-disable-line

/** Loop sizes in beats of pad page 2, pads 1 to 8. */
DJCiT7.LOOP_SIZES = [0.25, 0.5, 1, 2, 4, 8, 16, 32];
/** Loop roll sizes in beats of pad page 6, pads 1 to 8. */
DJCiT7.ROLL_SIZES = [0.0625, 0.125, 0.25, 0.5, 1, 2, 4, 8];
/** Beat jump sizes of pad page 5. Pads 1 to 4 jump back, pads 5 to 8 forward. */
DJCiT7.JUMP_SIZES = [1, 2, 4, 8];
/** Tempo fader ranges that the RANGE button cycles through. */
DJCiT7.RATE_RANGES = [0.08, 0.16, 0.5];
/** Pitch bend of the PARAM buttons, as a fraction of the speed. */
DJCiT7.NUDGE = 0.04;
/** The jog sensitivity of Mixxx's rate control: a jog value of 1 changes the speed by this much. */
DJCiT7.JOG_SENSITIVITY = 0.1;
/** The PARAM buttons write the jog value this often while held. */
DJCiT7.NUDGE_INTERVAL_MS = 20;
/** The crossfader is picked up again when it comes this close to Mixxx's crossfader position. */
DJCiT7.CROSSFADER_PICKUP_WINDOW = 0.03;

/** Blink half period of the pad lamps, in milliseconds. */
DJCiT7.BLINK_MS = 250;

/** The browse knob ring lamp is at this note of the deck's button status (0x91 / 0x92). */
DJCiT7.RING_NOTE = 0x61;
/** Beat flash colours of the ring (Hercules colour codes): beat 1 of the bar, the other beats, dark. */
DJCiT7.RING_COLOURS = {bar: 0x40, beat: 0x02, off: 0x00};
/** The ring stays lit for this fraction of one beat. */
DJCiT7.RING_LIT_FRACTION = 0.6;
/** Mixxx does not run timers shorter than this, in milliseconds. */
DJCiT7.MIN_TIMER_MS = 20;
/** Lit time of the ring when Mixxx reports no tempo, in milliseconds. */
DJCiT7.RING_FALLBACK_MS = 300;
/** Beats in a bar. */
DJCiT7.BEATS_PER_BAR = 4;

/**
 * Hot cue pads show the cue colour mapped to the nearest of these Hercules colour codes
 * (0bRRGGGBB). It is the palette of the Inpulse 500 mapping.
 */
DJCiT7.PadColorMapper = new ColorMapper({
    0xFF0000: 0x60,
    0xFFFF00: 0x7C,
    0x00FF00: 0x1C,
    0x00FFFF: 0x1F,
    0x0000FF: 0x03,
    0xFF00FF: 0x42,
    0xFF88FF: 0x63,
    0xFFFFFF: 0x7F,
    0x000088: 0x02,
    0x008800: 0x10,
    0x008888: 0x12,
    0x228800: 0x30,
    0x880000: 0x40,
    0x882200: 0x4C,
    0x888800: 0x50,
    0x888888: 0x52,
    0x88FF00: 0x5C,
    0xFF8800: 0x74,
});

/** Pad lamp colours (Hercules colour codes). */
DJCiT7.COLOURS = {
    off: 0x00,
    loopActive: 0x17,
    samplerLoaded: 0x63,
    jumpIdle: 0x10,
    jumpHeld: 0x1C,
    rollHeld: 0x1F,
    effectOff: 0x40,
    effectOn: 0x60,
    key: 0x52,
};

DJCiT7.deck1 = null;
DJCiT7.deck2 = null;
DJCiT7.fx = null;
DJCiT7.mixer = null;

/**
 * State of the crossfader and its enable switch.
 * @type {{enabled: boolean, msb: number, pickup: boolean, lastPosition: ?number}}
 */
DJCiT7.crossfaderState = {enabled: true, msb: 0, pickup: false, lastPosition: null};

/**
 * Whether SHIFT is held on either deck. The effect buttons are on the mixer channel, so the
 * T7 does not re-route them while SHIFT is held and the script has to remember it.
 * @returns {boolean} true if SHIFT is held
 */
DJCiT7.isShiftHeld = function() {
    return Boolean((DJCiT7.deck1 && DJCiT7.deck1.shiftHeld) || (DJCiT7.deck2 && DJCiT7.deck2.shiftHeld));
};

/** The pads that blink now, the one timer that makes them blink, and the phase it is in. */
DJCiT7.blinkers = new Set();
DJCiT7.blinkTimer = 0;
DJCiT7.blinkPhase = true;

/** One step of the shared blink timer: every blinking pad shows its colour or goes dark. */
DJCiT7.blinkTick = function() {
    DJCiT7.blinkPhase = !DJCiT7.blinkPhase;
    DJCiT7.blinkers.forEach(function(pad) {
        pad.send(DJCiT7.blinkPhase ? pad.blinkColour : DJCiT7.COLOURS.off);
    });
};

/**
 * Make a pad blink between a colour and dark. The shared timer runs only while a pad blinks.
 * @param {object} pad Pad component
 * @param {number} colour Colour code of the lit phase
 */
DJCiT7.startBlink = function(pad, colour) {
    pad.blinkColour = colour;
    DJCiT7.blinkers.add(pad);
    if (DJCiT7.blinkTimer === 0) {
        DJCiT7.blinkPhase = true;
        DJCiT7.blinkTimer = engine.beginTimer(DJCiT7.BLINK_MS, DJCiT7.blinkTick);
    }
    pad.send(DJCiT7.blinkPhase ? colour : DJCiT7.COLOURS.off);
};

/**
 * Stop a pad blinking (the lamp itself is left as it is). Stops the timer when no pad is left.
 * @param {object} pad Pad component
 */
DJCiT7.stopBlink = function(pad) {
    DJCiT7.blinkers.delete(pad);
    if (DJCiT7.blinkers.size === 0 && DJCiT7.blinkTimer !== 0) {
        engine.stopTimer(DJCiT7.blinkTimer);
        DJCiT7.blinkTimer = 0;
    }
};

/**
 * A sampler pad, played like a held cue: a press plays sampler N from its cue point and a release
 * stops it and returns it to the cue point (the same on the SHIFT layer). The lamp shows whether
 * the sampler has a track, and blinks while it plays.
 * Mixxx starts with 4 samplers: a press on a pad above that count first raises
 * [App] num_samplers to the pad's number. The lamp follows [SamplerN] track_loaded and play, and
 * those connections are made only while the sampler exists: when the pad is shown, and again
 * whenever num_samplers changes.
 * @param {object} options Component options; number is the sampler number, 1 to 8.
 */
DJCiT7.SamplerPad = function(options) {
    this.number = options.number;
    this.group = `[Sampler${this.number}]`;
    this.shifted = false;
    components.Button.call(this, options);
};
DJCiT7.SamplerPad.prototype = new components.Button({
    outKey: "track_loaded",
    unshift: function() {
        this.shifted = false;
    },
    shift: function() {
        this.shifted = true;
    },
    input: function(_channel, _control, value) {
        if (value === 0) {
            if (engine.getValue("[App]", "num_samplers") >= this.number) {
                engine.setValue(this.group, "cue_gotoandstop", 1);
            }
            return;
        }
        if (engine.getValue("[App]", "num_samplers") < this.number) {
            engine.setValue("[App]", "num_samplers", this.number);
        }
        engine.setValue(this.group, "cue_gotoandplay", 1);
    },
    connect: function() {
        this.connections[0] = engine.makeConnection("[App]", "num_samplers", this.countChanged.bind(this));
        this.countChanged();
    },
    disconnect: function() {
        DJCiT7.stopBlink(this);
        components.Button.prototype.disconnect.call(this);
    },
    /** Make or drop the lamp connections to the sampler, whichever its existence calls for. */
    countChanged: function() {
        const exists = engine.getValue("[App]", "num_samplers") >= this.number;
        if (exists && this.connections[1] === undefined) {
            this.connections[1] = engine.makeConnection(this.group, this.outKey, this.trigger.bind(this));
            this.connections[2] = engine.makeConnection(this.group, "play", this.trigger.bind(this));
        } else if (!exists && this.connections[1] !== undefined) {
            this.connections[1].disconnect();
            this.connections[2].disconnect();
            this.connections.length = 1;
        }
        this.trigger();
    },
    trigger: function() {
        const loaded = this.connections[1] !== undefined && engine.getValue(this.group, this.outKey) > 0;
        if (loaded && engine.getValue(this.group, "play") > 0) {
            DJCiT7.startBlink(this, DJCiT7.COLOURS.samplerLoaded);
            return;
        }
        DJCiT7.stopBlink(this);
        this.send(loaded ? DJCiT7.COLOURS.samplerLoaded : DJCiT7.COLOURS.off);
    },
});

/**
 * One deck of the T7: its buttons, encoders, knobs, lamps and the eight performance pads.
 * @param {number} number Deck number, 1 or 2.
 */
DJCiT7.Deck = function(number) {
    components.Deck.call(this, number);
    const deck = this;
    const group = `[Channel${number}]`;
    const toggle = components.Button.prototype.types.toggle;
    // Status bytes: buttons, buttons while SHIFT is held, encoders, pads.
    const plain = 0x90 + number;
    const shifted = 0x93 + number;
    const pads = 0x95 + number;
    // Lamps of buttons that exist on both layers are lit on both channels.
    const bothLayers = {sendShifted: true, shiftChannel: true, shiftOffset: 3};

    this.shiftHeld = false;
    this.page = 1;
    this.nudgeTimer = 0;
    this.effectHolders = {lever: false, pad: false};
    this.effectPreviousRouting = 0;
    this.heldPads = [null, null, null, null, null, null, null, null];
    // The MIDI channel, note and status of each held pad's press, so a release can be sent later.
    this.heldPadInputs = [null, null, null, null, null, null, null, null];
    // The pad note base (0x00, 0x10 ... 0x70) of the last unshifted pad press, or null.
    this.padBase = null;

    const button = function(options) {
        return new components.Button(Object.assign({group: group}, options));
    };
    // A button with a lamp on both layers at the given note.
    const lampButton = function(note, options) {
        return button(Object.assign({midi: [plain, note]}, bothLayers, options));
    };
    const pot = function(potGroup, key) {
        // No soft takeover: the T7 sends its positions at start-up and the knobs then follow.
        return new components.Pot({group: potGroup, inKey: key, softTakeover: false});
    };

    this.shiftButton = button({
        input: function(_channel, _control, value) {
            deck.shiftHeld = value > 0;
        },
    });

    // Transport. PLAY itself is handled by the platter script; its lamp is here.
    this.syncButton = lampButton(0x05, {type: toggle, inKey: "sync_enabled", outKey: "sync_enabled"});
    this.syncLeaderButton = button({type: toggle, inKey: "sync_leader"});
    this.cueButton = new components.CueButton(Object.assign({group: group, midi: [plain, 0x06]}, bothLayers));
    this.cueStopButton = button({inKey: "cue_gotoandstop"});
    this.playLamp = lampButton(0x07, {outKey: "play_indicator"});
    this.stutterButton = button({inKey: "play_stutter"});

    // Loops
    this.loopInButton = lampButton(0x09, {inKey: "loop_in", outKey: "loop_enabled"});
    this.loopOutButton = lampButton(0x0A, {inKey: "loop_out", outKey: "loop_enabled"});
    this.loopHalveButton = button({inKey: "loop_halve"});
    this.loopDoubleButton = button({inKey: "loop_double"});
    // The AUTOLOOP encoder sends 1 for a step one way and 127 for a step the other way.
    this.loopEncoder = new components.Encoder({
        input: function(_channel, _control, value) {
            if (engine.getValue(group, "loop_enabled")) {
                engine.setValue(group, value < 64 ? "loop_double" : "loop_halve", 1);
            } else {
                const size = engine.getValue(group, "beatloop_size");
                engine.setValue(group, "beatloop_size", value < 64 ? Math.min(size * 2, 512) : Math.max(size / 2, 1 / 32));
            }
        },
    });
    this.loopPushButton = button({
        input: function(_channel, _control, value) {
            if (value > 0) {
                engine.setValue(group, engine.getValue(group, "loop_enabled") ? "reloop_toggle" : "beatloop_activate", 1);
            }
        },
    });
    // With SHIFT: turning moves a running loop by one beat; without a loop it changes the beat jump size.
    this.loopMoveEncoder = new components.Encoder({
        input: function(_channel, _control, value) {
            if (engine.getValue(group, "loop_enabled")) {
                engine.setValue(group, "loop_move", value < 64 ? 1 : -1);
            } else {
                const size = engine.getValue(group, "beatjump_size") || 4;
                engine.setValue(group, "beatjump_size", value < 64 ? Math.min(size * 2, 512) : Math.max(size / 2, 1 / 32));
            }
        },
    });
    this.loopRemoveButton = button({inKey: "loop_remove"});

    // Tempo range and key lock. The key lock lamp exists on the SHIFT layer only.
    this.rangeButton = button({
        input: function(_channel, _control, value) {
            if (value === 0) {
                return;
            }
            const current = engine.getValue(group, "rateRange");
            let index = 0;
            DJCiT7.RATE_RANGES.forEach(function(range, i) {
                if (Math.abs(range - current) < 1e-3) {
                    index = i;
                }
            });
            engine.setValue(group, "rateRange", DJCiT7.RATE_RANGES[(index + 1) % DJCiT7.RATE_RANGES.length]);
        },
    });
    this.keylockButton = button({midi: [shifted, 0x52], type: toggle, inKey: "keylock", outKey: "keylock"});
    this.pflButton = lampButton(0x0C, {type: toggle, inKey: "pfl", outKey: "pfl"});

    // PARAM buttons: pitch bend this deck while held. It is written as jog because jog is
    // added to the speed even while the platter script holds the scratch control. Writing
    // the value every few milliseconds keeps the bend up, because Mixxx averages jog over
    // a short time.
    const nudgeButton = function(direction) {
        return button({
            input: function(_channel, _control, value, status) {
                deck.stopNudge();
                // A press on the SHIFT channel is ignored; a release on it still ends the nudge.
                if (value === 0 || deck.shiftHeld || status === shifted) {
                    return;
                }
                const jog = direction * DJCiT7.NUDGE / DJCiT7.JOG_SENSITIVITY;
                const write = function() {
                    engine.setValue(group, "jog", jog);
                };
                write();
                deck.nudgeTimer = engine.beginTimer(DJCiT7.NUDGE_INTERVAL_MS, write);
            },
        });
    };
    this.paramDownButton = nudgeButton(-1);
    this.paramUpButton = nudgeButton(1);

    // Browser knob and library buttons.
    this.browseEncoder = new components.Encoder({
        input: function(_channel, _control, value) {
            engine.setValue("[Playlist]", "SelectTrackKnob", value < 64 ? 1 : -1);
        },
    });
    this.browseFastEncoder = new components.Encoder({
        input: function(_channel, _control, value) {
            engine.setValue("[Playlist]", "SelectTrackKnob", value < 64 ? 10 : -10);
        },
    });
    this.loadButton = button({inKey: "LoadSelectedTrack"});
    this.assistButton = new components.Button({group: "[Library]", inKey: "GoToItem"});
    this.autoDjButton = new components.Button({group: "[Library]", inKey: "AutoDjAddBottom"});
    this.backButton = new components.Button({group: "[Library]", inKey: "MoveFocusBackward"});
    this.maximizeLibraryButton = new components.Button({group: "[Skin]", type: toggle, inKey: "show_maximized_library"});

    // Faders and knobs. The tempo fader and the channel fader also work with SHIFT held.
    this.volumePot = pot(group, "volume");
    this.gainPot = pot(group, "pregain");
    this.filterPot = pot(`[QuickEffectRack1_${group}]`, "super1");
    this.eqLowPot = pot(`[EqualizerRack1_${group}_Effect1]`, "parameter1");
    this.eqMidPot = pot(`[EqualizerRack1_${group}_Effect1]`, "parameter2");
    this.eqHighPot = pot(`[EqualizerRack1_${group}_Effect1]`, "parameter3");
    this.tempoPot = pot(group, "rate");

    // Level meter next to the deck: 0 to 120 in steps of 4.
    this.vuMeter = new components.Component({
        group: group,
        midi: [0xB0 + number, 0x40],
        outKey: "vu_meter",
        output: function(value) {
            const level = Math.min(Math.round(value * 30) * 4, 127);
            if (level !== this.lastLevel) {
                this.lastLevel = level;
                this.send(level);
            }
        },
    });

    // The FX lever turns this deck's effect unit on while it is held.
    this.effectLever = button({
        input: function(_channel, _control, value) {
            deck.holdEffect("lever", value > 0);
        },
    });

    // Performance pads. Each page has one component per pad. They are kept in a plain object,
    // not in the container, because the pad lamps follow only the page that is shown and a
    // pad has to undo exactly what its press did.
    // A pad lamp is lit at the pad's own note and at its SHIFT note (+ 8), so it stays lit while
    // SHIFT is held. If the hardware reported another note base for unshifted presses
    // (this.padBase), the same pad at that base is lit too.
    const padOptions = function(page, i) {
        return {
            midi: [pads, (page - 1) * 16 + i],
            outConnect: false,
            send: function(value) {
                midi.sendShortMsg(pads, this.midi[1], value);
                midi.sendShortMsg(pads, this.midi[1] + 8, value);
                if (deck.padBase !== null && deck.padBase !== (page - 1) * 16) {
                    midi.sendShortMsg(pads, deck.padBase + i, value);
                    midi.sendShortMsg(pads, deck.padBase + 8 + i, value);
                }
            },
        };
    };
    const nothing = function() {
        // nothing to do
    };
    const effectSlot = function(slot) {
        return `[EffectRack1_EffectUnit${number}_Effect${slot}]`;
    };
    // A pad that does nothing and shows a fixed lamp state.
    const fixedPad = function(page, i, lit) {
        return new components.Button(Object.assign(padOptions(page, i), {
            input: nothing,
            shift: nothing,
            unshift: nothing,
            connect: nothing,
            disconnect: nothing,
            trigger: function() {
                this.send(lit ? this.on : this.off);
            },
        }));
    };
    // A pad that writes a control: one on the plain layer, another with SHIFT. The press chooses.
    // Extra options are merged in last, for lamp colours and lamp behaviour.
    const keyedPad = function(page, i, plainKey, shiftKey, outKey, extra) {
        return new components.Button(Object.assign(padOptions(page, i), {
            group: group,
            outKey: outKey,
            unshift: function() {
                this.inKey = plainKey;
            },
            shift: function() {
                this.inKey = shiftKey;
            },
        }, extra));
    };
    // A loop pad blinks while the loop of its size is active.
    const blinkWhileActive = {
        output: function(value) {
            if (value > 0) {
                DJCiT7.startBlink(this, DJCiT7.COLOURS.loopActive);
            } else {
                DJCiT7.stopBlink(this);
                this.send(DJCiT7.COLOURS.off);
            }
        },
        disconnect: function() {
            DJCiT7.stopBlink(this);
            components.Button.prototype.disconnect.call(this);
        },
    };
    // A pad with a steady lamp that does not follow a Mixxx control: the idle colour, and while
    // the pad is held (if a held colour is given) the held colour.
    const steadyPad = function(pad, idle, held) {
        const press = pad.input;
        pad.isHeld = false;
        pad.connect = nothing;
        pad.trigger = function() {
            this.send(this.isHeld ? held : idle);
        };
        if (held !== undefined) {
            pad.input = function(channel, control, value, status, groupName) {
                press.call(this, channel, control, value, status, groupName);
                this.isHeld = value > 0;
                this.trigger();
            };
        }
        return pad;
    };
    const effectUnit = `[EffectRack1_EffectUnit${number}]`;
    this.padPages = {};
    for (let page = 1; page <= 8; page++) {
        this.padPages[page] = [];
        for (let i = 0; i < 8; i++) {
            let pad;
            if (page === 1) {
                pad = new components.HotcueButton(Object.assign(padOptions(page, i), {
                    group: group,
                    number: i + 1,
                    colorMapper: DJCiT7.PadColorMapper,
                }));
            } else if (page === 2) {
                const size = DJCiT7.LOOP_SIZES[i];
                pad = keyedPad(page, i, `beatloop_${size}_toggle`, `beatlooproll_${size}_activate`, `beatloop_${size}_enabled`, blinkWhileActive);
            } else if (page === 3) {
                pad = fixedPad(page, i, false);
            } else if (page === 4) {
                pad = new DJCiT7.SamplerPad(Object.assign(padOptions(page, i), {number: i + 1}));
            } else if (page === 5) {
                const size = DJCiT7.JUMP_SIZES[i % 4];
                const direction = i < 4 ? "backward" : "forward";
                pad = steadyPad(keyedPad(page, i, `beatjump_${size}_${direction}`, `beatjump_${size * 4}_${direction}`),
                    DJCiT7.COLOURS.jumpIdle, DJCiT7.COLOURS.jumpHeld);
            } else if (page === 6) {
                const size = DJCiT7.ROLL_SIZES[i];
                pad = keyedPad(page, i, `beatlooproll_${size}_activate`, `beatlooproll_${size}_activate`, `beatloop_${size}_enabled`,
                    {on: DJCiT7.COLOURS.rollHeld});
            } else if (page === 7 && i < 3) {
                // Pads 1 to 3 arm the effect slots of this deck's unit.
                pad = new components.Button(Object.assign(padOptions(page, i), {
                    group: effectSlot(i + 1),
                    type: toggle,
                    inKey: "enabled",
                    outKey: "enabled",
                    on: DJCiT7.COLOURS.effectOn,
                    off: DJCiT7.COLOURS.effectOff,
                    shift: nothing,
                    unshift: nothing,
                }));
            } else if (page === 7 && i === 3) {
                // Pad 4 holds the unit on, the same way the FX lever does.
                pad = new components.Button(Object.assign(padOptions(page, i), {
                    group: effectUnit,
                    outKey: "enabled",
                    on: DJCiT7.COLOURS.effectOn,
                    off: DJCiT7.COLOURS.effectOff,
                    shift: nothing,
                    unshift: nothing,
                    input: function(_channel, _control, value) {
                        deck.holdEffect("pad", value > 0);
                    },
                }));
            } else if (page === 7) {
                pad = fixedPad(page, i, false);
            } else {
                const keys = ["pitch_down", "pitch_up", "reset_key", "sync_key"];
                pad = steadyPad(keyedPad(page, i, keys[i % 4], keys[i % 4]), DJCiT7.COLOURS.key);
            }
            this.padPages[page].push(pad);
        }
    }

    this.ringGroup = group;
    this.ringTimer = 0;
    this.ringLit = false;
    this.ringConnections = [];
    this.connectRing();
};
DJCiT7.Deck.prototype = Object.create(components.Deck.prototype);

/**
 * Is the beat that has just started the first beat of a bar? Mixxx 2.5 has no bar position, so the
 * beats are counted from the main cue point: the beat is "beat 1" when its distance from the cue,
 * in beats, is a multiple of 4 (also before the cue). Without a cue, or if any value is missing,
 * this is false and every beat gets the ordinary colour.
 * Units, from the Mixxx 2.5 sources: beat_closest and cue_point are engine sample positions, that
 * is frames times 2 channels (src/audio/frame.h:45-47 toEngineSamplePos, kEngineChannelCount is
 * stereo in src/engine/engine.h:8; src/engine/controls/quantizecontrol.cpp:104-111 sets beat_closest;
 * src/engine/controls/cuecontrol.cpp:110-111 sets cue_point, -1 (src/track/cue.h:20) when none).
 * file_bpm is the track's beats per minute (src/mixer/basetrackplayer.cpp:470) and track_samplerate
 * its sample rate in Hz (src/engine/enginebuffer.cpp:549), so one beat is
 * 60 / file_bpm * track_samplerate * 2 engine samples.
 * @returns {boolean} true on beat 1 of the bar
 */
DJCiT7.Deck.prototype.isBarStart = function() {
    const group = this.ringGroup;
    const cue = engine.getValue(group, "cue_point");
    const closest = engine.getValue(group, "beat_closest");
    const fileBpm = engine.getValue(group, "file_bpm");
    const sampleRate = engine.getValue(group, "track_samplerate");
    // -1 is Mixxx's "no position" for both positions.
    if (!isFinite(cue) || !isFinite(closest) || cue === -1 || closest === -1) {
        return false;
    }
    if (!(fileBpm > 0) || !(sampleRate > 0) || !isFinite(fileBpm) || !isFinite(sampleRate)) {
        return false;
    }
    const beatSamples = 60 / fileBpm * sampleRate * 2;
    const index = Math.round((closest - cue) / beatSamples);
    return ((index % DJCiT7.BEATS_PER_BAR) + DJCiT7.BEATS_PER_BAR) % DJCiT7.BEATS_PER_BAR === 0;
};

/**
 * Send a colour to the browse knob ring of this deck.
 * @param {number} colour Hercules colour code
 */
DJCiT7.Deck.prototype.sendRing = function(colour) {
    midi.sendShortMsg(0x90 + this.deckNumbers[0], DJCiT7.RING_NOTE, colour);
    this.ringLit = colour !== DJCiT7.RING_COLOURS.off;
};

/**
 * Turn the ring off at once and stop its off timer. Nothing is sent if the ring is already dark.
 */
DJCiT7.Deck.prototype.stopRing = function() {
    if (this.ringTimer !== 0) {
        engine.stopTimer(this.ringTimer);
        this.ringTimer = 0;
    }
    if (this.ringLit) {
        this.sendRing(DJCiT7.RING_COLOURS.off);
    }
};

/**
 * Flash the ring for one beat: red on beat 1 of the bar, blue on the others, then dark after 60 %
 * of the beat period. One one-shot timer per deck; a new beat replaces the pending one.
 * Only a forward beat flashes: while playing backwards (or scratching backwards) Mixxx's
 * beat_active is 2 (src/engine/controls/clockcontrol.cpp:54-58) and the ring stays dark.
 * @param {number} value beat_active: 0 off, 1 forward beat, 2 reverse beat
 */
DJCiT7.Deck.prototype.beatFlash = function(value) {
    const group = this.ringGroup;
    if (value !== 1 || !engine.getValue(group, "play")) {
        return;
    }
    const bpm = engine.getValue(group, "bpm");
    let litMs = DJCiT7.RING_FALLBACK_MS;
    if (bpm > 0 && isFinite(bpm)) {
        litMs = Math.max(DJCiT7.MIN_TIMER_MS, Math.round(60000 / bpm * DJCiT7.RING_LIT_FRACTION));
    }
    if (this.ringTimer !== 0) {
        engine.stopTimer(this.ringTimer);
        this.ringTimer = 0;
    }
    const deck = this;
    this.sendRing(this.isBarStart() ? DJCiT7.RING_COLOURS.bar : DJCiT7.RING_COLOURS.beat);
    this.ringTimer = engine.beginTimer(litMs, function() {
        deck.ringTimer = 0;
        deck.sendRing(DJCiT7.RING_COLOURS.off);
    }, true);
};

/**
 * Connect the ring to the beat indicator, and make it go dark when the deck stops or loads or
 * ejects a track.
 */
DJCiT7.Deck.prototype.connectRing = function() {
    const deck = this;
    const group = this.ringGroup;
    this.ringConnections.push(engine.makeConnection(group, "beat_active", function(value) {
        deck.beatFlash(value);
    }));
    this.ringConnections.push(engine.makeConnection(group, "play", function(value) {
        if (!value) {
            deck.stopRing();
        }
    }));
    this.ringConnections.push(engine.makeConnection(group, "track_loaded", function() {
        deck.stopRing();
    }));
};

/**
 * Show a pad page: the page button lamps and the pad lamps follow it.
 * @param {number} page Page 1 to 8.
 */
DJCiT7.Deck.prototype.showPage = function(page) {
    this.padPages[this.page].forEach(function(pad) {
        pad.disconnect();
        pad.connections = [];
    });
    this.page = page;
    this.padPages[page].forEach(function(pad) {
        pad.connect();
        pad.trigger();
    });
    for (let note = 0x0F; note <= 0x16; note++) {
        midi.sendShortMsg(0x90 + this.deckNumbers[0], note, note - 0x0E === page ? 0x7F : 0x00);
    }
};

/**
 * Input of the pad page buttons: notes 0x0F to 0x16 select pages 1 to 8.
 * @param {number} _channel MIDI channel
 * @param {number} control MIDI note number
 * @param {number} value MIDI value
 */
DJCiT7.Deck.prototype.pageButton = function(_channel, control, value) {
    if (value > 0) {
        this.showPage(control - 0x0E);
    }
};

/**
 * Input of the performance pads. A pad's release always undoes what its press started, even
 * if the page or SHIFT has changed in between, so this is not done by the pad components'
 * own shift handling.
 * @param {number} channel MIDI channel
 * @param {number} control MIDI note number
 * @param {number} value MIDI value
 * @param {number} status MIDI status byte
 */
DJCiT7.Deck.prototype.pad = function(channel, control, value, status) {
    const index = control & 0x07;
    this.releasePad(index, channel, control, status);
    if (value === 0) {
        return;
    }
    const pad = this.padPages[this.page][index];
    if (this.shiftHeld || (control & 0x08) !== 0) {
        pad.shift();
    } else {
        pad.unshift();
        const base = control & 0x70;
        if (base !== this.padBase) {
            this.padBase = base;
            this.padPages[this.page].forEach(function(shown) {
                shown.trigger();
            });
        }
    }
    pad.input(channel, control, value, status);
    this.heldPads[index] = pad;
    this.heldPadInputs[index] = [channel, control, status];
};

/**
 * Release the pad that is held at this index, if any.
 * @param {number} index Pad number, 0 to 7.
 * @param {number} channel MIDI channel
 * @param {number} control MIDI note number
 * @param {number} status MIDI status byte
 */
DJCiT7.Deck.prototype.releasePad = function(index, channel, control, status) {
    const pad = this.heldPads[index];
    if (pad) {
        this.heldPads[index] = null;
        this.heldPadInputs[index] = null;
        pad.input(channel, control, 0, status);
    }
};

/**
 * Switch this deck's effect unit on while the lever or effect pad 4 is held. The unit is
 * routed to this deck while it is held, and the routing is put back afterwards. If no
 * effect slot of the unit is armed, slot 1 is armed first.
 * @param {string} holder "lever" or "pad"
 * @param {boolean} on true while that control is held
 */
DJCiT7.Deck.prototype.holdEffect = function(holder, on) {
    const number = this.deckNumbers[0];
    const unit = `[EffectRack1_EffectUnit${number}]`;
    const routing = `group_[Channel${number}]_enable`;
    const holders = this.effectHolders;
    const wasHeld = holders.lever || holders.pad;
    holders[holder] = on;
    const isHeld = holders.lever || holders.pad;
    if (isHeld && !wasHeld) {
        this.effectPreviousRouting = engine.getValue(unit, routing);
        engine.setValue(unit, routing, 1);
        let armed = false;
        for (let slot = 1; slot <= 3; slot++) {
            armed = armed || engine.getValue(`[EffectRack1_EffectUnit${number}_Effect${slot}]`, "enabled") > 0;
        }
        if (!armed) {
            engine.setValue(`[EffectRack1_EffectUnit${number}_Effect1]`, "enabled", 1);
        }
    }
    engine.setValue(unit, "enabled", isHeld ? 1 : 0);
    if (wasHeld && !isHeld) {
        engine.setValue(unit, routing, this.effectPreviousRouting);
    }
};

/** Stop the PARAM nudge timer of this deck. */
DJCiT7.Deck.prototype.stopNudge = function() {
    if (this.nudgeTimer) {
        engine.stopTimer(this.nudgeTimer);
        this.nudgeTimer = 0;
    }
};

/**
 * Let go of everything this deck holds, the way hardware releases would: every held pad, then
 * the FX lever. The effect routing is put back before the state is discarded.
 */
DJCiT7.Deck.prototype.releaseHeld = function() {
    for (let index = 0; index < this.heldPads.length; index++) {
        const input = this.heldPadInputs[index];
        if (this.heldPads[index] && input) {
            this.releasePad(index, input[0], input[1], input[2]);
        }
    }
    if (this.effectHolders.lever) {
        this.holdEffect("lever", false);
    }
};

/** Release every held action, then every connection and timer this deck owns. */
DJCiT7.Deck.prototype.release = function() {
    this.stopNudge();
    this.releaseHeld();
    this.ringConnections.forEach(function(connection) {
        connection.disconnect();
    });
    this.ringConnections = [];
    this.stopRing();
    this.forEachComponent(function(component) {
        component.disconnect();
    });
    const pages = this.padPages;
    Object.keys(pages).forEach(function(page) {
        pages[page].forEach(function(pad) {
            pad.disconnect();
        });
    });
};

/**
 * Turn off the lamps this deck lit.
 */
DJCiT7.Deck.prototype.clearLamps = function() {
    const number = this.deckNumbers[0];
    // INSTRUMENTAL (1), VOCAL (2), SYNC, CUE, PLAY, IN, OUT, headphones, key lock
    [0x01, 0x02, 0x05, 0x06, 0x07, 0x09, 0x0A, 0x0C, 0x52].forEach(function(note) {
        midi.sendShortMsg(0x90 + number, note, 0x00);
        midi.sendShortMsg(0x93 + number, note, 0x00);
    });
    for (let note = 0x0F; note <= 0x16; note++) {
        midi.sendShortMsg(0x90 + number, note, 0x00);
    }
    // Every pad lamp on both layers: the pad's note and its SHIFT note.
    for (let page = 0; page < 8; page++) {
        for (let i = 0; i < 8; i++) {
            midi.sendShortMsg(0x95 + number, page * 16 + i, 0x00);
            midi.sendShortMsg(0x95 + number, page * 16 + 8 + i, 0x00);
        }
    }
    midi.sendShortMsg(0xB0 + number, 0x40, 0x00);
};

/**
 * Crossfader curve switch (as on the Inpulse 500): 0 is the smooth mix curve, 127 the
 * sharp scratch curve.
 * @param {number} _channel MIDI channel
 * @param {number} _control MIDI control number
 * @param {number} value MIDI value
 */
DJCiT7.crossfaderCurve = function(_channel, _control, value) {
    if (value === 0x7F) {
        script.crossfaderCurve(127, 0, 127);
    } else if (value === 0) {
        script.crossfaderCurve(0, 0, 127);
    }
};

/**
 * Crossfader on/off switch. While it is off, the crossfader stays in the middle. When it is
 * switched on again, the fader takes over only when it reaches the position of Mixxx's
 * crossfader, so the sound does not jump.
 * @param {number} _channel MIDI channel
 * @param {number} _control MIDI note number
 * @param {number} value MIDI value
 */
DJCiT7.crossfaderEnable = function(_channel, _control, value) {
    const state = DJCiT7.crossfaderState;
    const enable = value > 0;
    if (enable && !state.enabled) {
        state.pickup = true;
    }
    state.enabled = enable;
    if (!enable) {
        engine.setValue("[Master]", "crossfader", 0);
    }
};

/**
 * Crossfader position, 14 bit: control change 0x00 carries the high seven bits and arrives
 * first, 0x20 the low seven bits.
 * @param {number} _channel MIDI channel
 * @param {number} control MIDI control number
 * @param {number} value MIDI value
 */
DJCiT7.crossfader = function(_channel, control, value) {
    const state = DJCiT7.crossfaderState;
    if (control === 0x00) {
        state.msb = value;
        return;
    }
    const position = ((state.msb << 7) | value) / 16383 * 2 - 1;
    const last = state.lastPosition;
    state.lastPosition = position;
    if (!state.enabled) {
        return;
    }
    if (state.pickup) {
        const current = engine.getValue("[Master]", "crossfader");
        const crossed = last !== null && (last - current) * (position - current) <= 0;
        if (Math.abs(position - current) > DJCiT7.CROSSFADER_PICKUP_WINDOW && !crossed) {
            return;
        }
        state.pickup = false;
    }
    engine.setValue("[Master]", "crossfader", position);
};

/**
 * Build the mixer section: headphone mix, split cue, the six effect slot buttons, the
 * effect depth knob and the effect parameter buttons.
 */
DJCiT7.buildMixer = function() {
    DJCiT7.mixer = new components.ComponentContainer();
    DJCiT7.mixer.headMixPot = new components.Pot({group: "[Master]", inKey: "headMix", softTakeover: false});
    DJCiT7.mixer.headSplitButton = new components.Button({
        group: "[Master]",
        type: components.Button.prototype.types.toggle,
        inKey: "headSplit",
    });

    DJCiT7.fx = new components.ComponentContainer();
    // Buttons 1 to 3 are the effect slots of unit 1, buttons 4 to 6 those of unit 2.
    // A button arms or disarms its slot; with SHIFT it loads the next effect into the slot.
    for (let button = 1; button <= 6; button++) {
        DJCiT7.fx[`slotButton${button}`] = new components.Button({
            group: `[EffectRack1_EffectUnit${button <= 3 ? 1 : 2}_Effect${(button - 1) % 3 + 1}]`,
            midi: [0x90, 0x1F + button],
            type: components.Button.prototype.types.toggle,
            inKey: "enabled",
            outKey: "enabled",
            input: function(channel, control, value, status) {
                if (DJCiT7.isShiftHeld()) {
                    if (value > 0) {
                        engine.setValue(this.group, "effect_selector", 1);
                    }
                } else {
                    components.Button.prototype.input.call(this, channel, control, value, status);
                }
            },
        });
    }
    // The depth knob sets the super knob of both effect units.
    DJCiT7.fx.depthPot = new components.Pot({
        group: "[EffectRack1_EffectUnit1]",
        inKey: "super1",
        softTakeover: false,
        inSetParameter: function(value) {
            engine.setParameter("[EffectRack1_EffectUnit1]", "super1", value);
            engine.setParameter("[EffectRack1_EffectUnit2]", "super1", value);
        },
    });
    // FX - and FX +: move the effect knob of every armed slot down or up by 1/16.
    const metaButton = function(step) {
        return new components.Button({
            input: function(_channel, _control, value) {
                if (value === 0) {
                    return;
                }
                for (let unit = 1; unit <= 2; unit++) {
                    for (let slot = 1; slot <= 3; slot++) {
                        const effect = `[EffectRack1_EffectUnit${unit}_Effect${slot}]`;
                        if (engine.getValue(effect, "enabled") > 0) {
                            engine.setValue(effect, "meta", Math.max(0, Math.min(1, engine.getValue(effect, "meta") + step)));
                        }
                    }
                }
            },
        });
    };
    DJCiT7.fx.metaDownButton = metaButton(-1 / 16);
    DJCiT7.fx.metaUpButton = metaButton(1 / 16);
};

/** Release every connection and timer of the script. */
DJCiT7.release = function() {
    DJCiT7.blinkers.clear();
    if (DJCiT7.blinkTimer !== 0) {
        engine.stopTimer(DJCiT7.blinkTimer);
        DJCiT7.blinkTimer = 0;
    }
    [DJCiT7.deck1, DJCiT7.deck2].forEach(function(deck) {
        if (deck) {
            deck.release();
        }
    });
    [DJCiT7.fx, DJCiT7.mixer].forEach(function(container) {
        if (container) {
            container.forEachComponent(function(component) {
                component.disconnect();
            });
        }
    });
};

/**
 * The setup message the T7 needs before it shows host lamps on the six FX buttons (notes 0x20-0x25). Without it the
 * T7 ignores those lamp messages. Byte 11 is 1 to switch the mode on and 0 to switch it off. Measured on the wire
 * from the T7's own host session (a hardware fact; no code copied).
 * @type {number[]}
 */
DJCiT7.FX_LIGHTS_ON = [0xF0, 0x00, 0x01, 0x4E, 0x20, 0x02, 0x07, 0x7F, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xF7];
DJCiT7.FX_LIGHTS_OFF = [0xF0, 0x00, 0x01, 0x4E, 0x20, 0x02, 0x07, 0x7F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF7];

/**
 * Mixxx calls this when the mapping is loaded. It can run again without a shutdown in between.
 */
DJCiT7.init = function() {
    DJCiT7.release();
    DJCiT7.crossfaderState = {enabled: true, msb: 0, pickup: false, lastPosition: null};
    DJCiT7.deck1 = new DJCiT7.Deck(1);
    DJCiT7.deck2 = new DJCiT7.Deck(2);
    DJCiT7.buildMixer();
    DJCiT7.deck1.showPage(1);
    DJCiT7.deck2.showPage(1);
    // Ask the T7 for the position of every knob, fader and switch.
    midi.sendShortMsg(0xB0, 0x7F, 0x7F);
    // Switch on the FX button lamps, then show which effect slots are on now.
    midi.sendSysexMsg(DJCiT7.FX_LIGHTS_ON, DJCiT7.FX_LIGHTS_ON.length);
    for (let button = 1; button <= 6; button++) {
        DJCiT7.fx[`slotButton${button}`].trigger();
    }
};

/**
 * Mixxx calls this when the mapping is unloaded.
 */
DJCiT7.shutdown = function() {
    DJCiT7.release();
    [DJCiT7.deck1, DJCiT7.deck2].forEach(function(deck) {
        if (deck) {
            deck.clearLamps();
        }
    });
    for (let note = 0x20; note <= 0x25; note++) {
        midi.sendShortMsg(0x90, note, 0x00);
    }
    midi.sendSysexMsg(DJCiT7.FX_LIGHTS_OFF, DJCiT7.FX_LIGHTS_OFF.length);
};
