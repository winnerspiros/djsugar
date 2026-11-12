// xone-k3-lib-controller-beat.js — timeline (loop clock),
// display-source resolution, beat count derivation, beat clock
// connections.
//
// Loaded after lib-controller-colour.js. Extends K3Controller.prototype.
// Pulse phase always from track position; cascade count from the
// active display source. See CONCEPTS §8.
//
// Boot settle
// -----------
// After init's 250ms reopen runs the first catch-up, _startBootSettle
// reschedules _catchUpChannel while boot-relevant CO fires are still
// landing. Fixes silent-stale state on slow CO trees — a stale
// track_loaded or play_indicator would otherwise leave a loaded deck
// looking dead until the next unrelated event.

'use strict';

if (typeof XoneK3 === 'undefined') {
    throw new Error('Xone K3: xone-k3-lib-controller-beat.js — xone-k3-lib-core.js must load first');
}
if (typeof XoneK3._requireLoad !== 'function') {
    throw new Error('Xone K3: xone-k3-lib-controller-beat.js — xone-k3-lib-core.js must load first');
}
XoneK3._requireLoad('xone-k3-lib-controller-beat.js',
    ['K3Controller', '_freshDeckState',
     'DEFAULT_BEAT_DECK', 'BEAT_REPAINT_THROTTLE_MS', 'BEAT_STALL_MS']);

