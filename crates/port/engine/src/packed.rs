//! Model files kept small: their 32 or 16-bit weights stored as 8 bits each, a run at a time, and put back when read.
//! Whatever is not a weight is kept as it is, so the file read back has the size and layout it had.

use std::path::Path;

/// What a packed file opens with: the first kind holds 32-bit weights only, the second says how wide each
/// stretch of weights was.
const MARKS: [&[u8; 8]; 2] = [b"DJSPACK1", b"DJSPACK2"];
/// How many weights share one range. Fewer follows the weights closer and saves less.
const RUN: usize = 256;
/// The fewest bytes of weights worth packing.
const LEAST: usize = 4096;

/// A model file's bytes, put back to full size if they were packed.
pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if !MARKS.iter().any(|m| bytes.starts_with(*m)) {
        return Ok(bytes);
    }
    unpack(&bytes).ok_or_else(|| format!("{} is damaged", path.display()))
}

/// Packs a model file: an ONNX graph or a safetensors table. `None` if it has no weights to pack.
pub fn pack(whole: &[u8]) -> Option<Vec<u8>> {
    let mut spans = tensors(whole).unwrap_or_else(|| {
        let mut spans = Vec::new();
        graph(whole, 0, whole.len(), Kind::Model, &mut spans);
        spans.into_iter().map(|(from, to)| (from, to, 4)).collect()
    });
    spans.retain(|s| s.1 - s.0 >= LEAST && (s.1 - s.0) % s.2 == 0);
    spans.sort_unstable();
    if spans.is_empty() {
        return None;
    }
    let mut out = MARKS[1].to_vec();
    out.extend((whole.len() as u64).to_le_bytes());
    let mut at = 0;
    for (from, to, wide) in spans {
        out.extend(((from - at) as u64).to_le_bytes());
        out.extend(&whole[at..from]);
        out.extend((((to - from) / wide) as u64).to_le_bytes());
        out.push(wide as u8);
        for run in whole[from..to].chunks(RUN * wide) {
            let values = || run.chunks_exact(wide).map(weight);
            let (low, high) = values().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
            let step = (high - low) / 255.0;
            out.extend(low.to_le_bytes());
            out.extend(step.to_le_bytes());
            out.extend(values().map(|v| if step > 0.0 { ((v - low) / step).round() as u8 } else { 0 }));
        }
        at = to;
    }
    out.extend(((whole.len() - at) as u64).to_le_bytes());
    out.extend(&whole[at..]);
    Some(out)
}

/// A weight as it is kept, 32 bits wide or 16.
fn weight(kept: &[u8]) -> f32 {
    match *kept {
        [a, b] => half::f16::from_le_bytes([a, b]).to_f32(),
        [a, b, c, d] => f32::from_le_bytes([a, b, c, d]),
        _ => 0.0,
    }
}

fn unpack(packed: &[u8]) -> Option<Vec<u8>> {
    let sized = packed.starts_with(MARKS[1]);
    let mut rest = packed.get(MARKS[0].len()..)?;
    let mut take = |n: usize| {
        let (head, tail) = rest.split_at_checked(n)?;
        rest = tail;
        Some(head)
    };
    let size = |b: &[u8]| Some(u64::from_le_bytes(b.try_into().ok()?) as usize);
    let whole = size(take(8)?)?;
    let mut out = Vec::with_capacity(whole);
    while out.len() < whole {
        let kept = size(take(8)?)?;
        out.extend(take(kept)?);
        if out.len() >= whole {
            break;
        }
        let mut left = size(take(8)?)?;
        let wide = if sized { take(1)?[0] } else { 4 };
        while left > 0 {
            let n = left.min(RUN);
            let low = f32::from_le_bytes(take(4)?.try_into().ok()?);
            let step = f32::from_le_bytes(take(4)?.try_into().ok()?);
            let values = take(n)?.iter().map(|b| low + step * f32::from(*b));
            match wide {
                2 => out.extend(values.flat_map(|v| half::f16::from_f32(v).to_le_bytes())),
                4 => out.extend(values.flat_map(f32::to_le_bytes)),
                _ => return None,
            }
            left -= n;
        }
    }
    (out.len() == whole).then_some(out)
}

/// Where a safetensors table keeps its weights and how many bytes wide each is, if this is one.
fn tensors(whole: &[u8]) -> Option<Vec<(usize, usize, usize)>> {
    let head = u64::from_le_bytes(whole.get(..8)?.try_into().ok()?) as usize;
    let table: serde_json::Value = serde_json::from_slice(whole.get(8..8 + head)?).ok()?;
    let mut spans = Vec::new();
    for (name, t) in table.as_object()? {
        let wide = match t["dtype"].as_str() {
            Some("F32") if name != "__metadata__" => 4,
            Some("F16") if name != "__metadata__" => 2,
            _ => continue,
        };
        let end = |i: usize| Some(8 + head + t["data_offsets"][i].as_u64()? as usize);
        spans.push((end(0)?, end(1).filter(|e| *e <= whole.len())?, wide));
    }
    Some(spans)
}

/// The messages of an ONNX file that lead to weights.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Model,
    Graph,
    Node,
    Attribute,
    Tensor,
}

