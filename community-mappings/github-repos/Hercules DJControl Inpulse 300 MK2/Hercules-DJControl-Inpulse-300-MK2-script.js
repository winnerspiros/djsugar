// Hercules DJControl Inpulse 300 MK2 additions for Mixxx.
//
// The mapping shipped with Mixxx targets the original Inpulse 300, which has a
// dedicated VINYL button. The MK2 has none: vinyl (scratch) mode is SHIFT + SLIP
// and SHIFT + Q changes the tempo range. This file is loaded after the official
// script (prefix DJCi300) and only adds what the MK2 needs.

/* global DJCi300, engine, midi */

var DJCi300MK2 = {}; // "var" on purpose: Mixxx resolves "DJCi300MK2.x" on the global object

// Tempo ranges cycled by SHIFT + Q (same set as Serato DJ Lite on this controller).
DJCi300MK2.rateRanges = [0.08, 0.16, 0.50];

DJCi300MK2.init = function() {
    // Vinyl mode starts enabled (see DJCi300.init): light the shifted SLIP LEDs.
    midi.sendShortMsg(0x94, 0x01, 0x7F);
    midi.sendShortMsg(0x95, 0x01, 0x7F);
};

DJCi300MK2.shutdown = function() {
    // Nothing to do: DJCi300.shutdown already turns every LED off.
};

// SHIFT + SLIP: toggle vinyl (scratch) mode. Delegates to the official handler,
// which also lights the LED of the button that was pressed (shifted SLIP here).
DJCi300MK2.vinylButton = function(channel, control, value, status, group) {
    DJCi300.vinylButton(channel, control, value, status, group);
};

// SHIFT + Q: cycle the tempo fader range.
DJCi300MK2.rateRangeCycle = function(_channel, _control, value, _status, group) {
    if (!value) {
        return;
    }
    const current = engine.getValue(group, "rateRange");
    let next = DJCi300MK2.rateRanges.findIndex(r => r > current + 0.001);
    if (next === -1) {
        next = 0;
    }
    engine.setValue(group, "rateRange", DJCi300MK2.rateRanges[next]);
};
