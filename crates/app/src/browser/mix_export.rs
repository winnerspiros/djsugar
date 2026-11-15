//! Mixed sets: recordings join the Mixed Sets source as soon as they stop;
//! File › Export › Mixcloud… turns one into a 320 kbps MP3 whose ID3 tag
//! carries the tracklist as chapters, with a `.tracklist.txt` beside it;
//! "Upload to Mixcloud" signs in to the DJ's Mixcloud account (see
//! [`crate::mixcloud_upload`]) and sends that MP3 with its tracklist.
//!
//! The tracklist comes from the `.cue` sheet the recorder writes next to
//! the recording (each track from when it became audible). Recordings
//! split into parts are joined back into one MP3. The work runs on its own
//! thread; long WAVs are streamed, never held in memory.

use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use djsugar_broadcast::mixcloud::{self, Mp3File, Section, Show};
use djsugar_engine::decode::Decoder;
use djsugar_engine::scan::read_track;
use slint::ComponentHandle;

use super::{file_path, Browser, MIXED_SETS};
use crate::mixcloud_upload::{self, UploadError};
use crate::spotify::open_in_browser;
use crate::{files, AppWindow, FilesUi};

/// What the export thread says.
pub(super) enum Progress {
    Status(String),
    /// The MP3 is made.
    Made(PathBuf),
    /// The show is on Mixcloud, at this address.
    Uploaded(String),
    /// Signed in, or the sign-in turned out to be revoked.
    SignedIn(bool),
    Failed(String),
}

/// What to do with a mixed set.
#[derive(Clone, Copy, PartialEq)]
enum Job {
    Export,
    Upload,
}

impl Browser {
    pub(super) fn install_mix_export(
        ui: &FilesUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        ui.set_mixcloud_signed_in(mixcloud_upload::signed_in());
        let w = with.clone();
        ui.on_export_mixcloud(move |row| w(&|b| b.mix_job(row as usize, Job::Export)));
        let w = with.clone();
        ui.on_upload_mixcloud(move |row| w(&|b| b.mix_job(row as usize, Job::Upload)));
        let w = with.clone();
        ui.on_mixcloud_sign_out(move || {
            w(&|b| {
                mixcloud_upload::forget_token();
                if let Some(app) = b.app.upgrade() {
                    app.global::<FilesUi>().set_mixcloud_signed_in(false);
                }
                b.set_mix_status("Signed out of Mixcloud.");
            })
        });
    }

    /// Adds the files of a recording that just stopped to the library, so
    /// they show in Mixed Sets straight away rather than after the next
    /// scan.
    pub fn add_recordings(&mut self, files: &[PathBuf]) {
        let mut added = false;
        for path in files {
            if self.path_index.contains_key(path) || !path.is_file() {
                continue;
            }
            let mut track = read_track(self.library.len(), path);
            if let Some(db) = &mut self.db {
                match db.add_file(&track) {
                    Ok(t) => track = t,
                    Err(e) => {
                        eprintln!("can't add {} to the library: {e}", path.display());
                        continue;
                    }
                }
            }
            if !self.index_of.contains_key(&track.id) {
                self.push_track(track);
                added = true;
            }
        }
        if !added {
            return;
        }
        self.collection_stale = true;
        if self.tree == MIXED_SETS {
            self.rebuild_folder_tree(MIXED_SETS);
            self.refresh_tree();
            self.refresh_rows();
        }
    }

