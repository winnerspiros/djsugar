#[derive(Debug, Default)]
pub struct Lyrics {
    pub lines: Vec<(f64, Box<str>)>,
}

impl Lyrics {
    pub fn parse(text: &str) -> Self {
        let mut offset = 0.0;
        let mut lines = Vec::new();
        for raw in text.lines() {
            let mut rest = raw.trim();
            let mut stamps = Vec::new();
            while let Some(tag) = rest.strip_prefix('[').and_then(|r| r.split_once(']')) {
                let (inner, after) = tag;
                rest = after;
                if let Some(v) = inner.strip_prefix("offset:") {
                    offset = v.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
                } else if let Some(t) = stamp(inner) {
                    stamps.push(t);
                }
            }
            for t in stamps {
                lines.push((t, rest.trim().into()));
            }
        }
        for l in &mut lines {
            l.0 = (l.0 - offset).max(0.0);
        }
        lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { lines }
    }

    pub fn current(&self, seconds: f64) -> Option<usize> {
        self.lines.partition_point(|l| l.0 <= seconds).checked_sub(1)
    }
}

fn stamp(s: &str) -> Option<f64> {
    let (m, sec) = s.split_once(':')?;
    let m: f64 = m.trim().parse().ok()?;
    let sec: f64 = sec.trim().replace(':', ".").parse().ok()?;
    Some(m * 60.0 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repeats_offset_and_finds_line() {
        let l = Lyrics::parse("[ar:Someone]\n[offset:500]\n[00:01.50][00:10.00]Chorus\n[00:05.25]Verse\nnot a line");
        assert_eq!(l.lines.len(), 3);
        assert_eq!(&*l.lines[0].1, "Chorus");
        assert!((l.lines[0].0 - 1.0).abs() < 1e-9);
        assert_eq!(l.current(0.5), None);
        assert_eq!(l.current(5.0), Some(1));
        assert_eq!(l.current(60.0), Some(2));
    }
}
