//! DJ Sugar's mod system.
//!
//! Everything the app ships (its look, its layouts, pad modes, controller
//! mappings, importers and exporters) is a mod, loaded exactly like one a
//! DJ installs: there is no back door for built-in parts. A mod can
//! therefore replace or extend anything, including what ships.
//!
//! A mod is a folder with a `mod.toml`:
//!
//! ```toml
//! id = "sugar.look"
//! name = "Sugar"
//! version = "1.2.0"
//! requires = ["sugar.core >= 1.0"]   # loaded after these, which must exist
//! after = ["someone.extra"]          # loaded after these if present
//!
//! [contributes.theme.color]
//! background = "#0b0a0f"
//! ```
//!
//! Everything under `contributes` is flattened into *slots* named by their
//! path (`theme.color.background`). For each slot the last mod in load order
//! wins; [`Registry::provenance`] tells which mod set it and which it
//! overrode, so the app can show it. Load order: dependencies first, then
//! `after`, then by id, so it is the same on every machine.
//!
//! Mods are found in folders given in order (the app's own first, then the
//! DJ's): a mod in a later folder with the same id replaces the earlier one,
//! which is how a DJ edits a shipped mod without touching the app.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "mod.toml";

/// A dotted version, compared part by part (`1.10.0` > `1.9`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Version(pub Vec<u32>);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let parts: Option<Vec<u32>> = s.trim().split('.').map(|p| p.parse().ok()).collect();
        let mut parts = parts.filter(|p| !p.is_empty() && p.len() <= 4)?;
        while parts.len() > 1 && parts.last() == Some(&0) {
            parts.pop();
        }
        Some(Version(parts))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&s.join("."))
    }
}

/// `other.id`, `other.id >= 1.2`, `other.id = 2`, `other.id < 3`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub id: String,
    pub op: Op,
    pub version: Version,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Any,
    AtLeast,
    Exactly,
    Below,
}

impl Requirement {
    pub fn parse(s: &str) -> Option<Requirement> {
        let s = s.trim();
        let split = s.find(['>', '<', '=']);
        let (id, rest) = match split {
            Some(i) => (s[..i].trim(), s[i..].trim()),
            None => (s, ""),
        };
        if !valid_id(id) {
            return None;
        }
        let (op, v) = if let Some(v) = rest.strip_prefix(">=") {
            (Op::AtLeast, v)
        } else if let Some(v) = rest.strip_prefix('<') {
            (Op::Below, v)
        } else if let Some(v) = rest.strip_prefix('=') {
            (Op::Exactly, v)
        } else if rest.is_empty() {
            (Op::Any, "0")
        } else {
            return None;
        };
        Some(Requirement {
            id: id.into(),
            op,
            version: Version::parse(v)?,
        })
    }

    pub fn accepts(&self, v: &Version) -> bool {
        match self.op {
            Op::Any => true,
            Op::AtLeast => *v >= self.version,
            Op::Exactly => *v == self.version,
            Op::Below => *v < self.version,
        }
    }
}

/// Lower-case letters, digits, `-` and `_`, in dot-separated words:
/// `sugar.look`, `someone.my-theme`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.split('.').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    id: String,
    name: Option<String>,
    version: String,
    description: Option<String>,
    author: Option<String>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    contributes: toml::Table,
}

/// A slot's value: what a mod put there.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    Flag(bool),
    List(Vec<Value>),
}

