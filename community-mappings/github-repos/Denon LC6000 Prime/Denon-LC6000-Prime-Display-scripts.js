// Denon-LC6000-Prime-Display-scripts.js
// Mixxx Controller Script für den LC6000 WD (Display) Port
// functionprefix: LC6000PrimeDisplay

var LC6000PrimeDisplay = {};

LC6000PrimeDisplay.kKeepAliveIntervalMs       = 40;
LC6000PrimeDisplay.kLoopTextHideMs            = 1000;
LC6000PrimeDisplay.kRingBlinkIntervalMs       = 400;
LC6000PrimeDisplay.kRingNearEndThreshold      = 0.97;
LC6000PrimeDisplay.kBeatsPerPlatterRevolution = 4;

LC6000PrimeDisplay.kBrightnessValues = {
    Low:  0x0F,
    Mid:  0x46,
    High: 0x69,
    Max:  0x7F,
};

LC6000PrimeDisplay.kDeckAssignments = {
    "Deck 1": 1,
    "Deck 2": 2,
    "Deck 3": 3,
    "Deck 4": 4,
};

LC6000PrimeDisplay.kDeckColors = {
    1: {r: 0.01, g: 0.91, b: 0.44, a: 1.0},
    2: {r: 0.11, g: 0.57, b: 1.0,  a: 1.0},
    3: {r: 1.0,  g: 0.45, b: 0.0,  a: 1.0},
    4: {r: 0.78, g: 0.15, b: 1.0,  a: 1.0},
};

LC6000PrimeDisplay.kDeckColorChoices = {
    "Deck Default": null,
    Green:          LC6000PrimeDisplay.kDeckColors[1],
    Blue:           LC6000PrimeDisplay.kDeckColors[2],
    Orange:         LC6000PrimeDisplay.kDeckColors[3],
    Purple:         LC6000PrimeDisplay.kDeckColors[4],
};

LC6000PrimeDisplay.kDisplayPlatterColor       = {r: 1.0, g: 1.0, b: 1.0, a: 1.0};
LC6000PrimeDisplay.kDisplayPlatterHiddenColor = {r: 1.0, g: 1.0, b: 1.0, a: 0.0};
LC6000PrimeDisplay.kDisplaySlipRingAlpha      = 0.35;

LC6000PrimeDisplay.kDisplayLoopTextMap = {
    "0.015625": "1/64",
    "0.03125":  "1/32",
    "0.0625":   "1/16",
    "0.125":    "1/8",
    "0.25":     "1/4",
    "0.5":      "1/2",
    "1":        "1",
    "2":        "2",
    "4":        "4",
    "8":        "8",
    "16":       "16",
    "32":       "32",
    "64":       "64",
};

LC6000PrimeDisplay.kDisplayLoopIndexMap = {
    "1/64": 0x0,
    "1/32": 0x1,
    "1/16": 0x2,
    "1/8":  0x3,
    "1/4":  0x4,
    "1/2":  0x5,
    "1":    0x6,
    "2":    0x7,
    "4":    0x8,
    "8":    0x9,
    "16":   0xA,
    "32":   0xB,
    "64":   0xC,
    "A":    0xE,
    "B":    0xF,
};

// ── State ─────────────────────────────────────────────────────────────────────

LC6000PrimeDisplay.state = {
    deckNumber:       1,
    group:            "[Channel1]",
    deckAssignment:   "Deck 1",
    deckColorSetting: "Deck Default",
    screenBrightness: "High",
    trackLoaded:      false,
    playposition:     0.0,
    duration:         0.0,
    playIndicator:    false,
    bpm:              0.0,
    fileBpm:          0.0,
    slipEnabled:      false,
    loopEnabled:      false,
    beatloopSize:     4,
    deckColor:        LC6000PrimeDisplay.kDeckColors[1],
    showLoopText:     false,
    loopText:         "",
    ringBlinkVisible: true,
};

LC6000PrimeDisplay.deckConnections = [];
LC6000PrimeDisplay.keepAliveTimer  = 0;
LC6000PrimeDisplay.loopTextTimer   = 0;
LC6000PrimeDisplay.ringBlinkTimer  = 0;

// ── Hilfsfunktionen ───────────────────────────────────────────────────────────

LC6000PrimeDisplay.clamp = function(value, minValue, maxValue) {
    return Math.max(minValue, Math.min(maxValue, value));
};