    fn set_mix_status(&self, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.global::<FilesUi>().set_mix_status(text.into());
        }
    }

    fn mix_job(&mut self, row: usize, job: Job) {
        if self.mix_export.is_some() {
            return;
        }
        if self.tree != MIXED_SETS || self.rows.get(row).copied().flatten().is_none() {
            if let Some(app) = self.app.upgrade() {
                app.set_alert("Pick a mixed set in Mixed Sets first.".into());
            }
            return;
        }
        let Some((path, title, artist)) = self.rows.get(row).copied().flatten().and_then(|i| {
            let t = &self.library[i];
            Some((
                file_path(t)?.to_path_buf(),
                t.title.clone(),
                t.artist.clone(),
            ))
        }) else {
            return;
        };
        let (tx, rx) = channel();
        self.mix_export = Some(rx);
        self.set_mix_status(match job {
            Job::Export => "Making the Mixcloud MP3…",
            Job::Upload if !mixcloud_upload::signed_in() => "Sign in to Mixcloud in your browser…",
            Job::Upload => "Getting ready to upload…",
        });
        std::thread::Builder::new()
            .name("mixcloud".into())
            .spawn(move || {
                let send = |p: Progress| {
                    let _ = tx.send(p);
                };
                let status = |s: String| send(Progress::Status(s));
                match job {
                    Job::Export => match export(&path, &title, &artist, &status) {
                        Ok((mp3, _)) => send(Progress::Made(mp3)),
                        Err(e) => send(Progress::Failed(format!(
                            "Couldn't make the Mixcloud MP3: {e}"
                        ))),
                    },
                    Job::Upload => upload(&path, &title, &artist, &send),
                }
            })
            .expect("spawn mixcloud thread");
    }

    /// Picks up what the Mixcloud thread said. Called every frame.
    pub fn poll_mix_export(&mut self, app: &AppWindow) {
        let Some(rx) = &self.mix_export else { return };
        let mut said = Vec::new();
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(p) => said.push(p),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        let ui = app.global::<FilesUi>();
        for p in said {
            match p {
                Progress::Status(s) => ui.set_mix_status(s.into()),
                Progress::SignedIn(on) => ui.set_mixcloud_signed_in(on),
                Progress::Made(mp3) => {
                    let name = mp3.file_name().unwrap_or_default().to_string_lossy();
                    ui.set_mix_status(format!("Made {name}, with its tracklist.").into());
                    files::reveal(&mp3);
                    self.add_recordings(&[mp3]);
                }
                Progress::Uploaded(url) => {
                    ui.set_mix_status(
                        "On Mixcloud, private for now: name it, add tags and a cover, then publish."
                            .into(),
                    );
                    open_in_browser(&url);
                }
                Progress::Failed(e) => ui.set_mix_status(e.into()),
            }
        }
        if finished {
            self.mix_export = None;
        }
    }
}

/// Signs in if need be, makes the MP3 if it isn't there, and uploads it as
/// a private show with the tracklist as its sections and description.
fn upload(path: &Path, title: &str, artist: &str, send: &dyn Fn(Progress)) {
    let status = |s: String| send(Progress::Status(s));
    let mut token = match mixcloud_upload::saved_token() {
        Some(t) => {
            send(Progress::SignedIn(true));
            t
        }
        None => match mixcloud_upload::sign_in() {
            Ok(t) => {
                send(Progress::SignedIn(true));
                t
            }
            Err(e) => return send(Progress::Failed(e)),
        },
    };
    let made = existing_mp3(path).map_or_else(
        || export(path, title, artist, &status),
        |mp3| tracklist(path).map(|t| (mp3, t.sections)),
    );
    let (mp3, sections) = match made {
        Ok(m) => m,
        Err(e) => {
            return send(Progress::Failed(format!(
                "Couldn't make the Mixcloud MP3: {e}"
            )))
        }
    };
    let show = Show {
        name: show_name(path, title),
        description: mixcloud::tracklist_text(&sections),
        tags: Vec::new(),
        unlisted: true,
        sections,
    };
    // A revoked sign-in gets one more go.
    for attempt in 0..2 {
        let mut last = -1i64;
        let result = mixcloud_upload::upload(&token, &show, &mp3, |share| {
            let pct = (share * 100.0).min(99.0) as i64;
            if pct != last {
                last = pct;
                status(format!("Uploading to Mixcloud… {pct}%"));
            }
        });
        match result {
            Ok(url) => return send(Progress::Uploaded(url)),
            Err(UploadError::SignedOut) if attempt == 0 => {
                mixcloud_upload::forget_token();
                send(Progress::SignedIn(false));
                status("Mixcloud signed you out. Sign in again in your browser…".into());
                match mixcloud_upload::sign_in() {
                    Ok(t) => {
                        send(Progress::SignedIn(true));
                        token = t;
                    }
                    Err(e) => return send(Progress::Failed(e)),
                }
            }
            Err(UploadError::SignedOut) => {
                return send(Progress::Failed(
                    "Mixcloud didn't accept the sign-in.".into(),
                ))
            }
            Err(UploadError::Other(e)) => {
                return send(Progress::Failed(format!("Mixcloud upload failed: {e}")))
            }
        }
    }
}

