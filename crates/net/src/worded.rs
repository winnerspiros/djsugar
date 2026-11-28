//! Lyrics timed word by word, as KuGou and NetEase keep them, rewritten as enhanced LRC: a time ahead of each line and one
//! ahead of each of its words. Both write a line as `[start,length]` and then its words, each behind its own start and
//! length in milliseconds; they differ in the brackets around those and in what a word's start is counted from.

use std::fmt::Write;
use std::io::Read;

/// What KuGou's word timed lyrics are scrambled with, over and over.
const KEY: [u8; 16] = [0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69];
/// What such a file starts with.
const MARK: &[u8] = b"krc1";
/// The most a file may unpack to; lyrics are a few thousand bytes.
const MOST: u64 = 1 << 20;

/// How the two services write a word's times.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    /// KuGou: `<start,length,0>`, the start counted from the line's.
    Krc,
    /// NetEase: `(start,length,0)`, the start counted from the track's.
    Yrc,
}

fn stamp(ms: u64) -> String {
    format!("{:02}:{:02}.{:02}", ms / 60_000, ms / 1000 % 60, ms % 1000 / 10)
}

/// One line as enhanced LRC; None for what is no line of words.
fn line(raw: &str, kind: Kind) -> Option<String> {
    let (open, close) = match kind {
        Kind::Krc => ('<', '>'),
        Kind::Yrc => ('(', ')'),
    };
    let (head, body) = raw.trim().strip_prefix('[')?.split_once(']')?;
    let start: u64 = head.split_once(',')?.0.parse().ok()?;
    let (mut out, mut said, mut end) = (format!("[{}]", stamp(start)), String::new(), None);
    for piece in body.split(open).skip(1) {
        let times = piece.split_once(close).and_then(|(times, text)| {
            let mut n = times.split(',').map(str::parse::<u64>);
            Some((n.next()?.ok()?, n.next()?.ok()?, text))
        });
        match times {
            Some((at, long, text)) => {
                let at = if matches!(kind, Kind::Krc) { start + at } else { at };
                let _ = write!(out, "<{}>{text}", stamp(at));
                said.push_str(text);
                end = Some(at + long);
            }
            // A bracket that is sung, not a time: it belongs to the word before it.
            None if end.is_some() => {
                out.push(open);
                out.push_str(piece);
            }
            None => {}
        }
    }
    let _ = write!(out, "<{}>", stamp(end?));
    (!said.trim().is_empty() && !crate::netease::credit(&said)).then_some(out)
}

/// Word timed lyrics as enhanced LRC; None when no line of them has words.
pub(crate) fn lrc(text: &str, kind: Kind) -> Option<String> {
    let lines: Vec<String> = text.lines().filter_map(|l| line(l, kind)).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// What KuGou sent as word timed lyrics, unscrambled and unpacked.
pub(crate) fn krc(sent: &[u8]) -> Option<String> {
    let packed: Vec<u8> = sent.strip_prefix(MARK)?.iter().zip(KEY.iter().cycle()).map(|(b, k)| b ^ k).collect();
    let mut text = String::new();
    flate2::read::ZlibDecoder::new(&packed[..]).take(MOST).read_to_string(&mut text).ok()?;
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_both_services_lines() {
        let krc = "[ti:one more time]\n[30438,1293]<0,608,0>One <608,532,0>more <1140,153,0>time\n[40000,10]";
        assert_eq!(lrc(krc, Kind::Krc).unwrap(), "[00:30.43]<00:30.43>One <00:31.04>more <00:31.57>time<00:31.73>");
        let yrc = "{\"t\":0,\"c\":[{\"tx\":\"x\"}]}\n[30260,2040](30260,480,0)One (30740,480,0)more (31220,1080,0)time";
        assert_eq!(lrc(yrc, Kind::Yrc).unwrap(), "[00:30.26]<00:30.26>One <00:30.74>more <00:31.22>time<00:32.30>");
        assert_eq!(lrc("[00:30.26]One more time", Kind::Yrc), None);
    }

    #[test]
    fn keeps_sung_brackets_and_drops_credits() {
        let yrc = "[1000,900](1000,400,0)Oh (1400,500,0)yeah (all right)\n[0,500](0,500,0)\u{4f5c}\u{8bcd}: Someone";
        assert_eq!(lrc(yrc, Kind::Yrc).unwrap(), "[00:01.00]<00:01.00>Oh <00:01.40>yeah (all right)<00:01.90>");
    }

    #[test]
    fn unscrambles_what_kugou_sends() {
        use std::io::Write;
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(b"[0,5]<0,5,0>Hi").unwrap();
        let mut sent = MARK.to_vec();
        sent.extend(z.finish().unwrap().iter().zip(KEY.iter().cycle()).map(|(b, k)| b ^ k));
        assert_eq!(krc(&sent).as_deref(), Some("[0,5]<0,5,0>Hi"));
        assert_eq!(krc(b"nope"), None);
    }
}
