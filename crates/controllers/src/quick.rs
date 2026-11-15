//! Quick map: a walk through the controls most controllers have, deck 1's
//! first, each learned as Map mode learns one (see [`crate::learn`]). The
//! other decks aren't walked: pressing one control of each (PLAY) says how
//! its messages differ from deck 1's (another MIDI channel, as on most
//! controllers, or other numbers), and deck 1's rows are copied over.

use crate::format::{DjSugarMapping, Midi, Row};

/// The layer of the rows for decks 3 and 4, which only work when the
/// controller is used with four decks (see [`DjSugarMapping::decks`]).
pub const FOUR_DECKS: &str = "decks=4";

/// One control the walk asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The control, `[Channel1],play`.
    pub control: String,
    /// The hardware control's name, for its row: "HIGH EQ".
    pub name: String,
    /// What to do: "Turn the HIGH EQ knob".
    pub prompt: String,
    /// The deck it belongs to, or 0 for the mixer's shared controls.
    pub deck: u8,
    /// Learned, it shows how deck `deck`'s messages differ from deck 1's,
    /// and deck 1's rows are copied to it.
    pub copies: bool,
}

impl Step {
    /// The name its row gets: "HIGH EQ · deck 1".
    pub fn row_name(&self) -> String {
        match self.deck {
            0 => self.name.clone(),
            d => format!("{} · deck {d}", self.name),
        }
    }
}

fn step(control: &str, name: &str, prompt: &str, deck: u8) -> Step {
    Step {
        control: control.to_string(),
        name: name.to_string(),
        prompt: prompt.to_string(),
        deck,
        copies: false,
    }
}

/// The walk, for two or four decks.
pub fn steps(decks: u8) -> Vec<Step> {
    let eq = "[EqualizerRack1_[Channel1]_Effect1]";
    let mut s = vec![
        step("[Channel1],play", "PLAY", "Press PLAY", 1),
        step("[Channel1],cue_default", "CUE", "Press CUE", 1),
        step("[Channel1],sync_enabled", "SYNC", "Press SYNC", 1),
        step("[Channel1],jog", "Jog wheel", "Turn the jog wheel", 1),
        step("[Channel1],rate", "Tempo", "Move the tempo fader", 1),
        step(
            "[Channel1],pfl",
            "Headphone cue",
            "Press the headphone cue button",
            1,
        ),
        step("[Channel1],pregain", "GAIN", "Turn the GAIN knob", 1),
        step(
            &format!("{eq},parameter3"),
            "HIGH EQ",
            "Turn the HIGH EQ knob",
            1,
        ),
        step(
            &format!("{eq},parameter2"),
            "MID EQ",
            "Turn the MID EQ knob",
            1,
        ),
        step(
            &format!("{eq},parameter1"),
            "LOW EQ",
            "Turn the LOW EQ knob",
            1,
        ),
        step(
            "[QuickEffectRack1_[Channel1]],super1",
            "FILTER",
            "Turn the FILTER knob",
            1,
        ),
        step(
            "[Channel1],volume",
            "Channel fader",
            "Move the channel fader",
            1,
        ),
        step(
            "[Channel1],hotcue_1_activate",
            "Pad 1",
            "Press pad 1 (hot cue 1)",
            1,
        ),
        step(
            "[Channel1],hotcue_2_activate",
            "Pad 2",
            "Press pad 2: pads 3 to 8 follow on",
            1,
        ),
        step(
            "[Channel1],LoadSelectedTrack",
            "LOAD",
            "Press the button that loads deck 1",
            1,
        ),
        step(
            "[Master],crossfader",
            "Crossfader",
            "Move the crossfader",
            0,
        ),
        step(
            "[Library],MoveVertical",
            "Browse",
            "Turn the browse knob",
            0,
        ),
    ];
    for deck in 2..=decks.clamp(2, 4) {
        s.push(step(
            &format!("[Channel{deck}],play"),
            "PLAY",
            &format!(
                "Press PLAY on deck {deck}. If a button switches the controller to it (C or D, say), press that first: the last button pressed counts"),
            deck,
        ));
        s.last_mut().unwrap().copies = true;
    }
    s
}

/// Whether `control` is one of deck 1's (its group names `[Channel1]`).
fn deck_one(control: &str) -> bool {
    control
        .split_once(',')
        .is_some_and(|(g, _)| g.contains("[Channel1]"))
}

/// Pads 3 to 8, going on from pads 1 and 2 the way they go: most
/// controllers number a row of pads one after another.
pub fn more_pads(one: Midi, two: Midi) -> Vec<Midi> {
    if one.status != two.status || one.number == two.number {
        return Vec::new();
    }
    let step = two.number as i16 - one.number as i16;
    (2..8)
        .map_while(|i| {
            let n = one.number as i16 + step * i;
            (0..=127)
                .contains(&n)
                .then(|| Midi::new(one.status, n as u8))
        })
        .collect()
}

