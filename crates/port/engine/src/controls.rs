//! Engine controls, created under their standard names so skins and controller mappings
//! find them: `[Channel1],play`, `[Master],crossfader`, ...

use crate::beats::GridEdit;
use djsugar_control::{Behavior, ConfigKey, ControlHandle, ControlRegistry};

pub const NUM_DECKS: usize = 4;
pub const NUM_HOTCUES: usize = 8;
/// Units 1 to 4 are the decks' effects, 5 the Beat FX (see
/// [`BEAT_FX_UNIT`]), 6 to 9 the decks' pad FX (see [`pad_fx_unit`]) and 10
/// the microphone's (see [`MIC_FX_UNIT`]).
pub const NUM_EFFECT_UNITS: usize = 10;
/// The Beat FX unit, `[EffectRack1_EffectUnit5]`, as on Pioneer mixers: one
/// effect in its first slot, sent to any of the decks.
pub const BEAT_FX_UNIT: usize = 4;
/// The unit a deck's Pad FX pads load their effect into.
pub const fn pad_fx_unit(deck: usize) -> usize {
    BEAT_FX_UNIT + 1 + deck
}
/// The microphone and auxiliary inputs' own unit, the last one (now
/// `[EffectRack1_EffectUnit10]`), so effects on the voice leave the decks'
/// alone. The other units can't be routed to the inputs, nor this one to
/// the decks. The UI reads its number from `EffectList.mic-unit`.
pub const MIC_FX_UNIT: usize = NUM_EFFECT_UNITS - 1;
pub const EFFECTS_PER_UNIT: usize = 3;

pub fn deck_group(deck: usize) -> String {
    format!("[Channel{}]", deck + 1)
}

/// Where a deck starts on the crossfader: odd decks left, even decks right.
pub fn default_orientation(deck: usize) -> f64 {
    if deck.is_multiple_of(2) {
        0.0
    } else {
        2.0
    }
}

pub fn eq_group(deck: usize) -> String {
    format!("[EqualizerRack1_[Channel{}]_Effect1]", deck + 1)
}

pub fn quick_effect_group(deck: usize) -> String {
    format!("[QuickEffectRack1_[Channel{}]]", deck + 1)
}

/// The Model 1-style sculpt filters (see [`crate::dsp::Sculpt`]).
pub fn sculpt_group(deck: usize) -> String {
    format!("[Sculpt_[Channel{}]]", deck + 1)
}

/// Where the sculpt's hpf, freq, gain and lpf knobs rest: flat.
pub(crate) const SCULPT_DEFAULTS: [f64; 4] = [0.0, 0.5, 0.5, 1.0];

pub fn effect_unit_group(unit: usize) -> String {
    format!("[EffectRack1_EffectUnit{}]", unit + 1)
}

pub fn effect_group(unit: usize, effect: usize) -> String {
    format!("[EffectRack1_EffectUnit{}_Effect{}]", unit + 1, effect + 1)
}

/// hotcue_N_type values.
/// The Scratch crossfader curve's cut-in out of the box: a side opens
/// fully within the last 2% of travel, under a millimetre on most
/// controllers.
pub const XFADER_CUT_IN: f64 = 0.02;
pub(crate) const HOTCUE_CUE: f64 = 1.0;
pub(crate) const HOTCUE_LOOP: f64 = 4.0;

pub(crate) struct Hotcue {
    pub(crate) activate: ControlHandle,
    pub(crate) clear: ControlHandle,
    pub(crate) enabled: ControlHandle,
    /// hotcue_N_status: 0 empty, 1 set, 2 active (held).
    pub(crate) status: ControlHandle,
    pub(crate) position: ControlHandle,
    /// A saved loop's end, -1 for a plain cue (hotcue_N_endposition).
    pub(crate) end_position: ControlHandle,
    /// hotcue_N_type: 0 empty, [`HOTCUE_CUE`] or [`HOTCUE_LOOP`].
    pub(crate) kind: ControlHandle,
    pub(crate) goto_play: ControlHandle,
    pub(crate) goto_stop: ControlHandle,
    /// Presses of activate, goto_play, goto_stop and clear, so a press
    /// shorter than a block still counts.
    pub(crate) edges: [Edge; 4],
}

/// One stem's controls, `[ChannelN_StemM]`.
pub(crate) struct StemControls {
    pub(crate) volume: ControlHandle,
    pub(crate) mute: ControlHandle,
    /// 0xRRGGBB, or -1 with no stem.
    pub(crate) color: ControlHandle,
    /// The stem is sent into the deck's stem effect (`stem_fx`), as
    /// `[QuickEffectRack1_[ChannelN_StemM]],enabled`.
    pub(crate) fx: ControlHandle,
}

pub fn stem_group(deck: usize, stem: usize) -> String {
    format!("[Channel{}_Stem{}]", deck + 1, stem + 1)
}

/// A stem's quick effect group.
pub fn stem_fx_group(deck: usize, stem: usize) -> String {
    format!("[QuickEffectRack1_[Channel{}_Stem{}]]", deck + 1, stem + 1)
}