impl Value {
    fn from_toml(v: &toml::Value) -> Option<Value> {
        Some(match v {
            toml::Value::String(s) => Value::Text(s.clone()),
            toml::Value::Integer(i) => Value::Number(*i as f64),
            toml::Value::Float(f) => Value::Number(*f),
            toml::Value::Boolean(b) => Value::Flag(*b),
            toml::Value::Array(a) => {
                Value::List(a.iter().map(Value::from_toml).collect::<Option<_>>()?)
            }
            toml::Value::Datetime(d) => Value::Text(d.to_string()),
            toml::Value::Table(_) => return None,
        })
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn flag(&self) -> Option<bool> {
        match self {
            Value::Flag(b) => Some(*b),
            _ => None,
        }
    }
}

/// One mod, as read from its folder.
#[derive(Clone, Debug)]
pub struct Mod {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub description: String,
    pub author: String,
    pub requires: Vec<Requirement>,
    pub after: Vec<String>,
    /// Slot path to value, in the order written.
    pub slots: Vec<(String, Value)>,
    /// The mod's folder: files it refers to (images, scripts, layouts) are
    /// relative to it.
    pub dir: PathBuf,
    /// Which of the search folders it came from (0 = the app's own).
    pub origin: usize,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Read(String),
    Syntax(String),
    BadId(String),
    BadVersion(String),
    BadRequirement(String),
    BadSlot(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(e) => write!(f, "can't read {MANIFEST}: {e}"),
            Error::Syntax(e) => write!(f, "{MANIFEST} isn't valid: {e}"),
            Error::BadId(id) => write!(
                f,
                "\"{id}\" isn't a usable id (lower-case words joined by dots)"
            ),
            Error::BadVersion(v) => write!(f, "\"{v}\" isn't a version (like 1.2.0)"),
            Error::BadRequirement(r) => {
                write!(f, "\"{r}\" isn't a requirement (like other.mod >= 1.2)")
            }
            Error::BadSlot(s) => write!(f, "{s}: a table can't be a value here"),
        }
    }
}

fn flatten(prefix: &str, table: &toml::Table, out: &mut Vec<(String, Value)>) -> Result<(), Error> {
    for (k, v) in table {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::Table(t) => flatten(&path, t, out)?,
            v => out.push((
                path.clone(),
                Value::from_toml(v).ok_or(Error::BadSlot(path))?,
            )),
        }
    }
    Ok(())
}

impl Mod {
    /// Reads a mod from the text of its manifest.
    pub fn parse(text: &str, dir: &Path, origin: usize) -> Result<Mod, Error> {
        let raw: RawManifest =
            toml::from_str(text).map_err(|e| Error::Syntax(e.message().to_string()))?;
        if !valid_id(&raw.id) {
            return Err(Error::BadId(raw.id));
        }
        let version = Version::parse(&raw.version).ok_or(Error::BadVersion(raw.version))?;
        let requires = raw
            .requires
            .iter()
            .map(|r| Requirement::parse(r).ok_or_else(|| Error::BadRequirement(r.clone())))
            .collect::<Result<_, _>>()?;
        if let Some(bad) = raw.after.iter().find(|a| !valid_id(a)) {
            return Err(Error::BadId(bad.clone()));
        }
        let mut slots = Vec::new();
        flatten("", &raw.contributes, &mut slots)?;
        Ok(Mod {
            name: raw.name.unwrap_or_else(|| raw.id.clone()),
            id: raw.id,
            version,
            description: raw.description.unwrap_or_default(),
            author: raw.author.unwrap_or_default(),
            requires,
            after: raw.after,
            slots,
            dir: dir.to_path_buf(),
            origin,
        })
    }

    pub fn load(dir: &Path, origin: usize) -> Result<Mod, Error> {
        let text =
            std::fs::read_to_string(dir.join(MANIFEST)).map_err(|e| Error::Read(e.to_string()))?;
        Mod::parse(&text, dir, origin)
    }
}

/// A mod that couldn't be used, and why.
#[derive(Debug, PartialEq)]
pub struct Problem {
    pub id: String,
    pub dir: PathBuf,
    pub reason: String,
}

/// Every mod found in `folders` (the app's own first): later folders
/// replace earlier mods of the same id. Folders that can't be read are
/// skipped; mods that can't be read are reported.
pub fn discover(folders: &[PathBuf]) -> (Vec<Mod>, Vec<Problem>) {
    let mut found: BTreeMap<String, Mod> = BTreeMap::new();
    let mut problems = Vec::new();
    for (origin, folder) in folders.iter().enumerate() {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join(MANIFEST).is_file())
            .collect();
        dirs.sort();
        for dir in dirs {
            match Mod::load(&dir, origin) {
                Ok(m) => {
                    found.insert(m.id.clone(), m);
                }
                Err(e) => problems.push(Problem {
                    id: dir
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    dir,
                    reason: e.to_string(),
                }),
            }
        }
    }
    (found.into_values().collect(), problems)
}

