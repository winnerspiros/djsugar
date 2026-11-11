// Hercules DJControl Inpulse T7 platter and motor script for Mixxx
// ***************************************************************************
// * Authors: Infinite-Grok
// *   Developed with AI assistance (Anthropic Claude, OpenAI Codex), reviewed and
// *   tested on hardware by the author.
// * Controller messages were measured from the hardware by the author; no code from
// *   other controller software is included.
// * Version 0.9.0-beta (October 2026)
// * Wiki: https://github.com/Infinite-Grok/mixxx-hercules-t7
// ***************************************************************************
//
// How the T7 platter talks, as measured on the hardware
// -----------------------------------------------------
// Each platter reports its angle as control change 0x09 on the deck channel (0xB1 / 0xB2,
// and 0xB4 / 0xB5 while SHIFT is held). The value is a 7 bit position that wraps from 127 to
// 0, so only the difference between two messages is meaningful. A positive difference below
// 64 is forward motion, a larger one is backward motion; a difference of exactly 64 has no
// recoverable direction and is ignored. The nominal resolution is about 2048 ticks per
// revolution, so a record at 33 1/3 RPM produces about 1138 ticks per second.
//
// Right after each position message the T7 sends a pitch bend message (0xE1 / 0xE2, and
// 0xE4 / 0xE5 with SHIFT). It carries a 14 bit timestamp of that encoder edge, counted by
// a 2.4 MHz device clock (the first data byte is the low seven bits, the second data byte is
// the high seven bits). The counter wraps every 16384 / 2.4 MHz = 6.83 ms. The device clock is
// steady, whereas the arrival time of the message at the host jitters by several
// milliseconds because of USB and MIDI batching. Dividing the ticks by the device clock
// gives a speed that is about six times less noisy than one measured from arrival times.
//
// Wrap recovery: at slow speed, or when the platter reverses, two edges can be more than one
// counter period apart, so whole wraps are lost. The script places every timestamp on the
// device clock against a lower envelope of (arrival time - device time), because a message
// can be late but never early. When a reading is longer than the current speed predicts by
// a whole number of wraps, the reading is held back for a few messages: a delivery backlog
// keeps the excess at exactly that many wraps, while a real slowdown makes it grow.
//
// Motor: note 0x7F on the deck channel (0x91 / 0x92) switches the motor on (127) or off (0).
// The motor speed is a 14 bit value sent as control change 0x08 (high seven bits) and
// 0x28 (low seven bits) on 0xB1 / 0xB2. Commanding values and reading the platter speed back
// from the position messages gave ratio = (value + 8192) / 16384, where 8192 is the nominal
// 33 1/3 RPM and the range is +/-50 %. The maximum error of this fit is 0.25 %.
//
// Playback: while the vinyl runs at the speed of the motor, the script releases scratch2 and
// Mixxx plays the track by itself, so tempo, sync, nudge and key lock work as on any deck.
// When the vinyl is pushed away from the motor speed (held, slowed, scratched, spinning up
// or braking), scratch2 carries the measured vinyl speed. The motor is always on: there is
// no setting to disable it (as in Serato and DJUCED, Hercules manual p.18).
//
// Mapping XML: list this file with functionprefix="DJCiT7Platter" and bind
// DJCiT7Platter.platter (control change 0x09), DJCiT7Platter.platterTime (pitch bend) and
// DJCiT7Platter.play (note 0x07) for each deck as script bindings.

var DJCiT7Platter = {}; // eslint-disable-line no-var

DJCiT7Platter.NOMINAL_TPS = 2048 * (100 / 3) / 60; // ticks per second at 1.0x
DJCiT7Platter.MOTOR_NOTE = 127;
DJCiT7Platter.MOTOR_CENTER = 8192;
DJCiT7Platter.MOTOR_FULL_SCALE = 16384;
DJCiT7Platter.MOTOR_MAX = 16383;

DJCiT7Platter.PB_HZ = 2400000;
DJCiT7Platter.PB_WRAP = 16384;
DJCiT7Platter.WRAP_MS = DJCiT7Platter.PB_WRAP / (DJCiT7Platter.PB_HZ / 1000); // 6.827 ms