pub(crate) struct DeckControls {
    /// Stems in the loaded track (0 without stems).
    pub(crate) stem_count: ControlHandle,
    pub(crate) stems: [StemControls; crate::stems::MAX_STEMS],
    /// The effect sent stems go into: 1 Echo, 2 Reverb (see
    /// [`crate::effects::STEM_FX`]).
    pub(crate) stem_fx: ControlHandle,
    pub(crate) play: ControlHandle,
    /// Gapless playback: the deck (1 to 4) that starts on the very sample
    /// this deck's track ends, or 0. Cleared once it has started.
    pub(crate) play_next: ControlHandle,
    pub(crate) pfl: ControlHandle,
    /// Crossfader assignment: 0 left, 1 thru (not faded), 2 right.
    /// Decks 1 and 3 start on the left, 2 and 4 on the right.
    pub(crate) orientation: ControlHandle,
    pub(crate) cue_default: ControlHandle,
    /// CUP: seeks to the cue point (or sets it when stopped), plays on release.
    pub(crate) cue_play: ControlHandle,
    pub(crate) cue_point: ControlHandle,
    /// What CUE does when stopped, as `cue_mode`: 3 (Numark) only
    /// sets and returns to the cue point; the others also play while CUE is
    /// held at the cue point.
    pub(crate) cue_mode: ControlHandle,
    pub(crate) sync_enabled: ControlHandle,
    pub(crate) rate: ControlHandle,
    pub(crate) rate_range: ControlHandle,
    /// Requested range, applied with its fader adjustment before sync so
    /// the tempo stays where it is. Zero means no pending request.
    pub(crate) rate_range_set: ControlHandle,
    /// `rate_dir`: which way the tempo fader changes the speed. `rate`
    /// is where the fader is (+1 at the top); the speed is
    /// `1 + rate * rateRange * rate_dir`. -1, the app's default, slows the
    /// deck down as the fader goes up, as on Pioneer players.
    pub(crate) rate_dir: ControlHandle,
    /// Pitch reset: while on, the tempo glides back to 0% and the control
    /// turns itself off when it gets there.
    pub(crate) rate_reset: ControlHandle,
    /// Puts the tempo at 0% at once (`rate_set_zero`).
    pub(crate) rate_set_zero: ControlHandle,
    pub(crate) playposition: ControlHandle,
    pub(crate) volume: ControlHandle,
    pub(crate) pregain: ControlHandle,
    /// The auto gain applied to the deck's track, as a factor (
    /// ReplayGain control of the same name).
    pub(crate) replaygain: ControlHandle,
    pub(crate) vu: ControlHandle,
    pub(crate) vu_lr: [ControlHandle; 2],
    pub(crate) track_loaded: ControlHandle,
    pub(crate) bpm: ControlHandle,
    pub(crate) duration: ControlHandle,
    pub(crate) beat_active: ControlHandle,
    pub(crate) loop_enabled: ControlHandle,
    pub(crate) loop_start: ControlHandle,
    pub(crate) loop_end: ControlHandle,
    /// Lit from loop_in until loop_out closes the loop (or another loop
    /// starts): the loop is being marked out.
    pub(crate) loop_in_pending: ControlHandle,
    pub(crate) beatloop_size: ControlHandle,
    pub(crate) quantize: ControlHandle,
    /// What quantize snaps to, in beats: 1/16 to 4 (a bar), 1 by default.
    pub(crate) quantize_beats: ControlHandle,
    /// 0, or 1 while synced decks follow this deck, or 2 while it is
    /// pinned as their leader (set 2 to pin, 0 to unpin).
    pub(crate) sync_leader: ControlHandle,
    /// beatloop_N_enabled lights, by loop size in beats.
    pub(crate) beatloop_lights: Vec<(f64, ControlHandle)>,
    pub(crate) pushes: Vec<Push>,
    /// `[Instrument1],keyboard_pads`: the KEYBOARD pads play the instrument
    /// rather than the deck.
    pub(crate) instrument_pads: ControlHandle,
    pub(crate) eq: [ControlHandle; 3],
    pub(crate) eq_kill: [ControlHandle; 3],
    pub(crate) filter: ControlHandle,
    pub(crate) quick_enabled: ControlHandle,
    /// The Color FX the filter knob plays (`loaded_chain_preset`), and the
    /// pushes that step it.
    pub(crate) cfx_preset: ControlHandle,
    pub(crate) cfx_next: ControlHandle,
    pub(crate) cfx_prev: ControlHandle,
    pub(crate) cfx_edges: [Edge; 2],
    /// `[Sculpt_[ChannelN]]` hpf, freq, gain and lpf.
    pub(crate) sculpt: [ControlHandle; 4],
    pub(crate) hotcues: Vec<Hotcue>,
    /// hotcue_focus: the last hot cue pressed (1-based), -1 for none.
    pub(crate) hotcue_focus: ControlHandle,
    /// Speed while scratch2_enable is on, in multiples of normal speed.
    pub(crate) scratch2: ControlHandle,
    pub(crate) scratch2_enable: ControlHandle,
    /// Timecode vinyl or CD drives the deck (DVS).
    pub(crate) vinylcontrol_enabled: ControlHandle,
    /// 0 absolute (the needle's place is the track's), 1 relative (speed only).
    pub(crate) vinylcontrol_mode: ControlHandle,
    /// The input the turntable is plugged into, 1-based; 0 for none.
    pub(crate) vinylcontrol_input: ControlHandle,
    /// Which record or CD, by its place in `djsugar_timecode::FORMATS`.
    pub(crate) vinylcontrol_format: ControlHandle,
    /// Set by the engine: 0 off or no signal, 1 speed only, 2 place known.
    pub(crate) vinylcontrol_status: ControlHandle,
    /// Holding the scrolling waveform: while `scrub_enable` is on the deck glides
    /// to `scrub_position` (0..1 through the track), playing what it passes.
    pub(crate) scrub_enable: ControlHandle,
    pub(crate) scrub_position: ControlHandle,
    /// Dragging on the small overview: while `seek_hold` is on the deck is
    /// silent and stays where `playposition` puts it; letting go carries on
    /// from there, playing or paused.
    pub(crate) seek_hold: ControlHandle,
    /// Jog wheel movement since the engine last looked; the engine zeroes it.
    pub(crate) jog: ControlHandle,
    /// Jog rim ticks since the engine last looked, 720 to a turn: a pitch
    /// bend that follows how fast the rim turns. The engine zeroes it.
    pub(crate) jog_bend: ControlHandle,
    /// Fast seek speed while non-zero, in multiples of normal speed.
    pub(crate) rate_search: ControlHandle,
    /// rate_temp_up, rate_temp_down, rate_temp_up_small, rate_temp_down_small.
    pub(crate) rate_temp: [ControlHandle; 4],
    pub(crate) reverse: ControlHandle,
    pub(crate) reverseroll: ControlHandle,
    /// Slip mode: while on, a scratch, loop, held hot cue, reverse or pause
    /// keeps a silent playhead running, and the track carries on from it
    /// when the trick ends.
    pub(crate) slip_enabled: ControlHandle,
    /// Where the silent playhead is while slipping, in seconds; -1 when not
    /// slipping (DJ Sugar's own, for the ghost playhead on the waveforms).
    pub(crate) slip_position: ControlHandle,
    pub(crate) mute: ControlHandle,
    pub(crate) play_indicator: ControlHandle,
    pub(crate) cue_indicator: ControlHandle,
    pub(crate) peak: ControlHandle,
    pub(crate) beatjump_size: ControlHandle,
    /// Set to N to jump N beats; the engine zeroes it.
    pub(crate) beatjump: ControlHandle,
    /// Set to N to multiply the loop length by N; the engine zeroes it.
    pub(crate) loop_scale: ControlHandle,
    /// Moves the loop by that many beats (loop_move), or by that many
    /// of its own lengths (DJ Sugar's loop_move_loops, VirtualDJ's loop_move).
    pub(crate) loop_move: [ControlHandle; 2],
    /// Set to N to move the beat grid N seconds later (negative: earlier),
    /// as a drag of the waveform does; the engine zeroes it.
    pub(crate) grid_move: ControlHandle,
    pub(crate) eject: ControlHandle,
    pub(crate) beatsync: ControlHandle,
    /// beatlooproll_N_activate by size, plus beatlooproll_activate as size 0
    /// (meaning beatloop_size). Each loops while held.
    pub(crate) rolls: Vec<(f64, ControlHandle, Edge)>,
    /// SLICER (DJ Sugar's own): while on, the 8 beats from where it was
    /// switched on loop, one beat on each pad.
    pub(crate) slicer_enabled: ControlHandle,
    /// `slicer_N_activate`: plays beat N of the section, in time, while held.
    pub(crate) slicer_pads: [(ControlHandle, Edge); 8],
    /// `slicer_N_status`: beat N of the section is playing (the pad's light).
    pub(crate) slicer_status: [ControlHandle; 8],
    /// `freeze_N_activate` (DJ Sugar's own): repeats the Nth eighth of the
    /// beat playing when the first pad went down, while held (slip).
    pub(crate) freeze_pads: [(ControlHandle, Edge); 8],
    pub(crate) keylock: ControlHandle,
    /// The loaded track's beat grid is locked (`bpmlock`): turning
    /// it on or off locks or unlocks the grid.
    pub(crate) bpmlock: ControlHandle,
    /// Clicks on each beat of the grid, louder on bar 1 (DJ Sugar's own), to
    /// check the grid by ear.
    pub(crate) metronome: ControlHandle,
    /// The deck's grid edit mode (DJ Sugar's own): while on, the scrolling
    /// waveform draws the grid louder and dragging it moves the grid. Turns
    /// off when a new track loads.
    pub(crate) beats_edit: ControlHandle,
    /// A live input played through the deck instead of a track (DJ Sugar's
    /// own): 0 none, else 1 + the input (see [`crate::inputs`]), so 1 the
    /// microphone, 2 the auxiliary input, 3 the instrument.
    pub(crate) live_input: ControlHandle,
    /// The live input's trim before the channel, in dB (-12 to +24), for
    /// gear that comes in quiet (DJ Sugar's own).
    pub(crate) live_trim: ControlHandle,
    /// The live input's balance, -1 left to 1 right (DJ Sugar's own).
    pub(crate) live_pan: ControlHandle,
    /// Key shift in semitones on top of the file's key (`pitch`;
    /// `pitch_adjust` is the same control here).
    pub(crate) pitch: ControlHandle,
    /// The key being played and the file's key, in the `key` control's numbering.
    pub(crate) key: ControlHandle,
    pub(crate) file_key: ControlHandle,
    pub(crate) sync_key: ControlHandle,
    pub(crate) sync_key_edge: Edge,
}