LC6000PrimeDisplay.currentGroup = function() {
    return LC6000PrimeDisplay.state.group;
};

LC6000PrimeDisplay.normalizeDeckAssignment = function(value) {
    if (Object.prototype.hasOwnProperty.call(LC6000PrimeDisplay.kDeckAssignments, value)) {
        return value;
    }
    return "Deck 1";
};

LC6000PrimeDisplay.normalizeDeckColorSetting = function(value) {
    if (Object.prototype.hasOwnProperty.call(LC6000PrimeDisplay.kDeckColorChoices, value)) {
        return value;
    }
    return "Deck Default";
};

LC6000PrimeDisplay.normalizeBrightnessSetting = function(value) {
    if (Object.prototype.hasOwnProperty.call(LC6000PrimeDisplay.kBrightnessValues, value)) {
        return value;
    }
    return "High";
};

LC6000PrimeDisplay.readSettings = function() {
    var deckAssignmentSetting = LC6000PrimeDisplay.normalizeDeckAssignment(
        String(engine.getSetting("deckAssignment") || "Deck 1"));
    var deckColorSetting = LC6000PrimeDisplay.normalizeDeckColorSetting(
        String(engine.getSetting("deckColor") || "Deck Default"));
    var brightnessSetting = LC6000PrimeDisplay.normalizeBrightnessSetting(
        String(engine.getSetting("screenBrightness") || "High"));

    var deckNumber = LC6000PrimeDisplay.kDeckAssignments[deckAssignmentSetting];
    LC6000PrimeDisplay.state.deckNumber       = deckNumber;
    LC6000PrimeDisplay.state.group            = "[Channel" + deckNumber + "]";
    LC6000PrimeDisplay.state.deckAssignment   = deckAssignmentSetting;
    LC6000PrimeDisplay.state.deckColorSetting = deckColorSetting;
    LC6000PrimeDisplay.state.screenBrightness = brightnessSetting;
    LC6000PrimeDisplay.state.deckColor        =
        LC6000PrimeDisplay.kDeckColorChoices[deckColorSetting] ||
        LC6000PrimeDisplay.kDeckColors[deckNumber] ||
        LC6000PrimeDisplay.kDeckColors[1];
};

// ── MIDI senden ───────────────────────────────────────────────────────────────

LC6000PrimeDisplay.sendShort = function(status, number, value) {
    midi.sendShortMsg(status, number, value);
};

LC6000PrimeDisplay.sendSysex = function(bytes) {
    midi.sendSysexMsg(bytes, bytes.length);
};

LC6000PrimeDisplay.sendDisplayCommand = function(command, payload) {
    var header = [0xF0, 0x00, 0x02, 0x0B, 0x01, 0x10, command, 0x00, payload.length];
    LC6000PrimeDisplay.sendSysex(header.concat(payload, [0xF7]));
};

// ── Display-Basis-Kommandos ───────────────────────────────────────────────────

LC6000PrimeDisplay.startImageDecoding = function() {
    LC6000PrimeDisplay.sendDisplayCommand(0x10, []);
};

LC6000PrimeDisplay.sendKeepAlive = function() {
    LC6000PrimeDisplay.sendDisplayCommand(0x7F, []);
};

LC6000PrimeDisplay.sendBrightness = function() {
    LC6000PrimeDisplay.sendDisplayCommand(
        0x7C,
        [LC6000PrimeDisplay.kBrightnessValues[LC6000PrimeDisplay.state.screenBrightness]]);
};

LC6000PrimeDisplay.toNibblePair = function(byteValue) {
    var value = LC6000PrimeDisplay.clamp(Math.round(byteValue), 0, 255);
    return [(value >> 4) & 0x0F, value & 0x0F];
};

LC6000PrimeDisplay.colorPayload = function(color) {
    return LC6000PrimeDisplay.toNibblePair(color.a * 255)
        .concat(LC6000PrimeDisplay.toNibblePair(color.r * 255))
        .concat(LC6000PrimeDisplay.toNibblePair(color.g * 255))
        .concat(LC6000PrimeDisplay.toNibblePair(color.b * 255));
};