/// Who set a slot, and whom they overrode (earliest first).
#[derive(Clone, Debug, PartialEq)]
pub struct Provenance {
    pub by: String,
    pub overrode: Vec<String>,
}

/// The mods in use, in load order, and every slot's winning value.
#[derive(Default)]
pub struct Registry {
    order: Vec<Mod>,
    slots: HashMap<String, (Value, Provenance)>,
    pub problems: Vec<Problem>,
}

impl Registry {
    /// Orders `mods` and merges their slots. Mods turned off in `disabled`,
    /// missing a requirement, or in a cycle of requirements are left out and
    /// reported, as is everything that needs them.
    pub fn build(mods: Vec<Mod>, disabled: &BTreeSet<String>) -> Registry {
        let mut problems = Vec::new();
        let mut pool: BTreeMap<String, Mod> = BTreeMap::new();
        for m in mods {
            if disabled.contains(&m.id) {
                continue;
            }
            pool.insert(m.id.clone(), m);
        }
        // Drop mods whose requirements aren't met, until none change.
        loop {
            let mut bad: Vec<(String, String)> = Vec::new();
            for m in pool.values() {
                for r in &m.requires {
                    let reason = match pool.get(&r.id) {
                        None if disabled.contains(&r.id) => {
                            Some(format!("needs {}, which is turned off", r.id))
                        }
                        None => Some(format!(
                            "needs {}, which isn't installed or can't load",
                            r.id
                        )),
                        Some(o) if !r.accepts(&o.version) => Some(format!(
                            "needs a different version of {} than {}",
                            r.id, o.version
                        )),
                        _ => None,
                    };
                    if let Some(reason) = reason {
                        bad.push((m.id.clone(), reason));
                        break;
                    }
                }
            }
            if bad.is_empty() {
                break;
            }
            for (id, reason) in bad {
                if let Some(m) = pool.remove(&id) {
                    problems.push(Problem {
                        id,
                        dir: m.dir,
                        reason,
                    });
                }
            }
        }
        // Kahn's algorithm, always taking the smallest ready id, so the
        // order is the same everywhere.
        let mut before: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for m in pool.values() {
            let deps = before.entry(&m.id).or_default();
            for d in m
                .requires
                .iter()
                .map(|r| r.id.as_str())
                .chain(m.after.iter().map(String::as_str))
            {
                if pool.contains_key(d) && d != m.id {
                    deps.insert(d);
                }
            }
        }
        let mut done: Vec<String> = Vec::new();
        let mut placed: BTreeSet<&str> = BTreeSet::new();
        loop {
            let ready = before
                .iter()
                .find(|(id, deps)| {
                    !placed.contains(**id) && deps.iter().all(|d| placed.contains(d))
                })
                .map(|(id, _)| *id);
            let Some(id) = ready else { break };
            placed.insert(id);
            done.push(id.to_string());
        }
        let cycle: Vec<String> = pool
            .keys()
            .filter(|id| !placed.contains(id.as_str()))
            .cloned()
            .collect();
        for id in cycle {
            let m = pool.remove(&id).expect("in the pool");
            problems.push(Problem {
                id,
                dir: m.dir,
                reason: "needs mods that in turn need it".into(),
            });
        }
        let order: Vec<Mod> = done.iter().filter_map(|id| pool.remove(id)).collect();
        let mut slots: HashMap<String, (Value, Provenance)> = HashMap::new();
        for m in &order {
            for (path, value) in &m.slots {
                match slots.get_mut(path) {
                    Some((v, p)) => {
                        *v = value.clone();
                        let previous = std::mem::replace(&mut p.by, m.id.clone());
                        if previous != m.id {
                            p.overrode.push(previous);
                        }
                    }
                    None => {
                        slots.insert(
                            path.clone(),
                            (
                                value.clone(),
                                Provenance {
                                    by: m.id.clone(),
                                    overrode: Vec::new(),
                                },
                            ),
                        );
                    }
                }
            }
        }
        Registry {
            order,
            slots,
            problems,
        }
    }