impl DeckControls {
    pub(crate) fn create(reg: &ControlRegistry, deck: usize) -> Self {
        let g = deck_group(deck);
        let g = g.as_str();
        let eqg = eq_group(deck);
        let qg = quick_effect_group(deck);
        alias(reg, g, "peak_indicator", g, "PeakIndicator");
        alias(reg, g, "vu_meter", g, "VuMeter");
        alias(reg, g, "vu_meter_left", g, "VuMeterL");
        alias(reg, g, "vu_meter_right", g, "VuMeterR");
        reg.indicator(g, "playposition_seconds", 3600.0);
        // A pad panel a controller's pad mode buttons ask the deck to show
        // (0 hot cues, 1 pad FX 1, 2 beat jump, 3 sampler, 4 stems, 5 beat
        // loop, 6 pad FX 2, 7 keyboard, 8 key shift, 9 loop roll, 10 slicer,
        // 11 freeze); the skin shows it and sets this back to -1.
        reg.create(g, "pad_panel", -1.0, -1.0, 16.0, Behavior::Free);
        // Touching the jog's top scratches (on, as on vinyl) or only bends
        // like the rim (off, CDJ mode). Only controller mappings read it;
        // on by default (DJ Sugar's own, as VirtualDJ's vinyl_mode).
        reg.create(g, "vinyl_mode", 1.0, 0.0, 1.0, Behavior::Toggle);
        // Asks the skin to open pad tab 0-7 by its place (a controller's pad
        // mode buttons: 0-3 plain, 4-7 with Shift), whatever panel it shows;
        // the skin sets this back to -1.
        reg.create(g, "pad_tab", -1.0, -1.0, 7.0, Behavior::Free);
        // The pad panel in each of the eight pad tabs (numbers as for
        // `pad_panel`), in the DDJ-FLX4's pad mode order. The skin changes
        // them; a controller's pads follow them.
        for (tab, panel) in [0.0, 1.0, 2.0, 3.0, 7.0, 6.0, 5.0, 8.0]
            .into_iter()
            .enumerate()
        {
            reg.create(
                g,
                &format!("pad_slot_{}", tab + 1),
                panel,
                0.0,
                16.0,
                Behavior::Free,
            );
        }
        // Which 8 sampler slots the deck's SAMPLER pads play: 0 slots 1-8,
        // 1 slots 9-16.
        reg.create(g, "sampler_page", 0.0, 0.0, 1.0, Behavior::Free);
        alias(reg, g, "pitch_adjust", g, "pitch");
        // The old name for reloop_toggle, which many mappings still use.
        alias(reg, g, "reloop_exit", g, "reloop_toggle");
        // The chain presets are the Color FX (see `crate::effects::COLOR_FX`):
        // 0 none, 1 Filter, then the others. One is chosen for all decks.
        let n_cfx = crate::effects::COLOR_FX.len() as f64;
        let cfx_preset = reg.create(&qg, "loaded_chain_preset", 1.0, 0.0, n_cfx, Behavior::Free);
        reg.indicator(&qg, "num_chain_presets", 64.0)
            .set(n_cfx + 1.0);
        let cfx_next = reg.push(&qg, "next_chain_preset");
        let cfx_prev = reg.push(&qg, "prev_chain_preset");
        // The EQ slot says how many knobs and kills it has;
        // mappings that build their EQ knobs from these find none otherwise.
        reg.indicator(&eqg, "num_parameters", 3.0).set(3.0);
        reg.indicator(&eqg, "num_button_parameters", 3.0).set(3.0);
        let hotcues = (1..=NUM_HOTCUES)
            .map(|n| Hotcue {
                activate: reg.push(g, &format!("hotcue_{n}_activate")),
                clear: reg.push(g, &format!("hotcue_{n}_clear")),
                enabled: reg.indicator(g, &format!("hotcue_{n}_enabled"), 1.0),
                status: reg.indicator(g, &format!("hotcue_{n}_status"), 2.0),
                position: reg.indicator(g, &format!("hotcue_{n}_position"), 3600.0),
                end_position: reg.create(
                    g,
                    &format!("hotcue_{n}_endposition"),
                    -1.0,
                    -1.0,
                    3600.0,
                    Behavior::Indicator,
                ),
                kind: reg.indicator(g, &format!("hotcue_{n}_type"), 8.0),
                goto_play: reg.push(g, &format!("hotcue_{n}_gotoandplay")),
                goto_stop: reg.push(g, &format!("hotcue_{n}_gotoandstop")),
                edges: [Edge::default(); 4],
            })
            .collect();
        let mut rolls: Vec<_> = BEATLOOP_SIZES
            .iter()
            .map(|&size| {
                let key = format!("beatlooproll_{}_activate", size_name(size));
                (size, reg.push(g, &key), Edge::default())
            })
            .collect();
        rolls.push((0.0, reg.push(g, "beatlooproll_activate"), Edge::default()));
        // The STEMS pads' Stems FX layer, where a stem's pad echoes it out
        // instead of muting it; the screen and controllers share it.
        reg.toggle(g, "stem_fx_mode");
        Self {
            stem_count: reg.indicator(g, "stem_count", crate::stems::MAX_STEMS as f64),
            stems: std::array::from_fn(|s| {
                let sg = stem_group(deck, s);
                let color = reg.create(&sg, "color", -1.0, -1.0, 16_777_215.0, Behavior::Indicator);
                StemControls {
                    // Up to +6 dB, so the mixer's stem knobs centre on full volume.
                    volume: reg.create(&sg, "volume", 1.0, 0.0, 2.0, Behavior::CenteredOnDefault),
                    mute: reg.toggle(&sg, "mute"),
                    color,
                    fx: reg.toggle(&stem_fx_group(deck, s), "enabled"),
                }
            }),
            stem_fx: reg.create(
                g,
                "stem_fx",
                1.0,
                1.0,
                crate::effects::STEM_FX.len() as f64,
                Behavior::Free,
            ),
            play: reg.toggle(g, "play"),
            play_next: reg.create(g, "play_next", 0.0, 0.0, 4.0, Behavior::Free),
            pfl: reg.toggle(g, "pfl"),
            orientation: reg.create(
                g,
                "orientation",
                default_orientation(deck),
                0.0,
                2.0,
                Behavior::Free,
            ),
            cue_default: reg.push(g, "cue_default"),
            cue_play: reg.push(g, "cue_play"),
            cue_point: reg.indicator(g, "cue_point", 3600.0),
            cue_mode: reg.create(g, "cue_mode", 1.0, 0.0, 5.0, Behavior::Free),
            sync_enabled: reg.toggle(g, "sync_enabled"),
            rate: reg.create(g, "rate", 0.0, -1.0, 1.0, Behavior::Linear),
            rate_range: reg.create(g, "rateRange", 0.08, 0.01, 1.0, Behavior::Linear),
            rate_range_set: reg.create(g, "rate_range_set", 0.0, 0.0, 1.0, Behavior::Free),
            rate_dir: reg.create(g, "rate_dir", 1.0, -1.0, 1.0, Behavior::Free),
            rate_reset: reg.toggle(g, "rate_reset"),
            rate_set_zero: reg.push(g, "rate_set_zero"),
            playposition: reg.create(g, "playposition", 0.0, 0.0, 1.0, Behavior::Linear),
            volume: reg.create(g, "volume", 1.0, 0.0, 1.0, Behavior::Linear),
            pregain: reg.create(g, "pregain", 1.0, 0.0, 4.0, Behavior::CenteredOnDefault),
            replaygain: reg.indicator(g, "replaygain", 16.0),
            vu: reg.indicator(g, "VuMeter", 1.0),
            vu_lr: ["L", "R"].map(|s| reg.indicator(g, &format!("VuMeter{s}"), 1.0)),
            track_loaded: reg.indicator(g, "track_loaded", 1.0),
            bpm: reg.indicator(g, "bpm", 300.0),
            duration: reg.indicator(g, "duration", 3600.0),
            beat_active: reg.indicator(g, "beat_active", 1.0),
            loop_enabled: reg.toggle(g, "loop_enabled"),
            // Mappings expect these in samples; the engine uses seconds.
            loop_start: reg.indicator(g, "loop_start_position", 3600.0),
            loop_end: reg.indicator(g, "loop_end_position", 3600.0),
            loop_in_pending: reg.indicator(g, "loop_in_pending", 1.0),
            beatloop_size: reg.create(g, "beatloop_size", 4.0, 1.0 / 32.0, 512.0, Behavior::Linear),
            quantize: {
                let q = reg.toggle(g, "quantize");
                q.set_bool(true);
                q
            },
            quantize_beats: reg.create(g, "quantize_beats", 1.0, 1.0 / 16.0, 4.0, Behavior::Free),
            sync_leader: reg.create(g, "sync_leader", 0.0, 0.0, 2.0, Behavior::Free),
            beatloop_lights: BEATLOOP_SIZES
                .iter()
                .map(|&size| {
                    (
                        size,
                        reg.indicator(g, &format!("beatloop_{}_enabled", size_name(size)), 1.0),
                    )
                })
                .collect(),
            pushes: deck_pushes(reg, g),
            instrument_pads: reg.toggle("[Instrument1]", "keyboard_pads"),
            eq: [1, 2, 3].map(|n| {
                reg.create(
                    &eqg,
                    &format!("parameter{n}"),
                    1.0,
                    0.0,
                    4.0,
                    Behavior::CenteredOnDefault,
                )
            }),
            eq_kill: [1, 2, 3].map(|n| reg.toggle(&eqg, &format!("button_parameter{n}"))),
            filter: reg.create(&qg, "super1", 0.5, 0.0, 1.0, Behavior::Linear),
            sculpt: {
                let sg = sculpt_group(deck);
                let keys = ["hpf", "freq", "gain", "lpf"];
                std::array::from_fn(|i| {
                    reg.create(&sg, keys[i], SCULPT_DEFAULTS[i], 0.0, 1.0, Behavior::Linear)
                })
            },
            quick_enabled: {
                let e = reg.toggle(&qg, "enabled");
                e.set_bool(true);
                e
            },
            cfx_preset,
            cfx_next,
            cfx_prev,
            cfx_edges: [Edge::default(); 2],
            hotcues,
            hotcue_focus: reg.create(
                g,
                "hotcue_focus",
                -1.0,
                -1.0,
                NUM_HOTCUES as f64,
                Behavior::Free,
            ),
            scratch2: reg.create(g, "scratch2", 0.0, -100.0, 100.0, Behavior::Free),
            scratch2_enable: reg.toggle(g, "scratch2_enable"),
            vinylcontrol_enabled: reg.toggle(g, "vinylcontrol_enabled"),
            vinylcontrol_mode: reg.create(g, "vinylcontrol_mode", 0.0, 0.0, 1.0, Behavior::Free),
            vinylcontrol_input: reg.create(
                g,
                "vinylcontrol_input",
                0.0,
                0.0,
                crate::inputs::NUM_INPUTS as f64,
                Behavior::Free,
            ),
            vinylcontrol_format: reg.create(
                g,
                "vinylcontrol_format",
                0.0,
                0.0,
                (djsugar_timecode::FORMATS.len() - 1) as f64,
                Behavior::Free,
            ),
            vinylcontrol_status: reg.create(
                g,
                "vinylcontrol_status",
                0.0,
                0.0,
                2.0,
                Behavior::Free,
            ),
            scrub_enable: reg.toggle(g, "scrub_enable"),
            scrub_position: reg.create(g, "scrub_position", 0.0, 0.0, 1.0, Behavior::Linear),
            seek_hold: reg.toggle(g, "seek_hold"),
            jog: reg.create(g, "jog", 0.0, -1000.0, 1000.0, Behavior::Free),
            jog_bend: reg.create(g, "jog_bend", 0.0, -1000.0, 1000.0, Behavior::Free),
            rate_search: reg.create(g, "rateSearch", 0.0, -300.0, 300.0, Behavior::Free),
            rate_temp: ["up", "down", "up_small", "down_small"]
                .map(|k| reg.push(g, &format!("rate_temp_{k}"))),
            reverse: reg.toggle(g, "reverse"),
            reverseroll: reg.push(g, "reverseroll"),
            slip_enabled: reg.toggle(g, "slip_enabled"),
            slip_position: reg.create(
                g,
                "slip_position",
                -1.0,
                -1.0,
                f64::MAX,
                Behavior::Indicator,
            ),
            mute: reg.toggle(g, "mute"),
            play_indicator: reg.indicator(g, "play_indicator", 1.0),
            cue_indicator: reg.indicator(g, "cue_indicator", 1.0),
            peak: reg.indicator(g, "PeakIndicator", 1.0),
            beatjump_size: reg.create(g, "beatjump_size", 4.0, 1.0 / 32.0, 512.0, Behavior::Linear),
            beatjump: reg.create(g, "beatjump", 0.0, -512.0, 512.0, Behavior::Free),
            loop_scale: reg.create(g, "loop_scale", 0.0, 0.0, 64.0, Behavior::Free),
            loop_move: ["loop_move", "loop_move_loops"]
                .map(|k| reg.create(g, k, 0.0, -512.0, 512.0, Behavior::Free)),
            grid_move: reg.create(g, "beats_translate_by", 0.0, -60.0, 60.0, Behavior::Free),
            eject: reg.push(g, "eject"),
            beatsync: reg.push(g, "beatsync"),
            rolls,
            slicer_enabled: reg.toggle(g, "slicer_enabled"),
            slicer_pads: std::array::from_fn(|n| {
                (
                    reg.push(g, &format!("slicer_{}_activate", n + 1)),
                    Edge::default(),
                )
            }),
            slicer_status: std::array::from_fn(|n| {
                reg.indicator(g, &format!("slicer_{}_status", n + 1), 1.0)
            }),
            freeze_pads: std::array::from_fn(|n| {
                (
                    reg.push(g, &format!("freeze_{}_activate", n + 1)),
                    Edge::default(),
                )
            }),
            keylock: reg.toggle(g, "keylock"),
            bpmlock: reg.toggle(g, "bpmlock"),
            metronome: reg.toggle(g, "metronome"),
            beats_edit: reg.toggle(g, "beats_edit"),
            live_input: reg.create(
                g,
                "live_input",
                0.0,
                0.0,
                crate::inputs::MIXER as f64,
                Behavior::Free,
            ),
            live_trim: reg.create(g, "live_trim", 0.0, -12.0, 24.0, Behavior::Linear),
            live_pan: reg.create(g, "live_pan", 0.0, -1.0, 1.0, Behavior::Linear),
            pitch: reg.create(g, "pitch", 0.0, -6.0, 6.0, Behavior::Linear),
            key: reg.indicator(g, "key", 24.0),
            file_key: reg.indicator(g, "file_key", 24.0),
            sync_key: reg.push(g, "sync_key"),
            sync_key_edge: Edge::default(),
        }
    }
}