// Speed estimate. A value of 1 publishes every observation as measured.
DJCiT7Platter.VEL_SMOOTH = 1.0;
DJCiT7Platter.DECAY_MS = 25; // shortest wait for the next tick before the record counts as stopped
DJCiT7Platter.RATE_EPSILON = 0.0015; // below this ratio the published rate is exactly zero

// Placement of timestamps on the device clock and hold-back of ambiguous intervals.
DJCiT7Platter.ARRIVAL_EPS_MS = 1.0; // arrival times are whole ms: one can read up to 1 ms early
DJCiT7Platter.QUEUE_MAX_MS = 16; // largest delivery delay considered possible
DJCiT7Platter.SEED_QUEUE_MS = 1.5; // typical delivery delay, used when the envelope is seeded
DJCiT7Platter.OFFSET_LEAK_MS_PER_S = 0.25; // the envelope may rise this fast (clock drift)
DJCiT7Platter.OFFSET_RESEED_MS = 10000; // after this much silence the envelope is seeded again
DJCiT7Platter.HOLD_TOL_MS = 0.1; // tolerance for "exactly j wraps over", plus HOLD_TOL_FRAC
DJCiT7Platter.HOLD_TOL_FRAC = 0.005; // of the predicted span time
DJCiT7Platter.HOLD_MIN_TPS = 2 * DJCiT7Platter.PB_HZ / DJCiT7Platter.PB_WRAP; // edges <= half a wrap apart
DJCiT7Platter.HOLD_MIN_MSGS = 8; // held messages that resolve a hold as delivery delay
DJCiT7Platter.HOLD_MAX_MS = 20; // a hold older than this takes the host reading
DJCiT7Platter.FRESH_MS = 50; // older estimates are not evidence of continuity
DJCiT7Platter.V1_SEASON = 32; // placements before the envelope reading is published
DJCiT7Platter.ENV_SLOW_REANCHOR_GAIN = 0.1;

// Hand-over between the motor (Mixxx plays) and the vinyl (scratch2 plays). The measured
// speed is compared with the commanded motor speed after a short exponential average.
DJCiT7Platter.ENGAGE_TOL = 0.03; // vinyl takes over after this deviation lasts ENGAGE_MS
DJCiT7Platter.ENGAGE_MS = 20;
DJCiT7Platter.ENGAGE_HARD = 0.25; // vinyl takes over at once after two readings this far off
DJCiT7Platter.RELEASE_TOL = 0.015; // Mixxx takes over after this stays within RELEASE_MS
DJCiT7Platter.RELEASE_MS = 150;
DJCiT7Platter.DETECT_ALPHA = 0.1;

DJCiT7Platter.connections = [];
DJCiT7Platter.decks = {1: {lastPos: null}, 2: {lastPos: null}};

/**
 * Name of the Mixxx group of a deck.
 * @param {number} deck Deck number, 1 or 2.
 * @returns {string} The group name, for example "[Channel1]".
 */
DJCiT7Platter.group = function(deck) {
    return `[Channel${deck}]`;
};

/**
 * Deck number of a Mixxx group.
 * @param {string} group Group name.
 * @returns {?number} 1 or 2, or null for any other group.
 */
DJCiT7Platter.deckFromGroup = function(group) {
    const m = (/^\[Channel([12])\]$/).exec(group);
    return m ? Number(m[1]) : null;
};

/**
 * Signed difference between two readings of the 7 bit wrapping platter position.
 * @param {number} last Previous reading.
 * @param {number} now Current reading.
 * @returns {?number} Ticks moved, or null when exactly half a cycle has no recoverable direction.
 */
DJCiT7Platter.tickDelta = function(last, now) {
    const d = (now - last) & 127;
    return d === 64 ? null : (d > 64 ? d - 128 : d);
};

/**
 * Switch the platter motor of a deck on or off.
 * @param {number} deck Deck number.
 * @param {boolean} on True to start the motor.
 */