LC6000PrimeDisplay.withAlpha = function(color, alpha) {
    return {
        r: color.r,
        g: color.g,
        b: color.b,
        a: LC6000PrimeDisplay.clamp(alpha, 0.0, 1.0),
    };
};

LC6000PrimeDisplay.setDisplayElementColor = function(elementId, color) {
    LC6000PrimeDisplay.sendDisplayCommand(
        0x0B,
        [elementId].concat(LC6000PrimeDisplay.colorPayload(color)));
};

LC6000PrimeDisplay.sendDisplayDefaultColors = function() {
    LC6000PrimeDisplay.setDisplayElementColor(0, {r: 1.0, g: 1.0, b: 1.0, a: 0.85});
    LC6000PrimeDisplay.setDisplayElementColor(1, LC6000PrimeDisplay.state.deckColor);
    LC6000PrimeDisplay.setDisplayElementColor(2, LC6000PrimeDisplay.kDisplayPlatterColor);
    LC6000PrimeDisplay.setDisplayElementColor(3, LC6000PrimeDisplay.kDisplayPlatterColor);
    LC6000PrimeDisplay.setDisplayElementColor(
        4,
        LC6000PrimeDisplay.withAlpha(
            LC6000PrimeDisplay.state.deckColor,
            LC6000PrimeDisplay.kDisplaySlipRingAlpha));
    LC6000PrimeDisplay.setDisplayElementColor(5, LC6000PrimeDisplay.state.deckColor);
    LC6000PrimeDisplay.setDisplayElementColor(8, {r: 1.0, g: 1.0, b: 1.0, a: 1.0});
};

// ── Layer-Visibility ──────────────────────────────────────────────────────────

LC6000PrimeDisplay.loopTextToIndex = function(text) {
    if (LC6000PrimeDisplay.kDisplayLoopIndexMap[text] !== undefined) {
        return LC6000PrimeDisplay.kDisplayLoopIndexMap[text];
    }
    return 0x0D;
};

LC6000PrimeDisplay.updateVisibleDisplayElements = function() {
    var flags1 = 0x00;
    var flags2 = 0x00;

    var platterEnabled  = LC6000PrimeDisplay.state.trackLoaded;
    var slipEnabled     = platterEnabled && LC6000PrimeDisplay.state.slipEnabled;
    var loopTextEnabled = platterEnabled && LC6000PrimeDisplay.state.showLoopText;

    flags1 |= platterEnabled  ? (1 << 2) : 0;  // Platter Position Ring
    flags1 |= slipEnabled     ? (1 << 4) : 0;  // Slip Position Ring
    flags2 |= loopTextEnabled ? (1 << 1) : 0;  // Loop Text

    LC6000PrimeDisplay.sendDisplayCommand(0x0A, [
        flags1,
        flags2,
        0x00,
        LC6000PrimeDisplay.loopTextToIndex(LC6000PrimeDisplay.state.loopText),
    ]);
};

// ── Loop-Text ─────────────────────────────────────────────────────────────────

LC6000PrimeDisplay.clearLoopTextTimer = function() {
    if (LC6000PrimeDisplay.loopTextTimer !== 0) {
        engine.stopTimer(LC6000PrimeDisplay.loopTextTimer);
        LC6000PrimeDisplay.loopTextTimer = 0;
    }
};

LC6000PrimeDisplay.showLoopText = function() {
    LC6000PrimeDisplay.clearLoopTextTimer();
    LC6000PrimeDisplay.state.showLoopText = LC6000PrimeDisplay.state.trackLoaded;
    LC6000PrimeDisplay.updateVisibleDisplayElements();
    if (!LC6000PrimeDisplay.state.loopEnabled && LC6000PrimeDisplay.state.trackLoaded) {
        LC6000PrimeDisplay.loopTextTimer = engine.beginTimer(
            LC6000PrimeDisplay.kLoopTextHideMs,
            function() {
                LC6000PrimeDisplay.state.showLoopText = false;
                LC6000PrimeDisplay.loopTextTimer = 0;
                LC6000PrimeDisplay.updateVisibleDisplayElements();
            },
            true);
    }
};

LC6000PrimeDisplay.formatLoopSize = function(value) {
    var values = [1/64, 1/32, 1/16, 1/8, 1/4, 1/2, 1, 2, 4, 8, 16, 32, 64];
    var closest   = values[0];
    var bestDelta = Math.abs(value - closest);
    values.forEach(function(candidate) {
        var delta = Math.abs(value - candidate);
        if (delta < bestDelta) { closest = candidate; bestDelta = delta; }
    });
    return LC6000PrimeDisplay.kDisplayLoopTextMap[String(closest)] || "";
};