/// Loop sizes with their own pads (`beatloop_N_activate`), from the
/// 1/16-beat loop rolls up.
pub const BEATLOOP_SIZES: [f64; 11] = [
    0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0,
];
/// The KEYBOARD pads, `keyboard_N_activate`: the hot cue played this many
/// semitones up or down, as on the DDJ-FLX4's KEYBOARD pads in rekordbox.
/// The top row climbs from the cue's own key, the bottom row goes below it.
pub const KEYBOARD_SEMITONES: [f64; 8] = [0.0, 1.0, 2.0, 3.0, -4.0, -3.0, -2.0, -1.0];
/// Beat jump sizes with their own buttons (`beatjump_N_forward` / `_backward`).
pub const BEATJUMP_SIZES: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

/// Formats a beat count the way control names do: `0.5`, `1`, `16`.
pub fn size_name(size: f64) -> String {
    if size.fract() == 0.0 {
        format!("{}", size as i64)
    } else {
        format!("{size}")
    }
}

/// What a momentary button does when pressed.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    BeatLoop(f64),
    BeatLoopToggle,
    LoopHalve,
    LoopDouble,
    LoopIn,
    LoopOut,
    Reloop,
    /// A tap of the deck's tempo: taps in time set the track's BPM.
    BpmTap,
    BeatJump(f64),
    /// Jumps beatjump_size beats in the given direction (+1 or -1).
    BeatJumpSized(f64),
    /// Nudges the tempo by a fraction of normal speed (0.001 = 0.1%).
    RateStep(f64),
    /// Seeks to the start; `Some(play)` also starts or stops playback.
    Start(Option<bool>),
    End,
    /// Seeks to the cue point, then plays or stops.
    CueGoto(bool),
    ReloopAndStop,
    /// Changes the loaded track's beat grid at the play position.
    Grid(GridEdit),
    /// Shifts the key by this many semitones.
    KeyStep(f64),
    /// Sets the key shift to this many semitones and plays the last hot cue
    /// pressed (else the first one set, else the cue point) from its start.
    Keyboard(f64),
    KeyReset,
}

