//! Time words: Whisper listens to the track's vocals and the lyrics shown
//! get a time for each word (see [`djsugar_engine::word_timing`]), saved as
//! an enhanced `.lrc` beside the track. Transcribe does the same for a
//! track with no lyrics, writing down the words heard, marked as made by
//! DJ Sugar ([`lyrics::AUTO`]). One track at a time, on a thread
//! of its own: the first time downloads the model (about 550 MB), and a
//! track without stems has its vocals separated first.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use djsugar_engine::lyrics::{self, Line, Timing};
use djsugar_engine::word_timing::{self, Stage};
use djsugar_engine::Source;
use slint::{ComponentHandle, SharedString};

use super::super::Browser;
use super::{store, LYRICS_TAB};
use crate::{AppWindow, LyricsUi};

enum Message {
    Stage(Stage),
    Done(Result<String, String>),
}

/// Words being timed for one track.
pub(super) struct Job {
    id: usize,
    path: PathBuf,
    rx: Receiver<Message>,
    stage: Stage,
    /// Writing down a track's words, not timing ones it has.
    transcribing: bool,
}

/// What the Lyrics tab says while words are timed.
fn describe(stage: Stage) -> String {
    match stage {
        Stage::Downloading => "Downloading the Whisper model (550 MB)…".into(),
        Stage::Separating => "Separating the vocals…".into(),
        Stage::Listening(p) => format!("Listening… {}%", p.clamp(0, 100)),
    }
}

impl Browser {
    /// Starts timing the words of the lyrics shown, or, for a track with
    /// none, writing them down.
    pub(super) fn time_words(&mut self) {
        if self.lyrics.timing.is_some() {
            return;
        }
        let Some((_, track)) = &self.lyrics.shown else {
            return;
        };
        let Source::File(path) = &track.source else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let lines: Option<Vec<Line>> = self.lyrics.lyrics.as_ref().map(|l| l.lines.clone());
        let transcribing = lines.is_none();
        let file = path.clone();
        let progress = tx.clone();
        std::thread::spawn(move || {
            let report: word_timing::Report = Arc::new(move |s| {
                let _ = progress.send(Message::Stage(s));
            });
            let done = match lines {
                Some(lines) => word_timing::time_words(&file, &lines, report)
                    .map(|timed| lyrics::to_lrc(&timed)),
                None => word_timing::transcribe(&file, report)
                    .map(|heard| format!("{}\n{}", lyrics::AUTO, lyrics::to_lrc(&heard))),
            };
            let _ = tx.send(Message::Done(done));
        });
        self.lyrics.timing = Some(Job {
            id: track.id,
            path: path.clone(),
            rx,
            stage: Stage::Listening(0),
            transcribing,
        });
        if let Some(app) = self.app.upgrade() {
            self.show_timing(&app.global::<LyricsUi>());
        }
    }

    /// Whether the tab offers Time words: lyrics for a file, not yet timed
    /// by word, and no other track's words being timed.
    pub(super) fn can_time_words(&self) -> bool {
        self.lyrics.timing.is_none()
            && self
                .lyrics
                .shown
                .as_ref()
                .is_some_and(|(_, t)| matches!(t.source, Source::File(_)))
            && self
                .lyrics
                .lyrics
                .as_ref()
                .is_some_and(|l| l.timing() != Timing::Words)
    }

    /// The Time words button and what timing says, for the track shown.
    pub(super) fn show_timing(&self, ui: &LyricsUi) {
        ui.set_can_time(self.can_time_words());
        ui.set_can_transcribe(
            self.lyrics.timing.is_none()
                && self.lyrics.job.is_none()
                && self.lyrics.lyrics.is_none()
                && self
                    .lyrics
                    .shown
                    .as_ref()
                    .is_some_and(|(_, t)| matches!(t.source, Source::File(_))),
        );
        let here = self.lyrics.timing.as_ref().filter(|j| {
            self.lyrics
                .shown
                .as_ref()
                .is_some_and(|(_, t)| t.id == j.id)
        });
        match here {
            Some(j) => ui.set_time_note(describe(j.stage).into()),
            None if self.lyrics.timing.is_some() => ui.set_time_note(SharedString::new()),
            None => {}
        }
    }

    /// Follows the words being timed; saves them when done. Called every
    /// frame, so a track's words are saved even with the tab hidden.
    pub(super) fn poll_timing(&mut self, app: &AppWindow) {
        let Some(job) = &mut self.lyrics.timing else {
            return;
        };
        let mut done = None;
        while let Ok(m) = job.rx.try_recv() {
            match m {
                Message::Stage(s) => job.stage = s,
                Message::Done(d) => done = Some(d),
            }
        }
        let ui = app.global::<LyricsUi>();
        let Some(done) = done else {
            self.show_timing(&ui);
            return;
        };
        let Some(job) = self.lyrics.timing.take() else {
            return;
        };
        let here = self
            .lyrics
            .shown
            .as_ref()
            .is_some_and(|(_, t)| t.id == job.id);
        let saved = done.and_then(|text| {
            lyrics::save(&job.path, store().as_deref(), &text)
                .map_err(|e| format!("couldn't save them: {e}"))
        });
        match saved {
            Ok(_) => {
                self.lyrics.wave.reload();
                ui.set_time_note(SharedString::new());
                if here {
                    // Read again, from the file just written.
                    self.lyrics.shown = None;
                    self.poll_lyrics(app, LYRICS_TAB);
                }
            }
            Err(e) if here => {
                let what = if job.transcribing {
                    "Couldn't transcribe the track"
                } else {
                    "Couldn't time the words"
                };
                ui.set_time_note(format!("{what}: {e}").into())
            }
            Err(_) => ui.set_time_note(SharedString::new()),
        }
        self.show_timing(&ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_it_is_doing() {
        assert_eq!(describe(Stage::Listening(42)), "Listening… 42%");
        assert_eq!(describe(Stage::Listening(140)), "Listening… 100%");
        assert!(describe(Stage::Downloading).contains("550 MB"));
    }
}
