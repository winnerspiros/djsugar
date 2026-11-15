//! Backups of the collection and settings, as one `.djsugarbackup` zip:
//! `backup.json`, `settings.json` (without sign-ins), a snapshot of
//! `library.sqlite`, `audio-plugins.json` and the own controller mappings.
//!
//! Automatic backups go to the backup folder once a day or week and the
//! newest [`KEEP`] are kept. A restore is checked and unpacked next to the
//! settings, then put in place at the next start ([`apply_pending`]),
//! before anything opens the library.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};

use crate::settings;

pub const EXTENSION: &str = "djsugarbackup";
/// Settings › Collection › Storage › Back up: 0 off, 1 daily, 2 weekly.
pub const EVERY_KEY: &str = "backup_every";
pub const DEFAULT_EVERY: usize = 1;
pub const EVERY_CHOICES: [&str; 3] = ["Off", "Daily", "Weekly"];
/// Where backups go; empty for `Backups` in the config folder.
pub const FOLDER_KEY: &str = "backup_folder";
/// How many automatic backups are kept in the folder.
pub const KEEP: usize = 10;

const MANIFEST: &str = "backup.json";
const SETTINGS: &str = "settings.json";
const LIBRARY: &str = "library.sqlite";
const AUDIO_PLUGINS: &str = "audio-plugins.json";
const MAPPINGS: &str = "controller-mappings";
/// A restore waiting for the next start, in the config folder.
const PENDING: &str = "restore-pending";
/// Automatic and Back up now files start with this; others in the folder
/// (a restore's "Before restore", the DJ's own) are never pruned.
const AUTO_PREFIX: &str = "DJ Sugar backup ";
/// More than this in one file of a backup is not ours.
const MAX_FILE: u64 = 8 << 30;

/// The folder backups go to.
pub fn folder() -> Option<PathBuf> {
    settings::path_of(FOLDER_KEY).or_else(default_folder)
}

pub fn default_folder() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("Backups"))
}

/// The chosen Back up interval, as an index into [`EVERY_CHOICES`].
pub fn every_choice() -> usize {
    (settings::number(EVERY_KEY, DEFAULT_EVERY as f64) as usize).min(EVERY_CHOICES.len() - 1)
}

fn interval(choice: usize) -> Option<Duration> {
    match choice {
        1 => Some(Duration::from_secs(24 * 3600)),
        2 => Some(Duration::from_secs(7 * 24 * 3600)),
        _ => None,
    }
}

/// A value in settings.json that is a sign-in and stays out of backups.
fn is_secret(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    ["token", "secret", "password", "passwd"]
        .iter()
        .any(|w| k.contains(w))
        || k == "acoustid_key"
        || k == "api_key"
        || k == "remote_devices"
}