DJCiT7Platter.setMotor = function(deck, on) {
    if (DJCiT7Platter.decks[deck]) {
        DJCiT7Platter.decks[deck].motorOn = !!on;
    }
    midi.sendShortMsg(0x90 | deck, DJCiT7Platter.MOTOR_NOTE, on ? 127 : 0);
};

/**
 * Drive the platter at the deck's tempo so that the vinyl rides at the rate of the track.
 * Because scratch2 replaces the playback speed rather than multiplying it, the tempo is
 * carried exactly once: here, by the motor.
 * @param {number} deck Deck number.
 * @param {number} ratio Speed ratio, 1 is nominal.
 */
DJCiT7Platter.setMotorSpeed = function(deck, ratio) {
    if (!isFinite(ratio) || ratio <= 0) {
        return;
    }
    let v = Math.round(ratio * DJCiT7Platter.MOTOR_FULL_SCALE - DJCiT7Platter.MOTOR_CENTER);
    if (v < 0) {
        v = 0;
    }
    if (v > DJCiT7Platter.MOTOR_MAX) {
        v = DJCiT7Platter.MOTOR_MAX;
    }
    const s = DJCiT7Platter.decks[deck];
    if (s.motorValue === v) {
        return; // suppress redundant traffic
    }
    s.motorValue = v;
    midi.sendShortMsg(0xB0 | deck, 8, v >> 7);
    midi.sendShortMsg(0xB0 | deck, 40, v & 127);
};

/**
 * Forward interval between two timestamps, completed with the whole wraps that the host
 * arrival time shows to be missing.
 * @param {number} last Previous timestamp.
 * @param {number} now Current timestamp.
 * @param {number} elapsedMs Host time between the two arrivals, 0 if unknown.
 * @returns {number} Device clock counts.
 */
DJCiT7Platter.pbForward = function(last, now, elapsedMs) {
    let d = (now - last) % DJCiT7Platter.PB_WRAP;
    if (d < 0) {
        d += DJCiT7Platter.PB_WRAP;
    }
    if (elapsedMs > 0) {
        const expected = elapsedMs * (DJCiT7Platter.PB_HZ / 1000);
        const wraps = Math.round((expected - d) / DJCiT7Platter.PB_WRAP);
        if (wraps > 0) {
            d += wraps * DJCiT7Platter.PB_WRAP;
        }
    }
    return d;
};

/**
 * Place timestamp pbNow, which arrived at nowMs, on the unwrapped device count.
 * @param {object} s Deck state.
 * @param {number} pbNow Timestamp of the edge, 0 to 16383.
 * @param {number} nowMs Host time of arrival in ms.
 * @returns {object} The placement: u (device count), q (delivery delay above the envelope), reanchor.
 */
DJCiT7Platter.pbPlace = function(s, pbNow, nowMs) {
    const perMs = DJCiT7Platter.PB_HZ / 1000;
    if (s.offsetMs === null || nowMs - s.offsetAtMs > DJCiT7Platter.OFFSET_RESEED_MS) {
        s.offsetMs = nowMs - pbNow / perMs - DJCiT7Platter.SEED_QUEUE_MS;
        s.pbU = null; // a new envelope frame: the next placement seeds rather than measures
    } else {
        s.offsetMs += DJCiT7Platter.OFFSET_LEAK_MS_PER_S * (nowMs - s.offsetAtMs) / 1000;
    }
    s.offsetAtMs = nowMs;
    // Latest device count congruent to pbNow that does not postdate the arrival.
    const limit = (nowMs - s.offsetMs + DJCiT7Platter.ARRIVAL_EPS_MS) * perMs;
    let u = pbNow + DJCiT7Platter.PB_WRAP * Math.floor((limit - pbNow) / DJCiT7Platter.PB_WRAP);
    let reanchor = false;
    if (s.pbU !== null && u <= s.pbU) {
        // Arrived earlier than the envelope allows: the envelope was too high. Take the
        // smallest forward interval; lowerEnvelope then moves the envelope down to it.
        let d = (pbNow - s.pbU) % DJCiT7Platter.PB_WRAP;
        if (d < 0) {
            d += DJCiT7Platter.PB_WRAP;
        }
        u = s.pbU + (d === 0 ? DJCiT7Platter.PB_WRAP : d);
        reanchor = true;
    }
    return {u: u, q: nowMs - s.offsetMs - u / perMs, reanchor: reanchor};
};