pub(crate) struct Push {
    pub(crate) control: ControlHandle,
    pub(crate) edge: Edge,
    pub(crate) action: Action,
}

pub(crate) fn deck_pushes(reg: &ControlRegistry, g: &str) -> Vec<Push> {
    let mut v = vec![
        ("beatloop_activate".to_string(), Action::BeatLoopToggle),
        ("loop_halve".into(), Action::LoopHalve),
        ("loop_double".into(), Action::LoopDouble),
        ("loop_in".into(), Action::LoopIn),
        ("loop_out".into(), Action::LoopOut),
        ("reloop_toggle".into(), Action::Reloop),
        ("bpm_tap".into(), Action::BpmTap),
        ("rate_perm_up_small".into(), Action::RateStep(0.001)),
        ("rate_perm_down_small".into(), Action::RateStep(-0.001)),
        // The default coarse permanent rate step is 0.5%.
        ("rate_perm_up".into(), Action::RateStep(0.005)),
        ("rate_perm_down".into(), Action::RateStep(-0.005)),
        ("beatjump_forward".into(), Action::BeatJumpSized(1.0)),
        ("beatjump_backward".into(), Action::BeatJumpSized(-1.0)),
        ("start".into(), Action::Start(None)),
        ("start_play".into(), Action::Start(Some(true))),
        ("start_stop".into(), Action::Start(Some(false))),
        ("end".into(), Action::End),
        ("cue_gotoandplay".into(), Action::CueGoto(true)),
        ("cue_gotoandstop".into(), Action::CueGoto(false)),
        ("reloop_andstop".into(), Action::ReloopAndStop),
        (
            "beats_translate_curpos".into(),
            Action::Grid(GridEdit::BeatHere),
        ),
        (
            "beats_translate_earlier".into(),
            Action::Grid(GridEdit::Shift(-0.005)),
        ),
        (
            "beats_translate_later".into(),
            Action::Grid(GridEdit::Shift(0.005)),
        ),
        (
            "beats_adjust_faster".into(),
            Action::Grid(GridEdit::Tempo(0.01)),
        ),
        (
            "beats_adjust_slower".into(),
            Action::Grid(GridEdit::Tempo(-0.01)),
        ),
        ("beats_undo_adjustment".into(), Action::Grid(GridEdit::Undo)),
        // bar 1 on the beat here, and back to the grid
        // analysis found.
        (
            "beats_downbeat_here".into(),
            Action::Grid(GridEdit::BarHere),
        ),
        ("beats_reset".into(), Action::Grid(GridEdit::Reset)),
        // tempo sections and halving or doubling.
        (
            "beats_section_add".into(),
            Action::Grid(GridEdit::AddSection),
        ),
        (
            "beats_section_remove".into(),
            Action::Grid(GridEdit::RemoveSection),
        ),
        (
            "beats_scale_half".into(),
            Action::Grid(GridEdit::Scale(0.5)),
        ),
        (
            "beats_scale_double".into(),
            Action::Grid(GridEdit::Scale(2.0)),
        ),
        ("pitch_up".into(), Action::KeyStep(1.0)),
        ("pitch_down".into(), Action::KeyStep(-1.0)),
        ("pitch_up_small".into(), Action::KeyStep(0.1)),
        ("pitch_down_small".into(), Action::KeyStep(-0.1)),
        ("reset_key".into(), Action::KeyReset),
    ];
    for size in BEATLOOP_SIZES {
        // _activate here already turns off a loop of the same size, as _toggle does.
        for suffix in ["activate", "toggle"] {
            v.push((
                format!("beatloop_{}_{suffix}", size_name(size)),
                Action::BeatLoop(size),
            ));
        }
    }
    for (n, semitones) in KEYBOARD_SEMITONES.iter().enumerate() {
        v.push((
            format!("keyboard_{}_activate", n + 1),
            Action::Keyboard(*semitones),
        ));
    }
    for size in BEATJUMP_SIZES {
        v.push((
            format!("beatjump_{}_forward", size_name(size)),
            Action::BeatJump(size),
        ));
        v.push((
            format!("beatjump_{}_backward", size_name(size)),
            Action::BeatJump(-size),
        ));
    }
    v.into_iter()
        .map(|(key, action)| Push {
            control: reg.push(g, &key),
            edge: Edge::default(),
            action,
        })
        .collect()
}