/// `v` without its sign-ins, at any depth.
fn without_secrets(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !is_secret(k))
                .map(|(k, v)| (k.clone(), without_secrets(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// `restored` with the sign-ins of `current` put back where `restored`
/// has none, at any depth.
fn keep_secrets(restored: &mut Value, current: &Value) {
    let (Value::Object(r), Value::Object(c)) = (restored, current) else {
        return;
    };
    for (k, v) in c {
        if is_secret(k) {
            r.entry(k.clone()).or_insert_with(|| v.clone());
        } else if let Some(rv) = r.get_mut(k) {
            keep_secrets(rv, v);
        }
    }
}

/// What a backup holds, for the notes in Settings.
#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub collection: bool,
    pub settings: bool,
    pub mappings: usize,
}

impl Summary {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.collection {
            parts.push("the collection".to_string());
        }
        if self.settings {
            parts.push("settings".to_string());
        }
        match self.mappings {
            0 => {}
            1 => parts.push("1 controller mapping".into()),
            n => parts.push(format!("{n} controller mappings")),
        }
        match parts.len() {
            0 => "nothing".into(),
            1 => parts.remove(0),
            _ => {
                let last = parts.pop().unwrap_or_default();
                format!("{} and {last}", parts.join(", "))
            }
        }
    }
}

/// The files under `dir`, as paths relative to it with `/` between parts.
fn files_under(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack = vec![(String::new(), dir.to_path_buf())];
    while let Some((prefix, d)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push((rel, e.path())),
                Ok(t) if t.is_file() => out.push((rel, e.path())),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Writes a backup of the config folder `config` to `to`.
pub fn write(config: &Path, to: &Path) -> Result<Summary, String> {
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let mut summary = Summary::default();
    // The library snapshot is made first, next to the backup, so a full
    // disk shows before anything is half written.
    let library = config.join(LIBRARY);
    let snapshot = to.with_extension("sqlite.partial");
    let _ = std::fs::remove_file(&snapshot);
    if library.is_file() {
        if let Err(e) = djsugar_library::snapshot(&library, &snapshot) {
            // A damaged collection (the very reason to restore) still goes
            // in, as it is.
            let _ = std::fs::remove_file(&snapshot);
            eprintln!("backup: can't snapshot the collection ({e}), copying it as it is");
            std::fs::copy(&library, &snapshot)
                .map_err(|e| format!("Couldn't copy the collection: {e}"))?;
        }
    }
    let result = (|| -> Result<(), String> {
        let file = std::fs::File::create(to).map_err(|e| err(&e))?;
        let mut zip = zip::ZipWriter::new(file);
        let deflate = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let add = |zip: &mut zip::ZipWriter<std::fs::File>, name: &str, path: &Path| {
            let mut f =
                std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
            zip.start_file(name, deflate).map_err(|e| err(&e))?;
            std::io::copy(&mut f, zip).map_err(|e| err(&e))?;
            Ok::<_, String>(())
        };
        if snapshot.is_file() {
            add(&mut zip, LIBRARY, &snapshot)?;
            summary.collection = true;
        }
        let saved = settings::load_from(&config.join(SETTINGS));
        if saved.as_object().is_some_and(|m| !m.is_empty()) {
            zip.start_file(SETTINGS, deflate).map_err(|e| err(&e))?;
            zip.write_all(
                serde_json::to_string_pretty(&without_secrets(&saved))
                    .unwrap_or_default()
                    .as_bytes(),
            )
            .map_err(|e| err(&e))?;
            summary.settings = true;
        }
        let plugins = config.join(AUDIO_PLUGINS);
        if plugins.is_file() {
            add(&mut zip, AUDIO_PLUGINS, &plugins)?;
        }
        for (rel, path) in files_under(&config.join(MAPPINGS)) {
            add(&mut zip, &format!("{MAPPINGS}/{rel}"), &path)?;
            summary.mappings += 1;
        }
        zip.start_file(MANIFEST, deflate).map_err(|e| err(&e))?;
        let manifest = json!({
            "app": "DJ Sugar",
            "version": env!("CARGO_PKG_VERSION"),
            "created": chrono::Local::now().to_rfc3339(),
            "collection": summary.collection,
            "settings": summary.settings,
            "mappings": summary.mappings,
        });
        zip.write_all(
            serde_json::to_string_pretty(&manifest)
                .unwrap_or_default()
                .as_bytes(),
        )
        .map_err(|e| err(&e))?;
        zip.finish().map_err(|e| err(&e))?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&snapshot);
    if result.is_err() {
        let _ = std::fs::remove_file(to);
    }
    result.map(|()| summary)
}

/// A file name in `dir` that isn't taken, from `stem`.
fn free_name(dir: &Path, stem: &str) -> PathBuf {
    let mut to = dir.join(format!("{stem}.{EXTENSION}"));
    let mut n = 2;
    while to.exists() {
        to = dir.join(format!("{stem} {n}.{EXTENSION}"));
        n += 1;
    }
    to
}

/// Backs up into `dir` under a dated name starting with `prefix`, through
/// a hidden partial file so a half-written backup never shows.
fn back_up_into(config: &Path, dir: &Path, prefix: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
    let to = free_name(dir, &format!("{prefix}{stamp}"));
    let partial = dir.join(format!(
        ".{}.partial",
        to.file_name().unwrap_or_default().to_string_lossy()
    ));
    write(config, &partial)?;
    std::fs::rename(&partial, &to).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        e.to_string()
    })?;
    Ok(to)
}

/// The automatic backups in `dir`, newest first.
fn automatic(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
    let mut out: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(AUTO_PREFIX) && name.ends_with(&format!(".{EXTENSION}"))
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    out.sort_by(|a, b| b.cmp(a));
    out
}

/// Removes all but the newest `keep` automatic backups in `dir`.
fn prune(dir: &Path, keep: usize) {
    for (_, path) in automatic(dir).into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

/// When the newest automatic backup in `dir` was made.
pub fn last_backup(dir: &Path) -> Option<SystemTime> {
    automatic(dir).first().map(|(t, _)| *t)
}

/// Whether a backup is due: none yet, or the newest is older than `every`.
fn due(last: Option<SystemTime>, every: Duration, now: SystemTime) -> bool {
    last.is_none_or(|t| now.duration_since(t).is_ok_and(|age| age >= every))
}

/// Backs up now into the backup folder; returns the file.
pub fn back_up_now() -> Result<PathBuf, String> {
    let config = settings::config_dir().ok_or("There is no settings folder.")?;
    let dir = folder().ok_or("There is no backup folder.")?;
    let to = back_up_into(&config, &dir, AUTO_PREFIX)?;
    prune(&dir, KEEP);
    Ok(to)
}

/// Starts a backup on its own thread if one is due; failures go to the
/// log and come back next time.
pub fn run_if_due() {
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(every) = interval(every_choice()) else {
        return;
    };
    let Some(dir) = folder() else { return };
    if !due(last_backup(&dir), every, SystemTime::now()) {
        return;
    }
    if RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = back_up_now() {
            eprintln!("backup failed: {e}");
        }
        RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// The part of a backup's entry name to unpack to, or `None` for anything
/// that isn't ours: absolute paths, `..`, hidden files.
fn wanted(name: &str) -> Option<PathBuf> {
    if [SETTINGS, LIBRARY, AUDIO_PLUGINS, MANIFEST].contains(&name) {
        return Some(PathBuf::from(name));
    }
    let rest = name.strip_prefix(&format!("{MAPPINGS}/"))?;
    let mut out = PathBuf::from(MAPPINGS);
    for part in rest.split('/') {
        if part.is_empty()
            || part.starts_with('.')
            || part.contains('\\')
            || part.contains(':')
            || Path::new(part).components().count() != 1
        {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

/// Checks the backup at `file` and unpacks it to be put in place at the
/// next start. Replaces a restore already waiting.
pub fn stage_restore(config: &Path, file: &Path) -> Result<Summary, String> {
    let bad = |what: &str| format!("This isn't an DJ Sugar backup: {what}");
    let f = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| bad(&e.to_string()))?;
    {
        let mut m = archive
            .by_name(MANIFEST)
            .map_err(|_| bad("it has no backup.json"))?;
        let mut text = String::new();
        (&mut m)
            .take(1 << 20)
            .read_to_string(&mut text)
            .map_err(|e| bad(&e.to_string()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| bad(&e.to_string()))?;
        if v["app"] != "DJ Sugar" {
            return Err(bad("it was made by another app"));
        }
    }
    let work = config.join(format!(".{PENDING}.partial"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut summary = Summary::default();
    let unpacked = (|| -> Result<(), String> {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| bad(&e.to_string()))?;
            if !entry.is_file() {
                continue;
            }
            let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
            let Some(rel) = wanted(&name) else {
                continue;
            };
            if entry.size() > MAX_FILE {
                return Err(bad("a file in it is too big"));
            }
            let out = work.join(&rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut (&mut entry).take(MAX_FILE), &mut w).map_err(|e| e.to_string())?;
            if rel.starts_with(MAPPINGS) {
                summary.mappings += 1;
            }
        }
        let library = work.join(LIBRARY);
        if library.is_file() {
            let mut head = [0u8; 16];
            let ok = std::fs::File::open(&library)
                .and_then(|mut f| f.read_exact(&mut head))
                .is_ok()
                && &head == b"SQLite format 3\0";
            if !ok {
                return Err(bad("its collection is damaged"));
            }
            summary.collection = true;
        }
        let s = work.join(SETTINGS);
        if s.is_file() {
            if !settings::load_from(&s).is_object() {
                return Err(bad("its settings are damaged"));
            }
            summary.settings = true;
        }
        Ok(())
    })();
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&work);
        return Err(e);
    }
    if summary == Summary::default() {
        let _ = std::fs::remove_dir_all(&work);
        return Err(bad("it is empty"));
    }
    let pending = config.join(PENDING);
    let _ = std::fs::remove_dir_all(&pending);
    std::fs::rename(&work, &pending).map_err(|e| e.to_string())?;
    Ok(summary)
}

/// Whether a restore is waiting for the next start.
pub fn restore_pending(config: &Path) -> bool {
    config.join(PENDING).is_dir()
}

/// Drops a restore that was waiting.
pub fn cancel_restore(config: &Path) {
    let _ = std::fs::remove_dir_all(config.join(PENDING));
}

/// Puts a waiting restore in place; call before anything reads settings or
/// opens the library. How things were is saved first as a "Before restore"
/// backup in `backups`. Returns a line for the DJ, or `None` if nothing was
/// waiting.
pub fn apply_pending(config: &Path, backups: Option<&Path>) -> Option<String> {
    let pending = config.join(PENDING);
    if !pending.is_dir() {
        return None;
    }
    let before = backups.map(|dir| back_up_into(config, dir, "Before restore "));
    if let Some(Err(e)) = &before {
        // Nothing is replaced without a way back.
        return Some(format!(
            "The backup wasn't restored: couldn't save how things are now first ({e})."
        ));
    }
    let result = (|| -> std::io::Result<()> {
        let restored = pending.join(SETTINGS);
        if restored.is_file() {
            let mut v = settings::load_from(&restored);
            keep_secrets(&mut v, &settings::load_from(&config.join(SETTINGS)));
            std::fs::write(config.join(SETTINGS), v.to_string())?;
        }
        let library = pending.join(LIBRARY);
        if library.is_file() {
            let to = config.join(LIBRARY);
            for side in ["-wal", "-shm"] {
                let mut p = to.clone().into_os_string();
                p.push(side);
                let _ = std::fs::remove_file(PathBuf::from(p));
            }
            let _ = std::fs::remove_file(&to);
            std::fs::rename(&library, &to)?;
        }
        let plugins = pending.join(AUDIO_PLUGINS);
        if plugins.is_file() {
            let _ = std::fs::remove_file(config.join(AUDIO_PLUGINS));
            std::fs::rename(&plugins, config.join(AUDIO_PLUGINS))?;
        }
        let mappings = pending.join(MAPPINGS);
        if mappings.is_dir() {
            let _ = std::fs::remove_dir_all(config.join(MAPPINGS));
            std::fs::rename(&mappings, config.join(MAPPINGS))?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&pending);
    Some(match result {
        Ok(()) => {
            "Restored the backup. How things were before is saved in the backup folder.".to_string()
        }
        Err(e) => format!("Part of the backup couldn't be restored: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "djsugar-backup-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn config_with_everything(dir: &Path) {
        std::fs::write(
            dir.join(SETTINGS),
            json!({
                "camelot_keys": false,
                "discogs_token": "t0k",
                "plugins": { "x": { "token": "abc", "folder": "/m" } },
            })
            .to_string(),
        )
        .unwrap();
        let db = djsugar_library::Library::open(&dir.join(LIBRARY)).unwrap();
        drop(db);
        std::fs::write(dir.join(AUDIO_PLUGINS), "{}").unwrap();
        std::fs::create_dir_all(dir.join(MAPPINGS).join("sub")).unwrap();
        std::fs::write(dir.join(MAPPINGS).join("mine.midi.xml"), "<x/>").unwrap();
        std::fs::write(dir.join(MAPPINGS).join("sub").join("b.js"), "1").unwrap();
        std::fs::write(dir.join("spotify.json"), "{\"refresh_token\":\"s\"}").unwrap();
    }

    #[test]
    fn secrets_stay_out_and_come_back() {
        let v =
            json!({ "a": 1, "discogs_token": "x", "plugins": { "p": { "token": "y", "b": 2 } } });
        let clean = without_secrets(&v);
        assert_eq!(clean, json!({ "a": 1, "plugins": { "p": { "b": 2 } } }));
        let mut restored = json!({ "a": 3, "plugins": { "p": { "b": 4 } } });
        keep_secrets(&mut restored, &v);
        assert_eq!(
            restored,
            json!({ "a": 3, "discogs_token": "x", "plugins": { "p": { "token": "y", "b": 4 } } })
        );
        assert!(!is_secret("camelot_keys"));
        assert!(!is_secret("key_notation"));
    }

    #[test]
    fn backs_up_and_restores_at_next_start() {
        let config = temp("config");
        let out = temp("out");
        config_with_everything(&config);
        let file = out.join("b.djsugarbackup");
        let s = write(&config, &file).unwrap();
        assert_eq!(
            s,
            Summary {
                collection: true,
                settings: true,
                mappings: 2
            }
        );
        let names: Vec<String> = zip::ZipArchive::new(std::fs::File::open(&file).unwrap())
            .unwrap()
            .file_names()
            .map(|n| n.unwrap().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.contains("spotify")));
        assert!(names.contains(&"controller-mappings/sub/b.js".to_string()));

        // Things change, then the backup is restored.
        std::fs::write(
            config.join(SETTINGS),
            json!({ "camelot_keys": true, "discogs_token": "new" }).to_string(),
        )
        .unwrap();
        std::fs::remove_dir_all(config.join(MAPPINGS)).unwrap();
        std::fs::write(config.join(LIBRARY), "not a db").unwrap();
        assert_eq!(stage_restore(&config, &file).unwrap(), s);
        assert!(restore_pending(&config));
        let backups = out.join("Backups");
        let note = apply_pending(&config, Some(&backups)).unwrap();
        assert!(note.starts_with("Restored"), "{note}");
        assert!(!restore_pending(&config));
        let v = settings::load_from(&config.join(SETTINGS));
        assert_eq!(v["camelot_keys"], json!(false));
        // The current sign-in is kept.
        assert_eq!(v["discogs_token"], json!("new"));
        assert!(config.join(MAPPINGS).join("sub").join("b.js").is_file());
        djsugar_library::Library::open(&config.join(LIBRARY)).unwrap();
        // How things were is saved, and isn't pruned as an automatic one.
        let saved: Vec<_> = std::fs::read_dir(&backups).unwrap().flatten().collect();
        assert_eq!(saved.len(), 1);
        assert!(saved[0]
            .file_name()
            .to_string_lossy()
            .starts_with("Before restore "));
        assert!(automatic(&backups).is_empty());
        assert_eq!(apply_pending(&config, Some(&backups)), None);
    }

    #[test]
    fn refuses_other_zips_and_sneaky_paths() {
        let config = temp("refuse");
        let bad = config.join("bad.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
            z.start_file("pack.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"{}").unwrap();
            z.finish().unwrap();
        }
        assert!(stage_restore(&config, &bad).is_err());
        assert!(!restore_pending(&config));
        assert_eq!(wanted("../settings.json"), None);
        assert_eq!(wanted("controller-mappings/../../x"), None);
        assert_eq!(wanted("controller-mappings/.hidden"), None);
        assert_eq!(wanted("/etc/passwd"), None);
        assert_eq!(
            wanted("controller-mappings/a/b.xml"),
            Some(PathBuf::from("controller-mappings/a/b.xml"))
        );
    }

    #[test]
    fn keeps_the_newest_and_knows_when_due() {
        let dir = temp("prune");
        for i in 0..4 {
            let p = dir.join(format!("{AUTO_PREFIX}{i}.{EXTENSION}"));
            std::fs::write(&p, "").unwrap();
            let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + i);
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        std::fs::write(dir.join("Before restore x.djsugarbackup"), "").unwrap();
        prune(&dir, 2);
        let left: Vec<String> = automatic(&dir)
            .into_iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            left,
            vec![
                format!("{AUTO_PREFIX}3.{EXTENSION}"),
                format!("{AUTO_PREFIX}2.{EXTENSION}")
            ]
        );
        assert!(dir.join("Before restore x.djsugarbackup").exists());
        let day = Duration::from_secs(86_400);
        let now = SystemTime::now();
        assert!(due(None, day, now));
        assert!(!due(Some(now - Duration::from_secs(60)), day, now));
        assert!(due(Some(now - day), day, now));
    }
}
