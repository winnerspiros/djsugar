//! rbox reads every `created_at` / `updated_at` in `master.db` with one
//! exact format (`2020-12-11 02:05:03.483 +00:00`) and fails the whole query
//! on any other, or on NULL ("Error deserializing field 'updated_at'").
//! Libraries that went through older rekordbox versions, migrations or other
//! tools can hold other spellings. When a library has any, it's opened from a
//! private copy whose timestamps are rewritten into rbox's format, and whose
//! NULLs in columns rbox can't read empty are filled (see [`super::nulls`]);
//! the DJ's own `master.db` is never written.

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};

/// The columns rbox parses as dates, in every table that has them.
const COLUMNS: [&str; 2] = ["created_at", "updated_at"];

/// rbox's own format.
const FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f %:z";

/// What rbox accepts as is: its format, optionally followed by rekordbox's
/// " (Time zone name)" note.
const GLOB: &str = "[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9] \
[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9] [+-][0-9][0-9]:[0-9][0-9]*";

/// rekordbox's SQLCipher passphrase, the same one rbox applies.
fn key() -> String {
    "513ge593d49928d46ggb9ggc9d8e:4254c85:f8e426eg8b92843b2gg547195:8"
        .bytes()
        .map(|b| (b - 1) as char)
        .collect()
}

fn connect(path: &Path, flags: OpenFlags) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(path, flags)?;
    c.pragma_update(None, "key", key())?;
    Ok(c)
}

/// `(table, column)` for every date column rbox reads.
fn date_columns(c: &Connection) -> rusqlite::Result<Vec<(String, &'static str)>> {
    let mut tables = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let tables: Vec<String> = tables
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for t in tables {
        let mut info = c.prepare(&format!("PRAGMA table_info(\"{t}\")"))?;
        let names: Vec<String> = info
            .query_map([], |r| r.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        for col in COLUMNS {
            if names.iter().any(|n| n == col) {
                out.push((t.clone(), col));
            }
        }
    }
    Ok(out)
}

fn odd_rows(table: &str, column: &str) -> String {
    format!(
        "SELECT rowid, \"{column}\" FROM \"{table}\" \
         WHERE \"{column}\" IS NULL OR typeof(\"{column}\") != 'text' \
         OR \"{column}\" NOT GLOB '{GLOB}'"
    )
}

/// Whether any date in the library is one rbox can't read, or a column rbox
/// reads as never empty holds a NULL.
pub fn needs_fixing(path: &Path) -> rusqlite::Result<bool> {
    let c = connect(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if super::nulls::any(&c)? {
        return Ok(true);
    }
    for (t, col) in date_columns(&c)? {
        let mut q = c.prepare(&format!("{} LIMIT 1", odd_rows(&t, col)))?;
        if q.exists([])? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Rewrites every date rbox can't read in the (copied) library at `path`,
/// and fills the NULLs rbox can't read. Returns how many values it changed.
pub fn fix(path: &Path) -> rusqlite::Result<usize> {
    let c = connect(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // The copy only has to be read back; rekordbox's triggers and keys
    // don't matter for it.
    c.pragma_update(None, "foreign_keys", false)?;
    c.pragma_update(None, "recursive_triggers", false)?;
    let mut changed = 0;
    let tx = c.unchecked_transaction()?;
    for (t, col) in date_columns(&tx)? {
        let rows: Vec<(i64, Value)> = {
            let mut q = tx.prepare(&odd_rows(&t, col))?;
            let rows = q
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };
        let mut update = tx.prepare(&format!(
            "UPDATE \"{t}\" SET \"{col}\" = ?1 WHERE rowid = ?2"
        ))?;
        for (rowid, v) in rows {
            changed += update.execute(rusqlite::params![normalise(&v), rowid])?;
        }
    }
    changed += super::nulls::fill(&tx)?;
    tx.commit()?;
    Ok(changed)
}

/// A rekordbox timestamp in rbox's format. Spellings it can't make sense
/// of (NULL, empty, garbage) become the Unix epoch: DJ Sugar doesn't use
/// these dates, rbox only needs them to parse.
pub fn normalise(v: &Value) -> String {
    let date = match v {
        Value::Text(s) => parse(s),
        Value::Integer(n) => from_number(*n as f64),
        Value::Real(n) => from_number(*n),
        _ => None,
    };
    date.unwrap_or(DateTime::UNIX_EPOCH)
        .format(FORMAT)
        .to_string()
}

fn from_number(n: f64) -> Option<DateTime<Utc>> {
    // Seconds, or milliseconds past 2001-09-09 (1e12 ms).
    let secs = if n.abs() >= 1e11 { n / 1000.0 } else { n };
    Utc.timestamp_opt(secs.trunc() as i64, (secs.fract().abs() * 1e9) as u32)
        .single()
}

fn parse(s: &str) -> Option<DateTime<Utc>> {
    let s = s.split_once('(').map_or(s, |(d, _)| d).trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return from_number(n);
    }
    let mut s = s.replacen('T', " ", 1);
    if let Some(rest) = s.strip_suffix('Z').or_else(|| s.strip_suffix(" UTC")) {
        s = format!("{} +00:00", rest.trim_end());
    }
    for f in [
        "%Y-%m-%d %H:%M:%S%.f %:z",
        "%Y-%m-%d %H:%M:%S%.f%:z",
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%d %H:%M:%S%.f%z",
        "%Y/%m/%d %H:%M:%S%.f %:z",
    ] {
        if let Ok(d) = DateTime::parse_from_str(&s, f) {
            return Some(d.with_timezone(&Utc));
        }
    }
    for f in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S%.f",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(d) = NaiveDateTime::parse_from_str(&s, f) {
            return Some(d.and_utc());
        }
    }
    for f in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(&s, f) {
            return d.and_hms_opt(0, 0, 0).map(|d| d.and_utc());
        }
    }
    None
}

/// A private copy of a library, removed when dropped.
pub struct FixedCopy {
    dir: PathBuf,
    pub db: PathBuf,
}

impl FixedCopy {
    /// Copies `master.db` (with its write-ahead log, when rekordbox is
    /// running) into a fresh folder and fixes its dates there.
    pub fn of(path: &Path) -> std::io::Result<Self> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("djsugar-rekordbox-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let copy = Self {
            db: dir.join("master.db"),
            dir,
        };
        std::fs::copy(path, &copy.db)?;
        for ext in ["-wal", "-shm"] {
            let mut side = path.as_os_str().to_owned();
            side.push(ext);
            let side = PathBuf::from(side);
            if side.exists() {
                let mut to = copy.db.as_os_str().to_owned();
                to.push(ext);
                std::fs::copy(&side, PathBuf::from(to))?;
            }
        }
        fix(&copy.db).map_err(std::io::Error::other)?;
        Ok(copy)
    }
}