pub(crate) struct MasterControls {
    pub(crate) crossfader: ControlHandle,
    pub(crate) gain: ControlHandle,
    pub(crate) head_gain: ControlHandle,
    pub(crate) head_mix: ControlHandle,
    /// Booth output level, on channels 5/6 of the main device.
    pub(crate) booth_gain: ControlHandle,
    /// Headphones hear the cue on the left and the main mix on the right,
    /// both in mono.
    pub(crate) head_split: ControlHandle,
    /// The crossfader's curve: 0 smooth (constant power), 1 linear, 2 a
    /// sharp cut at the ends for scratching.
    pub(crate) xfader_shape: ControlHandle,
    /// DJ Sugar: on the Scratch curve, how far from its end a side opens
    /// fully, as a share of the crossfader's travel (0.02 is 2%).
    pub(crate) xfader_cut_in: ControlHandle,
    pub(crate) xfader_reverse: ControlHandle,
    /// `[Mixer] crossfader_enabled`: off, every deck plays as if set to
    /// thru and the crossfader does nothing, unless Automix is mixing.
    pub(crate) xfader_enabled: ControlHandle,
    /// `[AutoDJ] enabled`, which keeps the crossfader working for Automix.
    pub(crate) autodj_enabled: ControlHandle,
    pub(crate) balance: ControlHandle,
    pub(crate) vu: ControlHandle,
    pub(crate) vu_l: ControlHandle,
    pub(crate) vu_r: ControlHandle,
    /// PeakIndicator, PeakIndicatorL and PeakIndicatorR.
    pub(crate) peak: [ControlHandle; 3],
    /// Auto gain: levels every track to `auto_gain_target` LUFS.
    pub(crate) auto_gain: ControlHandle,
    pub(crate) auto_gain_target: ControlHandle,
    /// How many semitones, at most, a track loaded while another deck plays
    /// is shifted so its key mixes with that deck's: 0 leaves it alone.
    pub(crate) key_match_on_load: ControlHandle,
    /// What loading a track puts back to normal on its deck: `[Controls]`
    /// auto_reset_fx (effects off), auto_reset_eq (EQ, kills, filter and
    /// sculpt), auto_reset_gain (the gain knob) and auto_reset_tempo (the
    /// tempo fader back to 0%). All on to start.
    pub(crate) auto_reset: [ControlHandle; 4],
    /// `[Controls] pfl_on_load`: loading a track cues its deck in the
    /// headphones and takes the other decks out of the cue. On to start;
    /// automix loads leave the cue alone.
    pub(crate) pfl_on_load: ControlHandle,
    /// `[Master] test_tone`: beeps for checking the outputs, 1 in the
    /// master mix, 2 in the headphones, 0 off. The guided tour plays it.
    pub(crate) test_tone: ControlHandle,
    /// `[Master] air_ceiling`: the limiter's ceiling in dBTP while recording
    /// or on air (-1 or -2).
    pub(crate) air_ceiling: ControlHandle,
    /// `[Master] deck_tap`: what the deck and sampler outputs carry, 0
    /// Clean, 1 Pre-fader (to start), 2 Post-fader (see
    /// [`crate::engine::DeckTap`]).
    pub(crate) deck_tap: ControlHandle,
    /// `[Master] external_mixer`: a hardware mixer does the mixing. Each
    /// deck goes out Pre-fader at full level, with DJ Sugar's EQ, filter,
    /// faders, crossfader, cue, master effects and limiter left out.
    pub(crate) external_mixer: ControlHandle,
    /// `[Master] record_source`: what recording, going on air and the
    /// loudness meter take: 0 the main mix, 1 to 8 Auxiliary input 1 to 8
    /// as it comes in (a hardware mixer's record out).
    pub(crate) record_source: ControlHandle,
    /// `[Master] latency_measure`: set to an input (as in
    /// `[ChannelN],vinylcontrol_input`) to play a short noise burst out of
    /// the main output and time it coming back in; the engine sets it back
    /// to 0 when the burst starts.
    pub(crate) latency_measure: ControlHandle,
    /// `[Master] input_latency`: seconds the last measurement found from the
    /// main output back into the input, or -1 if the input never heard it.
    pub(crate) input_latency: ControlHandle,
}