/// Walks one message of an ONNX file, noting where the 32-bit weights under it are.
fn graph(whole: &[u8], from: usize, to: usize, kind: Kind, spans: &mut Vec<(usize, usize)>) {
    let mut at = from;
    let number = |at: &mut usize| {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *whole.get(*at).filter(|_| *at < to)?;
            *at += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Some(v);
            }
        }
        None
    };
    let (mut floats, mut raw) = (false, None);
    while at < to {
        let Some(key) = number(&mut at) else { return };
        let (field, wire) = (key >> 3, key & 7);
        let len = match wire {
            0 => {
                let Some(v) = number(&mut at) else { return };
                floats |= kind == Kind::Tensor && field == 2 && v == 1;
                continue;
            }
            1 => 8,
            5 => 4,
            2 => match number(&mut at) {
                Some(n) => n as usize,
                None => return,
            },
            _ => return,
        };
        let end = at + len;
        if end > to {
            return;
        }
        let inner = match (kind, field) {
            (Kind::Model, 7) | (Kind::Attribute, 6 | 11) => Some(Kind::Graph),
            (Kind::Graph, 1) => Some(Kind::Node),
            (Kind::Graph, 5) | (Kind::Attribute, 5 | 10) => Some(Kind::Tensor),
            (Kind::Node, 5) => Some(Kind::Attribute),
            _ => None,
        };
        if let Some(inner) = inner.filter(|_| wire == 2) {
            graph(whole, at, end, inner, spans);
        } else if kind == Kind::Tensor && wire == 2 && (field == 9 || field == 4) {
            // Weights are kept raw, or as a packed list of floats; a list is floats whatever the type says.
            floats |= field == 4;
            raw = Some((at, end));
        }
        at = end;
    }
    if let Some(raw) = raw.filter(|_| floats) {
        spans.push(raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weights(n: usize) -> Vec<u8> {
        (0..n).flat_map(|i| ((i as f32 * 0.37).sin() * 0.2).to_le_bytes()).collect()
    }

    /// A length-prefixed field of an ONNX message.
    fn field(n: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![n << 3 | 2];
        let mut len = body.len();
        while len >= 0x80 {
            out.push(len as u8 | 0x80);
            len >>= 7;
        }
        out.push(len as u8);
        out.extend(body);
        out
    }

    fn close(a: &[u8], b: &[u8]) -> bool {
        let floats = |x: &[u8]| x.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect::<Vec<_>>();
        a.len() == b.len() && floats(a).iter().zip(floats(b)).all(|(a, b)| (a - b).abs() < 0.001)
    }

    #[test]
    fn packs_the_weights_of_a_graph_and_those_under_its_nodes() {
        let w = weights(3000);
        let tensor = [vec![2 << 3, 1], field(8, b"w"), field(9, &w)].concat();
        let whole_numbers = [vec![2 << 3, 7], field(9, &w)].concat();
        let node = field(5, &field(6, &field(5, &tensor)));
        let model = field(7, &[field(1, &node), field(5, &tensor), field(5, &whole_numbers)].concat());
        let packed = pack(&model).unwrap();
        assert!(packed.len() < model.len() * 6 / 10, "{} of {}", packed.len(), model.len());
        let back = unpack(&packed).unwrap();
        assert_eq!(back.len(), model.len());
        let mut spans = Vec::new();
        graph(&model, 0, model.len(), Kind::Model, &mut spans);
        assert_eq!(spans.len(), 2);
        for (from, to) in spans {
            assert!(close(&back[from..to], &model[from..to]));
        }
        assert_eq!(back[model.len() - w.len()..], model[model.len() - w.len()..], "whole numbers are left alone");
    }

    #[test]
    fn packs_a_table_of_tensors() {
        let w = weights(2000);
        let head = format!(r#"{{"a":{{"dtype":"F32","shape":[2000],"data_offsets":[0,{}]}}}}"#, w.len());
        let whole = [(head.len() as u64).to_le_bytes().to_vec(), head.into_bytes(), w.clone()].concat();
        let back = unpack(&pack(&whole).unwrap()).unwrap();
        assert_eq!(back[..whole.len() - w.len()], whole[..whole.len() - w.len()]);
        assert!(close(&back[whole.len() - w.len()..], &w));
    }

    #[test]
    fn packs_weights_16_bits_wide_to_half_their_size() {
        let w: Vec<u8> =
            (0..4000).flat_map(|i| half::f16::from_f32((i as f32 * 0.37).sin() * 0.2).to_le_bytes()).collect();
        let head = format!(r#"{{"a":{{"dtype":"F16","shape":[4000],"data_offsets":[0,{}]}}}}"#, w.len());
        let whole = [(head.len() as u64).to_le_bytes().to_vec(), head.into_bytes(), w.clone()].concat();
        let packed = pack(&whole).unwrap();
        assert!(packed.len() < whole.len() * 6 / 10, "{} of {}", packed.len(), whole.len());
        let back = unpack(&packed).unwrap();
        assert_eq!(back.len(), whole.len());
        let floats = |x: &[u8]| x.as_chunks::<2>().0.iter().map(|b| weight(b)).collect::<Vec<_>>();
        let (was, is) = (floats(&w), floats(&back[whole.len() - w.len()..]));
        assert!(was.iter().zip(is).all(|(a, b)| (a - b).abs() < 0.001));
    }

    #[test]
    fn reads_a_file_packed_before_weights_had_widths() {
        let w = weights(2000);
        let packed = pack(&field(7, &field(5, &[vec![2 << 3, 1], field(9, &w)].concat()))).unwrap();
        // The older layout: its own mark, and no width after the count of weights.
        let count = (2000u64).to_le_bytes();
        let at = packed.windows(8).position(|p| p == count).unwrap() + 8;
        let old = [MARKS[0].as_slice(), &packed[8..at], &packed[at + 1..]].concat();
        assert_eq!(unpack(&old), unpack(&packed));
        assert!(unpack(&old).is_some());
    }

    #[test]
    fn a_cut_file_is_not_read() {
        let packed = pack(&field(7, &field(5, &[vec![2 << 3, 1], field(9, &weights(3000))].concat()))).unwrap();
        assert!(unpack(&packed[..packed.len() - 9]).is_none());
    }
}
