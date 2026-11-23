//! What a scan finds, and the scans that run no plugin code.

use std::path::Path;

/// VST3's category for audio processors.
pub(crate) const AUDIO_MODULE: &str = "Audio Module Class";

/// One plugin a scan found: in a `.clap` file, a `.vst3` bundle or the
/// Audio Unit registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub features: Vec<String>,
}

impl Descriptor {
    /// Effects are what DJ Sugar can use; instruments and note effects aren't.
    pub fn is_audio_effect(&self) -> bool {
        self.features.iter().any(|f| f == "audio-effect") && !self.features.iter().any(|f| f == "instrument")
    }

    /// Instruments play notes; DJ Sugar plays one as an input of its own.
    pub fn is_instrument(&self) -> bool {
        self.features.iter().any(|f| f == "instrument")
    }
}

/// VST3 sub-categories ("Fx|Delay", "Instrument|Synth") as feature words,
/// with CLAP's names for the two that matter, so one test works for both.
pub(crate) fn features(sub: &str) -> Vec<String> {
    let mut out: Vec<String> = sub.split('|').filter(|s| !s.is_empty()).map(String::from).collect();
    if out.iter().any(|s| s == "Instrument") {
        out.push("instrument".into());
    } else if out.iter().any(|s| s == "Fx") {
        out.push("audio-effect".into());
    }
    out
}

/// Reads `Contents/Resources/moduleinfo.json` (VST 3.7.5+), which lists a
/// bundle's classes without running any of its code. `None` when there is
/// no such file or it can't be read, so the caller loads the module instead.
pub fn scan_moduleinfo(bundle: &Path) -> Option<Vec<Descriptor>> {
    let text = std::fs::read_to_string(bundle.join("Contents/Resources/moduleinfo.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let classes = json.get("Classes")?.as_array()?;
    let field = |c: &serde_json::Value, k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
    Some(
        classes
            .iter()
            .filter(|c| field(c, "Category") == AUDIO_MODULE)
            .map(|c| {
                let sub: Vec<&str> = c
                    .get("Sub Categories")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
                    .unwrap_or_default();
                Descriptor {
                    id: field(c, "CID").to_uppercase(),
                    name: field(c, "Name"),
                    vendor: field(c, "Vendor"),
                    version: field(c, "Version"),
                    features: features(&sub.join("|")),
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_categories_become_features() {
        assert!(features("Fx|Delay").contains(&"audio-effect".to_string()));
        assert!(features("Instrument|Synth").contains(&"instrument".to_string()));
    }

    #[test]
    fn moduleinfo_lists_audio_modules_only() {
        let dir = std::env::temp_dir().join(format!("djsugar-moduleinfo-{}", std::process::id()));
        let res = dir.join("Test.vst3/Contents/Resources");
        std::fs::create_dir_all(&res).unwrap();
        std::fs::write(
            res.join("moduleinfo.json"),
            r#"{"Name": "Test", "Classes": [
                {"CID": "6e33225254224a00aa69301af318797d", "Category": "Audio Module Class",
                 "Name": "Test Delay", "Vendor": "Someone", "Version": "1.0.0",
                 "Sub Categories": ["Fx", "Delay"]},
                {"CID": "1BA8A477EE0A4A2D80F50D1413D2EAA0", "Category": "Component Controller Class",
                 "Name": "Test Delay"}]}"#,
        )
        .unwrap();
        let found = scan_moduleinfo(&dir.join("Test.vst3")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "6E33225254224A00AA69301AF318797D");
        assert_eq!(found[0].name, "Test Delay");
        assert!(found[0].is_audio_effect());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
