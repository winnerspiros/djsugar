//! The Groove Box's library of named grooves, by genre. Grooves are text files in `data/grooves/`, built into the app (see `basics.txt` for the format). Each groove has a step
//! pattern for each of the kits' nine sounds ([`crate::kits::ROLES`]), and every step one of three levels: soft, normal or accented. The library is read once, the first time
//! it is asked for (when the Groove Boxes' controls are made, before any audio runs); after that it is only read, so the audio thread can load a groove without allocating.

use std::sync::OnceLock;

use crate::kits::ROLES;

/// How loud a step plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Normal,
    Soft,
    Accent,
}

impl Level {
    /// Step `step`'s level, from the masks of soft and accented steps.
    pub fn of(step: usize, soft: u32, accent: u32) -> Self {
        if accent & (1 << step) != 0 {
            Level::Accent
        } else if soft & (1 << step) != 0 {
            Level::Soft
        } else {
            Level::Normal
        }
    }

    /// The gain a hit at this level plays at.
    pub fn gain(self) -> f32 {
        match self {
            Level::Normal => 1.0,
            Level::Soft => 0.5,
            Level::Accent => 1.25,
        }
    }
}

/// One sound's steps in a groove: bit s of `hits` on = step s+1 plays,
/// softly when bit s of `soft` is on, accented when bit s of `accent` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Steps {
    pub hits: u32,
    pub soft: u32,
    pub accent: u32,
    /// Steps that play only half the time.
    pub maybe: u32,
}

impl Steps {
    /// Reads 16 steps written as text: `.` rest, `o` soft, `x` hit, `X`
    /// accent.
    pub fn parse(text: &str) -> Steps {
        let mut s = Steps::default();
        for (i, c) in text.chars().take(16).enumerate() {
            let level = match c {
                'x' => Level::Normal,
                'o' => Level::Soft,
                'X' => Level::Accent,
                '?' => {
                    s.maybe |= 1 << i;
                    Level::Normal
                }
                _ => continue,
            };
            s.hits |= 1 << i;
            match level {
                Level::Soft => s.soft |= 1 << i,
                Level::Accent => s.accent |= 1 << i,
                Level::Normal => {}
            }
        }
        s
    }

    pub fn level(&self, step: usize) -> Level {
        Level::of(step, self.soft, self.accent)
    }
}

pub struct Groove {
    pub name: String,
    pub about: String,
    pub rows: [Steps; ROLES.len()],
}

pub struct Genre {
    pub name: String,
    pub grooves: Vec<Groove>,
}

/// The files the library is read from, in order: DJ Sugar's own grooves,
/// then mpump's (AGPL-3.0, see NOTICE; written by
/// `scripts/import-mpump-grooves.py`).
const FILES: [&str; 2] = [include_str!("../data/grooves/basics.txt"), include_str!("../data/grooves/mpump.txt")];

/// Every genre, Basics first.
pub fn library() -> &'static [Genre] {
    static LIBRARY: OnceLock<Vec<Genre>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let mut genres = Vec::new();
        for file in FILES {
            parse(file, &mut genres);
        }
        genres
    })
}

/// The groove `index` (0-based) of genre `genre`, if there is one.
pub fn groove(genre: usize, index: usize) -> Option<&'static Groove> {
    library().get(genre)?.grooves.get(index)
}

/// Reads a grooves file into `genres`; a genre named again adds to the
/// one already there. Lines it doesn't understand are skipped.
fn parse(text: &str, genres: &mut Vec<Genre>) {
    let mut genre: Option<usize> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix("genre ") {
            let name = name.trim();
            genre = Some(match genres.iter().position(|g| g.name == name) {
                Some(i) => i,
                None => {
                    genres.push(Genre { name: name.to_string(), grooves: Vec::new() });
                    genres.len() - 1
                }
            });
        } else if let Some(rest) = line.strip_prefix("groove ") {
            let Some(g) = genre else { continue };
            let (name, about) = rest.split_once('|').unwrap_or((rest, ""));
            genres[g].grooves.push(Groove {
                name: name.trim().to_string(),
                about: about.trim().to_string(),
                rows: Default::default(),
            });
        } else if let Some((key, steps)) = line.split_once(char::is_whitespace) {
            let Some(groove) = genre.and_then(|g| genres[g].grooves.last_mut()) else {
                continue;
            };
            let role = ROLES.iter().position(|r| r.to_ascii_lowercase().replace(' ', "-") == key);
            if let Some(r) = role {
                groove.rows[r] = Steps::parse(steps.trim());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_keep_their_levels() {
        let s = Steps::parse("x.o.X...........");
        assert_eq!(s.hits, 0b10101);
        assert_eq!(s.level(0), Level::Normal);
        assert_eq!(s.level(2), Level::Soft);
        assert_eq!(s.level(4), Level::Accent);
    }

    #[test]
    fn the_library_reads_every_genre_and_groove() {
        let lib = library();
        assert_eq!(lib[0].name, "Basics");
        assert_eq!(lib[0].grooves[0].name, "Four to the floor");
        for g in lib {
            assert!(!g.grooves.is_empty(), "{} is empty", g.name);
            for groove in &g.grooves {
                assert!(groove.rows.iter().any(|r| r.hits != 0), "{} / {} has no hits", g.name, groove.name);
            }
        }
    }

    #[test]
    fn rows_are_read_by_sound_name() {
        let mut genres = Vec::new();
        parse("genre Test\ngroove One | about\nkick x...\nopen-hat ..x.\nnope xxxx\n", &mut genres);
        let g = &genres[0].grooves[0];
        assert_eq!(g.about, "about");
        assert_eq!(g.rows[0].hits, 1);
        assert_eq!(g.rows[3].hits, 4);
        assert_eq!(g.rows.iter().filter(|r| r.hits != 0).count(), 2);
    }
}