/**
 * Lower the delay envelope after a placement. An ordinary placement can lower it by at most
 * ARRIVAL_EPS_MS. A reanchor can lower it by up to a whole wrap, and at slow speed it usually
 * means the previous placement was a misread, so it is followed fully only at established speed.
 * @param {object} s Deck state.
 * @param {number} q Delivery delay of the placement; only a negative value lowers the envelope.
 * @param {boolean} full True to follow the reanchor in full.
 */
DJCiT7Platter.lowerEnvelope = function(s, q, full) {
    if (q < 0) {
        s.offsetMs += full ? q : DJCiT7Platter.ENV_SLOW_REANCHOR_GAIN * q;
    }
};

/**
 * Clear the timing state of a deck and stop its stop-check timer. It does not touch lastPos:
 * the platter handler writes it before the edge that triggered the call is processed.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.resetTiming = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    if (!s) {
        return;
    }
    s.lastPitchBend = null;
    s.pbU = null; // device count of the last accepted edge; offsetMs survives, because the
    s.spanTicks = 0; // device-host clock relation does not depend on the track
    s.acceptedMs = null;
    s.holdStartMs = null;
    s.holdMsgs = 0;
    s.holdLastU = null;
    s.dPrevPB = null; // previous timestamp and its arrival, for the interval rule
    s.dPrevMs = null;
    s.dSpanCounts = 0; // counts accumulated by the interval rule since the last accept
    s.tickCounts = [Infinity, Infinity]; // last two per-tick intervals
    s.v1Placed = 0;
    s.pendingTicks = 0;
    s.velTicksPerSec = 0;
    if (s.decayTimer) {
        engine.stopTimer(s.decayTimer);
        s.decayTimer = 0;
    }
};

/**
 * Speed the motor drives the vinyl at.
 * @param {number} deck Deck number.
 * @returns {number} Ratio to nominal speed, 0 while the motor is stopped.
 */
DJCiT7Platter.motorRatio = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    if (!s || !s.motorOn || s.motorValue === null || s.motorValue === undefined) {
        return 0;
    }
    return (s.motorValue + DJCiT7Platter.MOTOR_CENTER) / DJCiT7Platter.MOTOR_FULL_SCALE;
};

