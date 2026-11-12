var LC6000Prime = {};

LC6000Prime.kRingBlinkIntervalMs = 400;
LC6000Prime.kRingNearEndThreshold = 0.90;
LC6000Prime.kTrackSearchIntervalMs = 50;
LC6000Prime.kTrackSearchStep = 0.01;
LC6000Prime.kBeatgridHalfShift = 0.5;
LC6000Prime.kRgbGamma = 3.5;

LC6000Prime.kJogTicksPerRevolution = 1024;
LC6000Prime.kScratchAlpha = 1 / 8;
LC6000Prime.kScratchBeta = LC6000Prime.kScratchAlpha / 32;

LC6000Prime.kNotes = {
    play: 0x01,
    cue: 0x02,
    beatJumpBack: 0x03,
    beatJumpForward: 0x04,
    trackSkipPrev: 0x05,
    trackSkipNext: 0x06,
    censor: 0x07,
    loopIn: 0x08,
    loopOut: 0x09,
    autoLoopPress: 0x0A,
    back: 0x10,
    forward: 0x11,
    selectPress: 0x12,
    vinyl: 0x13,
    sync: 0x14,
    master: 0x15,
    keyLock: 0x16,
    slip: 0x17,
    pitchDown: 0x18,
    pitchUp: 0x19,
    shift: 0x1A,
    hotCueMode: 0x1B,
    rollMode: 0x1C,
    slicerMode: 0x1D,
    loopMode: 0x1E,
    platterTouch: 0x28,
    pitchArrowBack: 0x29,
    pitchCenter: 0x2A,
    pitchArrowForward: 0x2B,
    parameterBack: 0x44,
    parameterForward: 0x45,
    needleTouch: 0x46,
};

LC6000Prime.kPadNotes = [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27];

LC6000Prime.kCC = {
    autoLoopTurn: 0x03,
    selectTurn: 0x06,
    pitchMsb: 0x08,
    pitchLsb: 0x28,
    jogLsb: 0x31,
    jogMsb: 0x11,
    needleScrub: 0x40,
};

LC6000Prime.kDeckAssignments = {
    "Deck 1": 1,
    "Deck 2": 2,
    "Deck 3": 3,
    "Deck 4": 4,
};

LC6000Prime.kDeckColors = {
    1: {r: 0.01, g: 0.91, b: 0.44, a: 1.0},
    2: {r: 0.11, g: 0.57, b: 1.0, a: 1.0},
    3: {r: 1.0, g: 0.45, b: 0.0, a: 1.0},
    4: {r: 0.78, g: 0.15, b: 1.0, a: 1.0},
};

LC6000Prime.kDeckColorChoices = {
    "Deck Default": null,
    Green: LC6000Prime.kDeckColors[1],
    Blue: LC6000Prime.kDeckColors[2],
    Orange: LC6000Prime.kDeckColors[3],
    Purple: LC6000Prime.kDeckColors[4],
};

LC6000Prime.kDeckPadColors = {
    1: 0x09,
    2: 0x04,
    3: 0x3D,
    4: 0x11,
};

LC6000Prime.kPadColors = {
    off: 0x00,
    hotcueOn: 0x04,
    hotcueOff: 0x02,
    rollOn: 0x20,
    rollOff: 0x06,
    loopOn: 0x3D,
    loopOff: 0x15,
    slicerOn: 0x11,
    slicerOff: 0x12,
    placeholder: 0x01,
};

LC6000Prime.kRateRanges = [
    4 / 100,
    6 / 100,
    8 / 100,
    10 / 100,
    16 / 100,
    24 / 100,
    50 / 100,
    90 / 100,
];

LC6000Prime.kLoopSizeSteps = [
    1 / 32,
    1 / 16,
    1 / 8,
    1 / 4,
    1 / 2,
    1,
    2,
    4,
    8,
    16,
    32,
    64,
    128,
];

LC6000Prime.kAutoLoopPadSizes = [
    1 / 8,
    1 / 4,
    1 / 2,
    1,
    2,
    4,
    8,
    16,
];

LC6000Prime.kRollPadSizes = [
    1 / 8,
    1 / 6,
    1 / 4,
    1 / 3,
    1 / 2,
    2 / 3,
    1,
    2,
];

