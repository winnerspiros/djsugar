//! Export to USB (for Pioneer DJ and Engine DJ players) from File › Export
//! and the tree's menu: the dialog (see `ui/usb_export.slint`),
//! the lists and tracks of the node it was opened on, and the export
//! itself on a thread of its own (see [`djsugar_export`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use djsugar_engine::Track;
use djsugar_export::{List, Options, Report, Step};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::{Browser, Contents};
use crate::{files, AppWindow, LibraryEdit, UsbExport};

/// What the dialog was opened for, and the export once it runs.
#[derive(Default)]
pub(super) struct State {
    tracks: Vec<Track>,
    lists: Vec<List>,
    drives: Vec<PathBuf>,
    job: Option<Job>,
}

struct Job {
    messages: Receiver<Message>,
    stop: Arc<AtomicBool>,
}

enum Message {
    Step(Step),
    Done(Result<Report, djsugar_export::Error>),
}

impl Browser {
    pub(super) fn install_usb_export(
        app: &AppWindow,
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_usb(move |row| w(&|b| b.open_usb_export(row as usize)));
        let ui = app.global::<UsbExport>();
        let w = with.clone();
        ui.on_refresh_drives(move || w(&|b| b.show_drives()));
        let w = with.clone();
        ui.on_start(move || w(&|b| b.start_usb_export()));
        let w = with.clone();
        ui.on_cancel(move || {
            w(&|b| {
                if let Some(job) = &b.usb.job {
                    job.stop.store(true, Ordering::Relaxed);
                }
            })
        });
    }

    fn open_usb_export(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        if self.usb.job.is_some() {
            return;
        }
        let (tracks, lists) = self.usb_lists(node);
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let name = &self.nodes[node].name;
        let list_count = lists.iter().filter(|l| !l.folder).count();
        let what = if list_count > 1 {
            format!("{name} ({list_count} lists)")
        } else {
            name.clone()
        };
        let ui = app.global::<UsbExport>();
        ui.set_what(what.into());
        ui.set_track_count(tracks.len() as i32);
        ui.set_result(SharedString::new());
        ui.set_failed(false);
        ui.set_running(false);
        ui.set_progress(0.0);
        self.usb.tracks = tracks;
        self.usb.lists = lists;
        self.show_drives();
        ui.set_open(true);
    }

    fn show_drives(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<UsbExport>();
        let picked = usize::try_from(ui.get_drive())
            .ok()
            .and_then(|i| self.usb.drives.get(i).cloned());
        self.usb.drives = files::drives();
        let names: Vec<SharedString> = self
            .usb
            .drives
            .iter()
            .map(|d| drive_name(d).into())
            .collect();
        ui.set_drives(ModelRc::new(VecModel::from(names)));
        // Keep the drive picked before, else the only one there is.
        let keep = picked.and_then(|p| self.usb.drives.iter().position(|d| *d == p));
        let only = (self.usb.drives.len() == 1).then_some(0);
        ui.set_drive(keep.or(only).map_or(-1, |i| i as i32));
    }

    /// The tracks under a node, and the lists to make of them: a folder
    /// keeps its lists and subfolders, anything else is one list.
    fn usb_lists(&self, node: usize) -> (Vec<Track>, Vec<List>) {
        let mut tracks: Vec<Track> = Vec::new();
        let mut index: HashMap<usize, usize> = HashMap::new();
        let mut list_of = |b: &Browser, n: usize, parent: Option<usize>| -> List {
            let folder = matches!(b.nodes[n].contents, Contents::Folder);
            let mut list = List {
                name: b.nodes[n].name.clone(),
                parent,
                folder,
                tracks: Vec::new(),
            };
            if !folder {
                for t in b.node_tracks(n) {
                    let i = *index.entry(t).or_insert_with(|| {
                        tracks.push((*b.library[t]).clone());
                        tracks.len() - 1
                    });
                    list.tracks.push(i);
                }
            }
            list
        };
        let mut lists = vec![list_of(self, node, None)];
        if lists[0].folder {
            let top = &self.nodes[node];
            // The folders open above each depth, by their place in `lists`.
            let mut open: Vec<(i32, usize)> = vec![(top.depth, 0)];
            for n in node + 1..self.nodes.len() {
                let m = &self.nodes[n];
                if m.depth <= top.depth || m.history != top.history {
                    break;
                }
                while open.last().is_some_and(|&(d, _)| d >= m.depth) {
                    open.pop();
                }
                let parent = open.last().map(|&(_, i)| i);
                let list = list_of(self, n, parent);
                if list.folder {
                    open.push((m.depth, lists.len()));
                }
                lists.push(list);
            }
        }
        (tracks, lists)
    }