impl Drop for FixedCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn reads_the_spellings_rekordbox_libraries_hold() {
        for (v, want) in [
            (
                text("2020-12-11 02:05:03.483 +00:00"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11 02:05:03.483 +00:00 (Coordinated Universal Time)"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11 02:05:03 +00:00"),
                "2020-12-11 02:05:03.000 +00:00",
            ),
            (
                text("2020-12-11 02:05:03.4831234 +00:00"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11 11:05:03.483 +09:00"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11 02:05:03.483 +0000"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11T02:05:03.483Z"),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (
                text("2020-12-11T02:05:03"),
                "2020-12-11 02:05:03.000 +00:00",
            ),
            (
                text("2020-12-11 02:05:03"),
                "2020-12-11 02:05:03.000 +00:00",
            ),
            (text("2020/12/11 02:05"), "2020-12-11 02:05:00.000 +00:00"),
            (text("2020-12-11"), "2020-12-11 00:00:00.000 +00:00"),
            (
                Value::Integer(1_607_652_303),
                "2020-12-11 02:05:03.000 +00:00",
            ),
            (
                Value::Integer(1_607_652_303_483),
                "2020-12-11 02:05:03.483 +00:00",
            ),
            (Value::Null, "1970-01-01 00:00:00.000 +00:00"),
            (text(""), "1970-01-01 00:00:00.000 +00:00"),
            (text("not a date"), "1970-01-01 00:00:00.000 +00:00"),
        ] {
            assert_eq!(normalise(&v), want, "{v:?}");
        }
    }