LC6000Prime.state = {
    deckNumber: 1,
    group: "[Channel1]",
    deckAssignment: "Deck 1",
    deckColorSetting: "Deck Default",
    vinylJogSensitivity: 1,
    nonVinylJogSensitivity: 0.33,
    vinylMode: true,
    padMode: "hotcue",
    trackLoaded: false,
    playposition: 0.0,
    playIndicator: false,
    cueIndicator: false,
    syncEnabled: false,
    syncLeader: false,
    keylock: false,
    slipEnabled: false,
    loopEnabled: false,
    beatloopSize: 4,
    beatjumpSize: 16,
    rate: 0.0,
    shift: false,
    selectPressed: false,
    selectTurnedWhilePressed: false,
    needleTouched: false,
    platterTouched: false,
    pitchMsb: 0,
    deckColor: LC6000Prime.kDeckColors[1],
    ringColor: LC6000Prime.kDeckColors[1],
    ringBlinkVisible: true,
    hotcueEnabled: [false, false, false, false, false, false, false, false],
    activeRollPads: [false, false, false, false, false, false, false, false],
    searchDirection: 0,
};

LC6000Prime.deckConnections = [];
LC6000Prime.ringBlinkTimer = 0;
LC6000Prime.trackSearchTimer = 0;

LC6000Prime.clamp = function(value, minValue, maxValue) {
    return Math.max(minValue, Math.min(maxValue, value));
};

LC6000Prime.currentGroup = function() {
    return LC6000Prime.state.group;
};

LC6000Prime.currentDeck = function() {
    return LC6000Prime.state.deckNumber;
};

LC6000Prime.isPress = function(value) {
    return value > 0;
};

LC6000Prime.trigger = function(group, control) {
    script.triggerControl(group, control);
};

LC6000Prime.normalizeDeckAssignment = function(value) {
    if (Object.prototype.hasOwnProperty.call(LC6000Prime.kDeckAssignments, value)) {
        return value;
    }
    return "Deck 1";
};

LC6000Prime.normalizeDeckColorSetting = function(value) {
    if (Object.prototype.hasOwnProperty.call(LC6000Prime.kDeckColorChoices, value)) {
        return value;
    }
    return "Deck Default";
};

LC6000Prime.normalizeJogSensitivitySetting = function(value) {
  return value / 100;
}

LC6000Prime.readJogSensitivitySettings = function() {
    var vinylJogSensitivity = Number(engine.getSetting("vinylJogSensitivity"));
    var nonVinylJogSensitivity = Number(engine.getSetting("nonVinylJogSensitivity"));

    vinylJogSensitivity = LC6000Prime.normalizeJogSensitivitySetting(vinylJogSensitivity);
    nonVinylJogSensitivity = LC6000Prime.normalizeJogSensitivitySetting(nonVinylJogSensitivity);

    LC6000Prime.state.vinylJogSensitivity = vinylJogSensitivity;
    LC6000Prime.state.nonVinylJogSensitivity = nonVinylJogSensitivity;
};

LC6000Prime.readSettings = function() {
    var deckAssignmentSetting = LC6000Prime.normalizeDeckAssignment(
            String(engine.getSetting("deckAssignment") || "Deck 1"));
    var deckColorSetting = LC6000Prime.normalizeDeckColorSetting(
            String(engine.getSetting("deckColor") || "Deck Default"));
    var deckNumber = LC6000Prime.kDeckAssignments[deckAssignmentSetting];
    LC6000Prime.state.deckNumber = deckNumber;
    LC6000Prime.state.group = "[Channel" + deckNumber + "]";
    LC6000Prime.state.deckAssignment = deckAssignmentSetting;
    LC6000Prime.state.deckColorSetting = deckColorSetting;
    LC6000Prime.state.deckColor =
            LC6000Prime.kDeckColorChoices[deckColorSetting] ||
            LC6000Prime.kDeckColors[deckNumber] ||
            LC6000Prime.kDeckColors[1];
    LC6000Prime.state.ringColor = LC6000Prime.state.deckColor;

    var vinylSetting = engine.getSetting("vinylModeOn");
    LC6000Prime.state.vinylMode = vinylSetting !== false && vinylSetting !== "false";
    LC6000Prime.readJogSensitivitySettings();
};

LC6000Prime.sendShort = function(status, number, value) {
    midi.sendShortMsg(status, number, value);
};

LC6000Prime.sendLed = function(note, value) {
    LC6000Prime.sendShort(0x90, note, value);
};

LC6000Prime.sendBinaryLed = function(note, on, dimWhenOff) {
    LC6000Prime.sendLed(note, on ? 0x7F : (dimWhenOff ? 0x01 : 0x00));
};

LC6000Prime.sendPadColor = function(index, colorValue) {
    LC6000Prime.sendLed(LC6000Prime.kPadNotes[index], colorValue);
};

LC6000Prime.rgbLedChannel = function(value, gamma) {
    return LC6000Prime.clamp(
            Math.floor(Math.pow(LC6000Prime.clamp(value, 0.0, 1.0), gamma) * 127),
            0,
            127);
};