/// The message `m` sends on the deck whose PLAY sends `to`, when deck 1's
/// PLAY sends `from`. Another MIDI channel moves every message to it, and
/// other numbers too, for messages of PLAY's kind; on the same channel,
/// messages of PLAY's kind move by as many numbers, and others can't be
/// told.
pub fn shift(m: Midi, from: Midi, to: Midi) -> Option<Midi> {
    let kind = |s: u8| s & 0xF0;
    let channel = |s: u8| s & 0x0F;
    if kind(from.status) != kind(to.status) {
        return None;
    }
    let dch = (channel(to.status) + 16 - channel(from.status)) % 16;
    let dn = to.number as i16 - from.number as i16;
    let same_kind = kind(m.status) == kind(from.status);
    if dch == 0 && (dn == 0 || !same_kind) {
        return None;
    }
    let dn = if same_kind { dn } else { 0 };
    let move_one = |(s, n): (u8, u8)| -> Option<(u8, u8)> {
        let n = n as i16 + dn;
        (0..=127)
            .contains(&n)
            .then(|| (kind(s) | ((channel(s) + dch) % 16), n as u8))
    };
    let (status, number) = move_one((m.status, m.number))?;
    let lsb = match m.lsb {
        Some(l) => Some(move_one(l)?),
        None => None,
    };
    Some(Midi {
        status,
        number,
        lsb,
    })
}

/// Copies deck 1's rows of `own` to `deck`, whose PLAY sends `to` where
/// deck 1's sends `from`. Rows for messages already used are left out.
/// With four decks, the copies for decks 3 and 4 only work then. Returns
/// how many rows were added.
pub fn copy_deck(own: &mut DjSugarMapping, from: Midi, to: Midi, deck: u8) -> usize {
    let channel = format!("[Channel{deck}]");
    let used = |own: &DjSugarMapping, m: &Midi| {
        own.rows
            .iter()
            .any(|r| (r.midi.status, r.midi.number) == (m.status, m.number))
    };
    let copies: Vec<Row> = own
        .rows
        .iter()
        .filter(|r| r.layer.is_empty())
        .filter_map(|r| {
            let crate::format::Action::Bind { control, options } = &r.action else {
                return None;
            };
            if !deck_one(control) {
                return None;
            }
            let midi = shift(r.midi, from, to)?;
            let options: Vec<&str> = options.iter().map(String::as_str).collect();
            let mut row = Row::bind(midi, &control.replace("[Channel1]", &channel), &options);
            row.name = r.name.replace("deck 1", &format!("deck {deck}"));
            row.when = r.when;
            if deck > 2 {
                row.layer = FOUR_DECKS.to_string();
            }
            Some(row)
        })
        .collect();
    let mut added = 0;
    for row in copies {
        if !used(own, &row.midi) {
            own.rows.push(row);
            added += 1;
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Action;

    #[test]
    fn walks_deck_one_then_the_others() {
        let two = steps(2);
        assert_eq!(two.iter().filter(|s| s.copies).count(), 1);
        assert_eq!(steps(4).iter().filter(|s| s.copies).count(), 3);
        assert!(two
            .iter()
            .filter(|s| s.deck == 1)
            .all(|s| deck_one(&s.control)));
    }

    #[test]
    fn pads_go_on() {
        let pads = more_pads(Midi::new(0x97, 0x00), Midi::new(0x97, 0x01));
        assert_eq!(pads.len(), 6);
        assert_eq!(pads[5], Midi::new(0x97, 0x07));
        assert!(more_pads(Midi::new(0x97, 0x7E), Midi::new(0x97, 0x7F)).is_empty());
    }

    #[test]
    fn deck_two_follows_play() {
        let (p1, p2) = (Midi::new(0x90, 0x0B), Midi::new(0x91, 0x0B));
        let eq = Midi {
            lsb: Some((0xB0, 0x27)),
            ..Midi::new(0xB0, 0x07)
        };
        assert_eq!(shift(eq, p1, p2).unwrap().lsb, Some((0xB1, 0x27)));
        assert_eq!(
            shift(Midi::new(0x90, 0x0C), p1, p2),
            Some(Midi::new(0x91, 0x0C))
        );
        // Same channel: notes move by as many numbers, knobs can't be told.
        let p2 = Midi::new(0x90, 0x4B);
        assert_eq!(
            shift(Midi::new(0x90, 0x0C), p1, p2),
            Some(Midi::new(0x90, 0x4C))
        );
        assert_eq!(shift(eq, p1, p2), None);

        let mut own = DjSugarMapping::new("Quick");
        own.rows.push(Row::bind(p1, "[Channel1],play", &[]));
        own.rows.push(Row::bind(
            Midi::new(0xB0, 0x07),
            "[EqualizerRack1_[Channel1]_Effect1],parameter3",
            &[],
        ));
        own.rows
            .push(Row::bind(Midi::new(0xB6, 0x1F), "[Master],crossfader", &[]));
        assert_eq!(copy_deck(&mut own, p1, Midi::new(0x91, 0x0B), 2), 2);
        let Action::Bind { control, .. } = &own.rows[4].action else {
            panic!("a binding");
        };
        assert_eq!(control, "[EqualizerRack1_[Channel2]_Effect1],parameter3");
        // Again, nothing new.
        assert_eq!(copy_deck(&mut own, p1, Midi::new(0x91, 0x0B), 2), 0);
        assert_eq!(copy_deck(&mut own, p1, Midi::new(0x92, 0x0B), 3), 2);
        assert_eq!(own.rows.last().unwrap().layer, FOUR_DECKS);
    }
}