(function() {
    var freshDeckState = XoneK3._freshDeckState;

    // Hotcue slots scanned for phrase-marker colours. Default is
    // the top row of the hotcue2 overlay; override in the preset
    // config next to PHRASE_MARKER_COLORS.
    XoneK3.PHRASE_HOTCUE_SLOTS = [9, 10, 11, 12];

    // Mixxx pre-instantiates 36 hotcue COs, but four of those are
    // the intro/outro markers — 32 real hotcues per deck.
    XoneK3.MIXXX_HOTCUE_LIMIT = 32;

    // Validated, deduped, in-range slot list. Single source for
    // every consumer in the origin path. Invalid entries are
    // dropped here; _validatePhraseHotcueSlots warns about them
    // at boot.
    //
    // Memoized by array identity. _deriveOriginFromPosition calls
    // this per throttled playposition tick; rebuilding the array
    // plus seen map per tick would allocate ~130 times/sec across
    // four channels. PHRASE_HOTCUE_SLOTS is documented
    // set-before-boot, so replace-array is the supported way to
    // change it (mutation in place keeps the identity and the
    // cache; that's not the contract).
    XoneK3._phraseHotcueSlots = function() {
        var slots = XoneK3.PHRASE_HOTCUE_SLOTS;
        if (XoneK3._phraseHotcueSlotsCacheSrc === slots &&
            XoneK3._phraseHotcueSlotsCache) {
            return XoneK3._phraseHotcueSlotsCache;
        }
        var out = [];
        if (Array.isArray(slots)) {
            var seen = {};
            for (var i = 0; i < slots.length; i++) {
                var n = slots[i];
                if (typeof n !== 'number' || n !== Math.floor(n) ||
                    n < 1 || n > XoneK3.MIXXX_HOTCUE_LIMIT) continue;
                if (seen[n]) continue;
                seen[n] = true;
                out.push(n);
            }
        }
        XoneK3._phraseHotcueSlotsCache = out;
        XoneK3._phraseHotcueSlotsCacheSrc = slots;
        return out;
    };

    // ============================================================
    // CHANNEL / LOOP STATE
    // ============================================================
    XoneK3.K3Controller.prototype._channelOf = function(group) {
        if (typeof group !== 'string') return null;
        var m = /^\[Channel(\d)\]$/.exec(group);
        if (!m) return null;
        var n = parseInt(m[1], 10);
        return (n >= 1 && n <= 4) ? n : null;
    };

    // loopActive is the only loop state _syncVirtualClock and
    // _inspectBeat consume — the loop bounds were fed only to
    // the removed trackTempo wrap heuristic.
    XoneK3.K3Controller.prototype._readLoopState = function(ch) {
        this._decks[ch].loopActive =
            engine.getValue('[Channel' + ch + ']', 'loop_enabled') > 0;
    };

    // ============================================================
    // PLAYBACK RATE
    // ============================================================
    // Mixxx's `rate` CO is the rate *slider position*
    // (~-0.08..+0.08), not a multiplier. `rate_ratio` is the
    // actual ratio. Prefer the cached rate_ratio value populated
    // by the listener in _openChannelBeatClocks and the initial
    // read in _catchUpChannel — falls back to a live CO read on
    // a cold cache. The fallback to `1 + rate` covers Mixxx
    // builds that predate rate_ratio.
    XoneK3.K3Controller.prototype._channelRate = function(ch) {
        var cached = this._rateCache[ch];
        if (typeof cached === 'number' && isFinite(cached) && cached > 0) {
            return cached;
        }
        var grp = '[Channel' + ch + ']';
        var ratio = engine.getValue(grp, 'rate_ratio');
        if (typeof ratio === 'number' && isFinite(ratio) && ratio > 0) {
            this._rateCache[ch] = ratio;
            return ratio;
        }
        var slider = engine.getValue(grp, 'rate');
        if (typeof slider === 'number' && isFinite(slider)) {
            var m = 1 + slider;
            if (m > 0) return m;
        }
        return 1;
    };

    // ============================================================
    // VIRTUAL CLOCK (accumulated position)
    // ============================================================
    // clock.pos accumulates playhead movement through loop wraps.
    // It's the cascade source in loop mode.
    XoneK3.K3Controller.prototype._startVirtualClock = function(ch) {
        var st = this._decks[ch];
        var clock = st.loopClock;
        var grp = '[Channel' + ch + ']';
        // Sync st.playing from the CO so _advanceVirtualClock's
        // gate sees the right value regardless of listener state.
        // One CO read per clock start — negligible.
        st.playing = engine.getValue(grp, 'play_indicator') > 0;
        clock.pos = engine.getValue(grp, 'playposition');
        clock.lastTs = Date.now();
        clock.running = true;
        this._resolveDisplaySource(ch);
    };

    XoneK3.K3Controller.prototype._stopVirtualClock = function(ch) {
        var st = this._decks[ch];
        st.loopClock.running = false;
        this._resolveDisplaySource(ch);
    };

    // Single start/stop decision. The loop clock runs iff the
    // channel is in a loop AND the deck is playing — pause is
    // equivalent to exiting the loop, resume to entering it.
    // Returns true if the clock state changed, false on no-op.
    // The bool lets callers (`_onPlayIndicator`) distinguish
    // "the clock-start path already repainted" from "nothing
    // happened, I need an explicit refresh".
    XoneK3.K3Controller.prototype._syncVirtualClock = function(ch) {
        var st = this._decks[ch];
        var shouldRun = st.loopActive && st.playing;
        if (shouldRun && !st.loopClock.running) {
            this._startVirtualClock(ch);
            return true;
        }
        if (!shouldRun && st.loopClock.running) {
            this._stopVirtualClock(ch);
            return true;
        }
        return false;
    };

    XoneK3.K3Controller.prototype._advanceVirtualClock = function(ch) {
        var st = this._decks[ch];
        var clock = st.loopClock;
        if (!clock.running) return;

        // Real-seconds accumulation: forward-only, session-clock
        // semantics. Wall time doesn't reverse when the deck does,
        // so reverse mode and jog-scrub don't move the clock
        // backward. Wrap detection is unnecessary — dt always
        // advances.
        //
        // st.playing and _channelRate both read from caches
        // populated by listeners (see _openChannelBeatClocks);
        // no CO reads on this path. Date.now() is not monotonic —
        // dt clamped to zero.
        var now = Date.now();
        if (st.playing && clock.lastTs > 0) {
            var dt = (now - clock.lastTs) / 1000;
            if (dt < 0) dt = 0;
            var dur = this._trackMeta(ch).duration;
            var rate = this._channelRate(ch);
            if (dur > 0) clock.pos += (dt * rate) / dur;
        }

        clock.lastTs = now;
    };

    // ============================================================
    // DISPLAY SOURCE
    // ============================================================
    // Called from _startVirtualClock and _stopVirtualClock only.
    // No change means nothing to repaint — the display source is
    // already what the loop state implies, and every reader is
    // up to date.
    XoneK3.K3Controller.prototype._resolveDisplaySource = function(ch) {
        var st = this._decks[ch];
        var choice = st.loopClock.running ? 'loop' : 'track';
        if (choice === st.displaySource) return;

        st.displaySource = choice;
        this._syncBeatCount(ch);
        if (this.selectedDeckCurrent === ch) {
            this._refreshDisplayFlags();
            this._refreshAllLeds('display-source');
        } else {
            // Non-active channel: pause-in-loop changed the mode
            // (playing → paused) but _syncBeatCount only refreshes
            // on count change, and _refreshAllLeds only sweeps the
            // active deck. Sweep this channel's beat LEDs here so
            // BEAT_PAUSED_MODE lands immediately.
            this._refreshBeatLeds(ch);
        }
    };

    // ============================================================
    // TRACK METADATA / BEAT MATH
    // ============================================================
    // start is always 0 (file start) — intro_start is never read.
    // end is outro_end_position / samples, or 1 when unset.
    XoneK3.K3Controller.prototype._getContentBounds = function(ch) {
        var meta = this._trackMeta(ch);
        if (!(meta.samples > 0)) return null;
        var endN = (meta.outroEnd > 0 && meta.outroEnd <= meta.samples)
            ? (meta.outroEnd / meta.samples) : 1;
        return { start: 0, end: endN };
    };

    XoneK3.K3Controller.prototype._readFileBpm = function(ch) {
        var grp = '[Channel' + ch + ']';
        var fileBpm = engine.getValue(grp, 'file_bpm');
        if (fileBpm > 0) return fileBpm;
        var bpm = engine.getValue(grp, 'bpm');
        if (!(bpm > 0)) return null;
        return bpm / this._channelRate(ch);
    };

    // Three-state load model. Every reader of this method (beat
    // math, countdown, content bounds, virtual clock) already
    // tolerates empty meta — duration/samples gates kick in.
    //
    //   loaded=false       → synthetic empty meta, zero CO reads.
    //                        Unloaded deck can never leak stale
    //                        duration.
    //   inside settle      → read live, skip the cache write.
    //                        Mixed old/new tuple from a swap
    //                        never poisons the cache.
    //   outside settle     → read live, cache when duration > 0.
    //
    // Both gates are controller-scoped so they survive
    // _resetBeat / _closeBeatClocks replacing _decks[ch].
    // Shared empty-meta sentinel for unloaded decks. Callers read
    // a field (`duration`, `samples`, etc.); none mutates. Saves
    // one object allocation per call on cold paths — countdown
    // LED repaint on an empty deck fires this ~32×/sec across
    // 4 channels. Same pattern as BOOT_WHITE_ENTRY.
    var EMPTY_META = { duration: 0, fileBpm: null, samples: 0, outroEnd: 0 };

    XoneK3.K3Controller.prototype._trackMeta = function(ch) {
        var cached = this._trackMetaCache[ch];
        if (cached !== undefined) return cached;

        if (this._loadedState[ch] !== true) {
            return EMPTY_META;
        }

        var grp = '[Channel' + ch + ']';
        var dur = engine.getValue(grp, 'duration');
        var samples = engine.getValue(grp, 'track_samples');
        var outroS = engine.getValue(grp, 'outro_end_position');
        var meta = {
            duration: (dur > 0) ? dur : 0,
            fileBpm:  this._readFileBpm(ch),
            samples:  (samples > 0) ? samples : 0,
            outroEnd: (outroS > 0) ? outroS : 0
        };
        // Settle window spans from the last metadata CO fire, not
        // just track_loaded — every observer that stamps also
        // clears the cache, so the first read after the last
        // event has a full 300ms of un-cached reads before the
        // cache engages. This covers slow metadata arrival paths
        // without a per-event state machine.
        if (meta.duration > 0 &&
            Date.now() - this._loadSettlingAt[ch] >= XoneK3.META_SETTLE_MS) {
            this._trackMetaCache[ch] = meta;
        }
        return meta;
    };

    XoneK3.K3Controller.prototype._fileBpm = function(ch) {
        return this._trackMeta(ch).fileBpm;
    };

    XoneK3.K3Controller.prototype._beatF = function(ch, pos) {
        if (typeof pos !== 'number' || !isFinite(pos)) return null;
        var meta = this._trackMeta(ch);
        if (!(meta.duration > 0)) return null;
        if (!meta.fileBpm) return null;
        if (!(meta.samples > 0)) return null;
        // bounds.start is always 0 (file start); bounds.end feeds
        // only the track-end countdown, not beat math. Skipping
        // _getContentBounds eliminates one outro_end_position CO
        // read per call — two per throttled playposition tick in
        // the common path.
        return pos * meta.duration * meta.fileBpm / 60;
    };

    XoneK3.K3Controller.prototype._trackBeatF = function(ch) {
        var pos = engine.getValue('[Channel' + ch + ']', 'playposition');
        return this._beatF(ch, pos);
    };

    // Legacy accessor for inspect / tests. The cascade uses
    // _cascadeCount directly.
    XoneK3.K3Controller.prototype._phraseSourcePos = function(ch) {
        var st = this._decks[ch];
        if (st.displaySource === 'loop' && st.loopClock.running)
            return st.loopClock.pos;
        return engine.getValue('[Channel' + ch + ']', 'playposition');
    };

    XoneK3.K3Controller.prototype._cascadeCount = function(ch, trackBeatF) {
        var st = this._decks[ch];
        if (st.displaySource === 'loop' && st.loopClock.running) {
            var loopBeatF = this._beatF(ch, st.loopClock.pos);
            if (loopBeatF !== null) {
                return Math.floor(loopBeatF) - this._phraseOrigin[ch] + 1;
            }
        }
        return Math.floor(trackBeatF) - this._phraseOrigin[ch] + 1;
    };

    XoneK3.K3Controller.prototype._virtualBeatFor = function(ch) {
        var st = this._decks[ch];
        if (!st.loopClock.running) return null;
        var beatF = this._beatF(ch, st.loopClock.pos);
        if (beatF === null) return null;
        return Math.floor(beatF) - this._phraseOrigin[ch] + 1;
    };

    XoneK3.K3Controller.prototype._syncBeatCount = function(ch, pos) {
        var st = this._decks[ch];
        var beatF = (pos !== undefined)
            ? this._beatF(ch, pos)
            : this._trackBeatF(ch);
        if (beatF === null) return false;
        var count = this._cascadeCount(ch, beatF);
        var wasHasBeat = st.hasBeat;
        st.hasBeat = true;
        if (st.count === count && wasHasBeat) return false;
        st.count = count;
        this._refreshBeatLeds(ch);
        return true;
    };

    // Snapshot every declared hotcue CO into this deck's hotcueState.
    // Called at clock-open, on track_loaded (post-reset fallback),
    // and on track_samples change (load-over-current). After this,
    // _deriveOriginFromPosition reads only st.hotcueState — the
    // per-tick origin scan used to fire 32 CO reads per deck every
    // 30ms and that's what thrashed the CO mutex against the audio
    // thread.
    //
    // Self-guarding: if PHRASE_MARKER_COLORS is empty the feature
    // is off; leave the cache empty and skip the reads.
    XoneK3.K3Controller.prototype._seedHotcueCache = function(ch) {
        var st = this._decks[ch];
        st.hotcueState = {};
        if (!Array.isArray(XoneK3.PHRASE_MARKER_COLORS) ||
            XoneK3.PHRASE_MARKER_COLORS.length === 0) return;

        var grp = '[Channel' + ch + ']';
        var slots = XoneK3._phraseHotcueSlots();
        for (var i = 0; i < slots.length; i++) {
            var n = slots[i];
            st.hotcueState[n] = {
                color: engine.getValue(grp, 'hotcue_' + n + '_color'),
                pos:   engine.getValue(grp, 'hotcue_' + n + '_position')
            };
        }
    };

    // Position-derived origin from marker hotcues. Any hotcue
    // whose stored colour is in PHRASE_MARKER_COLORS defines a
    // phrase-start position. Origin is the beat of the last such
    // marker at or before the playhead. Before the first marker,
    // origin derives from the nearest marker AHEAD — pre-phrase
    // playback shows a negative count rather than a stale "0"
    // origin. With no markers at all, origin is 0.
    //
    // Frozen in loop mode: origin stays fixed for the loop's
    // duration. First playposition tick after the loop stops
    // re-derives.
    //
    // Pure cache iteration — zero CO reads. The hotcue observers
    // keep st.hotcueState current; this is the per-tick consumer.
    XoneK3.K3Controller.prototype._deriveOriginFromPosition = function(ch, pos) {
        var markers = XoneK3.PHRASE_MARKER_COLORS;
        if (!Array.isArray(markers) || markers.length === 0) return;

        var st = this._decks[ch];
        if (st.displaySource === 'loop' && st.loopClock.running) return;

        var meta = this._trackMeta(ch);
        if (!(meta.samples > 0)) return;

        var best = null;
        var bestAhead = null;
        var cache = st.hotcueState;
        var slots = XoneK3._phraseHotcueSlots();

        // Array walk over the declared slot list. Only those slots
        // are seeded; anything else in the cache is stale and
        // ignored.
        for (var si = 0; si < slots.length; si++) {
            var h = cache[slots[si]];
            if (!h) continue;
            if (markers.indexOf(h.color) === -1) continue;
            if (!(h.pos >= 0) || h.pos > meta.samples) continue;
            var markerPos = h.pos / meta.samples;
            if (markerPos <= pos) {
                if (best === null || markerPos > best) best = markerPos;
            } else {
                if (bestAhead === null || markerPos < bestAhead) bestAhead = markerPos;
            }
        }

        var refPos = (best !== null) ? best : bestAhead;
        var newOrigin;
        if (refPos !== null) {
            var beatF = this._beatF(ch, refPos);
            if (beatF === null) return;
            newOrigin = Math.floor(beatF);
        } else {
            newOrigin = 0;
        }

        if (this._phraseOrigin[ch] !== newOrigin) {
            this._phraseOrigin[ch] = newOrigin;
            this._syncBeatCount(ch, pos);
        }
    };

    XoneK3.K3Controller.prototype._resetBeat = function(ch) {
        this._decks[ch] = freshDeckState();
        this._phraseOrigin[ch] = 0;
        if (this.selectedDeckCurrent === ch) {
            this._refreshDisplayFlags();
            var depthBefore = this._refreshAllLedsDepth;
            // _refreshAllLeds refreshes every binding, including
            // every beat LED. The explicit _refreshBeatLeds below
            // would be a strict subset of that sweep.
            this._refreshAllLeds('reset-beat');
            // Fallback if the sweep was skipped by the re-entrancy
            // guard. Not reachable today — _resetBeat only runs from
            // _onTrackLoaded, which runs outside sweeps — but kept
            // for defensive correctness.
            if (depthBefore > 0) {
                this._refreshBeatLeds(ch);
            }
        } else {
            // Non-active channel: _refreshAllLeds only sweeps when
            // the active deck is affected. Refresh this channel's
            // beat LEDs directly.
            this._refreshBeatLeds(ch);
        }
    };

    XoneK3.K3Controller.prototype._repaintBeat = function(ch) {
        var refreshed = this._syncBeatCount(ch);
        if (!refreshed) this._refreshBeatLeds(ch);
    };

    // Pulse phase comes from the actual track position in BOTH
    // display modes. The loop clock's position never feeds the
    // pulse.
    //
    // `pos` may be passed by the caller to avoid re-reading
    // playposition — the caller just read it for `_advanceVirtualClock`.
    XoneK3.K3Controller.prototype._repaintFromPosition = function(ch, pos) {
        var st = this._decks[ch];
        var trackBeatF = (pos !== undefined)
            ? this._beatF(ch, pos)
            : this._trackBeatF(ch);
        if (trackBeatF === null) return;

        var trackFrac = trackBeatF - Math.floor(trackBeatF);
        var count = this._cascadeCount(ch, trackBeatF);

        var wasHasBeat = st.hasBeat;
        var wasActive  = st.active;
        var wasCount   = st.count;

        st.count = count;
        st.hasBeat = true;
        st.active = trackFrac < 0.5;

        if (!wasHasBeat || wasActive !== st.active || wasCount !== st.count) {
            this._refreshBeatLeds(ch);
        }
    };

    // ============================================================
    // DISPLAY FLAGS
    // ============================================================
    XoneK3.K3Controller.prototype._refreshDisplayFlags = function() {
        var st = this._decks[this.selectedDeckCurrent];
        if (!st) {
            this.activeDeckVirtual   = false;
            this.activeLoopVirtual   = false;
            this.activeDisplaySource = 'track';
            return;
        }
        this.activeDeckVirtual   = (st.displaySource !== 'track');
        this.activeLoopVirtual   = st.loopClock.running;
        this.activeDisplaySource = st.displaySource;
    };

    // ============================================================
    // BEAT CLOCKS — OPEN
    // ============================================================
    XoneK3.K3Controller.prototype._openBeatClocks = function() {
        for (var ch = 1; ch <= 4; ch++) this._openChannelBeatClocks(ch);
        for (var ch2 = 1; ch2 <= 4; ch2++) this._catchUpChannel(ch2);
    };

    XoneK3.K3Controller.prototype._openChannelBeatClocks = function(ch) {
        var self = this;
        var grp = '[Channel' + ch + ']';
        function wire(conns, co, fn) {
            conns.push(engine.makeConnection(grp, co, fn));
        }

        wire(this._beatConns, 'beat_active', function(v) {
            self._onBeatActive(ch, v);
        });
        // makeConnection delivers the new playposition as the first
        // callback argument. Use it instead of re-reading the CO —
        // eliminates a mutex acquisition per audio buffer per deck.
        wire(this._playConns, 'playposition', function(v) {
            self._onPlayposition(ch, v);
        });
        wire(this._playConns, 'play_indicator', function(v) {
            if (self._bootSettling[ch]) self._bootSettledAt[ch] = Date.now();
            self._onPlayIndicator(ch, v);
        });
        wire(this._playConns, 'track_loaded', function(v) {
            if (self._bootSettling[ch]) self._bootSettledAt[ch] = Date.now();
            self._onTrackLoaded(ch, v);
        });
        wire(this._playConns, 'loop_enabled', function() {
            if (self._bootSettling[ch]) self._bootSettledAt[ch] = Date.now();
            self._onLoopEnabled(ch);
        });
        wire(this._playConns, 'slip_enabled', function() {
            self._syncBeatCount(ch);
        });
        // rate_ratio changes only when the deck's effective rate
        // changes (pitch slider move, tempo follow). Cached here
        // so _channelRate doesn't have to read the CO on every
        // throttled playposition tick.
        wire(this._playConns, 'rate_ratio', function(v) {
            if (self._bootSettling[ch]) self._bootSettledAt[ch] = Date.now();
            self._rateCache[ch] = v;
        });
        // reverse changes only on user action (reverse toggle).
        // Cached so the beat-edge guard and _advanceVirtualClock
        // don't read the CO per throttled playposition tick.
        wire(this._playConns, 'reverse', function(v) {
            if (self._bootSettling[ch]) self._bootSettledAt[ch] = Date.now();
            self._reverseCache[ch] = v > 0 ? 1 : 0;
        });

        // Track identity changed. `track_loaded` fires 0→1 on the
        // first load but may not fire when a new track replaces
        // the current one, and even when it does fire it can
        // arrive before Mixxx has settled `duration` / `file_bpm`
        // — the resulting cache fill grabs the previous track's
        // metadata. `track_samples` changes reliably on every
        // load and fires regardless of play state.
        //
        // Also re-seeds hotcueState: if track_loaded doesn't
        // re-fire, the hotcue cache would keep the previous
        // track's markers until the new track's hotcues load and
        // their observers fire. One seed per load closes that
        // window.
        //
        // Re-derives origin as well — a load that bypasses
        // track_loaded (load-over-current) would otherwise leave
        // _phraseOrigin at the previous track's beat, and the
        // cascade would show the wrong colour (paused deck, no
        // playposition tick to correct it).
        wire(this._playConns, 'track_samples', function() {
            // Stamp settle before the read path below — the
            // swap case changes samples first, duration second,
            // and the mixed tuple must not reach the cache.
            self._loadSettlingAt[ch] = Date.now();
            self._trackMetaCache[ch] = undefined;
            self._seedHotcueCache(ch);
            var pos = engine.getValue(grp, 'playposition');
            self._deriveOriginFromPosition(ch, pos);
            self._syncBeatCount(ch, pos);
        });

        // outro_end_position changes when the user drags the outro
        // marker. Invalidate the meta cache so countdownScheme
        // picks up the new end on the next beat edge. No explicit
        // refresh — a drag fires this CO at UI rate, and refreshing
        // per fire would re-read all four metadata COs 30-100x/sec
        // for a number that changes by one bar phase at most.
        wire(this._playConns, 'outro_end_position', function() {
            self._trackMetaCache[ch] = undefined;
        });

        // duration and file_bpm fire after track_samples on swap
        // (queued main-thread emission in BaseTrackPlayer). They
        // are the only signals that arrive *after* the new
        // metadata has landed. The track_loaded / track_samples
        // handlers above already ran _deriveOriginFromPosition
        // and _syncBeatCount — but against pre-settle metadata,
        // so _phraseOrigin[ch] and st.count are stale for the
        // entire paused duration. _refreshBeatLeds alone would
        // repaint those stale values. Re-derive and re-sync here
        // with freshly-read playposition; _syncBeatCount
        // internally refreshes beat LEDs when count changes, so
        // no separate _refreshBeatLeds call is needed.
        wire(this._playConns, 'duration', function() {
            self._loadSettlingAt[ch] = Date.now();
            self._trackMetaCache[ch] = undefined;
            var pos = engine.getValue(grp, 'playposition');
            self._deriveOriginFromPosition(ch, pos);
            self._syncBeatCount(ch, pos);
        });
        wire(this._playConns, 'file_bpm', function() {
            self._loadSettlingAt[ch] = Date.now();
            self._trackMetaCache[ch] = undefined;
            var pos = engine.getValue(grp, 'playposition');
            self._deriveOriginFromPosition(ch, pos);
            self._syncBeatCount(ch, pos);
        });

        if (Array.isArray(XoneK3.PHRASE_MARKER_COLORS) &&
            XoneK3.PHRASE_MARKER_COLORS.length > 0) {
            this._wireHotcueObservers(ch, grp);
        }
    };

    // Seed the cache once, then wire per-slot observers that keep
    // it current. Two observers per hotcue slot: colour catches
    // recolour-to-marker (the event that can add or remove a
    // marker), position catches place/delete/drag. Each fires a
    // 1-CO read (the CO that changed), updates one cache field,
    // then re-derives origin from the cache — no scan.
    //
    // The previous implementation used one shared `rederive`
    // closure that re-scanned all hotcues (32 CO reads) on every
    // single observer fire. During a track load, Mixxx writes
    // every hotcue position + colour per channel — that's 32
    // fires × 32 reads = 1024 CO reads per channel in a burst,
    // which is what contended the CO mutex against the audio
    // thread.
    //
    // Observer bodies read self._decks[ch] fresh on every fire.
    // _resetBeat (from track_loaded) replaces this._decks[ch]
    // with a new freshDeckState; a captured reference would keep
    // writing into the pre-reset cache while
    // _deriveOriginFromPosition reads the current one, so user
    // hotcue changes after a load would never reach origin
    // derivation.
    XoneK3.K3Controller.prototype._wireHotcueObservers = function(ch, grp) {
        // Seed first so the observer's cache write always has a
        // slot to write into, even if a CO fires before the
        // wiring loop completes.
        this._seedHotcueCache(ch);

        var self = this;
        var slots = XoneK3._phraseHotcueSlots();

        function onColor(n, v) {
            var st = self._decks[ch];
            // _onTrackLoaded and _onLoopEnabled wipe hotcueState
            // (via _resetBeat) before re-seeding it a few CO reads
            // later. Mixxx clearing the outgoing track's hotcues
            // fires these observers inside that window, when the
            // slot is still undefined. Create it on demand —
            // _deriveOriginFromPosition treats half-populated
            // entries as inert until both fields arrive.
            if (!st.hotcueState[n]) st.hotcueState[n] = {};
            // `v` is delivered by makeConnection's callback — same
            // value the previous getValue would have read, one
            // fewer mutex acquisition per fire (32 across 4
            // channels per track load). Playposition stays a live
            // read: it's a different CO this observer isn't
            // watching.
            st.hotcueState[n].color = v;
            var pos = engine.getValue(grp, 'playposition');
            self._deriveOriginFromPosition(ch, pos);
        }
        function onPosition(n, v) {
            var st = self._decks[ch];
            if (!st.hotcueState[n]) st.hotcueState[n] = {};
            st.hotcueState[n].pos = v;
            var pos = engine.getValue(grp, 'playposition');
            self._deriveOriginFromPosition(ch, pos);
        }

        for (var hi = 0; hi < slots.length; hi++) {
            (function(n) {
                self._playConns.push(
                    engine.makeConnection(grp, 'hotcue_' + n + '_color',
                        function(v) { onColor(n, v); }));
                self._playConns.push(
                    engine.makeConnection(grp, 'hotcue_' + n + '_position',
                        function(v) { onPosition(n, v); }));
            })(slots[hi]);
        }
    };

    // Warn-once at boot for a malformed PHRASE_HOTCUE_SLOTS. The
    // runtime filters bad entries via _phraseHotcueSlots, so this
    // is diagnostic only.
    XoneK3.K3Controller.prototype._validatePhraseHotcueSlots = function() {
        var slots = XoneK3.PHRASE_HOTCUE_SLOTS;
        if (!Array.isArray(slots)) {
            XoneK3._warnOnce('phraseSlotsShape',
                'PHRASE_HOTCUE_SLOTS must be an array — phrase origin ' +
                'derivation disabled');
            return;
        }
        var seen = {};
        for (var i = 0; i < slots.length; i++) {
            var n = slots[i];
            if (typeof n !== 'number' || n !== Math.floor(n) ||
                n < 1 || n > XoneK3.MIXXX_HOTCUE_LIMIT) {
                XoneK3._warnOnce('phraseSlotRange:' + n,
                    'PHRASE_HOTCUE_SLOTS contains ' + n + ' — must be ' +
                    'an integer between 1 and ' + XoneK3.MIXXX_HOTCUE_LIMIT +
                    '. Slot skipped.');
                continue;
            }
            if (seen[n]) {
                XoneK3._warnOnce('phraseSlotDup:' + n,
                    'PHRASE_HOTCUE_SLOTS contains duplicate ' + n +
                    ' — extra entry ignored');
                continue;
            }
            seen[n] = true;
        }
    };

    // Boot settle: after the initial catch-up, reschedule while CO
    // fires are still landing (the tree is still populating).
    // Capped to prevent runaway reschedules. Warn-once on cap hit
    // so a slow-settling tree is diagnosable.
    XoneK3.K3Controller.prototype._startBootSettle = function() {
        for (var ch = 1; ch <= 4; ch++) {
            this._bootSettledAt[ch] = Date.now();
            this._bootAttempt[ch] = 0;
            this._bootSettling[ch] = true;
            this._checkBootChannel(ch);
        }
    };

    XoneK3.K3Controller.prototype._checkBootChannel = function(ch) {
        var elapsed = Date.now() - this._bootSettledAt[ch];
        if (elapsed < XoneK3.BOOT_SETTLE_MS &&
            this._bootAttempt[ch] < XoneK3.BOOT_MAX_ATTEMPTS) {
            this._bootAttempt[ch]++;
            var self = this;
            engine.beginTimer(XoneK3.BOOT_SETTLE_MS, function() {
                if (self._shuttingDown) return;
                self._catchUpChannel(ch);
                self._checkBootChannel(ch);
            }, true);
            return;
        }
        if (elapsed < XoneK3.BOOT_SETTLE_MS) {
            XoneK3._warnOnce('bootSettle:' + ch,
                'channel ' + ch + ' boot CO tree did not settle after ' +
                this._bootAttempt[ch] + ' attempts — reads may be stale');
        }
        this._bootAttempt[ch] = 0;
        this._bootSettling[ch] = false;
    };

    // Startup catch-up: CO connections fire on change, not on
    // setup. A preset reload during an active loop would leave
    // the loop clock idle; a reload during playback would leave
    // st.playing = false, rendering the paused beat mode on a
    // playing deck.
    XoneK3.K3Controller.prototype._catchUpChannel = function(ch) {
        // track_loaded and track_samples listeners were disconnected
        // for the whole closed-window. Any load during it left the
        // cache stale — clear it so _syncBeatCount below reads
        // fresh COs. No-op when clocks were never closed (cache
        // was already empty or freshly populated by the same track).
        this._trackMetaCache[ch] = undefined;
        // Seed the load gate from the live CO. A swap during the
        // closed window changes track_samples/duration without
        // any observer firing; at reopen we must know whether a
        // track is loaded. Stamp settle too — a load landing
        // just before reopen leaves file_bpm still in flight on
        // the queued main-thread emission, and the first read
        // here could otherwise cache a mixed tuple.
        this._loadedState[ch] =
            engine.getValue('[Channel' + ch + ']', 'track_loaded') > 0;
        this._loadSettlingAt[ch] = Date.now();
        // playing first — the clock only runs while playing, and
        // a reload during an active loop should leave the clock
        // stopped until the deck is actually playing.
        this._decks[ch].playing =
            engine.getValue('[Channel' + ch + ']', 'play_indicator') > 0;
        // Populate the rate cache so _channelRate's first call
        // doesn't have to live-read. The listener wired in
        // _openChannelBeatClocks keeps it current from here on.
        this._rateCache[ch] =
            engine.getValue('[Channel' + ch + ']', 'rate_ratio');
        this._reverseCache[ch] =
            engine.getValue('[Channel' + ch + ']', 'reverse') > 0 ? 1 : 0;
        this._readLoopState(ch);
        // If clocks were closed during a track load, the
        // track_loaded observer never fired and _phraseOrigin still
        // holds the previous track's value. _closeBeatClocks
        // replaces _decks[ch] with freshDeckState, so hotcueState is
        // empty in that case. Seed + derive so the first render
        // after reopen uses the current track's markers. On the
        // normal boot path _wireHotcueObservers already seeded, so
        // the guard skips the redundant seed.
        if (Object.keys(this._decks[ch].hotcueState).length === 0) {
            this._seedHotcueCache(ch);
        }
        var pos = engine.getValue('[Channel' + ch + ']', 'playposition');
        this._deriveOriginFromPosition(ch, pos);
        this._syncVirtualClock(ch);
        // Pass `pos` — avoids a redundant getValue in
        // _syncBeatCount's _trackBeatF fallback. Boot settle runs
        // this path up to 5 × 4 = 20 times per boot.
        this._syncBeatCount(ch, pos);
    };

    // ============================================================
    // BEAT CLOCKS — CO HANDLERS
    // ============================================================
    XoneK3.K3Controller.prototype._onBeatActive = function(ch, v) {
        var st = this._decks[ch];
        if (v === 1) {
            st.lastBeatEdge = Date.now();
            st.active = true;
            st.hasBeat = true;
            if (st.lastBeatEdge - st.lastBeatRepaint >=
                XoneK3.BEAT_REPAINT_THROTTLE_MS) {
                st.lastBeatRepaint = st.lastBeatEdge;
                this._repaintBeat(ch);
            }
        } else {
            st.active = false;
            this._refreshBeatLeds(ch);
        }
        if (this._onBeat) this._onBeat(ch);
    };

    // ============================================================
    // BEAT CLOCKS — POSITION TICK
    // ============================================================
    // Position pipeline:
    //   1. Read playhead — update lastPlayPos, detect backward
    //   2. Throttle gate — bail if too soon since last tick
    //   3. Advance virtual clock (loop-mode accumulation)
    //   4. Re-derive phrase origin from cached hotcues
    //   5. Dispatch — sync-count-only or full position repaint
    //
    // `newPos` comes from the makeConnection callback. The
    // fallback covers test mocks or any future Mixxx version that
    // might invoke the callback without a value — keeps behaviour
    // identical to the old read-in-callback path in that case.
    XoneK3.K3Controller.prototype._onPlayposition = function(ch, newPos) {
        var st = this._decks[ch];
        if (newPos === undefined) {
            newPos = engine.getValue('[Channel' + ch + ']', 'playposition');
        }
        var movingBackward = this._trackPlayhead(st, newPos);

        var now = Date.now();
        if (now - st.lastPositionSync < XoneK3.BEAT_REPAINT_THROTTLE_MS) return;
        st.lastPositionSync = now;

        // Throttled. The clock accumulates by dt (real seconds),
        // forward-only. st.playing and rate_ratio are cached by
        // listeners (_onPlayIndicator, the rate_ratio listener in
        // _openChannelBeatClocks), so this body reads zero COs.
        this._advanceVirtualClock(ch);

        this._deriveOriginFromPosition(ch, newPos);

        this._dispatchPositionTick(ch, newPos, movingBackward, now);
    };

    // Backward movement — jog-scrub leftward while playing, or
    // a manual reverse. Either way `reverse` may be 0 (jog
    // doesn't toggle it), so the guard can't rely on it alone.
    // Recorded on every tick, before the throttle gate, so the
    // next non-gated tick sees the direction that actually
    // happened — not a stale comparison against a pre-gate
    // position. Returns true if the playhead moved backward
    // since the previous tick.
    XoneK3.K3Controller.prototype._trackPlayhead = function(st, newPos) {
        var movingBackward = (st.lastPlayPos !== undefined) && (newPos < st.lastPlayPos);
        st.lastPlayPos = newPos;
        return movingBackward;
    };

    // The beat-edge guard. When beat_active has been heard
    // recently, only _syncBeatCount runs and st.active is written
    // solely by _onBeatActive. When beat_active has stalled, the
    // guard releases and _repaintFromPosition takes over to avoid
    // a frozen pulse phase.
    //
    // Guard exists to prevent double-writer flicker. Without it,
    // _repaintFromPosition writes st.active on every throttled
    // playposition tick AND _onBeatActive writes st.active on
    // every edge. The two fire on different schedules and
    // disagree briefly at beat boundaries — the LED flickers,
    // sometimes showing two pulses per beat.
    //
    // Backward motion bypasses the guard unconditionally:
    // `beat_active` fires reliably forward but irregularly on
    // backward jog. With the guard active, st.active (pulse
    // phase) is left frozen for up to BEAT_STALL_MS while count
    // keeps moving via _syncBeatCount — the "held bar" symptom.
    // Force the position repaint on backward movement so both
    // st.count and st.active update from the same source.
    //
    // st.playing is kept current by the play_indicator CO
    // listener; _reverseCache by the reverse CO listener. If
    // either transitions in the same engine frame as this tick,
    // the corresponding handler catches up in the same
    // event-loop batch and repaints — no missed paint.
    //
    // The guard isn't loop-gated. Where trackFrac is continuous
    // (outside a loop) it acts as a pure dedupe; where it jumps
    // (loop wrap, track end → start) it's what prevents the
    // flicker.
    XoneK3.K3Controller.prototype._shouldSkipPulse = function(ch, movingBackward, now) {
        var st = this._decks[ch];
        var playing   = st.playing;
        var reversing = this._reverseCache[ch] > 0;
        return playing && !reversing && !movingBackward &&
            (now - st.lastBeatEdge < XoneK3.BEAT_STALL_MS);
    };

    // Dispatch after the throttle gate. Skip-pulse path only
    // re-syncs the count (st.active stays under _onBeatActive's
    // sole control); fallthrough path calls _repaintFromPosition
    // for a full pulse + count update.
    //
    // newPos is passed through — already read by _onPlayposition,
    // no need for _repaintFromPosition to re-read it.
    XoneK3.K3Controller.prototype._dispatchPositionTick = function(ch, newPos, movingBackward, now) {
        if (this._shouldSkipPulse(ch, movingBackward, now)) {
            this._syncBeatCount(ch, newPos);
            return;
        }
        this._repaintFromPosition(ch, newPos);
    };

    XoneK3.K3Controller.prototype._onPlayIndicator = function(ch, v) {
        var st = this._decks[ch];
        var wasPlaying = st.playing;
        st.playing = v > 0;
        // Two independent refresh triggers:
        //   - state change (playing ↔ paused): sync the virtual
        //     clock (may flip displaySource) and repaint beat LEDs
        //     so BEAT_PLAYING_MODE / BEAT_PAUSED_MODE swap in.
        //   - v === 1: resync the beat count in case the pause
        //     skipped a beat edge. Fires even when the state didn't
        //     change (redundant play press on a playing deck) so a
        //     stalled count can be recovered.
        // On play-start both fire; the double refresh is harmless
        // (_ledCache dedupes unchanged frames).
        if (wasPlaying !== st.playing) {
            this._syncVirtualClock(ch);
            this._refreshBeatLeds(ch);
        }
        if (v === 1) {
            this._syncBeatCount(ch);
        }
    };

    XoneK3.K3Controller.prototype._onTrackLoaded = function(ch, v) {
        // Stamp settle, gate, and clear — all before _resetBeat.
        // _resetBeat internally calls _refreshBeatLeds, which
        // reads _trackMeta. If the gate/clear lands after that
        // refresh, the internal read uses the pre-reset state and
        // can poison the cache with the outgoing (unload) or
        // incoming-but-not-settled (load) tuple.
        this._loadSettlingAt[ch] = Date.now();
        this._loadedState[ch] = (v === 1);
        this._trackMetaCache[ch] = undefined;

        if (v !== 1) {
            this._lastLoopTouchMs[ch] = 0;
            this._resetBeat(ch);
            return;
        }
        var grp = '[Channel' + ch + ']';
        this._resetBeat(ch);
        this._lastLoopTouchMs[ch] = 0;
        // _resetBeat wiped st.playing. Re-read before the clock
        // start — a track can load while the deck is playing, and
        // the clock only runs when playing.
        this._decks[ch].playing = engine.getValue(grp, 'play_indicator') > 0;
        // Refresh the rate cache too — a load with sync engaged
        // can change the effective rate_ratio. The listener also
        // catches this if rate_ratio changes, but the track-loaded
        // path is the natural point to be sure.
        this._rateCache[ch] = engine.getValue(grp, 'rate_ratio');
        this._reverseCache[ch] = engine.getValue(grp, 'reverse') > 0 ? 1 : 0;
        this._readLoopState(ch);
        // _resetBeat also wiped hotcueState. Re-seed before
        // _deriveOriginFromPosition below — if the clocks were
        // already open, _syncBeatClocks won't reopen them and
        // _wireHotcueObservers won't re-seed on its own. Reads
        // may return the previous track's hotcues here; the
        // track_samples listener re-seeds once the new track's
        // hotcues settle.
        this._seedHotcueCache(ch);
        this._syncBeatClocks();
        this._syncVirtualClock(ch);
        // Derive from markers for the current position so a load
        // landing past a marker anchors immediately.
        var pos = engine.getValue(grp, 'playposition');
        this._deriveOriginFromPosition(ch, pos);
        this._syncBeatCount(ch, pos);
    };

    // Enabling the loop while paused does not start the clock —
    // `_syncVirtualClock` gates on both loopActive and playing.
    XoneK3.K3Controller.prototype._onLoopEnabled = function(ch) {
        this._readLoopState(ch);
        this._syncVirtualClock(ch);
    };

    // ============================================================
    // BEAT CLOCKS — CLOSE / SYNC
    // ============================================================
    XoneK3.K3Controller.prototype._closeBeatClocks = function() {
        for (var i = 0; i < this._beatConns.length; i++)
            if (this._beatConns[i]) this._beatConns[i].disconnect();
        for (var j = 0; j < this._playConns.length; j++)
            if (this._playConns[j]) this._playConns[j].disconnect();
        this._beatConns = [];
        this._playConns = [];
        for (var ch = 1; ch <= 4; ch++) {
            this._decks[ch] = freshDeckState();
            // _phraseOrigin preserved — user intent, not derived state.
        }
        this._refreshDisplayFlags();
    };

    XoneK3.K3Controller.prototype._needsBeatClocks = function() {
        if (this._beatLeds.length > 0) return true;
        var applied = this._applied || {};
        for (var name in applied) {
            if (!applied.hasOwnProperty(name)) continue;
            var cfg = applied[name];
            if (!cfg || !cfg.colorScheme) continue;
            var cs = cfg.colorScheme;
            if (typeof cs.scheme === 'function') return true;
            if (cs.schemeFromState) return true;
            var s = cs.scheme;
            if (s && typeof s === 'object' && !Array.isArray(s) && s.beat) return true;
        }
        return false;
    };

    XoneK3.K3Controller.prototype._syncBeatClocks = function() {
        var want = this._needsBeatClocks();
        var have = this._beatConns.length > 0;
        if (XoneK3._debugPrintsOn) {
            var appliedCount = 0;
            var applied = this._applied || {};
            for (var k in applied) if (applied.hasOwnProperty(k)) appliedCount++;
            print('[k3] _syncBeatClocks want=' + want +
                  ' have=' + have +
                  ' _beatLeds=' + this._beatLeds.length +
                  ' applied=' + appliedCount);
        }
        if (want && !have) this._openBeatClocks();
        else if (!want && have) this._closeBeatClocks();
    };

    // ============================================================
    // BEAT LED BOOKKEEPING
    // ============================================================
    XoneK3.K3Controller.prototype._refreshBeatLeds = function(ch) {
        for (var i = 0; i < this._beatLeds.length; i++) {
            var b = this._beatLeds[i];
            try {
                if (ch === undefined) { b._refresh(); continue; }
                // Use the same compiled scheme the render path uses
                // (_evaluateScheme). That's the fn's current output
                // when _csSchemeFn is set, else the base scheme. A
                // binding with both a base scheme AND a function
                // would otherwise gate on the base's beat deck while
                // the LED actually renders the fn's output — wrong
                // deck, spurious refreshes on the base deck's edges.
                // _evaluateScheme is memoized on ref identity, so
                // a stable scheme is one property-return per call.
                // A function scheme can also change shape at runtime
                // (countdownScheme idle → beat), which is what skips
                // the per-beat churn on FX cells whose composed
                // closure has no beat block.
                var live = b._evaluateScheme();
                var beat = live && live.beat;
                if (!beat) continue;
                var deckCh = this._resolveDeckRef(beat.deck);
                if (deckCh === undefined || deckCh === null) { b._refresh(); continue; }
                if (deckCh === ch) b._refresh();
            } catch (e) {
                XoneK3._warnOnce('refreshBeatThrow:' + b._name,
                    '_refreshBeatLeds: ' + b._name + ' threw: ' +
                    (e && e.message ? e.message : e));
            }
        }
    };

    XoneK3.K3Controller.prototype._attachBeatLed = function(b) {
        if (this._beatLeds.indexOf(b) === -1) this._beatLeds.push(b);
    };

    XoneK3.K3Controller.prototype._detachBeatLed = function(b) {
        var i = this._beatLeds.indexOf(b);
        if (i >= 0) this._beatLeds.splice(i, 1);
    };

    XoneK3.K3Controller.prototype._resolveDeckRef = function(ref) {
        if (ref === undefined || ref === null) ref = XoneK3.DEFAULT_BEAT_DECK;
        if (typeof ref === 'number') return (ref >= 1 && ref <= 4) ? ref : null;
        if (typeof ref === 'string') {
            // Fast path for the three deck tokens every shipped
            // scheme uses. Skips the 16-token resolve loop + regex
            // that runs on every beat-enabled LED refresh (~1.3k
            // token comparisons/sec at steady-state playback, plus
            // another sweep per patch swap). Same numeric result
            // as the generic path for these tokens.
            if (ref === '$DECK_ACTIVE$') return this.selectedDeckCurrent;
            if (ref === '$DECK_A$')      return this.selectedDeckA;
            if (ref === '$DECK_B$')      return this.selectedDeckB;
            var s = XoneK3.resolve(ref, this);
            var m = /\[Channel(\d)\]/.exec(s);
            return m ? parseInt(m[1], 10) : null;
        }
        return null;
    };

    XoneK3.K3Controller.prototype._deckStateFor = function(compiled) {
        if (!compiled || !compiled.beat) return null;
        var ch = this._resolveDeckRef(compiled.beat.deck);
        return ch ? this._decks[ch] : null;
    };
})();