LC6000Prime.sendRgbLedColor = function(index, color, gamma) {
    var ledColor = color || {r: 0.0, g: 0.0, b: 0.0};
    LC6000Prime.sendSysex([
        0xF0,
        0x00,
        0x02,
        0x0B,
        0x7F,
        0x10,
        0x03,
        0x00,
        0x04,
        index,
        LC6000Prime.rgbLedChannel(ledColor.r, gamma),
        LC6000Prime.rgbLedChannel(ledColor.g, gamma),
        LC6000Prime.rgbLedChannel(ledColor.b, gamma),
        0xF7,
    ]);
};

LC6000Prime.sendRingColor = function(color) {
    LC6000Prime.sendRgbLedColor(0x28, color, LC6000Prime.kRgbGamma);
};

LC6000Prime.sendSysex = function(bytes) {
    midi.sendSysexMsg(bytes, bytes.length);
};

LC6000Prime.relativeSigned = function(value) {
    return value < 0x40 ? value : value - 0x80;
};

LC6000Prime.nearestStep = function(value, steps) {
    var nearest = steps[0];
    var bestDelta = Math.abs(value - nearest);

    steps.forEach(function(step) {
        var delta = Math.abs(value - step);
        if (delta < bestDelta) {
            nearest = step;
            bestDelta = delta;
        }
    });

    return nearest;
};

LC6000Prime.stepValue = function(currentValue, direction, steps) {
    var nearest = LC6000Prime.nearestStep(currentValue, steps);
    var index = steps.indexOf(nearest);
    var nextIndex = LC6000Prime.clamp(index + direction, 0, steps.length - 1);
    return steps[nextIndex];
};

LC6000Prime.setSteppedControl = function(group, control, direction, steps) {
    var currentValue = engine.getValue(group, control);
    var nextValue = LC6000Prime.stepValue(currentValue, direction, steps);
    engine.setValue(group, control, nextValue);
};

LC6000Prime.getLoopMoveSize = function(group) {
    return LC6000Prime.nearestStep(engine.getValue(group, "beatloop_size"), LC6000Prime.kLoopSizeSteps);
};