/**
 * Let the vinyl drive the audio through scratch2.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.engage = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    s.engaged = true;
    s.devSince = null;
    s.okSince = null;
};

/**
 * Give the audio back to the Mixxx engine, which continues from the current position.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.release = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    const group = DJCiT7Platter.group(deck);
    s.engaged = false;
    s.devSince = null;
    s.okSince = null;
    if (engine.getValue(group, "scratch2_enable")) {
        engine.setValue(group, "scratch2", 0);
        engine.setValue(group, "scratch2_enable", 0);
    }
};

/**
 * Publish the measured vinyl speed and decide who drives the audio.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.publishRate = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    let ratio = s.velTicksPerSec / DJCiT7Platter.NOMINAL_TPS;
    if (!isFinite(ratio) || Math.abs(ratio) < DJCiT7Platter.RATE_EPSILON) {
        ratio = 0;
    }
    const group = DJCiT7Platter.group(deck);
    const nowMs = Date.now();
    const motor = DJCiT7Platter.motorRatio(deck);
    s.detect = (s.detect === null || s.detect === undefined || ratio === 0) ? ratio
        : s.detect + DJCiT7Platter.DETECT_ALPHA * (ratio - s.detect);
    const dev = Math.abs(s.detect - motor);
    if (!s.engaged) {
        // One far-off reading can be a wrap misread during a delivery stall, so a hard
        // engage needs two in a row, or a real stop (0).
        const far = Math.abs(ratio - motor) > DJCiT7Platter.ENGAGE_HARD;
        if (far && dev > DJCiT7Platter.ENGAGE_TOL && (ratio === 0 || s.farLast)) {
            DJCiT7Platter.engage(deck);
        } else if (dev > DJCiT7Platter.ENGAGE_TOL) {
            if (s.devSince === null || s.devSince === undefined) {
                s.devSince = nowMs;
            }
            if (nowMs - s.devSince >= DJCiT7Platter.ENGAGE_MS) {
                DJCiT7Platter.engage(deck);
            }
        } else {
            s.devSince = null;
        }
    } else if (dev < DJCiT7Platter.RELEASE_TOL) {
        if (s.okSince === null || s.okSince === undefined) {
            s.okSince = nowMs;
        }
        if (nowMs - s.okSince >= DJCiT7Platter.RELEASE_MS) {
            DJCiT7Platter.release(deck);
        }
    } else {
        s.okSince = null;
    }
    s.farLast = Math.abs(ratio - motor) > DJCiT7Platter.ENGAGE_HARD;
    if (!s.engaged) {
        return;
    }
    if (!engine.getValue(group, "scratch2_enable")) {
        engine.setValue(group, "scratch2_enable", 1);
    }
    engine.setValue(group, "scratch2", ratio);
};

/**
 * Arm the stop check: a record that delivers no new tick within the timeout is stopped.
 * The check only ever reduces speed, so a still record stays silent.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.armDecay = function(deck) {
    const s = DJCiT7Platter.decks[deck];
    if (s.decayTimer) {
        engine.stopTimer(s.decayTimer);
    }
    s.ticksAtArm = s.netTicks;
    // Allow twice the current per-tick interval before declaring a stop. Only consecutive
    // same-direction readings define a speed; one-tick dither (a held record) keeps DECAY_MS.
    let timeoutMs = DJCiT7Platter.DECAY_MS;
    if (s.velTicksPerSec !== 0 && s.sameDirection) {
        const capMs = 2000 / (DJCiT7Platter.RATE_EPSILON * DJCiT7Platter.NOMINAL_TPS);
        timeoutMs = Math.max(DJCiT7Platter.DECAY_MS, Math.min(capMs, Math.ceil(2000 / Math.abs(s.velTicksPerSec))));
    }
    s.decayDeadline = Date.now() + timeoutMs;
    DJCiT7Platter.startDecayTimer(deck, timeoutMs);
};

/**
 * Start the stop-check timer. Its callback acts only once the intended deadline has passed.
 * @param {number} deck Deck number.
 * @param {number} ms Delay in ms.
 */
DJCiT7Platter.startDecayTimer = function(deck, ms) {
    const s = DJCiT7Platter.decks[deck];
    s.decayTimer = engine.beginTimer(ms, function() {
        const st = DJCiT7Platter.decks[deck];
        st.decayTimer = 0;
        const early = st.decayDeadline - Date.now();
        if (early > 0) {
            DJCiT7Platter.startDecayTimer(deck, early);
            return;
        }
        if (st.netTicks !== st.ticksAtArm) {
            // Movement arrived but may never be published (held interval, lost timestamp):
            // keep watching, otherwise the last command would stand indefinitely.
            DJCiT7Platter.armDecay(deck);
            return;
        }
        st.velTicksPerSec = 0; // no new ticks: stop, do not coast
        DJCiT7Platter.publishRate(deck);
        if (st.engaged && DJCiT7Platter.motorRatio(deck) === 0) {
            DJCiT7Platter.release(deck);
        }
    }, true);
};

/**
 * Whether the deck has a track, the precondition for reading the platter.
 * @param {number} deck Deck number.
 * @returns {boolean} True when a track is loaded.
 */
DJCiT7Platter.trackLoaded = function(deck) {
    return !!engine.getValue(DJCiT7Platter.group(deck), "track_loaded");
};

/**
 * Switch scratching off and clear the timing state of a deck.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.stopScratch = function(deck) {
    const g = DJCiT7Platter.group(deck);
    engine.setValue(g, "scratch2", 0);
    engine.setValue(g, "scratch2_enable", 0);
    DJCiT7Platter.resetTiming(deck);
    DJCiT7Platter.decks[deck].lastPos = null;
};

/**
 * Bring the motor in line with the play state of a deck.
 * @param {number} deck Deck number.
 */
