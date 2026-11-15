//! The side panel's Info tab: the details of the track selected in the
//! list (or loaded on a deck), editable in place. Tags are written into the
//! file and kept in the library; the library's own fields (User Tags 1 and
//! 2, the gain) only in the library. It also has a preview player for the
//! headphones, and finds and saves album art.

use std::sync::mpsc::{self, Receiver, Sender};

use super::cleanup::{info_proposals, query_for, QueryFor};
use crate::lookup::{Found, Miss};

use djsugar_engine::cover::{self, Cover};
use djsugar_engine::preview::PREVIEW_GROUP;
use djsugar_engine::scan::is_cloud_only;
use djsugar_engine::tags::{self, TagFields};
use djsugar_engine::{BeatGrid, Source};

use std::collections::BTreeMap;

use djsugar_library::cleanup::Field;

use super::tag_chips;
use super::*;
use crate::coverart::{self, Event as CoverEvent, Found as FoundCover};
use crate::{CoverChoice, InfoField, InfoLine, TagChip, TrackInfo, ValueChoice};

/// The side panel's Info tab.
pub(super) const INFO_TAB: usize = 3;

/// Longest side of the cover the panel shows.
const COVER_SIZE: u32 = 160;

const TEXT: i32 = 0;
const READ: i32 = 1;
const READ_WITH_BUTTON: i32 = 4;
/// A tag field: its tags as chips, and a box to add more.
const CHIPS: i32 = 5;

/// The tabs of the Info tab: the names, the DJ's own tags and comment,
/// and the file with its stats.
const TRACK_TAB: i32 = 0;
const TAGS_TAB: i32 = 1;
const FILE_TAB: i32 = 2;

pub(super) struct InfoState {
    /// 0 the selected track, 1 deck A, 2 deck B.
    pub(super) subject: i32,
    /// The library's copy of the track shown, and where it is in `library`.
    pub(super) shown: Option<(usize, Arc<Track>)>,
    fields: Rc<VecModel<InfoLine>>,
    /// The cover being read for the track shown, and the generation it is for.
    cover_job: Option<(u64, Receiver<Option<Cover>>)>,
    generation: u64,
    /// The cover dialog's search and what it has found.
    search: Option<Receiver<CoverEvent>>,
    found: Vec<FoundCover>,
    /// Pictures picked with Load File…, read on the dialog's thread.
    picked_tx: Sender<Result<FoundCover, String>>,
    picked_rx: Receiver<Result<FoundCover, String>>,
    /// The preview player's controls, and the track (an index into
    /// `library`) it has.
    play: ControlHandle,
    position: ControlHandle,
    duration: ControlHandle,
    previewing: Option<usize>,
    /// What the preview bar shows, to set only on change.
    shown_position: (bool, i64),
    /// Each deck's gain knob, set when the gain of its track is edited.
    pregain: Vec<ControlHandle>,
    /// The tab shown: Track, Tags or File.
    tab: i32,
    /// Several tracks picked: where they are in `library`, and the copies
    /// shown. The tab edits them together.
    pub(super) many: Vec<(usize, Arc<Track>)>,
    /// Changes typed for the tracks in `many`, by field id, until Apply.
    pending: BTreeMap<String, String>,
    /// The fields in `pending` a look-up filled, not the DJ: Apply leaves
    /// them open to later look-ups and clean-up.
    looked_up: HashSet<String>,
    /// Look up online for the track shown: where it is in `library`, and
    /// what comes back.
    lookup: Option<(usize, Receiver<Result<Found, Miss>>)>,
    /// Tags added lately (tag field, tag), newest first, which the track
    /// menu's Add Tag offers first.
    pub(super) recent_tags: Vec<(i64, String)>,
}

impl InfoState {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let c = |k: &str| registry.get(PREVIEW_GROUP, k).expect("preview control");
        let (picked_tx, picked_rx) = mpsc::channel();
        Self {
            subject: 0,
            shown: None,
            fields: Rc::new(VecModel::default()),
            cover_job: None,
            generation: 0,
            search: None,
            found: Vec::new(),
            picked_tx,
            picked_rx,
            play: c("play"),
            position: c("playposition"),
            duration: c("duration"),
            previewing: None,
            shown_position: (false, -1),
            pregain: (0..NUM_DECKS)
                .map(|d| registry.get(&deck_group(d), "pregain").expect("pregain"))
                .collect(),
            tab: (crate::settings::number("info_tab", 0.0) as i32).clamp(TRACK_TAB, FILE_TAB),
            many: Vec::new(),
            pending: BTreeMap::new(),
            looked_up: HashSet::new(),
            lookup: None,
            recent_tags: Vec::new(),
        }
    }
}