    fn start_usb_export(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<UsbExport>();
        let Some(root) = usize::try_from(ui.get_drive())
            .ok()
            .and_then(|i| self.usb.drives.get(i).cloned())
        else {
            return;
        };
        let format = ui.get_format();
        let options = Options {
            name: drive_name(&root),
            root,
            device_library: matches!(format, 0 | 1 | 4),
            one_library: matches!(format, 0 | 2 | 4),
            engine_dj: matches!(format, 3 | 4),
            camelot: self.camelot,
        };
        let tracks = std::mem::take(&mut self.usb.tracks);
        let lists = std::mem::take(&mut self.usb.lists);
        let (send, messages) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::Builder::new()
            .name("usb export".into())
            .spawn(move || {
                let steps = send.clone();
                let result = djsugar_export::export(&tracks, &lists, &options, &mut |step| {
                    let _ = steps.send(Message::Step(step));
                    !stopped.load(Ordering::Relaxed)
                });
                let _ = send.send(Message::Done(result));
            })
            .expect("usb export thread");
        self.usb.job = Some(Job { messages, stop });
        ui.set_running(true);
        ui.set_progress(0.0);
        ui.set_status("Starting…".into());
    }

    /// Shows how the export is going; called from the UI timer.
    pub fn poll_usb_export(&mut self) {
        let Some(job) = &self.usb.job else {
            return;
        };
        let mut last = None;
        let mut done = None;
        loop {
            match job.messages.try_recv() {
                Ok(Message::Step(s)) => last = Some(s),
                Ok(Message::Done(r)) => {
                    done = Some(r);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    done = Some(Err(djsugar_export::Error::Other(
                        "the export stopped".into(),
                    )));
                    break;
                }
            }
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<UsbExport>();
        match last {
            Some(Step::Track { done, total, title }) => {
                ui.set_progress(done as f32 / (total.max(1) + 1) as f32);
                ui.set_status(format!("{} of {total}: {title}", done + 1).into());
            }
            Some(Step::Library) => {
                ui.set_progress(0.95);
                ui.set_status("Writing the library…".into());
            }
            None => {}
        }
        if let Some(result) = done {
            self.usb.job = None;
            ui.set_running(false);
            let (text, failed) = match result {
                Ok(report) => (summary(&report), false),
                Err(djsugar_export::Error::Cancelled) => (
                    "Stopped. The stick's library is as it was.".to_string(),
                    false,
                ),
                Err(e) => (format!("The export failed: {e}"), true),
            };
            ui.set_result(text.into());
            ui.set_failed(failed);
        }
    }
}

/// What players show for the stick: its folder's name.
fn drive_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "DJ Sugar".into())
}

fn summary(r: &Report) -> String {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut text = format!(
        "Done: {} in {} on the stick ({} copied).",
        plural(r.tracks, "track", "tracks"),
        plural(r.lists, "list", "lists"),
        r.copied
    );
    if r.hot_cues_dropped > 0 {
        text += &format!(
            " {} past H left out, as players have eight.",
            plural(r.hot_cues_dropped, "hot cue", "hot cues")
        );
    }
    if !r.skipped.is_empty() {
        let names: Vec<String> = r
            .skipped
            .iter()
            .take(3)
            .map(|(t, why)| format!("{t} ({why})"))
            .collect();
        let more = r.skipped.len().saturating_sub(3);
        text += &format!(
            " Left out {}: {}{}.",
            plural(r.skipped.len(), "track", "tracks"),
            names.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        );
    }
    text
}