LC6000Prime.toggleVinylMode = function() {
    LC6000Prime.state.vinylMode = !LC6000Prime.state.vinylMode;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.changePadMode = function(mode) {
    LC6000Prime.state.padMode = mode;
    LC6000Prime.refreshPads();
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.selectPadMode = function(mode) {
    if (LC6000Prime.state.padMode === mode) {
        return;
    }
    LC6000Prime.changePadMode(mode);
};

LC6000Prime.startTrackSearch = function(direction) {
    LC6000Prime.stopTrackSearch();
    LC6000Prime.state.searchDirection = direction;
    LC6000Prime.trackSearchTimer = engine.beginTimer(
            LC6000Prime.kTrackSearchIntervalMs,
            function() {
                var group = LC6000Prime.currentGroup();
                var currentPosition = engine.getValue(group, "playposition");
                engine.setValue(
                        group,
                        "playposition",
                        LC6000Prime.clamp(
                                currentPosition + (direction * LC6000Prime.kTrackSearchStep),
                                0.0,
                                1.0));
            });
};

LC6000Prime.stopTrackSearch = function() {
    if (LC6000Prime.trackSearchTimer !== 0) {
        engine.stopTimer(LC6000Prime.trackSearchTimer);
        LC6000Prime.trackSearchTimer = 0;
        LC6000Prime.state.searchDirection = 0;
    }
};

LC6000Prime.shiftBeatgridHalf = function(direction) {
    var control = direction < 0 ? "beats_translate_earlier" : "beats_translate_later";
    engine.setParameter(LC6000Prime.currentGroup(), control, LC6000Prime.kBeatgridHalfShift);
};

LC6000Prime.stopRingBlink = function() {
    if (LC6000Prime.ringBlinkTimer !== 0) {
        engine.stopTimer(LC6000Prime.ringBlinkTimer);
        LC6000Prime.ringBlinkTimer = 0;
    }
    LC6000Prime.state.ringBlinkVisible = true;
};

LC6000Prime.isRingNearEnd = function() {
    return LC6000Prime.state.trackLoaded &&
            LC6000Prime.state.playIndicator &&
            LC6000Prime.state.playposition >= LC6000Prime.kRingNearEndThreshold;
};

LC6000Prime.applyRingBlinkFrame = function() {
    LC6000Prime.sendRingColor(
            LC6000Prime.state.ringBlinkVisible ? LC6000Prime.state.ringColor : null);
};

LC6000Prime.refreshRing = function() {
    if (!LC6000Prime.state.trackLoaded || !LC6000Prime.state.playIndicator) {
        LC6000Prime.stopRingBlink();
        LC6000Prime.sendRingColor(null);
        return;
    }

    if (LC6000Prime.isRingNearEnd()) {
        if (LC6000Prime.ringBlinkTimer === 0) {
            LC6000Prime.state.ringBlinkVisible = true;
            LC6000Prime.ringBlinkTimer = engine.beginTimer(
                    LC6000Prime.kRingBlinkIntervalMs,
                    function() {
                        LC6000Prime.state.ringBlinkVisible = !LC6000Prime.state.ringBlinkVisible;
                        LC6000Prime.applyRingBlinkFrame();
                    });
        }
        LC6000Prime.applyRingBlinkFrame();
        return;
    }

    LC6000Prime.stopRingBlink();
    LC6000Prime.sendRingColor(LC6000Prime.state.ringColor);
};

LC6000Prime.applyPadPress = function(index, value) {
    var group = LC6000Prime.currentGroup();

    switch (LC6000Prime.state.padMode) {
    case "hotcue":
        if (LC6000Prime.isPress(value)) {
            if (LC6000Prime.state.shift) {
                LC6000Prime.trigger(group, "hotcue_" + (index + 1) + "_clear");
            } else {
                LC6000Prime.trigger(group, "hotcue_" + (index + 1) + "_activate");
            }
        }
        break;
    case "autoLoop":
        if (LC6000Prime.isPress(value)) {
            LC6000Prime.trigger(group, "beatloop_" + LC6000Prime.kAutoLoopPadSizes[index] + "_toggle");
        }
        break;
    case "roll":
        LC6000Prime.state.activeRollPads[index] = LC6000Prime.isPress(value);
        engine.setValue(group, "beatlooproll_" + LC6000Prime.kRollPadSizes[index] + "_activate", value > 0 ? 1 : 0);
        LC6000Prime.refreshPads();
        break;
    case "manualLoop":
        if (LC6000Prime.isPress(value)) {
            // Saved loop slots are documented by Denon, but Mixxx does not expose a
            // matching generic 8-slot saved loop API for manual XML/JS mappings.
            console.log("LC6000Prime: Manual Loop pad " + (index + 1) + " is documented but left unresolved.");
        }
        break;
    case "slicer":
    case "slicerLoop":
        if (LC6000Prime.isPress(value)) {
            console.log("LC6000Prime: Slicer pad " + (index + 1) + " requires additional beatgrid state logic.");
        }
        break;
    }
};

LC6000Prime.refreshPitchLeds = function() {
    var rate = LC6000Prime.state.rate;
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.pitchArrowBack, rate < -0.001, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.pitchCenter, Math.abs(rate) <= 0.001, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.pitchArrowForward, rate > 0.001, false);
};

LC6000Prime.refreshTransportLeds = function() {
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.play, LC6000Prime.state.playIndicator, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.cue, LC6000Prime.state.cueIndicator, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.vinyl, LC6000Prime.state.vinylMode, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.sync, LC6000Prime.state.syncEnabled, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.master, LC6000Prime.state.syncLeader, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.keyLock, LC6000Prime.state.keylock, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.slip, LC6000Prime.state.slipEnabled, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.shift, LC6000Prime.state.shift, false);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.autoLoopPress, LC6000Prime.state.loopEnabled, true);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.selectPress, LC6000Prime.state.trackLoaded, true);
    LC6000Prime.sendLed(LC6000Prime.kNotes.hotCueMode, LC6000Prime.state.padMode === "hotcue" ? 0x01 : 0x00);
    LC6000Prime.sendLed(LC6000Prime.kNotes.loopMode, LC6000Prime.state.padMode === "autoLoop" ? 0x01 : 0x00);
    LC6000Prime.sendLed(LC6000Prime.kNotes.rollMode, LC6000Prime.state.padMode === "roll" ? 0x01 : 0x00);
    LC6000Prime.sendLed(LC6000Prime.kNotes.slicerMode, LC6000Prime.state.padMode === "slicer" ? 0x01 : 0x00);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.parameterBack, LC6000Prime.state.padMode !== "hotcue", true);
    LC6000Prime.sendBinaryLed(LC6000Prime.kNotes.parameterForward, LC6000Prime.state.padMode !== "hotcue", true);
    LC6000Prime.refreshPitchLeds();
};

