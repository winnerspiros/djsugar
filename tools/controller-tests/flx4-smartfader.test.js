const PioneerDDJFLX4 = {SMART_FADER_MAX_LOW_CUT: 1.0};
PioneerDDJFLX4.smartFaderLowGains = function(xf) {
    const x = Math.max(-1, Math.min(1, xf));
    const cut = PioneerDDJFLX4.SMART_FADER_MAX_LOW_CUT;
    const left = 1 - cut * Math.max(0, x);   // leaving the left deck when x > 0
    const right = 1 - cut * Math.max(0, -x); // leaving the right deck when x < 0
    return {ch1: left, ch2: right};
};
const eq=(a,b)=>Math.abs(a-b)<1e-9; let ok=true;
const chk=(n,c)=>{console.log((c?"PASS ":"FAIL ")+n); ok=ok&&c;};
const g=PioneerDDJFLX4.smartFaderLowGains;
chk("centre keeps both", eq(g(0).ch1,1)&&eq(g(0).ch2,1));
chk("right kills deck1", eq(g(1).ch1,0)&&eq(g(1).ch2,1));
chk("left kills deck2", eq(g(-1).ch1,1)&&eq(g(-1).ch2,0));
chk("clamps", eq(g(5).ch1,0)&&eq(g(-5).ch2,0));
chk("monotonic", g(0.2).ch1>g(0.8).ch1);
process.exit(ok?0:1);