// ── Platter-Ring ──────────────────────────────────────────────────────────────

LC6000PrimeDisplay.stopRingBlink = function() {
    if (LC6000PrimeDisplay.ringBlinkTimer !== 0) {
        engine.stopTimer(LC6000PrimeDisplay.ringBlinkTimer);
        LC6000PrimeDisplay.ringBlinkTimer = 0;
    }
    LC6000PrimeDisplay.state.ringBlinkVisible = true;
};

LC6000PrimeDisplay.isRingNearEnd = function() {
    return LC6000PrimeDisplay.state.trackLoaded &&
           LC6000PrimeDisplay.state.playIndicator &&
           LC6000PrimeDisplay.state.playposition >= LC6000PrimeDisplay.kRingNearEndThreshold;
};

LC6000PrimeDisplay.applyDisplayPlatterBlinkFrame = function() {
    var platterColor = LC6000PrimeDisplay.state.ringBlinkVisible
        ? LC6000PrimeDisplay.kDisplayPlatterColor
        : LC6000PrimeDisplay.kDisplayPlatterHiddenColor;
    LC6000PrimeDisplay.setDisplayElementColor(2, platterColor);
    LC6000PrimeDisplay.setDisplayElementColor(3, platterColor);
};

LC6000PrimeDisplay.refreshDisplayPlatter = function() {
    if (!LC6000PrimeDisplay.state.trackLoaded || !LC6000PrimeDisplay.state.playIndicator) {
        LC6000PrimeDisplay.stopRingBlink();
        LC6000PrimeDisplay.applyDisplayPlatterBlinkFrame();
        return;
    }
    if (LC6000PrimeDisplay.isRingNearEnd()) {
        if (LC6000PrimeDisplay.ringBlinkTimer === 0) {
            LC6000PrimeDisplay.state.ringBlinkVisible = true;
            LC6000PrimeDisplay.ringBlinkTimer = engine.beginTimer(
                LC6000PrimeDisplay.kRingBlinkIntervalMs,
                function() {
                    LC6000PrimeDisplay.state.ringBlinkVisible =
                        !LC6000PrimeDisplay.state.ringBlinkVisible;
                    LC6000PrimeDisplay.applyDisplayPlatterBlinkFrame();
                });
        }
        LC6000PrimeDisplay.applyDisplayPlatterBlinkFrame();
        return;
    }
    LC6000PrimeDisplay.stopRingBlink();
    LC6000PrimeDisplay.applyDisplayPlatterBlinkFrame();
};

// ── Platter-Position ──────────────────────────────────────────────────────────

LC6000PrimeDisplay.currentTrackBpm = function() {
    if (LC6000PrimeDisplay.state.bpm > 0) {
        return LC6000PrimeDisplay.state.bpm;
    }
    return LC6000PrimeDisplay.state.fileBpm;
};

LC6000PrimeDisplay.currentDisplayRotation = function() {
    var durationSeconds = LC6000PrimeDisplay.state.duration;
    var bpm             = LC6000PrimeDisplay.currentTrackBpm();
    if (durationSeconds > 0 && bpm > 0) {
        var elapsedSeconds = LC6000PrimeDisplay.state.playposition * durationSeconds;
        var rotations      = elapsedSeconds * bpm /
                             (60 * LC6000PrimeDisplay.kBeatsPerPlatterRevolution);
        return rotations - Math.floor(rotations);
    }
    return LC6000PrimeDisplay.state.playposition;
};

LC6000PrimeDisplay.sendPitchBend = function(channel, normalizedValue) {
    var value = LC6000PrimeDisplay.clamp(Math.round(normalizedValue * 0x3FFF), 0, 0x3FFF);
    LC6000PrimeDisplay.sendShort(0xE0 + channel, value & 0x7F, (value >> 7) & 0x7F);
};

LC6000PrimeDisplay.updateDisplayPlatterPosition = function() {
    LC6000PrimeDisplay.sendPitchBend(
        0,
        LC6000PrimeDisplay.state.trackLoaded
            ? LC6000PrimeDisplay.currentDisplayRotation()
            : 0.0);
};