LC6000Prime.refreshPads = function() {
    var i;

    switch (LC6000Prime.state.padMode) {
    case "hotcue":
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(
                    i,
                    LC6000Prime.state.hotcueEnabled[i]
                            ? LC6000Prime.kPadColors.hotcueOn
                            : LC6000Prime.kPadColors.hotcueOff);
        }
        break;
    case "autoLoop":
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(
                    i,
                    Math.abs(LC6000Prime.state.beatloopSize - LC6000Prime.kAutoLoopPadSizes[i]) < 0.0001
                            ? LC6000Prime.kPadColors.loopOn
                            : LC6000Prime.kPadColors.loopOff);
        }
        break;
    case "roll":
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(
                    i,
                    LC6000Prime.state.activeRollPads[i]
                            ? LC6000Prime.kPadColors.rollOn
                            : LC6000Prime.kPadColors.rollOff);
        }
        break;
    case "manualLoop":
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(i, LC6000Prime.kPadColors.loopOff);
        }
        break;
    case "slicer":
    case "slicerLoop":
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(i, LC6000Prime.kPadColors.slicerOff);
        }
        break;
    default:
        for (i = 0; i < 8; i++) {
            LC6000Prime.sendPadColor(i, LC6000Prime.kPadColors.off);
        }
        break;
    }

};

LC6000Prime.clearAllLeds = function() {
    var outputNotes = [
        LC6000Prime.kNotes.play,
        LC6000Prime.kNotes.cue,
        LC6000Prime.kNotes.beatJumpBack,
        LC6000Prime.kNotes.beatJumpForward,
        LC6000Prime.kNotes.trackSkipPrev,
        LC6000Prime.kNotes.trackSkipNext,
        LC6000Prime.kNotes.censor,
        LC6000Prime.kNotes.loopIn,
        LC6000Prime.kNotes.loopOut,
        LC6000Prime.kNotes.autoLoopPress,
        LC6000Prime.kNotes.selectPress,
        LC6000Prime.kNotes.vinyl,
        LC6000Prime.kNotes.sync,
        LC6000Prime.kNotes.master,
        LC6000Prime.kNotes.keyLock,
        LC6000Prime.kNotes.slip,
        LC6000Prime.kNotes.pitchDown,
        LC6000Prime.kNotes.pitchUp,
        LC6000Prime.kNotes.shift,
        LC6000Prime.kNotes.pitchArrowBack,
        LC6000Prime.kNotes.pitchCenter,
        LC6000Prime.kNotes.pitchArrowForward,
        LC6000Prime.kNotes.parameterBack,
        LC6000Prime.kNotes.parameterForward,
        LC6000Prime.kNotes.hotCueMode,
        LC6000Prime.kNotes.loopMode,
        LC6000Prime.kNotes.rollMode,
        LC6000Prime.kNotes.slicerMode,
    ];

    outputNotes.forEach(function(note) {
        LC6000Prime.sendLed(note, 0x00);
    });
    LC6000Prime.kPadNotes.forEach(function(note) {
        LC6000Prime.sendLed(note, 0x00);
    });
    LC6000Prime.stopRingBlink();
    LC6000Prime.sendRingColor(null);
};

LC6000Prime.refreshAll = function() {
    LC6000Prime.refreshTransportLeds();
    LC6000Prime.refreshPads();
    LC6000Prime.refreshRing();
};

LC6000Prime.disconnectDeckConnections = function() {
    LC6000Prime.deckConnections.forEach(function(connection) {
        connection.disconnect();
    });
    LC6000Prime.deckConnections = [];
};

LC6000Prime.connectDeckControl = function(control, callback) {
    var connection = engine.makeConnection(LC6000Prime.currentGroup(), control, callback);
    if (connection) {
        LC6000Prime.deckConnections.push(connection);
        connection.trigger();
    }
};

LC6000Prime.bindDeckConnections = function() {
    var i;

    LC6000Prime.disconnectDeckConnections();
    LC6000Prime.connectDeckControl("track_loaded", LC6000Prime.onTrackLoadedChanged);
    LC6000Prime.connectDeckControl("playposition", LC6000Prime.onPlaypositionChanged);
    LC6000Prime.connectDeckControl("play_indicator", LC6000Prime.onPlayIndicatorChanged);
    LC6000Prime.connectDeckControl("cue_indicator", LC6000Prime.onCueIndicatorChanged);
    LC6000Prime.connectDeckControl("sync_enabled", LC6000Prime.onSyncEnabledChanged);
    LC6000Prime.connectDeckControl("sync_leader", LC6000Prime.onSyncLeaderChanged);
    LC6000Prime.connectDeckControl("keylock", LC6000Prime.onKeylockChanged);
    LC6000Prime.connectDeckControl("slip_enabled", LC6000Prime.onSlipEnabledChanged);
    LC6000Prime.connectDeckControl("loop_enabled", LC6000Prime.onLoopEnabledChanged);
    LC6000Prime.connectDeckControl("beatloop_size", LC6000Prime.onBeatloopSizeChanged);
    LC6000Prime.connectDeckControl("beatjump_size", LC6000Prime.onBeatjumpSizeChanged);
    LC6000Prime.connectDeckControl("rate", LC6000Prime.onRateChanged);

    for (i = 1; i <= 8; i++) {
        (function(index) {
            LC6000Prime.connectDeckControl("hotcue_" + index + "_status", function(value) {
                LC6000Prime.state.hotcueEnabled[index - 1] = value > 0;
                LC6000Prime.refreshPads();
            });
        }(i));
    }
};

