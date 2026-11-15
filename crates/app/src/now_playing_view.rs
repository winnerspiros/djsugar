//! Player mode's Now Playing view (ui/nowplaying.slint): fills
//! `NowPlayingUi` with the track the Player bar shows, its art, what the
//! file is and its Details, while the view is open.

use std::time::Duration;

use djsugar_engine::{LoadedTracks, Track};
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};

use crate::{AppWindow, NowDetail, NowPlayingUi, Theme, UpNextUi};

/// Starts filling the view. Keep the timer for as long as the app runs.
pub fn install(app: &AppWindow, loaded: LoadedTracks) -> Timer {
    let weak = app.as_weak();
    // The track and cover last shown, so the view is only set again when
    // they change.
    let mut shown = String::new();
    // The same for Up Next's Now playing row.
    let mut up_shown = String::new();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        let Some(app) = weak.upgrade() else { return };
        show_up_next(&app, &mut up_shown);
        let ui = app.global::<NowPlayingUi>();
        if !ui.get_open() {
            shown.clear();
            return;
        }
        let d = app.get_player_deck().clamp(0, 1) as usize;
        let Some(info) = app.get_decks().row_data(d) else {
            return;
        };
        let track = loaded.read().ok().and_then(|t| t.get(d).cloned().flatten());
        let key = format!(
            "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
            info.loaded,
            info.title,
            info.artist,
            info.has_cover,
            info.quality,
            track.as_ref().map_or(0, |t| t.info.play_count)
        );
        if key == shown {
            return;
        }
        shown = key;
        let theme = app.global::<Theme>();
        ui.set_loaded(info.loaded);
        ui.set_title(info.title.clone());
        ui.set_artist(info.artist.clone());
        ui.set_cover(info.cover.clone());
        ui.set_has_cover(info.has_cover);
        ui.set_initials(info.initials.clone());
        ui.set_tint(theme.invoke_deck_color(d as i32));
        let bpm = info
            .bpm
            .parse::<f64>()
            .ok()
            .filter(|b| *b > 0.0)
            .map(|b| format!("{} BPM", (b * 10.0).round() / 10.0))
            .unwrap_or_default();
        ui.set_bpm(bpm.clone().into());
        ui.set_key(info.key.clone());
        ui.set_key_color(theme.invoke_key_wheel_color(info.key_wheel));
        ui.set_quality(info.quality.clone());
        ui.set_lossless(info.lossless);
        let album = track
            .as_ref()
            .map(|t| album_line(&t.album, &t.info.year))
            .unwrap_or_default();
        ui.set_album(album.into());
        let rows = if info.loaded {
            details(
                track.as_deref(),
                &bpm,
                &info.key,
                &info.quality,
                info.duration,
            )
        } else {
            Vec::new()
        };
        ui.set_details(ModelRc::new(VecModel::from(rows)));
    });
    timer
}

/// Fills Up Next's Now playing row with the track the Player bar shows,
/// when it changed since `shown`.
fn show_up_next(app: &AppWindow, shown: &mut String) {
    let d = app.get_player_deck().clamp(0, 1) as usize;
    let Some(info) = app.get_decks().row_data(d) else {
        return;
    };
    let key = format!(
        "{}\u{1}{}\u{1}{}\u{1}{}",
        info.loaded, info.title, info.artist, info.has_cover
    );
    if key == *shown {
        return;
    }
    *shown = key;
    let ui = app.global::<UpNextUi>();
    ui.set_now_loaded(info.loaded);
    ui.set_now_title(info.title.clone());
    ui.set_now_artist(info.artist.clone());
    ui.set_now_cover(info.cover.clone());
    ui.set_now_has_cover(info.has_cover);
    ui.set_now_initials(info.initials.clone());
    ui.set_now_tint(app.global::<Theme>().invoke_deck_color(d as i32));
}

/// "Night Shift · 2024": the album and its year, either alone, or nothing.
fn album_line(album: &str, year: &str) -> String {
    match (album.trim(), year.trim()) {
        ("", "") => String::new(),
        (a, "") => a.to_string(),
        ("", y) => y.to_string(),
        (a, y) => format!("{a} · {y}"),
    }
}

/// The Details tab's rows: what the collection knows about the track, the
/// empty ones left out.
fn details(
    track: Option<&Track>,
    bpm: &str,
    key: &str,
    quality: &str,
    seconds: f32,
) -> Vec<NowDetail> {
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(t) = track {
        rows.push(("Title", t.title.clone()));
        rows.push(("Artist", t.artist.clone()));
        if !t.info.remix.is_empty() {
            rows.push(("Remix", t.info.remix.clone()));
        }
        rows.push(("Album", t.album.clone()));
        if t.info.track_number > 0 {
            let n = if t.info.disc_number > 1 {
                format!("{} on disc {}", t.info.track_number, t.info.disc_number)
            } else {
                t.info.track_number.to_string()
            };
            rows.push(("Track", n));
        }
        rows.push(("Year", t.info.year.clone()));
        rows.push(("Genre", t.genre.clone()));
    }
    let s = seconds.max(0.0).round() as u32;
    if s > 0 {
        rows.push(("Length", format!("{}:{:02}", s / 60, s % 60)));
    }
    rows.push(("Tempo", bpm.to_string()));
    rows.push(("Key", key.to_string()));
    rows.push(("File", quality.to_string()));
    if let Some(t) = track {
        if t.rating > 0 {
            rows.push(("Rating", "★".repeat(t.rating.min(5) as usize)));
        }
        if t.info.play_count > 0 {
            let times = if t.info.play_count == 1 {
                "time"
            } else {
                "times"
            };
            rows.push(("Played", format!("{} {times}", t.info.play_count)));
        }
        if let Some(last) = t.info.last_played {
            rows.push(("Last played", date(last)));
        }
        if t.info.added > 0 {
            rows.push(("Added", date(t.info.added)));
        }
        rows.push(("Comment", t.info.comment.clone()));
    }
    rows.into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(label, value)| NowDetail {
            label: label.into(),
            value: value.trim().into(),
        })
        .collect()
}

/// A Unix time as "9 Nov 2026".
fn date(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%-d %b %Y")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_and_year() {
        assert_eq!(album_line("Night Shift", "2024"), "Night Shift · 2024");
        assert_eq!(album_line("Night Shift", ""), "Night Shift");
        assert_eq!(album_line(" ", "2024"), "2024");
        assert_eq!(album_line("", ""), "");
    }

    #[test]
    fn details_leave_out_empty_rows() {
        let rows = details(None, "126 BPM", "", "FLAC 16-BIT 44.1 KHZ", 200.4);
        let labels: Vec<_> = rows.iter().map(|r| r.label.to_string()).collect();
        assert_eq!(labels, ["Length", "Tempo", "File"]);
        assert_eq!(rows[0].value, "3:20");
    }
}