/// `dir/name` without its extension and a " (part N)" ending: the name
/// the recorder's `.cue` and `.tracklist.txt` share.
fn recording_base(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let stem = stem
        .strip_suffix(" (Mixcloud)")
        .unwrap_or(&stem)
        .to_string();
    let stem = match stem.rfind(" (part ") {
        Some(at) if stem.ends_with(')') => stem[..at].to_string(),
        _ => stem,
    };
    path.with_file_name(stem)
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut name = base.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    base.with_file_name(name)
}

/// The parts to join and the tracklist: from the `.cue` beside the
/// recording if there is one, else just the file with no tracks.
fn plan(path: &Path) -> (Vec<PathBuf>, Vec<mixcloud::CueFile>) {
    let base = recording_base(path);
    let dir = path.parent().unwrap_or(Path::new(""));
    let cue = std::fs::read_to_string(with_suffix(&base, ".cue"))
        .map(|t| mixcloud::parse_cue(&t))
        .unwrap_or_default();
    let parts: Vec<PathBuf> = cue
        .iter()
        .map(|f| dir.join(&f.name))
        .filter(|p| p.is_file())
        .collect();
    // A cue sheet naming files that aren't there (renamed, say) is no use
    // for timing; keep the file picked.
    let is_ours = parts.iter().any(|p| p == path) || existing_mp3(path).as_deref() == Some(path);
    if cue.is_empty() || parts.len() != cue.len() || !is_ours {
        return (vec![path.to_path_buf()], Vec::new());
    }
    (parts, cue)
}

/// The MP3 already made for this recording, if it is newer than the
/// recording.
fn existing_mp3(path: &Path) -> Option<PathBuf> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
        && path.to_string_lossy().ends_with(" (Mixcloud).mp3")
    {
        return Some(path.to_path_buf());
    }
    let mp3 = with_suffix(&recording_base(path), " (Mixcloud).mp3");
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    (modified(&mp3)? >= modified(path)?).then_some(mp3)
}