LC6000Prime.onTrackLoadedChanged = function(value) {
    LC6000Prime.state.trackLoaded = value > 0;
    if (!LC6000Prime.state.trackLoaded) {
        LC6000Prime.state.playposition = 0.0;
    }
    LC6000Prime.refreshTransportLeds();
    LC6000Prime.refreshRing();
};

LC6000Prime.onPlaypositionChanged = function(value) {
    LC6000Prime.state.playposition = LC6000Prime.clamp(value, 0.0, 1.0);
    LC6000Prime.refreshRing();
};

LC6000Prime.onPlayIndicatorChanged = function(value) {
    LC6000Prime.state.playIndicator = value > 0;
    LC6000Prime.refreshTransportLeds();
    LC6000Prime.refreshRing();
};

LC6000Prime.onCueIndicatorChanged = function(value) {
    LC6000Prime.state.cueIndicator = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onSyncEnabledChanged = function(value) {
    LC6000Prime.state.syncEnabled = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onSyncLeaderChanged = function(value) {
    LC6000Prime.state.syncLeader = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onKeylockChanged = function(value) {
    LC6000Prime.state.keylock = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onSlipEnabledChanged = function(value) {
    LC6000Prime.state.slipEnabled = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onLoopEnabledChanged = function(value) {
    LC6000Prime.state.loopEnabled = value > 0;
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.onBeatloopSizeChanged = function(value) {
    LC6000Prime.state.beatloopSize = value;
    LC6000Prime.refreshPads();
};

LC6000Prime.onBeatjumpSizeChanged = function(value) {
    LC6000Prime.state.beatjumpSize = value;
};

LC6000Prime.onRateChanged = function(value) {
    LC6000Prime.state.rate = value;
    LC6000Prime.refreshPitchLeds();
};

LC6000Prime.play = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        LC6000Prime.trigger(group, "cue_gotoandplay");
    } else {
        script.toggleControl(group, "play");
    }
};

LC6000Prime.cue = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        engine.setValue(group, "start_stop", value > 0 ? 1 : 0);
        return;
    }
    engine.setValue(group, "cue_default", value > 0 ? 1 : 0);
};

LC6000Prime.trackSkipPrev = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        if (LC6000Prime.isPress(value)) {
            LC6000Prime.shiftBeatgridHalf(-1);
        }
        return;
    }
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (engine.getValue(group, "play_indicator") === 0 && engine.getValue(group, "playposition") > 0.01) {
        engine.setValue(group, "playposition", 0.0);
        return;
    }
    engine.setValue("[Playlist]", "SelectPrevTrack", 1);
    engine.setValue(group, "LoadSelectedTrack", 1);
};

LC6000Prime.trackSkipNext = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        if (LC6000Prime.isPress(value)) {
            LC6000Prime.shiftBeatgridHalf(1);
        }
        return;
    }
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    engine.setValue("[Playlist]", "SelectNextTrack", 1);
    engine.setValue(group, "LoadSelectedTrack", 1);
};

LC6000Prime.beatJumpBack = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        LC6000Prime.setSteppedControl(group, "beatjump_size", -1, LC6000Prime.kLoopSizeSteps);
    } else {
        LC6000Prime.trigger(group, "beatjump_backward");
    }
};

LC6000Prime.beatJumpForward = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        LC6000Prime.setSteppedControl(group, "beatjump_size", 1, LC6000Prime.kLoopSizeSteps);
    } else {
        LC6000Prime.trigger(group, "beatjump_forward");
    }
};

LC6000Prime.censor = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        engine.setValue(group, "reverseroll", value > 0 ? 1 : 0);
        return;
    }
    engine.setValue(group, "reverse", value > 0 ? 1 : 0);
};

LC6000Prime.loopIn = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.trigger(LC6000Prime.currentGroup(), "loop_in");
    }
};

LC6000Prime.loopOut = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.trigger(LC6000Prime.currentGroup(), "loop_out");
    }
};

LC6000Prime.autoLoopPress = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (engine.getValue(group, "loop_enabled") > 0) {
        LC6000Prime.trigger(group, "reloop_exit");
    } else {
        LC6000Prime.trigger(group, "beatloop_activate");
    }
};

LC6000Prime.parameterBack = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.padMode === "manualLoop" || LC6000Prime.state.padMode === "autoLoop") {
        LC6000Prime.setSteppedControl(group, "beatloop_size", -1, LC6000Prime.kLoopSizeSteps);
    } else {
        LC6000Prime.setSteppedControl(group, "beatjump_size", -1, LC6000Prime.kLoopSizeSteps);
    }
};

