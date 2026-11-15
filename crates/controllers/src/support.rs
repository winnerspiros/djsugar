//! The controllers DJ Sugar looks after itself, beyond running Mixxx's
//! mappings: each has a picture in the Controller Editor and a guide page,
//! and is marked Beta until someone tries it on the hardware, then
//! Supported. The list is `support.json`, which the website reads too.

use serde_json::Value;

const LIST: &str = include_str!("../support.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Tried on the hardware, and its mapping settled.
    Supported,
    /// Works on paper; waiting for someone to try it on the hardware.
    Beta,
    /// No mapping yet.
    Planned,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Supported => "Supported",
            Status::Beta => "Beta",
            Status::Planned => "Coming soon",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Supported {
    pub name: String,
    /// The mapping file in the mappings folder; none while planned.
    pub file: Option<String>,
    pub status: Status,
    pub note: String,
}

/// Every controller on the list, in its order.
pub fn all() -> Vec<Supported> {
    let Ok(v) = serde_json::from_str::<Value>(LIST) else {
        return Vec::new();
    };
    v["controllers"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let status = match c["status"].as_str()? {
                        "supported" => Status::Supported,
                        "beta" => Status::Beta,
                        "planned" => Status::Planned,
                        _ => return None,
                    };
                    Some(Supported {
                        name: c["name"].as_str()?.into(),
                        file: c["file"].as_str().map(Into::into),
                        status,
                        note: c["note"].as_str().unwrap_or("").into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// How far along the mapping in `file` is, if it's on the list.
pub fn status(file: &str) -> Option<Status> {
    all()
        .into_iter()
        .find(|c| c.file.as_deref() == Some(file))
        .map(|c| c.status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_parses_and_its_mappings_exist() {
        let list = all();
        assert_eq!(list.len(), LIST.matches("\"status\"").count());
        let dir = crate::default_mappings_dir().unwrap();
        for c in &list {
            match &c.file {
                Some(f) => assert!(dir.join(f).is_file(), "{f} is missing"),
                None => assert_eq!(c.status, Status::Planned, "{}", c.name),
            }
        }
        assert_eq!(status("Pioneer-DDJ-400.midi.xml"), Some(Status::Beta));
        assert_eq!(status("Akai MPD24.midi.xml"), None);
    }

    /// Every controller with a MIDI mapping has a picture in the editor.
    #[test]
    fn listed_mappings_have_pictures() {
        let pictured = crate::layout::Layout::files();
        for c in all() {
            // HID mappings are all script: no rows to place on a picture.
            if let Some(f) = c.file.filter(|f| !f.ends_with(".hid.xml")) {
                assert!(pictured.contains(&f), "{f} has no layout");
            }
        }
    }
}