pub(crate) fn create_global_controls(reg: &ControlRegistry) -> MasterControls {
    for g in ["[Master]", "[App]"] {
        reg.indicator(g, "num_decks", 4.0).set(NUM_DECKS as f64);
        reg.indicator(g, "num_samplers", 64.0);
    }
    for g in ["[Master]", "[Main]"] {
        for (new, old) in [
            ("peak_indicator", "PeakIndicator"),
            ("peak_indicator_left", "PeakIndicatorL"),
            ("peak_indicator_right", "PeakIndicatorR"),
            ("vu_meter", "VuMeter"),
            ("vu_meter_left", "VuMeterL"),
            ("vu_meter_right", "VuMeterR"),
        ] {
            alias(reg, g, new, "[Master]", old);
        }
    }
    // What the four knobs down each side of the mixer control: 0 EQ and
    // filter, 1 stem volumes, 2 sculpt. Controllers' EQ knobs follow it.
    reg.create("[Mixer]", "knob_mode", 0.0, 0.0, 2.0, Behavior::Free);
    // Off: a track can't be loaded onto a deck that is playing.
    reg.create(
        "[Controls]",
        "AllowTrackLoadToPlayingDeck",
        0.0,
        0.0,
        1.0,
        Behavior::Free,
    );
    MasterControls {
        balance: reg.create("[Master]", "balance", 0.0, -1.0, 1.0, Behavior::Linear),
        vu: reg.indicator("[Master]", "VuMeter", 1.0),
        peak: ["", "L", "R"].map(|s| reg.indicator("[Master]", &format!("PeakIndicator{s}"), 1.0)),
        crossfader: reg.create("[Master]", "crossfader", 0.0, -1.0, 1.0, Behavior::Linear),
        gain: reg.create("[Master]", "gain", 0.5, 0.0, 1.0, Behavior::Linear),
        head_gain: reg.create("[Master]", "headGain", 0.25, 0.0, 1.0, Behavior::Linear),
        head_mix: reg.create("[Master]", "headMix", -1.0, -1.0, 1.0, Behavior::Linear),
        booth_gain: reg.create("[Master]", "booth_gain", 1.0, 0.0, 1.0, Behavior::Linear),
        head_split: reg.toggle("[Master]", "headSplit"),
        xfader_shape: reg.create(
            "[Mixer Profile]",
            "xFaderShape",
            0.0,
            0.0,
            2.0,
            Behavior::Free,
        ),
        xfader_cut_in: reg.create(
            "[Mixer Profile]",
            "xFaderCutIn",
            XFADER_CUT_IN,
            0.001,
            0.5,
            Behavior::Free,
        ),
        xfader_reverse: reg.toggle("[Mixer Profile]", "xFaderReverse"),
        xfader_enabled: reg.create(
            "[Mixer]",
            "crossfader_enabled",
            1.0,
            0.0,
            1.0,
            Behavior::Toggle,
        ),
        autodj_enabled: reg.toggle("[AutoDJ]", "enabled"),
        test_tone: reg.create("[Master]", "test_tone", 0.0, 0.0, 2.0, Behavior::Free),
        air_ceiling: reg.create(
            "[Master]",
            "air_ceiling",
            crate::engine::DEFAULT_AIR_CEILING_DB,
            -6.0,
            0.0,
            Behavior::Free,
        ),
        deck_tap: reg.create("[Master]", "deck_tap", 1.0, 0.0, 2.0, Behavior::Free),
        external_mixer: reg.toggle("[Master]", "external_mixer"),
        record_source: reg.create(
            "[Master]",
            "record_source",
            0.0,
            0.0,
            crate::inputs::NUM_AUXILIARIES as f64,
            Behavior::Free,
        ),
        latency_measure: reg.create(
            "[Master]",
            "latency_measure",
            0.0,
            0.0,
            crate::inputs::NUM_INPUTS as f64,
            Behavior::Free,
        ),
        input_latency: reg.create("[Master]", "input_latency", 0.0, -1.0, 10.0, Behavior::Free),
        vu_l: reg.indicator("[Master]", "VuMeterL", 1.0),
        vu_r: reg.indicator("[Master]", "VuMeterR", 1.0),
        // The ReplayGain switch lives in the [ReplayGain] group too.
        auto_gain: reg.create(
            "[ReplayGain]",
            "ReplayGainEnabled",
            1.0,
            0.0,
            1.0,
            Behavior::Toggle,
        ),
        auto_gain_target: reg.create(
            "[ReplayGain]",
            "target_lufs",
            DEFAULT_AUTO_GAIN_TARGET,
            -30.0,
            0.0,
            Behavior::Free,
        ),
        key_match_on_load: reg.create(
            "[Controls]",
            "key_match_on_load",
            0.0,
            0.0,
            2.0,
            Behavior::Free,
        ),
        auto_reset: ["fx", "eq", "gain", "tempo"].map(|k| {
            let c = reg.toggle("[Controls]", &format!("auto_reset_{k}"));
            c.set_bool(true);
            c
        }),
        pfl_on_load: {
            let c = reg.toggle("[Controls]", "pfl_on_load");
            c.set_bool(true);
            c
        },
    }
}

/// The loudness auto gain levels tracks to by default, in LUFS: where
/// streaming services level music, about 6 dB below a typical club master.
pub const DEFAULT_AUTO_GAIN_TARGET: f64 = -14.0;

/// Newer mappings name the meters differently (`VuMeter` is now `vu_meter`, ...); mappings
/// use both names.
pub(crate) fn alias(
    reg: &ControlRegistry,
    group: &str,
    key: &str,
    target_group: &str,
    target: &str,
) {
    reg.insert_alias(
        ConfigKey::new(group, key),
        ConfigKey::new(target_group, target),
    );
}

/// Detects presses of a momentary control. Also catches a press and release that
/// both happened between two ticks, which a plain "was it down" check would miss.
#[derive(Default, Clone, Copy)]
pub(crate) struct Edge {
    down: bool,
    version: u32,
}

impl Edge {
    pub(crate) fn pressed(&mut self, c: &ControlHandle) -> bool {
        let (down, version) = (c.get_bool(), c.version());
        let changes = version.wrapping_sub(self.version);
        let pressed = (down && !self.down) || (!down && changes >= 2);
        self.down = down;
        self.version = version;
        pressed
    }
}