/// The show's name: the track's title, else the recording's file name.
fn show_name(path: &Path, title: &str) -> String {
    if title.trim().is_empty() {
        recording_base(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        title.trim().to_string()
    }
}

/// A recording opened for export: its parts, their decoders and the
/// tracklist timed across them.
struct Tracklist {
    parts: Vec<PathBuf>,
    decoders: Vec<Decoder>,
    sections: Vec<Section>,
    total: f64,
}

fn tracklist(path: &Path) -> Result<Tracklist, String> {
    let (parts, cue) = plan(path);
    let decoders = parts
        .iter()
        .map(|p| Decoder::open(p))
        .collect::<Result<Vec<_>, _>>()?;
    let lengths: Vec<f64> = decoders
        .iter()
        .map(|d| d.seconds().unwrap_or(0.0))
        .collect();
    Ok(Tracklist {
        parts,
        decoders,
        sections: mixcloud::sections(&cue, &lengths),
        total: lengths.iter().sum(),
    })
}

fn export(
    path: &Path,
    title: &str,
    artist: &str,
    status: &dyn Fn(String),
) -> Result<(PathBuf, Vec<Section>), String> {
    let Tracklist {
        parts,
        decoders,
        sections,
        total,
    } = tracklist(path)?;
    let base = recording_base(&parts[0]);
    let name = show_name(&parts[0], title);
    let mp3 = with_suffix(&base, " (Mixcloud).mp3");
    if mp3 == path {
        return Err("this is already a Mixcloud MP3".into());
    }
    let tag = mixcloud::id3_tag(&name, artist.trim(), &sections, total, None);
    let rate = decoders[0].audio().sample_rate;
    let mut out = Mp3File::create(&mp3, rate, &tag).map_err(|e| e.to_string())?;
    let mut done = 0.0;
    let mut last = -1i64;
    for d in decoders {
        let mut failed = None;
        let res = d.stream(|r, samples| {
            if r != rate {
                failed = Some("the parts have different sample rates".to_string());
                return false;
            }
            if let Err(e) = out.write(samples) {
                failed = Some(e.to_string());
                return false;
            }
            done += (samples.len() / 2) as f64 / r as f64;
            if total > 0.0 {
                let pct = (done / total * 100.0).min(99.0) as i64;
                if pct != last {
                    last = pct;
                    status(format!("Making the Mixcloud MP3… {pct}%"));
                }
            }
            true
        });
        if let Some(e) = failed {
            let _ = std::fs::remove_file(&mp3);
            return Err(e);
        }
        if let Err(e) = res {
            let _ = std::fs::remove_file(&mp3);
            return Err(e);
        }
    }
    out.finish().map_err(|e| e.to_string())?;
    if !sections.is_empty() {
        let list = with_suffix(&base, ".tracklist.txt");
        std::fs::write(&list, mixcloud::tracklist_text(&sections))
            .map_err(|e| format!("{}: {e}", list.display()))?;
    }
    Ok((mp3, sections))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_share_the_recordings_name() {
        assert_eq!(
            recording_base(Path::new("/s/DJSugar 2026-11-03 21.05 (part 2).wav")),
            Path::new("/s/DJSugar 2026-11-03 21.05")
        );
        assert_eq!(
            recording_base(Path::new("/s/DJSugar 2026-11-03 21.05.flac")),
            Path::new("/s/DJSugar 2026-11-03 21.05")
        );
    }

    /// A two-part recording with its cue sheet becomes one MP3 with both
    /// parts' tracks as chapters.
    #[test]
    fn joins_parts_and_times_their_tracks() {
        let dir = std::env::temp_dir().join(format!("djsugar-mix-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rate = 44_100u32;
        let wav = |name: &str, seconds: u32| {
            let frames = rate * seconds;
            let mut b = Vec::new();
            b.extend_from_slice(b"RIFF");
            b.extend_from_slice(&(36 + frames * 4).to_le_bytes());
            b.extend_from_slice(b"WAVEfmt ");
            b.extend_from_slice(&16u32.to_le_bytes());
            b.extend_from_slice(&1u16.to_le_bytes());
            b.extend_from_slice(&2u16.to_le_bytes());
            b.extend_from_slice(&rate.to_le_bytes());
            b.extend_from_slice(&(rate * 4).to_le_bytes());
            b.extend_from_slice(&4u16.to_le_bytes());
            b.extend_from_slice(&16u16.to_le_bytes());
            b.extend_from_slice(b"data");
            b.extend_from_slice(&(frames * 4).to_le_bytes());
            for i in 0..frames {
                let v = ((i as f32 * 0.05).sin() * 8000.0) as i16;
                b.extend_from_slice(&v.to_le_bytes());
                b.extend_from_slice(&v.to_le_bytes());
            }
            std::fs::write(dir.join(name), b).unwrap();
        };
        wav("set.wav", 2);
        wav("set (part 2).wav", 1);
        std::fs::write(
            dir.join("set.cue"),
            "FILE \"set.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"One\"\n    PERFORMER \"A\"\n    INDEX 01 00:00:00\n\
             FILE \"set (part 2).wav\" WAVE\n  TRACK 02 AUDIO\n    TITLE \"Two\"\n    PERFORMER \"B\"\n    INDEX 01 00:00:38\n",
        )
        .unwrap();
        let (mp3, sections) = export(&dir.join("set.wav"), "", "DJ", &|_| {}).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(existing_mp3(&dir.join("set.wav")), Some(mp3.clone()));
        // Uploading the MP3 itself still finds the recording's tracklist.
        assert_eq!(tracklist(&mp3).unwrap().sections, sections);
        assert_eq!(mp3, dir.join("set (Mixcloud).mp3"));
        let list = std::fs::read_to_string(dir.join("set.tracklist.txt")).unwrap();
        assert_eq!(list, "0:00 A - One\n0:02 B - Two\n");
        // Three seconds of audio at 320 kbps.
        let len = std::fs::metadata(&mp3).unwrap().len();
        assert!((100_000..140_000).contains(&len), "{len}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