DJCiT7Platter.syncTransport = function(deck) {
    const group = DJCiT7Platter.group(deck);
    if (engine.getValue(group, "track_loaded")) {
        const playing = engine.getValue(group, "play") > 0;
        const st = DJCiT7Platter.decks[deck];
        if (st.motorOn !== playing) {
            // Motor start/stop: the vinyl lags the motor, so it drives the audio until it
            // reaches the new speed. scratch2 starts from the vinyl's current speed.
            let v = st.velTicksPerSec / DJCiT7Platter.NOMINAL_TPS;
            if (!isFinite(v) || Math.abs(v) < DJCiT7Platter.RATE_EPSILON) {
                v = 0;
            }
            engine.setValue(group, "scratch2", v);
            engine.setValue(group, "scratch2_enable", 1);
            DJCiT7Platter.engage(deck);
        }
        DJCiT7Platter.setMotorSpeed(deck, engine.getValue(group, "rate_ratio"));
        DJCiT7Platter.setMotor(deck, playing);
    } else {
        DJCiT7Platter.setMotor(deck, false);
        DJCiT7Platter.stopScratch(deck);
    }
};

/**
 * Mixxx entry point. Drops the connections and timers of an earlier run, and connects each
 * deck. Safe to call repeatedly.
 */
DJCiT7Platter.init = function() {
    DJCiT7Platter.connections.forEach(function(c) {
        c.disconnect();
    });
    DJCiT7Platter.connections = [];
    [1, 2].forEach(function(deck) {
        // A repeated init discards the velocity estimate, so it must also discard the command
        // that estimate produced: cancel the old stop timer and publish zero.
        if (DJCiT7Platter.decks[deck] && DJCiT7Platter.decks[deck].decayTimer) {
            engine.stopTimer(DJCiT7Platter.decks[deck].decayTimer);
        }
        if (engine.getValue(DJCiT7Platter.group(deck), "scratch2_enable")) {
            engine.setValue(DJCiT7Platter.group(deck), "scratch2", 0);
        }
        DJCiT7Platter.decks[deck] = {
            lastPos: null, netTicks: 0, motorValue: null,
            lastPitchBend: null, pendingTicks: 0, velTicksPerSec: 0, decayTimer: 0, ticksAtArm: 0,
            pbU: null, spanTicks: 0, acceptedMs: null, holdStartMs: null, holdMsgs: 0,
            offsetMs: null, offsetAtMs: 0, holdLastU: null, dPrevPB: null, dPrevMs: null,
            dSpanCounts: 0, tickCounts: [Infinity, Infinity], v1Placed: 0,
            motorOn: false, engaged: false, detect: null, devSince: null, okSince: null
        };
        const group = DJCiT7Platter.group(deck);
        DJCiT7Platter.connections.push(engine.makeConnection(group, "play", function() {
            DJCiT7Platter.syncTransport(deck);
        }));
        DJCiT7Platter.connections.push(engine.makeConnection(group, "rate_ratio", function(value) {
            DJCiT7Platter.setMotorSpeed(deck, value);
        }));
        DJCiT7Platter.connections.push(engine.makeConnection(group, "track_loaded", function() {
            DJCiT7Platter.resetTiming(deck);
            DJCiT7Platter.decks[deck].lastPos = null;
            DJCiT7Platter.syncTransport(deck);
        }));
        DJCiT7Platter.syncTransport(deck);
    });
};

/**
 * Mixxx exit point. Disconnects everything and stops every timer. The motor is switched off
 * and returned to nominal speed; nothing is sent after return.
 */
DJCiT7Platter.shutdown = function() {
    DJCiT7Platter.connections.forEach(function(c) {
        c.disconnect();
    });
    DJCiT7Platter.connections = [];
    [1, 2].forEach(function(deck) {
        DJCiT7Platter.setMotor(deck, false);
        DJCiT7Platter.setMotorSpeed(deck, 1.0);
        DJCiT7Platter.stopScratch(deck);
    });
};