LC6000PrimeDisplay.updateDisplaySlipPosition = function() {
    LC6000PrimeDisplay.sendPitchBend(
        1,
        LC6000PrimeDisplay.state.trackLoaded
            ? LC6000PrimeDisplay.currentDisplayRotation()
            : 0.0);
};

LC6000PrimeDisplay.refreshDisplayMotion = function() {
    if (LC6000PrimeDisplay.state.trackLoaded) {
        LC6000PrimeDisplay.state.playposition = LC6000PrimeDisplay.clamp(
            engine.getValue(LC6000PrimeDisplay.currentGroup(), "playposition"), 0.0, 1.0);
        LC6000PrimeDisplay.state.duration = Math.max(
            Number(engine.getValue(LC6000PrimeDisplay.currentGroup(), "duration")) || 0.0, 0.0);
        LC6000PrimeDisplay.state.bpm = Math.max(
            Number(engine.getValue(LC6000PrimeDisplay.currentGroup(), "bpm")) || 0.0, 0.0);
        LC6000PrimeDisplay.state.fileBpm = Math.max(
            Number(engine.getValue(LC6000PrimeDisplay.currentGroup(), "file_bpm")) || 0.0, 0.0);
    } else {
        LC6000PrimeDisplay.state.playposition = 0.0;
        LC6000PrimeDisplay.state.duration     = 0.0;
        LC6000PrimeDisplay.state.bpm          = 0.0;
        LC6000PrimeDisplay.state.fileBpm      = 0.0;
    }
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

// ── Deck-Connections ──────────────────────────────────────────────────────────

LC6000PrimeDisplay.disconnectDeckConnections = function() {
    LC6000PrimeDisplay.deckConnections.forEach(function(connection) {
        connection.disconnect();
    });
    LC6000PrimeDisplay.deckConnections = [];
};

LC6000PrimeDisplay.connectDeckControl = function(control, callback) {
    var connection = engine.makeConnection(LC6000PrimeDisplay.currentGroup(), control, callback);
    if (connection) {
        LC6000PrimeDisplay.deckConnections.push(connection);
        connection.trigger();
    }
};

LC6000PrimeDisplay.bindDeckConnections = function() {
    LC6000PrimeDisplay.disconnectDeckConnections();
    LC6000PrimeDisplay.connectDeckControl("track_loaded",   LC6000PrimeDisplay.onTrackLoadedChanged);
    LC6000PrimeDisplay.connectDeckControl("playposition",   LC6000PrimeDisplay.onPlaypositionChanged);
    LC6000PrimeDisplay.connectDeckControl("duration",       LC6000PrimeDisplay.onDurationChanged);
    LC6000PrimeDisplay.connectDeckControl("bpm",            LC6000PrimeDisplay.onBpmChanged);
    LC6000PrimeDisplay.connectDeckControl("file_bpm",       LC6000PrimeDisplay.onFileBpmChanged);
    LC6000PrimeDisplay.connectDeckControl("play_indicator", LC6000PrimeDisplay.onPlayIndicatorChanged);
    LC6000PrimeDisplay.connectDeckControl("slip_enabled",   LC6000PrimeDisplay.onSlipEnabledChanged);
    LC6000PrimeDisplay.connectDeckControl("loop_enabled",   LC6000PrimeDisplay.onLoopEnabledChanged);
    LC6000PrimeDisplay.connectDeckControl("beatloop_size",  LC6000PrimeDisplay.onBeatloopSizeChanged);
};

// ── Callbacks ─────────────────────────────────────────────────────────────────

LC6000PrimeDisplay.onTrackLoadedChanged = function(value) {
    LC6000PrimeDisplay.state.trackLoaded = value > 0;
    if (!LC6000PrimeDisplay.state.trackLoaded) {
        LC6000PrimeDisplay.state.playposition = 0.0;
        LC6000PrimeDisplay.state.duration     = 0.0;
        LC6000PrimeDisplay.state.bpm          = 0.0;
        LC6000PrimeDisplay.state.fileBpm      = 0.0;
        LC6000PrimeDisplay.state.showLoopText = false;
    }
    LC6000PrimeDisplay.refreshDisplayPlatter();
    LC6000PrimeDisplay.updateVisibleDisplayElements();
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

LC6000PrimeDisplay.onPlaypositionChanged = function(value) {
    LC6000PrimeDisplay.state.playposition = LC6000PrimeDisplay.clamp(value, 0.0, 1.0);
    LC6000PrimeDisplay.refreshDisplayPlatter();
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

LC6000PrimeDisplay.onDurationChanged = function(value) {
    LC6000PrimeDisplay.state.duration = Math.max(Number(value) || 0.0, 0.0);
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

LC6000PrimeDisplay.onBpmChanged = function(value) {
    LC6000PrimeDisplay.state.bpm = Math.max(Number(value) || 0.0, 0.0);
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

LC6000PrimeDisplay.onFileBpmChanged = function(value) {
    LC6000PrimeDisplay.state.fileBpm = Math.max(Number(value) || 0.0, 0.0);
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

LC6000PrimeDisplay.onPlayIndicatorChanged = function(value) {
    LC6000PrimeDisplay.state.playIndicator = value > 0;
    LC6000PrimeDisplay.refreshDisplayPlatter();
};

LC6000PrimeDisplay.onSlipEnabledChanged = function(value) {
    LC6000PrimeDisplay.state.slipEnabled = value > 0;
    LC6000PrimeDisplay.updateVisibleDisplayElements();
};

LC6000PrimeDisplay.onLoopEnabledChanged = function(value) {
    LC6000PrimeDisplay.state.loopEnabled = value > 0;
    if (LC6000PrimeDisplay.state.loopEnabled) {
        LC6000PrimeDisplay.showLoopText();
    } else if (LC6000PrimeDisplay.state.showLoopText) {
        LC6000PrimeDisplay.showLoopText();
    } else {
        LC6000PrimeDisplay.updateVisibleDisplayElements();
    }
};

LC6000PrimeDisplay.onBeatloopSizeChanged = function(value) {
    LC6000PrimeDisplay.state.beatloopSize = value;
    LC6000PrimeDisplay.state.loopText     = LC6000PrimeDisplay.formatLoopSize(value);
    if (LC6000PrimeDisplay.state.loopText !== "") {
        LC6000PrimeDisplay.showLoopText();
    }
};

// ── Lifecycle ─────────────────────────────────────────────────────────────────

LC6000PrimeDisplay.refreshAll = function() {
    LC6000PrimeDisplay.sendBrightness();
    LC6000PrimeDisplay.sendDisplayDefaultColors();
    LC6000PrimeDisplay.updateVisibleDisplayElements();
    LC6000PrimeDisplay.refreshDisplayMotion();
    LC6000PrimeDisplay.refreshDisplayPlatter();
};

LC6000PrimeDisplay.init = function(_id) {
    LC6000PrimeDisplay.readSettings();
    LC6000PrimeDisplay.startImageDecoding();
    LC6000PrimeDisplay.sendKeepAlive();
    LC6000PrimeDisplay.bindDeckConnections();
    LC6000PrimeDisplay.refreshAll();
    LC6000PrimeDisplay.keepAliveTimer = engine.beginTimer(
        LC6000PrimeDisplay.kKeepAliveIntervalMs,
        function() {
            LC6000PrimeDisplay.sendKeepAlive();
            LC6000PrimeDisplay.refreshDisplayMotion();
        });
    console.log("LC6000PrimeDisplay initialized for " + LC6000PrimeDisplay.currentGroup());
    console.log("LC6000PrimeDisplay expects the WD port and the same deckAssignment as the PR mapping.");
};

LC6000PrimeDisplay.shutdown = function() {
    LC6000PrimeDisplay.stopRingBlink();
    LC6000PrimeDisplay.clearLoopTextTimer();
    LC6000PrimeDisplay.disconnectDeckConnections();
    if (LC6000PrimeDisplay.keepAliveTimer !== 0) {
        engine.stopTimer(LC6000PrimeDisplay.keepAliveTimer);
        LC6000PrimeDisplay.keepAliveTimer = 0;
    }
    LC6000PrimeDisplay.state.trackLoaded  = false;
    LC6000PrimeDisplay.state.showLoopText = false;
    LC6000PrimeDisplay.updateVisibleDisplayElements();
    LC6000PrimeDisplay.updateDisplayPlatterPosition();
    LC6000PrimeDisplay.updateDisplaySlipPosition();
};