/// "1.2 MB".
fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1.0 {
        format!("{mb:.1} MB")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

/// A Unix time as a local date, "19 Oct 2026".
fn date_text(at: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(at, 0).single() {
        Some(t) if at > 0 => t.format("%-d %b %Y").to_string(),
        _ => String::new(),
    }
}

/// "+1.5 dB", or "" for none.
fn gain_text(db: Option<f32>) -> String {
    match db {
        Some(g) if g > 0.0 => format!("+{g:.1} dB"),
        Some(g) => format!("{g:.1} dB"),
        None => String::new(),
    }
}

/// A gain typed in: "-2.5", "+3 dB"; empty for none.
fn parse_gain(text: &str) -> Result<Option<f32>, String> {
    let t = text
        .trim()
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.trim_start_matches('+')
        .replace(',', ".")
        .replace('−', "-")
        .parse::<f32>()
        .ok()
        .filter(|g| (-24.0..=12.0).contains(g))
        .map(Some)
        .ok_or_else(|| "Gain is a number of dB from -24 to +12.".to_string())
}

fn field(id: &str, label: &str, value: impl Into<SharedString>, kind: i32) -> InfoField {
    InfoField {
        id: id.into(),
        label: label.into(),
        value: value.into(),
        kind,
        hint: SharedString::new(),
        action: SharedString::new(),
        tab: TRACK_TAB,
        half: false,
        mixed: 0,
        edited: false,
        chips: ModelRc::default(),
    }
}

/// A tag field of `tracks` as chips.
fn chips_field(tracks: &[&Track], group: i64, label: &str) -> InfoField {
    let id = Field::Tag(group).name();
    InfoField {
        chips: tag_chips::chip_model(tracks, group),
        hint: "+ tag".into(),
        ..on(TAGS_TAB, field(&id, label, "", CHIPS))
    }
}

/// `f` on tab `tab`.
fn on(tab: i32, f: InfoField) -> InfoField {
    InfoField { tab, ..f }
}

/// `f` sharing its line with the field next to it.
fn half(f: InfoField) -> InfoField {
    InfoField { half: true, ..f }
}

/// The info list for `t`, on three tabs: Track (the names, then genre,
/// year and track number, tempo and key, gain), Tags (the DJ's tag groups,
/// grouping and comment) and File (format, size, where it is, plays).
/// Short fields share a line. The rating is in the panel's header.
fn fields_of(t: &Track, camelot: bool, file_manager: &str) -> Vec<InfoField> {
    let names = query::field_names();
    let text = |id: &str, label: &str, value: &str| field(id, label, value, TEXT);
    let bpm = if t.bpm > 0.0 {
        format!("{:.2}", t.bpm)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        String::new()
    };
    let mut out = vec![
        text("title", "TRACK", &t.title),
        text("artist", "ARTIST", &t.artist),
        text("remix", "REMIX", &t.info.remix),
        text("album", "ALBUM", &t.album),
        text("genre", "GENRE", &t.genre),
        half(text("year", "YEAR", &t.info.year)),
        half(text(
            "track_number",
            "TRACK #",
            &if t.info.track_number > 0 {
                t.info.track_number.to_string()
            } else {
                String::new()
            },
        )),
        half(text("bpm", "BPM", &bpm)),
        half(InfoField {
            hint: "8A or Am".into(),
            ..text("key", "KEY", &show_key(&t.key, camelot))
        }),
        half(InfoField {
            hint: "0.0 dB".into(),
            ..text("gain", "GAIN", &gain_text(t.info.gain_db))
        }),
        half(field("", "TIME", short_time(t.duration), READ)),
    ];
    // The DJ's tag groups, by the names they gave them.
    for g in &names.tags {
        out.push(chips_field(&[t], g.id, &g.name.to_uppercase()));
    }
    out.extend(
        [
            text("grouping", &names.grouping.to_uppercase(), &t.info.grouping),
            text("comment", "COMMENT", &t.info.comment),
        ]
        .map(|f| on(TAGS_TAB, f)),
    );
    let mut file = Vec::new();
    if let Source::File(path) = &t.source {
        let f = if t.info.missing || is_cloud_only(path) {
            tags::FileInfo::default()
        } else {
            tags::file_info(path)
        };
        let format = match f.bitrate {
            Some(b) if !f.format.is_empty() => format!("{} · {b} kbps", f.format),
            _ => f.format.clone(),
        };
        file.push(field("", "FORMAT", format, READ));
        let mut audio = Vec::new();
        if let Some(r) = f.sample_rate {
            audio.push(format!("{:.1} kHz", r as f64 / 1000.0).replace(".0 ", " "));
        }
        if let Some(b) = f.bit_depth {
            audio.push(format!("{b}-bit"));
        }
        match f.channels {
            Some(1) => audio.push("Mono".into()),
            Some(2) => audio.push("Stereo".into()),
            Some(n) => audio.push(format!("{n} channels")),
            None => {}
        }
        file.push(field("", "AUDIO", audio.join(" · "), READ));
        if f.size > 0 {
            file.push(field("", "SIZE", size_text(f.size), READ));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        file.push(InfoField {
            action: format!("Show in {file_manager}").into(),
            ..field(
                "",
                "FILE",
                if t.info.missing {
                    format!("{name} (missing)")
                } else {
                    name
                },
                READ_WITH_BUTTON,
            )
        });
        let folder = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        file.push(field("", "FOLDER", folder, READ));
    }
    file.extend([
        half(field("", "PLAYS", t.info.play_count.to_string(), READ)),
        half(field("", "ADDED", date_text(t.info.added), READ)),
        field(
            "",
            "LAST PLAYED",
            t.info.last_played.map(date_text).unwrap_or_default(),
            READ,
        ),
    ]);
    out.extend(file.into_iter().map(|f| on(FILE_TAB, f)));
    out
}

/// The fields of tab `tab`.
fn shown_fields(fields: Vec<InfoField>, tab: i32) -> Vec<InfoField> {
    fields.into_iter().filter(|f| f.tab == tab).collect()
}

/// The lines of the list: a short field and the one after it share a
/// line.
fn lines(fields: Vec<InfoField>) -> Vec<InfoLine> {
    let mut out = Vec::new();
    let mut fields = fields.into_iter().peekable();
    while let Some(a) = fields.next() {
        let b = if a.half && fields.peek().is_some_and(|b| b.half) {
            fields.next()
        } else {
            None
        };
        out.push(InfoLine {
            two: b.is_some(),
            a,
            b: b.unwrap_or_else(|| field("", "", "", READ)),
        });
    }
    out
}

/// The two lines show the same, chips and all (models compare by identity).
fn same_line(a: &InfoLine, b: &InfoLine) -> bool {
    let chips = |f: &InfoField| -> Vec<TagChip> { f.chips.iter().collect() };
    let bare = |f: &InfoField| InfoField {
        chips: ModelRc::default(),
        ..f.clone()
    };
    a.two == b.two
        && bare(&a.a) == bare(&b.a)
        && bare(&a.b) == bare(&b.b)
        && chips(&a.a) == chips(&b.a)
        && chips(&a.b) == chips(&b.b)
}

/// Which tab field `f` is on.
fn tab_of(f: Field) -> i32 {
    match f {
        Field::Tag(_) | Field::Grouping | Field::Comment => TAGS_TAB,
        _ => TRACK_TAB,
    }
}

/// The fields several tracks can be given together, in the order of the
/// list for one track.
fn shared_fields(groups: &[i64]) -> Vec<Field> {
    let mut out = vec![
        Field::Title,
        Field::Artist,
        Field::Remix,
        Field::Album,
        Field::Genre,
        Field::Year,
    ];
    out.extend(groups.iter().map(|&g| Field::Tag(g)));
    out.extend([Field::Grouping, Field::Comment]);
    out
}

/// What the tracks have in field `f`: each value and how many tracks have
/// it, most first (then A to Z).
fn values_of(tracks: &[&Track], f: Field) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in tracks {
        *counts.entry(f.get(t).trim().to_string()).or_default() += 1;
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// "Lane 8, Tycho, +3".
fn names_summary<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for n in names.into_iter().map(str::trim).filter(|n| !n.is_empty()) {
        if !seen.contains(&n) {
            seen.push(n);
        }
    }
    match seen.len() {
        0 => String::new(),
        1 | 2 => seen.join(", "),
        n => format!("{}, {}, +{}", seen[0], seen[1], n - 2),
    }
}

/// "118–124" for the tempos of several tracks, "" with none.
fn bpm_range(tracks: &[&Track]) -> String {
    let bpms = tracks.iter().map(|t| t.bpm).filter(|&b| b > 0.0);
    let (lo, hi) = bpms.fold((f64::MAX, 0.0f64), |(lo, hi), b| (lo.min(b), hi.max(b)));
    if hi == 0.0 {
        String::new()
    } else if lo.round() == hi.round() {
        format!("{lo:.0}")
    } else {
        format!("{lo:.0}–{hi:.0}")
    }
}

/// The keys of several tracks, each once, as the DJ reads keys.
fn keys_of(tracks: &[&Track], camelot: bool) -> Vec<String> {
    let mut keys: Vec<String> = tracks
        .iter()
        .map(|t| show_key(&t.key, camelot))
        .filter(|k| !k.is_empty())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// "8A", or "4 keys".
fn keys_summary(keys: &[String]) -> String {
    match keys.len() {
        0 => String::new(),
        1 => keys[0].clone(),
        n => format!("{n} keys"),
    }
}

/// The info list for several tracks: each field they can be given
/// together shows their shared value, how many values they have, or what
/// was typed for them all; tempo, key and the files are summed up.
fn many_fields(
    tracks: &[&Track],
    pending: &BTreeMap<String, String>,
    camelot: bool,
    writable_files: usize,
) -> Vec<InfoField> {
    let names = query::field_names();
    let mut out = Vec::new();
    let groups: Vec<i64> = names.tags.iter().map(|g| g.id).collect();
    for f in shared_fields(&groups) {
        let id = f.name().into_owned();
        let label = match f {
            Field::Title => "TRACK".to_string(),
            Field::Grouping => names.grouping.to_uppercase(),
            other => other.label().to_uppercase(),
        };
        if let Field::Tag(group) = f {
            out.push(chips_field(tracks, group, &label));
            continue;
        }
        let values = values_of(tracks, f);
        let mut line = on(tab_of(f), field(&id, &label, "", TEXT));
        if let Some(v) = pending.get(&id) {
            line.value = v.into();
            line.edited = true;
            if v.is_empty() {
                line.hint = "cleared in all".into();
            }
        } else if values.len() == 1 {
            line.value = values[0].0.clone().into();
        } else {
            line.mixed = values.len() as i32;
            line.hint = format!("{} values", values.len()).into();
        }
        out.push(line);
    }
    // Tempo and key can't be given to several tracks: their range shows.
    let at = out
        .iter()
        .position(|f| f.tab != TRACK_TAB)
        .unwrap_or(out.len());
    out.splice(
        at..at,
        [
            half(field("", "BPM", bpm_range(tracks), READ)),
            half(field(
                "",
                "KEY",
                keys_summary(&keys_of(tracks, camelot)),
                READ,
            )),
        ],
    );
    let n = tracks.len();
    let only_here = n.saturating_sub(writable_files);
    out.extend(
        [
            field("", "FILES", n.to_string(), READ),
            field("", "ONLY IN COLLECTION", only_here.to_string(), READ),
            field(
                "",
                "TIME",
                short_time(tracks.iter().map(|t| t.duration).sum()),
                READ,
            ),
        ]
        .map(|f| on(FILE_TAB, f)),
    );
    out
}

/// The tags of `t` as they go in its file.
pub(super) fn tag_fields(t: &Track) -> TagFields {
    TagFields {
        title: t.title.clone(),
        artist: t.artist.clone(),
        remix: t.info.remix.clone(),
        album: t.album.clone(),
        year: t.info.year.clone(),
        genre: t.genre.clone(),
        track_number: t.info.track_number,
        grouping: t.info.grouping.clone(),
        bpm: t.bpm,
        key: t.key.clone(),
        rating: t.rating,
        comment: t.info.comment.clone(),
    }
}

/// The file whose tags can be written, if `t` has one on disk.
pub(super) fn writable(t: &Track) -> Option<&Path> {
    match &t.source {
        Source::File(p) if !t.info.missing && p.exists() && !is_cloud_only(p) => Some(p),
        _ => None,
    }
}

/// `edit` applied to `t`; Err says why the value can't be taken.
fn apply(t: &mut Track, id: &str, value: &str) -> Result<(), String> {
    let v = value.trim().to_string();
    match id {
        "title" => t.title = v,
        "artist" => t.artist = v,
        "remix" => t.info.remix = v,
        "album" => t.album = v,
        "year" => t.info.year = v,
        "genre" => t.genre = v,
        "grouping" => t.info.grouping = v,
        "comment" => t.info.comment = v,
        tag if tag.starts_with("tag") => match Field::from_name(tag) {
            Some(f @ Field::Tag(_)) => f.set(t, v),
            _ => return Err(format!("No field {tag}.")),
        },
        "key" => t.key = v,
        "track_number" => {
            t.info.track_number = if v.is_empty() {
                0
            } else {
                v.parse()
                    .map_err(|_| "Track # is a whole number.".to_string())?
            }
        }
        "rating" => t.rating = v.parse::<u8>().unwrap_or(0).min(5),
        "gain" => t.info.gain_db = parse_gain(&v)?,
        "bpm" => {
            let bpm = if v.is_empty() {
                0.0
            } else {
                v.replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|b| (20.0..=400.0).contains(b))
                    .ok_or_else(|| "BPM is a tempo from 20 to 400.".to_string())?
            };
            if bpm > 0.0 {
                // The grid keeps its first beat and takes the new tempo.
                let grid = t.beat_grid();
                let grid = if grid.is_dynamic() {
                    grid.with_bpm_at(grid.first_beat(), bpm)
                } else {
                    BeatGrid::constant(bpm, t.first_beat)
                };
                t.set_grid(grid);
                t.bpm = bpm;
            } else {
                t.bpm = 0.0;
                t.grid = BeatGrid::default();
            }
        }
        _ => return Err(String::new()),
    }
    Ok(())
}

impl Browser {
    pub(super) fn install_info(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<TrackInfo>();
        ui.set_fields(ModelRc::from(b.borrow().info.fields.clone()));
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        Self::install_tag_chips(&ui, with.clone());
        let w = with.clone();
        ui.on_set_subject(move |s| {
            w(&|b| {
                b.info.subject = s.clamp(0, NUM_DECKS as i32);
                if let Some(app) = b.app.upgrade() {
                    app.global::<TrackInfo>().set_subject(b.info.subject);
                }
            })
        });
        let w = with.clone();
        ui.on_edit(move |id, value| w(&|b| b.info_edit(&id, &value)));
        let w = with.clone();
        ui.on_values(move |id| w(&|b| b.many_values(&id)));
        let w = with.clone();
        ui.on_keep(move |id| {
            w(&|b| {
                b.info.pending.remove(id.as_str());
                b.info.looked_up.remove(id.as_str());
                if let Some(app) = b.app.upgrade() {
                    b.refresh_info(&app);
                }
            })
        });
        let w = with.clone();
        ui.on_discard(move || {
            w(&|b| {
                b.info.pending.clear();
                b.info.looked_up.clear();
                if let Some(app) = b.app.upgrade() {
                    b.refresh_info(&app);
                }
            })
        });
        let w = with.clone();
        ui.on_apply(move || w(&|b| b.apply_many()));
        let w = with.clone();
        ui.on_set_tab(move |tab| {
            w(&|b| {
                b.info.tab = tab.clamp(TRACK_TAB, FILE_TAB);
                crate::settings::set("info_tab", b.info.tab.into());
                if let Some(app) = b.app.upgrade() {
                    b.refresh_info(&app);
                }
            })
        });
        let w = with.clone();
        ui.on_reveal(move || {
            w(&|b| {
                if let Some((_, t)) = &b.info.shown {
                    if let Source::File(p) = &t.source {
                        files::reveal(p);
                    }
                }
            })
        });
        let w = with.clone();
        ui.on_preview_toggle(move || w(&|b| b.preview_toggle()));
        let w = with.clone();
        ui.on_preview_seek(move |at| {
            w(&|b| {
                if b.info.previewing.is_some() && b.info.previewing == b.info_index() {
                    b.info.position.set(at.clamp(0.0, 1.0) as f64);
                } else if let Some(i) = b.info_index() {
                    // Seeking a track not in the player starts it there.
                    b.engine.preview(Some(&b.library[i]), true);
                    b.info.previewing = Some(i);
                    b.info.shown_position = (true, -1);
                }
            })
        });
        let w = with.clone();
        ui.on_find_covers(move || w(&|b| b.find_covers()));
        let w = with.clone();
        ui.on_load_cover_file(move || w(&|b| b.load_cover_file()));
        let w = with.clone();
        ui.on_look_up(move || w(&|b| b.info_look_up()));
        let w = with.clone();
        ui.on_save_cover(move |i| w(&|b| b.save_cover(i.max(0) as usize)));
    }

    /// Where in `library` the track the Info tab shows is.
    fn info_index(&self) -> Option<usize> {
        self.info.shown.as_ref().map(|(i, _)| *i)
    }

    /// The track the Info tab is to show: the one selected in the list,
    /// or the one on the deck it follows.
    fn info_subject(&self, app: &AppWindow) -> Option<usize> {
        match self.info.subject {
            0 => usize::try_from(app.get_current_track())
                .ok()
                .and_then(|r| self.rows.get(r).copied().flatten()),
            d => self.loaded[(d - 1) as usize].and_then(|id| self.index_of.get(&id).copied()),
        }
    }

    /// Keeps the Info tab up to date; called every frame.
    pub(super) fn poll_info(&mut self, app: &AppWindow, tab: usize) {
        if tab != INFO_TAB {
            return;
        }
        let subject = self.info_subject(app);
        let now = subject.map(|i| (i, self.library[i].clone()));
        let changed = match (&now, &self.info.shown) {
            (Some((i, t)), Some((j, u))) => i != j || !Arc::ptr_eq(t, u),
            (None, None) => false,
            _ => true,
        };
        // Several picked: the tab edits them together.
        let many: Vec<usize> = if self.info.subject == 0 {
            self.picked_many()
        } else {
            Vec::new()
        };
        let many_changed = many.len() != self.info.many.len()
            || many
                .iter()
                .zip(&self.info.many)
                .any(|(&i, (j, t))| i != *j || !Arc::ptr_eq(&self.library[i], t));
        if many_changed {
            let same_tracks = many.len() == self.info.many.len()
                && many.iter().zip(&self.info.many).all(|(&i, (j, _))| i == *j);
            if !same_tracks {
                self.info.pending.clear();
                self.info.looked_up.clear();
            }
            self.info.many = many.iter().map(|&i| (i, self.library[i].clone())).collect();
        }
        if changed {
            let other_track = now.as_ref().map(|(i, _)| *i) != self.info_index();
            self.info.shown = now;
            if other_track {
                self.info_switched(app);
            }
            self.refresh_info(app);
        } else if many_changed {
            self.refresh_info(app);
        }
        // The cover for the panel.
        if let Some((generation, rx)) = &self.info.cover_job {
            if let Ok(cover) = rx.try_recv() {
                if *generation == self.info.generation {
                    let ui = app.global::<TrackInfo>();
                    ui.set_has_cover(cover.is_some());
                    ui.set_cover(cover.map(|c| image(&c)).unwrap_or_default());
                }
                self.info.cover_job = None;
            }
        }
        self.poll_cover_search(app);
        self.poll_cover_file(app);
        self.poll_info_lookup(app);
        self.poll_preview(app);
    }

    /// Another track is shown: the preview stops and the cover is read.
    fn info_switched(&mut self, app: &AppWindow) {
        let ui = app.global::<TrackInfo>();
        if self.info.previewing.take().is_some() {
            self.engine.preview(None, false);
        }
        self.fill_info_wave(app);
        ui.set_status(SharedString::new());
        ui.set_cover_open(false);
        self.info.search = None;
        // What a look-up found was for the track that was shown.
        if self.info.many.is_empty() {
            self.info.pending.clear();
            self.info.looked_up.clear();
        }
        self.info.lookup = None;
        ui.set_looking_up(false);
        self.info.found.clear();
        self.read_info_cover();
    }

    /// Reads the cover of the track shown, off the UI thread.
    fn read_info_cover(&mut self) {
        self.info.generation += 1;
        let thumb = self.info.shown.as_ref().and_then(|(_, t)| self.art.get(t));
        if let Some(app) = self.app.upgrade() {
            // The list's thumbnail until the larger one is read.
            let ui = app.global::<TrackInfo>();
            ui.set_has_cover(thumb.is_some());
            ui.set_cover(thumb.unwrap_or_default());
        }
        self.info.cover_job = None;
        let Some((_, t)) = &self.info.shown else {
            return;
        };
        let Source::File(path) = &t.source else {
            return;
        };
        if t.info.missing || is_cloud_only(path) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (path, album) = (path.clone(), t.album.clone());
        let spawned = std::thread::Builder::new()
            .name("info-cover".into())
            .spawn(move || {
                let _ = tx.send(cover::find_sized(&path, &album, COVER_SIZE));
            });
        if spawned.is_ok() {
            self.info.cover_job = Some((self.info.generation, rx));
        }
    }

    /// Fills the Info tab for the track shown.
    pub(super) fn refresh_info(&mut self, app: &AppWindow) {
        let ui = app.global::<TrackInfo>();
        ui.set_subject(self.info.subject);
        ui.set_tab(self.info.tab);
        let edited: Vec<bool> = (TRACK_TAB..=FILE_TAB)
            .map(|tab| {
                self.info
                    .pending
                    .keys()
                    .any(|id| Field::from_name(id).is_some_and(|f| tab_of(f) == tab))
            })
            .collect();
        ui.set_tab_edited(ModelRc::new(VecModel::from(edited)));
        ui.set_many(self.info.many.len() as i32);
        ui.set_pending(self.info.pending.len() as i32);
        if !self.info.many.is_empty() {
            self.refresh_many(app);
            return;
        }
        let Some((_, t)) = &self.info.shown else {
            ui.set_has_track(false);
            self.info.fields.set_vec(Vec::new());
            return;
        };
        ui.set_has_track(true);
        ui.set_editable(writable(t).is_some() || djsugar_library::row_id(t.id).is_some());
        ui.set_title(t.title.clone().into());
        ui.set_artist(t.artist.clone().into());
        ui.set_rating(t.rating as i32);
        let mut fields = fields_of(t, self.camelot, files::file_manager());
        // What a look-up found waits for Apply, as typing does for
        // several tracks.
        for f in &mut fields {
            if let Some(v) = self.info.pending.get(f.id.as_str()) {
                f.value = v.clone().into();
                f.edited = true;
            }
        }
        let fields = shown_fields(fields, self.info.tab);
        self.show_info_fields(fields);
    }

    /// Puts `fields` in the list, changing rows in place so a field being
    /// typed in keeps its focus.
    fn show_info_fields(&self, fields: Vec<InfoField>) {
        let fields = lines(fields);
        let model = &self.info.fields;
        if model.row_count() == fields.len() {
            for (i, f) in fields.into_iter().enumerate() {
                if !model.row_data(i).is_some_and(|l| same_line(&l, &f)) {
                    model.set_row_data(i, f);
                }
            }
        } else {
            model.set_vec(fields);
        }
    }

    /// The DJ changed field `id` of the track shown to `value`: it is saved
    /// to the file's tags and the library.
    pub(super) fn info_edit(&mut self, id: &str, value: &str) {
        if !self.info.many.is_empty() {
            self.many_edit(id, value);
            return;
        }
        // Typed over what a look-up found: what was typed wins.
        self.info.pending.remove(id);
        self.info.looked_up.remove(id);
        let Some((index, _)) = self.info.shown.clone() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<TrackInfo>();
        ui.set_status_ok(false);
        // A file shown in Files joins the collection, as rating it does.
        self.adopt(&[index]);
        let mut t = (*self.library[index]).clone();
        let before = tag_fields(&t);
        if let Err(why) = apply(&mut t, id, value) {
            ui.set_status(why.into());
            // Puts the field back as it was.
            self.refresh_info(&app);
            return;
        }
        let mut status = String::new();
        if tag_fields(&t) != before {
            match writable(&t) {
                Some(path) => {
                    if let Err(e) = tags::write(path, &tag_fields(&t)) {
                        status = format!("Couldn't write the tags to the file: {e}");
                    }
                }
                None => {
                    status =
                        "The file isn't on this computer, so only your collection has the change."
                            .into()
                }
            }
        }
        if let Some(db) = &self.db {
            if let Err(e) = db.save_info(t.id, &t) {
                status = format!("Couldn't save to your collection: {e}");
            }
            // Clean-up and Look up online leave what the DJ typed alone.
            if djsugar_library::cleanup::Field::from_name(id).is_some() {
                let _ = db.mark_hand_edited(t.id, id);
            }
            if id == "bpm" && t.bpm > 0.0 {
                if let Err(e) = db.save_grid(t.id, &t.beat_grid(), t.bpm) {
                    status = format!("Couldn't save the beat grid: {e}");
                }
            }
        }
        if id == "bpm" {
            t.info.fixed_grid = t.bpm > 0.0;
        }
        // A deck with the track takes a new gain at once.
        if id == "gain" {
            for (d, loaded) in self.loaded.iter().enumerate() {
                if *loaded == Some(t.id) {
                    let gain = 10f64.powf(t.info.gain_db.unwrap_or(0.0) as f64 / 20.0);
                    self.info.pregain[d].set(gain.clamp(0.0, 4.0));
                }
            }
        }
        ui.set_status(status.into());
        self.indexed[index] = Indexed::new(&t);
        self.library[index] = Arc::new(t);
        self.refresh_rows();
        self.refresh_side();
    }

    /// Fills the Info tab for several tracks.
    fn refresh_many(&mut self, app: &AppWindow) {
        let ui = app.global::<TrackInfo>();
        let tracks: Vec<&Track> = self.info.many.iter().map(|(_, t)| &**t).collect();
        let writable_files = tracks.iter().filter(|t| writable(t).is_some()).count();
        ui.set_has_track(true);
        ui.set_editable(
            tracks
                .iter()
                .any(|t| writable(t).is_some() || djsugar_library::row_id(t.id).is_some()),
        );
        ui.set_title(format!("{} tracks", tracks.len()).into());
        ui.set_artist(names_summary(tracks.iter().map(|t| t.artist.as_str())).into());
        let ratings: HashSet<u8> = tracks.iter().map(|t| t.rating).collect();
        ui.set_rating(if ratings.len() == 1 {
            tracks[0].rating as i32
        } else {
            0
        });
        let fields = shown_fields(
            many_fields(&tracks, &self.info.pending, self.camelot, writable_files),
            self.info.tab,
        );
        self.show_info_fields(fields);
    }

    /// A field typed in while several tracks are shown waits for Apply;
    /// the rating is given to them all at once.
    fn many_edit(&mut self, id: &str, value: &str) {
        if id == "rating" {
            let rating = value.parse::<u8>().unwrap_or(0).min(5);
            let tracks: Vec<usize> = self.info.many.iter().map(|(i, _)| *i).collect();
            self.change_tracks(&tracks, |t, db| {
                t.rating = rating;
                if let Some(Err(e)) = db.map(|db| db.set_rating(t.id, rating)) {
                    eprintln!("can't save the rating: {e}");
                }
            });
            return;
        }
        let Some(f) = Field::from_name(id) else {
            return;
        };
        let v = value.trim().to_string();
        self.info.looked_up.remove(id);
        let tracks: Vec<&Track> = self.info.many.iter().map(|(_, t)| &**t).collect();
        let values = values_of(&tracks, f);
        // Typing back what they all have already changes nothing.
        if values.len() == 1 && values[0].0 == v {
            self.info.pending.remove(id);
        } else {
            self.info.pending.insert(id.to_string(), v);
        }
        if let Some(app) = self.app.upgrade() {
            self.refresh_info(&app);
        }
    }

    /// Offers the values the tracks shown have in field `id`.
    fn many_values(&mut self, id: &str) {
        let Some(f) = Field::from_name(id) else {
            return;
        };
        let tracks: Vec<&Track> = self.info.many.iter().map(|(_, t)| &**t).collect();
        let choices: Vec<ValueChoice> = values_of(&tracks, f)
            .into_iter()
            .map(|(value, count)| ValueChoice {
                value: value.into(),
                count: count as i32,
            })
            .collect();
        if let Some(app) = self.app.upgrade() {
            app.global::<TrackInfo>()
                .set_choices(ModelRc::new(VecModel::from(choices)));
        }
    }

    /// Writes what was typed to every track shown, in the library and the
    /// files' tags, as one change Undo takes back. Typed fields are left
    /// alone by clean-up and look-ups from then on.
    fn apply_many(&mut self) {
        let pending = std::mem::take(&mut self.info.pending);
        let looked_up = std::mem::take(&mut self.info.looked_up);
        // One track shown: what a look-up found for it.
        let targets: Vec<usize> = if self.info.many.is_empty() {
            let Some(i) = self.info_index() else {
                return;
            };
            // A file shown in Files joins the collection, as editing it does.
            self.adopt(&[i]);
            vec![i]
        } else {
            self.info.many.iter().map(|(i, _)| *i).collect()
        };
        let mut changes = Vec::new();
        for &index in &targets {
            for (id, value) in &pending {
                if let Some(f) = Field::from_name(id) {
                    changes.push((index, f, value.clone()));
                }
            }
        }
        let status = self.make_changes(changes, "edit");
        if let Some(db) = &self.db {
            for &i in &targets {
                let id = self.library[i].id;
                for field in pending.keys().filter(|k| !looked_up.contains(*k)) {
                    let _ = db.mark_hand_edited(id, field);
                }
            }
        }
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<TrackInfo>();
            ui.set_status(status.into());
            ui.set_status_ok(true);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_side();
    }

    /// Looks the track shown up online; what's found waits in its fields
    /// for Apply. Several tracks shown are looked up together, and the
    /// review table opens with what was found.
    fn info_look_up(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<TrackInfo>();
        if !self.info.many.is_empty() {
            let picked: Vec<usize> = self.info.many.iter().map(|(i, _)| *i).collect();
            let status = self.batch_lookup(&picked);
            ui.set_status(status.into());
            ui.set_status_ok(true);
            return;
        }
        let Some((index, t)) = self.info.shown.clone() else {
            return;
        };
        if self.info.lookup.is_some() {
            return;
        }
        let (key, token) = (
            crate::settings::acoustid_key(),
            crate::settings::discogs_token(),
        );
        let Some(QueryFor { query, .. }) = query_for(&t, &key, &token) else {
            ui.set_status(
                "There's no artist and title to look this track up by, in its tags or \
                 file name. An AcoustID key in Settings › Collection lets DJ Sugar \
                 recognise it by its sound."
                    .into(),
            );
            ui.set_status_ok(false);
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("info-lookup".into())
            .spawn(move || {
                let _ = tx.send(crate::lookup::look_up(&query));
            });
        if spawned.is_err() {
            return;
        }
        self.info.lookup = Some((index, rx));
        ui.set_looking_up(true);
        ui.set_status("Looking up online…".into());
        ui.set_status_ok(true);
    }

    /// Takes in what Look up online found for the track shown.
    fn poll_info_lookup(&mut self, app: &AppWindow) {
        let Some((index, rx)) = &self.info.lookup else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        let index = *index;
        self.info.lookup = None;
        let ui = app.global::<TrackInfo>();
        ui.set_looking_up(false);
        if self.info_index() != Some(index) || !self.info.many.is_empty() {
            return;
        }
        let t = self.library[index].clone();
        let hand = self
            .db
            .as_ref()
            .and_then(|db| db.hand_edited().ok())
            .unwrap_or_default();
        let key = crate::settings::acoustid_key();
        let name = query_for(&t, &key, "").and_then(|q| q.name);
        let (found, status) = match &result {
            Ok(f) => (
                Some(f),
                format!("Found on {} ({:.0}% sure).", f.source, f.score * 100.0),
            ),
            Err(Miss::NotFound) => (None, "Not found online.".to_string()),
            Err(Miss::Unreachable) => {
                ui.set_status(
                    "Couldn't reach MusicBrainz. Check the connection and try again.".into(),
                );
                ui.set_status_ok(false);
                return;
            }
        };
        let changes = info_proposals(&t, found, name.as_ref(), &hand);
        let status = match (changes.is_empty(), found.is_some()) {
            (true, true) => format!("{status} Nothing to change."),
            (true, false) => status,
            (false, true) => format!("{status} Check the ● fields, then Apply."),
            (false, false) => {
                format!("{status} The file name gives the ● fields; check them, then Apply.")
            }
        };
        for c in changes {
            let id = c.field.name().into_owned();
            self.info.pending.insert(id.clone(), c.new);
            self.info.looked_up.insert(id);
        }
        ui.set_status(status.into());
        ui.set_status_ok(true);
        self.refresh_info(app);
    }

    /// Starts looking for covers for the track shown.
    fn find_covers(&mut self) {
        let Some((_, t)) = &self.info.shown else {
            return;
        };
        let path = match &t.source {
            Source::File(p) if !t.info.missing && !is_cloud_only(p) => Some(p.clone()),
            _ => None,
        };
        self.info.found.clear();
        self.info.search = Some(coverart::search(coverart::Wanted {
            path,
            artist: t.artist.clone(),
            album: t.album.clone(),
            title: t.title.clone(),
            discogs_token: crate::settings::discogs_token(),
        }));
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<TrackInfo>();
            ui.set_cover_searching(true);
            ui.set_cover_note(SharedString::new());
            ui.set_cover_picked(-1);
            ui.set_covers(ModelRc::new(VecModel::from(Vec::<CoverChoice>::new())));
        }
    }

    fn poll_cover_search(&mut self, app: &AppWindow) {
        let Some(rx) = &self.info.search else {
            return;
        };
        let mut changed = false;
        let mut done = false;
        while let Ok(e) = rx.try_recv() {
            match e {
                CoverEvent::Found(f) => {
                    // The same picture from two places shows once.
                    if !self.info.found.iter().any(|o| o.data == f.data) {
                        self.info.found.push(f);
                        changed = true;
                    }
                }
                CoverEvent::Done => done = true,
            }
        }
        let ui = app.global::<TrackInfo>();
        if changed {
            self.show_covers(app);
        }
        if done {
            self.info.search = None;
            ui.set_cover_searching(false);
        }
    }

    /// Puts what the cover dialog has found on show.
    fn show_covers(&self, app: &AppWindow) {
        let choices: Vec<CoverChoice> = self
            .info
            .found
            .iter()
            .map(|f| CoverChoice {
                image: image(&f.thumb),
                source: f.source.clone().into(),
                size: format!("{}x{}", f.width, f.height).into(),
            })
            .collect();
        app.global::<TrackInfo>()
            .set_covers(ModelRc::new(VecModel::from(choices)));
    }

    /// Asks for a picture file to use as the track's cover (Load File… in
    /// the cover dialog); it is read on the dialog's thread.
    fn load_cover_file(&mut self) {
        let tx = self.info.picked_tx.clone();
        crate::prefs::choose_file("Choose a cover picture (JPEG or PNG)", move |path| {
            let _ = tx.send(coverart::from_file(&path));
        });
    }

    /// Puts a picture picked with Load File… first in the cover dialog,
    /// picked, so Save to Tag takes it.
    fn poll_cover_file(&mut self, app: &AppWindow) {
        while let Ok(picked) = self.info.picked_rx.try_recv() {
            let ui = app.global::<TrackInfo>();
            match picked {
                Ok(f) => {
                    // A second file replaces the first.
                    self.info.found.retain(|o| o.source != f.source);
                    self.info.found.retain(|o| o.data != f.data);
                    self.info.found.insert(0, f);
                    self.show_covers(app);
                    ui.set_cover_picked(0);
                    ui.set_cover_note(SharedString::new());
                }
                Err(e) => ui.set_cover_note(e.into()),
            }
        }
    }

    /// Embeds the cover found at `index` in the track's tags.
    fn save_cover(&mut self, index: usize) {
        let (Some((i, _)), Some(found)) = (self.info.shown.clone(), self.info.found.get(index))
        else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let data = found.data.clone();
        let ui = app.global::<TrackInfo>();
        if let Err(e) = self.keep_cover(i, &data) {
            ui.set_status(e.into());
            return;
        }
        ui.set_status(SharedString::new());
        ui.set_cover_open(false);
    }

    /// Embeds `data` (an image file) as library track `i`'s cover, and
    /// shows it everywhere the track is; Err says why it couldn't be.
    pub(super) fn keep_cover(&mut self, i: usize, data: &[u8]) -> Result<(), String> {
        let Some(t) = self.library.get(i).cloned() else {
            return Err("The track is gone.".into());
        };
        let Some(path) = writable(&t) else {
            return Err("The file isn't on this computer, so the cover can't be saved.".into());
        };
        tags::write_cover(path, data).map_err(|e| format!("Couldn't save the cover: {e}"))?;
        self.adopt(&[i]);
        let t = self.library[i].clone();
        if let Some(db) = &self.db {
            // Notes the file's new time, so a scan doesn't read it again.
            if let Err(e) = db.save_info(t.id, &t) {
                eprintln!("can't save the track: {e}");
            }
        }
        // Collection Health looks again, so the track leaves No Cover.
        self.cover_scan.add(t.id, path, &t.album);
        // Every list showing the track, and any deck it's on, shows the
        // new cover at once.
        self.art.set(&t, data);
        self.album_art.set(&t, data);
        let cover = cover::decode(data, cover::COVER_SIZE).map(Arc::new);
        self.engine.set_cover(path, cover);
        if self
            .info
            .shown
            .as_ref()
            .is_some_and(|(shown, _)| *shown == i)
        {
            self.read_info_cover();
        }
        self.refresh_rows();
        self.refresh_side();
        Ok(())
    }

    /// Plays or pauses the track shown in the headphones.
    fn preview_toggle(&mut self) {
        let Some(i) = self.info_index() else {
            return;
        };
        if self.info.previewing == Some(i) {
            self.info.play.set_bool(!self.info.play.get_bool());
        } else {
            self.engine.preview(Some(&self.library[i]), true);
            self.info.previewing = Some(i);
        }
        self.info.shown_position = (true, -1);
    }

    /// Shows the waveform of the track shown in the preview bar, once its
    /// overview has been read (see [`crate::waves`]).
    pub(super) fn fill_info_wave(&self, app: &AppWindow) {
        let wave = self
            .info
            .shown
            .as_ref()
            .and_then(|(_, t)| self.waves.get(t));
        let ui = app.global::<TrackInfo>();
        ui.set_has_wave(wave.is_some());
        ui.set_wave(wave.unwrap_or_default());
    }

    fn poll_preview(&mut self, app: &AppWindow) {
        let mine = self.info.previewing.is_some() && self.info.previewing == self.info_index();
        let playing = mine && self.info.play.get_bool();
        let at = if mine { self.info.position.get() } else { 0.0 };
        // Updated about 20 times a second at most.
        let step = (at * 1000.0) as i64;
        if (playing, step) == self.info.shown_position {
            return;
        }
        self.info.shown_position = (playing, step);
        let ui = app.global::<TrackInfo>();
        ui.set_preview_playing(playing);
        ui.set_preview_position(at as f32);
        let duration = self.info.duration.get();
        ui.set_preview_time(if mine && duration > 0.0 {
            short_time(at * duration)
        } else {
            SharedString::new()
        });
    }
}

fn image(c: &Cover) -> slint::Image {
    let pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&c.rgba, c.width, c.height);
    slint::Image::from_rgba8(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            id: 1,
            artist: "Maribou State".into(),
            title: "Home".into(),
            album: "Portraits".into(),
            genre: String::new(),
            rating: 0,
            bpm: 100.0,
            key: "10B".into(),
            duration: 196.0,
            first_beat: 0.5,
            source: Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        }
    }

    #[test]
    fn takes_edits() {
        let mut t = track();
        apply(&mut t, "remix", " Club Mix ").unwrap();
        assert_eq!(t.info.remix, "Club Mix");
        apply(&mut t, "track_number", "7").unwrap();
        assert_eq!(t.info.track_number, 7);
        assert!(apply(&mut t, "track_number", "seven").is_err());
        apply(&mut t, "gain", "+2.5 dB").unwrap();
        assert_eq!(t.info.gain_db, Some(2.5));
        apply(&mut t, "gain", "").unwrap();
        assert_eq!(t.info.gain_db, None);
        assert!(apply(&mut t, "gain", "40").is_err());
        apply(&mut t, "rating", "4").unwrap();
        apply(&mut t, "tag:3", "#dusk, Late Night").unwrap();
        assert_eq!(t.info.tag_text(3), "dusk, Late Night");
        assert!(apply(&mut t, "tag0", "x").is_err());
        assert_eq!(t.rating, 4);
        apply(&mut t, "bpm", "124").unwrap();
        assert_eq!(t.bpm, 124.0);
        // The beats stay where they were: one still falls at 0.5 s.
        let beat = t.beat_at(0.5);
        assert!((beat - beat.round()).abs() < 1e-9, "{beat}");
        assert!(apply(&mut t, "bpm", "1000").is_err());
    }

    #[test]
    fn lists_the_fields_by_tab() {
        let mut t = track();
        t.info.gain_db = Some(-1.5);
        t.info.play_count = 3;
        let f = fields_of(&t, true, "Finder");
        let labels = |tab: i32| -> Vec<String> {
            shown_fields(f.clone(), tab)
                .iter()
                .map(|f| f.label.to_string())
                .collect()
        };
        assert_eq!(
            labels(TRACK_TAB),
            [
                "TRACK", "ARTIST", "REMIX", "ALBUM", "GENRE", "YEAR", "TRACK #", "BPM", "KEY",
                "GAIN", "TIME"
            ]
        );
        assert!(
            labels(TAGS_TAB)[0].starts_with("TAG"),
            "the DJ's tags first"
        );
        assert!(labels(TAGS_TAB).ends_with(&["GROUPING".to_string(), "COMMENT".to_string()]));
        assert!(labels(FILE_TAB).contains(&"PLAYS".to_string()));
        let line = |label: &str| f.iter().find(|f| f.label == label).unwrap();
        assert_eq!(line("BPM").value, "100");
        assert_eq!(line("GAIN").value, "-1.5 dB");
        assert_eq!(line("PLAYS").value, "3");
    }

    #[test]
    fn short_fields_share_a_line() {
        let t = track();
        let l = lines(shown_fields(fields_of(&t, true, "Finder"), TRACK_TAB));
        let pairs: Vec<(String, String)> = l
            .iter()
            .filter(|l| l.two)
            .map(|l| (l.a.label.to_string(), l.b.label.to_string()))
            .collect();
        assert_eq!(
            pairs,
            [("YEAR", "TRACK #"), ("BPM", "KEY"), ("GAIN", "TIME")]
                .map(|(a, b)| (a.to_string(), b.to_string()))
        );
        assert_eq!(l.len(), 8, "five long lines and three shared");
    }

    #[test]
    fn sums_up_several_tracks() {
        let mut a = track();
        a.genre = "House".into();
        a.info.year = "2015".into();
        let mut b = track();
        b.artist = "Tycho".into();
        b.genre = "House".into();
        b.bpm = 124.0;
        b.key = "8A".into();
        let mut c = track();
        c.artist = "Bonobo".into();
        c.genre = "Downtempo".into();
        c.bpm = 0.0;
        let tracks = [&a, &b, &c];
        assert_eq!(
            values_of(&tracks, Field::Genre),
            [("House".to_string(), 2), ("Downtempo".to_string(), 1)]
        );
        assert_eq!(
            names_summary(tracks.iter().map(|t| t.artist.as_str())),
            "Maribou State, Tycho, +1"
        );
        assert_eq!(bpm_range(&tracks), "100–124");
        assert_eq!(keys_summary(&keys_of(&tracks, true)), "2 keys");

        let mut pending = BTreeMap::new();
        pending.insert("year".to_string(), "2016".to_string());
        let f = many_fields(&tracks, &pending, true, 1);
        let line = |id: &str| f.iter().find(|f| f.id == id).unwrap();
        assert_eq!(line("genre").mixed, 2);
        assert_eq!(line("genre").hint, "2 values");
        assert_eq!(line("album").value, "Portraits", "shared by all");
        assert!(line("year").edited);
        assert_eq!(line("year").value, "2016");
        let label = |l: &str| f.iter().find(|f| f.label == l).unwrap();
        assert_eq!(label("BPM").value, "100–124");
        assert_eq!(label("KEY").value, "2 keys");
        assert_eq!(label("ONLY IN COLLECTION").value, "2");
        assert_eq!(line("comment").tab, TAGS_TAB);
        assert_eq!(
            shown_fields(f.clone(), TRACK_TAB).len(),
            8,
            "six names, then BPM and key"
        );
    }

    #[test]
    fn writes_sizes_and_gains() {
        assert_eq!(size_text(9_856_000), "9.4 MB");
        assert_eq!(size_text(2_000), "2 KB");
        assert_eq!(gain_text(Some(1.5)), "+1.5 dB");
        assert_eq!(parse_gain("−3").unwrap(), Some(-3.0));
    }
}