    /// A library with odd and missing dates: the copy reads in rbox, the
    /// DJ's file is left as it was.
    #[test]
    fn opens_a_library_with_odd_dates_through_a_copy() {
        let dir = std::env::temp_dir().join(format!("djsugar-rb-dates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("master.db");
        {
            let c = connect(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .unwrap();
            c.execute_batch(
                "CREATE TABLE djmdColor (ID VARCHAR(255) PRIMARY KEY, ColorCode INTEGER,
                    SortKey INTEGER, Commnt VARCHAR(255), UUID VARCHAR(255),
                    rb_data_status INTEGER DEFAULT 0, rb_local_data_status INTEGER DEFAULT 0,
                    rb_local_deleted TINYINT(1) DEFAULT 0, rb_local_synced TINYINT(1) DEFAULT 0,
                    usn BIGINT DEFAULT NULL, rb_local_usn BIGINT DEFAULT NULL,
                    created_at DATETIME, updated_at DATETIME);
                 INSERT INTO djmdColor VALUES
                    ('1', 16711935, 1, 'Pink', 'a', 0, 0, 0, 0, 1, 1,
                     '2020-12-11 02:05:01.385 +00:00', '2020-12-11 02:05:01.385 +00:00'),
                    ('2', 16711680, 2, 'Red', 'b', 0, 0, 0, 0, 2, 2,
                     '2020-12-11 02:05:01 +00:00', NULL),
                    ('3', 16753920, 3, 'Orange', 'c', 0, 0, 0, 0, 3, 3,
                     '2020-12-11T02:05:01Z', '');",
            )
            .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        // rbox alone can't read it.
        assert!(rbox_masterdb::MasterDb::new(&path)
            .unwrap()
            .get_colors()
            .is_err());
        assert!(needs_fixing(&path).unwrap());
        let copy = FixedCopy::of(&path).unwrap();
        assert!(!needs_fixing(&copy.db).unwrap());
        let mut db = rbox_masterdb::MasterDb::new(&copy.db).unwrap();
        let colors = db.get_colors().unwrap();
        assert_eq!(colors.len(), 3);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let copied = copy.dir.clone();
        drop(db);
        drop(copy);
        assert!(!copied.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// A library with NULLs where rbox wants a value (issue #699): the copy
    /// reads in rbox, the DJ's file is left as it was.
    #[test]
    fn opens_a_library_with_nulls_through_a_copy() {
        let dir = std::env::temp_dir().join(format!("djsugar-rb-nulls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("master.db");
        {
            let c = connect(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .unwrap();
            c.execute_batch(
                "CREATE TABLE djmdColor (ID VARCHAR(255) PRIMARY KEY, ColorCode INTEGER,
                    SortKey INTEGER, Commnt VARCHAR(255), UUID VARCHAR(255),
                    rb_data_status INTEGER DEFAULT 0, rb_local_data_status INTEGER DEFAULT 0,
                    rb_local_deleted TINYINT(1) DEFAULT 0, rb_local_synced TINYINT(1) DEFAULT 0,
                    usn BIGINT DEFAULT NULL, rb_local_usn BIGINT DEFAULT NULL,
                    created_at DATETIME, updated_at DATETIME);
                 INSERT INTO djmdColor VALUES
                    ('1', 16711935, 1, 'Pink', NULL, NULL, 0, NULL, 0, 1, 1,
                     '2020-12-11 02:05:01.385 +00:00', '2020-12-11 02:05:01.385 +00:00'),
                    ('2', 16711680, 2, 'Red', 'b', 0, 0, 0, 0, 2, 2,
                     '2020-12-11 02:05:01.385 +00:00', '2020-12-11 02:05:01.385 +00:00');",
            )
            .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        // rbox alone can't read it.
        assert!(rbox_masterdb::MasterDb::new(&path)
            .unwrap()
            .get_colors()
            .is_err());
        assert!(needs_fixing(&path).unwrap());
        let copy = FixedCopy::of(&path).unwrap();
        assert!(!needs_fixing(&copy.db).unwrap());
        let mut db = rbox_masterdb::MasterDb::new(&copy.db).unwrap();
        assert_eq!(db.get_colors().unwrap().len(), 2);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(db);
        drop(copy);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