/**
 * Handler for the platter position (control change 0x09). Counts the ticks moved since the
 * last message; the speed is computed when the paired timestamp arrives.
 * @param {number} channel MIDI channel.
 * @param {number} control Controller number.
 * @param {number} value Position, 0 to 127.
 * @param {number} status MIDI status byte.
 * @param {string} group Group of the deck.
 */
DJCiT7Platter.platter = function(channel, control, value, status, group) {
    const deck = DJCiT7Platter.deckFromGroup(group);
    if (deck === null || value < 0 || value > 127) {
        return;
    }
    const s = DJCiT7Platter.decks[deck];
    const previous = s.lastPos;
    s.lastPos = value;
    if (!DJCiT7Platter.trackLoaded(deck) || previous === null) {
        return;
    }
    const ticks = DJCiT7Platter.tickDelta(previous, value);
    if (ticks === null) {
        return;
    }
    s.netTicks += ticks;
    s.pendingTicks += ticks;
};

/**
 * Handler for the paired timestamp (pitch bend) of the tick just reported on the position
 * control. Turns the ticks and device time into a speed and publishes it.
 * @param {number} channel MIDI channel.
 * @param {number} control Low seven bits of the timestamp.
 * @param {number} value High seven bits of the timestamp.
 * @param {number} status MIDI status byte.
 * @param {string} group Group of the deck.
 */