LC6000Prime.parameterForward = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.padMode === "manualLoop" || LC6000Prime.state.padMode === "autoLoop") {
        LC6000Prime.setSteppedControl(group, "beatloop_size", 1, LC6000Prime.kLoopSizeSteps);
    } else {
        LC6000Prime.setSteppedControl(group, "beatjump_size", 1, LC6000Prime.kLoopSizeSteps);
    }
};

LC6000Prime.back = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        if (LC6000Prime.state.shift) {
            engine.setValue("[Library]", "MoveFocusBackward", 1);
        } else {
            engine.setValue("[Tab]", "overview", 1);
        }
    }
};

LC6000Prime.forward = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        if (LC6000Prime.state.shift) {
            engine.setValue("[Library]", "MoveFocusForward", 1);
        } else {
            engine.setValue("[Tab]", "library", 1);
        }
    }
};

LC6000Prime.selectPress = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.state.selectPressed = true;
        LC6000Prime.state.selectTurnedWhilePressed = false;
        return;
    }

    if (!LC6000Prime.state.selectPressed) {
        return;
    }

    LC6000Prime.state.selectPressed = false;
    if (LC6000Prime.state.selectTurnedWhilePressed) {
        LC6000Prime.state.selectTurnedWhilePressed = false;
        return;
    }

    if (LC6000Prime.state.shift) {
        engine.setValue("[PreviewDeck1]", "LoadSelectedTrackAndPlay", 1);
    } else {
        engine.setValue(LC6000Prime.currentGroup(), "LoadSelectedTrack", 1);
    }
};

LC6000Prime.selectTurn = function(_channel, _control, value) {
    var direction = LC6000Prime.relativeSigned(value) < 0 ? -1 : 1;
    if (LC6000Prime.state.selectPressed) {
        LC6000Prime.state.selectTurnedWhilePressed = true;
        LC6000Prime.trigger(
                LC6000Prime.currentGroup(),
                direction > 0 ? "waveform_zoom_down" : "waveform_zoom_up");
    } else if (LC6000Prime.state.shift) {
        LC6000Prime.trigger(
                LC6000Prime.currentGroup(),
                direction > 0 ? "waveform_zoom_down" : "waveform_zoom_up");
    } else {
        engine.setValue("[Library]", "MoveVertical", direction);
    }
};

LC6000Prime.vinyl = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.toggleVinylMode();
    }
};

LC6000Prime.sync = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        engine.setValue(group, "sync_enabled", 0);
    } else {
        engine.setValue(group, "beatsync", 1);
    }
};

LC6000Prime.master = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    engine.setValue(group, "sync_leader", engine.getValue(group, "sync_leader") > 0 ? 0 : 1);
};

LC6000Prime.keyLock = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        script.toggleControl(group, "quantize");
    } else {
        script.toggleControl(group, "keylock");
    }
};

LC6000Prime.slip = function(_channel, _control, value) {
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    if (LC6000Prime.state.shift) {
        console.log("LC6000Prime: Deck assignment is fixed by the loaded Deck1-Deck4 preset.");
    } else {
        script.toggleControl(LC6000Prime.currentGroup(), "slip_enabled");
    }
};

LC6000Prime.pitchDown = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        if (LC6000Prime.isPress(value)) {
            LC6000Prime.setSteppedControl(group, "rateRange", -1, LC6000Prime.kRateRanges);
        }
        return;
    }
    engine.setValue(group, "rate_temp_down", value > 0 ? 1 : 0);
};

LC6000Prime.pitchUp = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    if (LC6000Prime.state.shift) {
        if (LC6000Prime.isPress(value)) {
            LC6000Prime.setSteppedControl(group, "rateRange", 1, LC6000Prime.kRateRanges);
        }
        return;
    }
    engine.setValue(group, "rate_temp_up", value > 0 ? 1 : 0);
};

LC6000Prime.shift = function(_channel, _control, value) {
    LC6000Prime.state.shift = value > 0;
    if (!LC6000Prime.state.shift) {
        LC6000Prime.stopTrackSearch();
    }
    LC6000Prime.refreshTransportLeds();
};

LC6000Prime.hotCueMode = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.selectPadMode("hotcue");
    }
};

LC6000Prime.loopMode = function(_channel, _control, value) {
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    LC6000Prime.selectPadMode("autoLoop");
};

LC6000Prime.rollMode = function(_channel, _control, value) {
    if (LC6000Prime.isPress(value)) {
        LC6000Prime.selectPadMode("roll");
    }
};

LC6000Prime.slicerMode = function(_channel, _control, value) {
    if (!LC6000Prime.isPress(value)) {
        return;
    }
    LC6000Prime.selectPadMode("slicer");
};

