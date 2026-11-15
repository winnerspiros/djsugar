//! Settings › Collection › Storage: backing up the collection and settings.

use super::*;

impl Prefs {
    /// Settings › Collection › Storage: how often the collection and
    /// settings are backed up, where to, Back up now and Restore.
    pub(super) fn backup_items(&self) -> Vec<PrefItem> {
        let dir = backup::folder();
        let last = dir
            .as_deref()
            .and_then(backup::last_backup)
            .map(|t| {
                let t: chrono::DateTime<chrono::Local> = t.into();
                let today = chrono::Local::now().date_naive();
                if t.date_naive() == today {
                    format!("Last backup today at {}", t.format("%H:%M"))
                } else {
                    format!("Last backup {}", t.format("%b %-d at %H:%M"))
                }
            })
            .unwrap_or_else(|| "No backups yet".into());
        let pending = settings::config_dir().is_some_and(|d| backup::restore_pending(&d));
        let now_note = if self.backup_running {
            "Backing up…".to_string()
        } else if !self.backup_note.is_empty() && !pending {
            self.backup_note.clone()
        } else {
            last
        };
        vec![
            item("backups", HEADING, "BACKUPS", ""),
            PrefItem {
                choices: strings(&backup::EVERY_CHOICES),
                chosen: backup::every_choice() as i32,
                ..item(
                    backup::EVERY_KEY,
                    MENU,
                    "Back up",
                    &format!(
                        "Your collection (cues, tags, crates, playlists) and settings. The newest \
                         {} backups are kept. Sign-ins stay out.",
                        backup::KEEP
                    ),
                )
            },
            folder_list(
                backup::FOLDER_KEY,
                "Backup folder",
                "Pick a cloud folder to keep backups off this computer.",
                &dir.into_iter().collect::<Vec<_>>(),
                true,
                false,
            ),
            PrefItem {
                action: if self.backup_running {
                    ""
                } else {
                    "Back Up Now"
                }
                .into(),
                ..item("backup_now", BUTTON, "Back up now", &now_note)
            },
            PrefItem {
                action: if pending { "Cancel" } else { "Restore…" }.into(),
                ..item(
                    "backup_restore",
                    BUTTON,
                    "Restore a backup",
                    &if pending && !self.backup_note.is_empty() {
                        self.backup_note.clone()
                    } else if pending {
                        "A backup is restored when DJ Sugar next starts.".to_string()
                    } else {
                        "Replaces the collection and settings. How things are now is backed up \
                         first."
                            .into()
                    },
                )
            },
        ]
    }

    pub(super) fn backup_pressed(&mut self, id: &str) -> bool {
        match id {
            "backup_now" if !self.backup_running => {
                self.backup_running = true;
                let tx = self.backup_tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(match backup::back_up_now() {
                        Ok(path) => format!(
                            "Saved {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        Err(e) => format!("Couldn't back up: {e}"),
                    });
                });
            }
            "backup_now" => {}
            "backup_restore" => {
                let config = settings::config_dir();
                if let Some(d) = config.as_deref().filter(|d| backup::restore_pending(d)) {
                    backup::cancel_restore(d);
                    self.backup_note.clear();
                } else {
                    choose(
                        "backup_restore",
                        "Choose an DJ Sugar backup (.djsugarbackup)",
                        false,
                        self.chosen_tx.clone(),
                    );
                    return true;
                }
            }
            _ => return false,
        }
        self.refresh();
        true
    }
}