DJCiT7Platter.platterTime = function(channel, control, value, status, group) {
    const deck = DJCiT7Platter.deckFromGroup(group);
    if (deck === null) {
        return;
    }
    const s = DJCiT7Platter.decks[deck];
    if (s === undefined) {
        return;
    }
    const pbNow = ((value & 0x7F) << 7) | (control & 0x7F);
    const nowMs = Date.now();
    const moved = s.pendingTicks;
    s.pendingTicks = 0;
    // Interval rule: its reference moves on every timestamp.
    const dCounts = s.dPrevPB === null ? null
        : DJCiT7Platter.pbForward(s.dPrevPB, pbNow, s.dPrevMs === null ? 0 : nowMs - s.dPrevMs);
    s.dPrevPB = pbNow;
    s.dPrevMs = nowMs;
    if (!DJCiT7Platter.trackLoaded(deck)) {
        return;
    }
    // An unpaired timestamp carries no movement and its edge's position is unknown, so it
    // is not a reference for the next interval either: the next position delta is measured
    // from the last paired edge, so its time must be too.
    if (moved === 0 && s.pbU !== null) {
        return;
    }
    const place = DJCiT7Platter.pbPlace(s, pbNow, nowMs);
    if (s.pbU === null) {
        s.pbU = place.u;
        s.acceptedMs = nowMs;
        s.spanTicks = 0;
        s.holdStartMs = null;
        s.holdMsgs = 0;
        s.holdLastU = null;
        s.dSpanCounts = 0;
        s.tickCounts = [Infinity, Infinity];
        s.v1Placed = 0;
        DJCiT7Platter.lowerEnvelope(s, place.q, true);
        return;
    }
    s.spanTicks += moved;
    if (dCounts !== null && dCounts > 0) {
        s.dSpanCounts += dCounts;
    }
    const span = s.spanTicks;
    let u = place.u;
    const fast = s.v1Placed >= DJCiT7Platter.V1_SEASON
        && s.tickCounts[0] < DJCiT7Platter.PB_WRAP / 2
        && s.tickCounts[1] < DJCiT7Platter.PB_WRAP / 2;
    // A backlog holds back fast ticks; a held message more than half a wrap after the
    // previous held one contradicts that reading and ends the hold.
    const contradicted = s.holdLastU !== null && u - s.holdLastU >= DJCiT7Platter.PB_WRAP / 2;
    if (fast && !contradicted && !place.reanchor
            && Math.abs(s.velTicksPerSec) >= DJCiT7Platter.HOLD_MIN_TPS
            && nowMs - s.acceptedMs <= DJCiT7Platter.FRESH_MS
            && span !== 0 && (span > 0) === (s.velTicksPerSec > 0)) {
        // Excess of the host reading over the time the current estimate predicts.
        const predictedMs = span / s.velTicksPerSec * 1000;
        const excessMs = (u - s.pbU) / (DJCiT7Platter.PB_HZ / 1000) - predictedMs;
        const j = Math.round(excessMs / DJCiT7Platter.WRAP_MS);
        if (j >= 1 && u - j * DJCiT7Platter.PB_WRAP > s.pbU
                && place.q + j * DJCiT7Platter.WRAP_MS <= DJCiT7Platter.QUEUE_MAX_MS
                && Math.abs(excessMs - j * DJCiT7Platter.WRAP_MS)
                    <= DJCiT7Platter.HOLD_TOL_MS + DJCiT7Platter.HOLD_TOL_FRAC * predictedMs) {
            if (s.holdStartMs === null) {
                s.holdStartMs = nowMs;
                s.holdMsgs = 0;
            }
            s.holdMsgs++;
            if (s.holdMsgs >= DJCiT7Platter.HOLD_MIN_MSGS) {
                // Enough fast ticks behind the gap: it was delivery delay, remove the wraps.
                u -= j * DJCiT7Platter.PB_WRAP;
                place.q += j * DJCiT7Platter.WRAP_MS;
            } else if (nowMs - s.holdStartMs < DJCiT7Platter.HOLD_MAX_MS) {
                // Hold: the current command stands, the ticks stay pending.
                s.holdLastU = u;
                DJCiT7Platter.armDecay(deck);
                return;
            }
            // Otherwise the hold has expired and the host reading is taken.
        }
    }
    if (span === 0) {
        // Net-zero movement since the last accepted edge (for example +1 then -1 across a
        // hold): re-reference time, publish nothing; the stop timer governs stillness.
        DJCiT7Platter.lowerEnvelope(s, place.q,
            !place.reanchor || Math.abs(s.velTicksPerSec) >= DJCiT7Platter.HOLD_MIN_TPS);
        s.pbU = u;
        s.acceptedMs = nowMs;
        s.holdStartMs = null;
        s.holdLastU = null;
        s.dSpanCounts = 0;
        return;
    }
    const v1Counts = u - s.pbU;
    // Published reading: the envelope reading in the fast regime, otherwise the interval
    // rule (if it has a reading).
    const useInterval = !fast && s.dSpanCounts > 0;
    const counts = useInterval ? s.dSpanCounts : v1Counts;
    const observed = span / (counts / DJCiT7Platter.PB_HZ);
    s.sameDirection = s.lastSpanSign === (span > 0 ? 1 : -1);
    s.lastSpanSign = span > 0 ? 1 : -1;
    s.pbU = u;
    s.acceptedMs = nowMs;
    s.spanTicks = 0;
    s.holdStartMs = null;
    s.holdLastU = null;
    s.dSpanCounts = 0;
    s.tickCounts = [s.tickCounts[1], v1Counts / Math.abs(span)];
    s.v1Placed++;
    DJCiT7Platter.lowerEnvelope(s, place.q,
        !place.reanchor || Math.abs(s.velTicksPerSec) >= DJCiT7Platter.HOLD_MIN_TPS);
    s.velTicksPerSec = s.velTicksPerSec === 0
        ? observed
        : s.velTicksPerSec + DJCiT7Platter.VEL_SMOOTH * (observed - s.velTicksPerSec);
    DJCiT7Platter.publishRate(deck);
    DJCiT7Platter.armDecay(deck);
};

/**
 * Handler for the PLAY button (note 0x07): toggles play on the deck.
 * @param {number} channel MIDI channel.
 * @param {number} control Note number.
 * @param {number} value Velocity; 0 is the release and is ignored.
 * @param {number} status MIDI status byte.
 * @param {string} group Group of the deck.
 */
DJCiT7Platter.play = function(channel, control, value, status, group) {
    const deck = DJCiT7Platter.deckFromGroup(group);
    if (deck === null || (status & 0xF0) !== 0x90 || value === 0) {
        return;
    }
    engine.setValue(group, "play", engine.getValue(group, "play") ? 0 : 1);
};