LC6000Prime.pad1 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(0, value);
};

LC6000Prime.pad2 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(1, value);
};

LC6000Prime.pad3 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(2, value);
};

LC6000Prime.pad4 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(3, value);
};

LC6000Prime.pad5 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(4, value);
};

LC6000Prime.pad6 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(5, value);
};

LC6000Prime.pad7 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(6, value);
};

LC6000Prime.pad8 = function(_channel, _control, value) {
    LC6000Prime.applyPadPress(7, value);
};


LC6000Prime.platterTouch = function(_channel, _control, value) {
  LC6000Prime.state.platterTouched = value > 0;

  if(!LC6000Prime.state.vinylMode) {
    return;
  }

  var deck = LC6000Prime.currentDeck();
  if(LC6000Prime.state.platterTouched) {
    engine.scratchEnable(
            deck,
            LC6000Prime.kJogTicksPerRevolution,
            33 + (1 / 3),
            LC6000Prime.kScratchAlpha,
            LC6000Prime.kScratchBeta,
            true);
  }
  else {
    engine.scratchDisable(deck, true);
  }
};

LC6000Prime.jogDirection = function(_channel, _control, value) {
  LC6000Prime.state.jogDirection = value === 0;
};

LC6000Prime.jogRotation = function(_channel, _control, value) {
  var group = LC6000Prime.currentGroup();
  var deck = LC6000Prime.currentDeck();

  var newValue = LC6000Prime.state.jogDirection ? value : (127 - value)*-1

  if(engine.isScratching(deck)) {
    newValue = newValue * LC6000Prime.state.vinylJogSensitivity;
    engine.scratchTick(deck, newValue)
  }
  else {
    newValue = newValue * LC6000Prime.state.nonVinylJogSensitivity;
    engine.setValue(group, 'jog', newValue);
  }
};

LC6000Prime.jogMsb = function(_channel, _control, value) {
  // unused, undocumented too?
};

LC6000Prime.jogLsb = function(_channel, _control, value) {
  // unused, undocumented too?
};

LC6000Prime.pitchMsb = function(_channel, _control, value) {
    LC6000Prime.state.pitchMsb = value;
};

LC6000Prime.pitchLsb = function(_channel, _control, value) {
    var fullValue = (LC6000Prime.state.pitchMsb << 7) + value;
    var position = 1 - (fullValue / 0x3FFF);
    engine.setParameter(LC6000Prime.currentGroup(), "rate", position);
};

LC6000Prime.autoLoopTurn = function(_channel, _control, value) {
    var group = LC6000Prime.currentGroup();
    var direction = LC6000Prime.relativeSigned(value) < 0 ? -1 : 1;
    if (LC6000Prime.state.shift && LC6000Prime.state.loopEnabled) {
        LC6000Prime.trigger(
                group,
                "loop_move_" + LC6000Prime.getLoopMoveSize(group) +
                        (direction > 0 ? "_forward" : "_backward"));
        return;
    }
    if (LC6000Prime.state.loopEnabled) {
        LC6000Prime.trigger(group, direction > 0 ? "loop_double" : "loop_halve");
        return;
    }
    LC6000Prime.setSteppedControl(group, "beatloop_size", direction, LC6000Prime.kLoopSizeSteps);
};

LC6000Prime.needleTouch = function(_channel, _control, value) {
    LC6000Prime.state.needleTouched = value > 0;
};

LC6000Prime.needleScrub = function(_channel, _control, value) {
    if (!LC6000Prime.state.needleTouched) {
        return;
    }
    engine.setValue(LC6000Prime.currentGroup(), "playposition", value / 127);
};

LC6000Prime.init = function(_id) {
    LC6000Prime.readSettings();

    LC6000Prime.bindDeckConnections();
    LC6000Prime.refreshAll();
    console.log("LC6000Prime initialized for " + LC6000Prime.currentGroup());
    console.log("LC6000Prime deck assignment: "
            + LC6000Prime.state.deckAssignment
            + ", color="
            + LC6000Prime.state.deckColorSetting);
    console.log("LC6000Prime jog sensitivity settings: vinyl="
            + LC6000Prime.state.vinylJogSensitivity
            + ", nonVinyl="
            + LC6000Prime.state.nonVinylJogSensitivity
            + ", vinylMode="
            + LC6000Prime.state.vinylMode);
    console.log("Manual Loop saved slots and Slicer need hardware-validated Mixxx logic before full implementation.");
};

LC6000Prime.shutdown = function() {
    LC6000Prime.stopTrackSearch();
    LC6000Prime.stopRingBlink();
    LC6000Prime.disconnectDeckConnections();

    LC6000Prime.clearAllLeds();
};
