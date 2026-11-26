// Pioneer DDJ-RX: what the RX adds to Mixxx's DDJ-SX mapping, which drives
// everything else (the RX sends the DDJ-SX2's messages). On the RX, note
// 0x35 on the deck channels is QUANTIZE, where the SX reads it as a jog
// touch without vinyl mode.

var PioneerDDJRX = {};

PioneerDDJRX.connections = [];

PioneerDDJRX.init = function() {
    for (var i = 0; i < 4; i++) {
        PioneerDDJRX.connections.push(
            engine.makeConnection("[Channel" + (i + 1) + "]", "quantize", PioneerDDJRX.quantizeLed));
        PioneerDDJRX.quantizeLed(engine.getValue("[Channel" + (i + 1) + "]", "quantize"), "[Channel" + (i + 1) + "]");
    }
};

PioneerDDJRX.shutdown = function() {
    for (var i = 0; i < 4; i++) {
        midi.sendShortMsg(0x90 + i, 0x35, 0x00);
    }
};

PioneerDDJRX.quantizeLed = function(value, group) {
    var deck = parseInt(group.substring(8, 9)) - 1;
    midi.sendShortMsg(0x90 + deck, 0x35, value ? 0x7F : 0x00);
};

PioneerDDJRX.quantize = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "quantize");
    }
};