    /// Mods in use, in load order.
    pub fn mods(&self) -> &[Mod] {
        &self.order
    }

    pub fn get(&self, slot: &str) -> Option<&Value> {
        self.slots.get(slot).map(|(v, _)| v)
    }

    pub fn provenance(&self, slot: &str) -> Option<&Provenance> {
        self.slots.get(slot).map(|(_, p)| p)
    }

    /// The mod that set `slot`, for finding files it names.
    pub fn owner(&self, slot: &str) -> Option<&Mod> {
        let by = &self.provenance(slot)?.by;
        self.order.iter().find(|m| &m.id == by)
    }

    /// Every slot under `prefix.` (or exactly `prefix`), sorted by path.
    pub fn under<'a>(&'a self, prefix: &str) -> Vec<(&'a str, &'a Value)> {
        let dotted = format!("{prefix}.");
        let mut v: Vec<(&str, &Value)> = self
            .slots
            .iter()
            .filter(|(k, _)| k.as_str() == prefix || k.starts_with(&dotted))
            .map(|(k, (v, _))| (k.as_str(), v))
            .collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v
    }
}

/// A stamp of every manifest in `folders` (path, size, change time): when
/// it differs, mods were added, removed or edited and the registry should be
/// built again. Cheap enough to check every second.
pub fn fingerprint(folders: &[PathBuf]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        let mut found: Vec<(PathBuf, u64, Option<std::time::SystemTime>)> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path().join(MANIFEST);
                let meta = std::fs::metadata(&p).ok()?;
                Some((p, meta.len(), meta.modified().ok()))
            })
            .collect();
        found.sort();
        found.hash(&mut h);
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(text: &str) -> Mod {
        Mod::parse(text, Path::new("/m"), 0).unwrap()
    }

    #[test]
    fn versions_compare_by_part() {
        let v = |s| Version::parse(s).unwrap();
        assert!(v("1.10") > v("1.9.9"));
        assert_eq!(v("2.0.0"), v("2"));
        assert_eq!(Version::parse("1.x"), None);
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
    }

    #[test]
    fn requirements_read_and_match() {
        let r = Requirement::parse("sugar.core >= 1.2").unwrap();
        assert!(r.accepts(&Version::parse("1.2").unwrap()));
        assert!(!r.accepts(&Version::parse("1.1.9").unwrap()));
        assert!(Requirement::parse("a.b<2")
            .unwrap()
            .accepts(&Version::parse("1.9").unwrap()));
        assert_eq!(Requirement::parse("a.b").unwrap().op, Op::Any);
        assert_eq!(Requirement::parse("Bad Id"), None);
        assert_eq!(Requirement::parse("a.b ~ 1"), None);
    }

    #[test]
    fn a_manifest_flattens_into_slots() {
        let x = m(r##"
            id = "sugar.look"
            version = "1.0"
            [contributes.theme.color]
            background = "#0b0a0f"
            [contributes.theme]
            radius = 11
            rounded = true
        "##);
        assert_eq!(x.name, "sugar.look");
        let r = Registry::build(vec![x], &BTreeSet::new());
        assert_eq!(
            r.get("theme.color.background").unwrap().text(),
            Some("#0b0a0f")
        );
        assert_eq!(r.get("theme.radius").unwrap().number(), Some(11.0));
        assert_eq!(r.get("theme.rounded").unwrap().flag(), Some(true));
        assert_eq!(r.under("theme.color").len(), 1);
    }

    #[test]
    fn bad_manifests_say_why() {
        let e = |t| Mod::parse(t, Path::new("/m"), 0).unwrap_err();
        assert_eq!(
            e("id = \"Nope\"\nversion = \"1\""),
            Error::BadId("Nope".into())
        );
        assert_eq!(
            e("id = \"a\"\nversion = \"one\""),
            Error::BadVersion("one".into())
        );
        assert!(matches!(
            e("id = \"a\"\nversion = \"1\"\ncolour = 1"),
            Error::Syntax(_)
        ));
    }

    #[test]
    fn later_mods_win_and_say_whom_they_overrode() {
        let base =
            m("id = \"sugar.look\"\nversion = \"1\"\n[contributes]\n\"theme.accent\" = \"pink\"");
        let mine = m("id = \"me.look\"\nversion = \"1\"\nrequires = [\"sugar.look\"]\n[contributes]\n\"theme.accent\" = \"green\"");
        let r = Registry::build(vec![mine, base], &BTreeSet::new());
        assert_eq!(r.get("theme.accent").unwrap().text(), Some("green"));
        let p = r.provenance("theme.accent").unwrap();
        assert_eq!(
            (p.by.as_str(), p.overrode.clone()),
            ("me.look", vec!["sugar.look".to_string()])
        );
        assert_eq!(r.owner("theme.accent").unwrap().id, "me.look");
    }

    #[test]
    fn order_follows_requirements_then_after_then_id() {
        let a = m("id = \"z.first\"\nversion = \"1\"");
        let b = m("id = \"a.second\"\nversion = \"1\"\nrequires = [\"z.first\"]");
        let c = m("id = \"b.third\"\nversion = \"1\"\nafter = [\"a.second\", \"not.installed\"]");
        let d = m("id = \"a.alone\"\nversion = \"1\"");
        let r = Registry::build(vec![c, b, a, d], &BTreeSet::new());
        let ids: Vec<&str> = r.mods().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a.alone", "z.first", "a.second", "b.third"]);
        assert!(r.problems.is_empty());
    }

    #[test]
    fn unmet_requirements_and_cycles_leave_mods_out() {
        let old = m("id = \"core\"\nversion = \"1.0\"");
        let needs_new = m("id = \"x\"\nversion = \"1\"\nrequires = [\"core >= 2\"]");
        let needs_x = m("id = \"y\"\nversion = \"1\"\nrequires = [\"y2\", \"x\"]");
        let y2 = m("id = \"y2\"\nversion = \"1\"");
        let c1 = m("id = \"c1\"\nversion = \"1\"\nrequires = [\"c2\"]");
        let c2 = m("id = \"c2\"\nversion = \"1\"\nrequires = [\"c1\"]");
        let off = m("id = \"off\"\nversion = \"1\"");
        let needs_off = m("id = \"n\"\nversion = \"1\"\nrequires = [\"off\"]");
        let r = Registry::build(
            vec![old, needs_new, needs_x, y2, c1, c2, off, needs_off],
            &BTreeSet::from(["off".to_string()]),
        );
        let ids: Vec<&str> = r.mods().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["core", "y2"]);
        let mut why: Vec<(String, String)> = r
            .problems
            .iter()
            .map(|p| (p.id.clone(), p.reason.clone()))
            .collect();
        why.sort();
        assert_eq!(why[0].0, "c1");
        assert!(why
            .iter()
            .any(|(id, r)| id == "n" && r.contains("turned off")));
        assert!(why
            .iter()
            .any(|(id, r)| id == "x" && r.contains("different version")));
        assert!(why.iter().any(|(id, r)| id == "y" && r.contains("x")));
    }

    #[test]
    fn a_djs_folder_replaces_a_shipped_mod_and_changes_are_noticed() {
        let root = std::env::temp_dir().join(format!("djsugar-mods-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (app, user) = (root.join("app"), root.join("user"));
        let write = |dir: &Path, text: &str| {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(MANIFEST), text).unwrap();
        };
        write(
            &app.join("look"),
            "id = \"sugar.look\"\nversion = \"1\"\n[contributes]\naccent = \"pink\"",
        );
        write(&app.join("broken"), "id = \"BROKEN\"\nversion = \"1\"");
        let folders = [app.clone(), user.clone()];
        let before = fingerprint(&folders);
        write(
            &user.join("look-edit"),
            "id = \"sugar.look\"\nversion = \"1.1\"\n[contributes]\naccent = \"gold\"",
        );
        assert_ne!(fingerprint(&folders), before);
        let (mods, problems) = discover(&folders);
        assert_eq!(problems.len(), 1);
        let r = Registry::build(mods, &BTreeSet::new());
        assert_eq!(r.get("accent").unwrap().text(), Some("gold"));
        assert_eq!(r.mods()[0].origin, 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
