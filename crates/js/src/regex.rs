//! Regular expressions with JavaScript semantics, matched by backtracking
//! over UTF-16 code units.

#[derive(Debug)]
enum Node {
    Char(u16),
    Any,
    Class(Vec<(u16, u16)>, bool),
    Start,
    End,
    WordB(bool),
    Group(Box<Node>, Option<usize>),
    Look(Box<Node>, bool, bool),
    Backref(usize),
    Alt(Vec<Node>),
    Seq(Vec<Node>),
    Repeat(Box<Node>, u32, u32, bool),
}

#[derive(Debug)]
pub struct Regex {
    node: Node,
    pub groups: usize,
    pub names: Vec<(String, usize)>,
    pub global: bool,
    pub ignore_case: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub sticky: bool,
    pub unicode: bool,
    pub source: String,
    pub flags: String,
}

const DIGIT: &[(u16, u16)] = &[(b'0' as u16, b'9' as u16)];
const WORD: &[(u16, u16)] = &[
    (b'0' as u16, b'9' as u16),
    (b'A' as u16, b'Z' as u16),
    (b'_' as u16, b'_' as u16),
    (b'a' as u16, b'z' as u16),
];
const SPACE: &[(u16, u16)] = &[
    (9, 13),
    (32, 32),
    (0xa0, 0xa0),
    (0x1680, 0x1680),
    (0x2000, 0x200a),
    (0x2028, 0x2029),
    (0x202f, 0x202f),
    (0x205f, 0x205f),
    (0x3000, 0x3000),
    (0xfeff, 0xfeff),
];

fn negate(r: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    let mut next = 0u32;
    let mut r = r.to_vec();
    r.sort();
    for (a, b) in r {
        if a as u32 > next {
            out.push((next as u16, a - 1));
        }
        next = next.max(b as u32 + 1);
    }
    if next <= 0xffff {
        out.push((next as u16, 0xffff));
    }
    out
}

struct P<'a> {
    s: &'a [u16],
    i: usize,
    groups: usize,
    names: Vec<(String, usize)>,
}

impl P<'_> {
    fn peek(&self) -> Option<u16> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c as u16) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn err<T>(&self, m: &str) -> Result<T, String> {
        Err(format!("invalid regular expression: {m}"))
    }

    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.seq()?];
        while self.eat(b'|') {
            alts.push(self.seq()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Node::Alt(alts)
        })
    }

    fn seq(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == b'|' as u16 || c == b')' as u16 {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quant(atom)?);
        }
        Ok(Node::Seq(items))
    }

    fn num(&mut self) -> Option<u32> {
        let st = self.i;
        let mut n: u32 = 0;
        while let Some(c) = self
            .peek()
            .filter(|c| (b'0' as u16..=b'9' as u16).contains(c))
        {
            n = n
                .saturating_mul(10)
                .saturating_add((c - b'0' as u16) as u32);
            self.i += 1;
        }
        (self.i > st).then_some(n)
    }

    fn quant(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek().map(|c| c as u8) {
            Some(b'*') => (0, u32::MAX),
            Some(b'+') => (1, u32::MAX),
            Some(b'?') => (0, 1),
            Some(b'{') => {
                let st = self.i;
                self.i += 1;
                let Some(a) = self.num() else {
                    self.i = st;
                    return Ok(atom);
                };
                let b = if self.eat(b',') {
                    self.num().unwrap_or(u32::MAX)
                } else {
                    a
                };
                if !self.eat(b'}') {
                    self.i = st;
                    return Ok(atom);
                }
                self.i -= 1;
                (a, b)
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        if matches!(atom, Node::Start | Node::End | Node::WordB(_)) {
            return self.err("nothing to repeat");
        }
        let lazy = self.eat(b'?');
        Ok(Node::Repeat(Box::new(atom), min, max, !lazy))
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().unwrap();
        self.i += 1;
        Ok(match c as u8 {
            b'.' if c < 128 => Node::Any,
            b'^' if c < 128 => Node::Start,
            b'$' if c < 128 => Node::End,
            b'(' if c < 128 => {
                let mut cap = None;
                let node = if self.eat(b'?') {
                    if self.eat(b':') {
                        self.alt()?
                    } else if self.eat(b'=') || self.eat(b'!') {
                        let neg = self.s[self.i - 1] == b'!' as u16;
                        let n = self.alt()?;
                        if !self.eat(b')') {
                            return self.err("missing )");
                        }
                        return Ok(Node::Look(Box::new(n), neg, false));
                    } else if self.eat(b'<') {
                        if self.eat(b'=') || self.eat(b'!') {
                            let neg = self.s[self.i - 1] == b'!' as u16;
                            let n = self.alt()?;
                            if !self.eat(b')') {
                                return self.err("missing )");
                            }
                            return Ok(Node::Look(Box::new(n), neg, true));
                        }
                        let st = self.i;
                        while self.peek().is_some_and(|c| c != b'>' as u16) {
                            self.i += 1;
                        }
                        let name = String::from_utf16_lossy(&self.s[st..self.i]);
                        self.i += 1;
                        self.groups += 1;
                        cap = Some(self.groups);
                        self.names.push((name, self.groups));
                        self.alt()?
                    } else {
                        return self.err("bad group");
                    }
                } else {
                    self.groups += 1;
                    cap = Some(self.groups);
                    self.alt()?
                };
                if !self.eat(b')') {
                    return self.err("missing )");
                }
                Node::Group(Box::new(node), cap)
            }
            b')' if c < 128 => return self.err("unmatched )"),
            b'[' if c < 128 => self.class()?,
            b'\\' if c < 128 => self.escape(false)?,
            b'*' | b'+' | b'?' if c < 128 => return self.err("nothing to repeat"),
            _ => Node::Char(c),
        })
    }

    fn class(&mut self) -> Result<Node, String> {
        let neg = self.eat(b'^');
        let mut ranges = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return self.err("missing ]");
            };
            if c == b']' as u16 {
                self.i += 1;
                break;
            }
            let lo = self.class_atom()?;
            if self.peek() == Some(b'-' as u16)
                && self.s.get(self.i + 1).is_some_and(|&c| c != b']' as u16)
            {
                self.i += 1;
                let hi = self.class_atom()?;
                match (lo, hi) {
                    (Err(a), Err(b)) if a <= b => ranges.push((a, b)),
                    (Err(_), Err(_)) => return self.err("range out of order"),
                    (lo, hi) => {
                        for x in [lo, hi] {
                            match x {
                                Ok(r) => ranges.extend(r),
                                Err(c) => ranges.push((c, c)),
                            }
                        }
                        ranges.push((b'-' as u16, b'-' as u16));
                    }
                }
            } else {
                match lo {
                    Ok(r) => ranges.extend(r),
                    Err(c) => ranges.push((c, c)),
                }
            }
        }
        Ok(Node::Class(ranges, neg))
    }

    /// One class member: `Err(code unit)` for a single unit, `Ok` for a set.
    fn class_atom(&mut self) -> Result<Result<Vec<(u16, u16)>, u16>, String> {
        let c = self.peek().unwrap();
        self.i += 1;
        if c != b'\\' as u16 {
            return Ok(Err(c));
        }
        Ok(match self.escape(true)? {
            Node::Char(c) => Err(c),
            Node::Class(r, false) => Ok(r),
            Node::Class(r, true) => Ok(negate(&r)),
            Node::WordB(_) => Err(8),
            _ => return self.err("bad escape in class"),
        })
    }

    fn hex(&mut self, n: usize) -> Option<u16> {
        let h = self.s.get(self.i..self.i + n)?;
        let v = u16::from_str_radix(&String::from_utf16_lossy(h), 16).ok()?;
        self.i += n;
        Some(v)
    }

    fn escape(&mut self, in_class: bool) -> Result<Node, String> {
        let Some(c) = self.peek() else {
            return self.err("trailing \\");
        };
        self.i += 1;
        let ch = if c < 128 { c as u8 } else { 0 };
        Ok(match ch {
            b'd' => Node::Class(DIGIT.to_vec(), false),
            b'D' => Node::Class(DIGIT.to_vec(), true),
            b'w' => Node::Class(WORD.to_vec(), false),
            b'W' => Node::Class(WORD.to_vec(), true),
            b's' => Node::Class(SPACE.to_vec(), false),
            b'S' => Node::Class(SPACE.to_vec(), true),
            b'b' if !in_class => Node::WordB(true),
            b'B' if !in_class => Node::WordB(false),
            b'b' => Node::Char(8),
            b'n' => Node::Char(10),
            b'r' => Node::Char(13),
            b't' => Node::Char(9),
            b'v' => Node::Char(11),
            b'f' => Node::Char(12),
            b'0' if !self
                .peek()
                .is_some_and(|c| (b'0' as u16..=b'9' as u16).contains(&c)) =>
            {
                Node::Char(0)
            }
            b'1'..=b'9' if !in_class => {
                self.i -= 1;
                Node::Backref(self.num().unwrap() as usize)
            }
            b'k' if self.peek() == Some(b'<' as u16) => {
                let st = self.i + 1;
                while self.peek().is_some_and(|c| c != b'>' as u16) {
                    self.i += 1;
                }
                let name = String::from_utf16_lossy(&self.s[st..self.i]);
                self.i += 1;
                // Resolved after parsing; names may be declared later.
                Node::Backref(usize::MAX - self.name_ref(&name))
            }
            b'x' => match self.hex(2) {
                Some(v) => Node::Char(v),
                None => Node::Char(b'x' as u16),
            },
            b'u' => {
                if self.eat(b'{') {
                    let st = self.i;
                    while self.peek().is_some_and(|c| c != b'}' as u16) {
                        self.i += 1;
                    }
                    let v = u32::from_str_radix(&String::from_utf16_lossy(&self.s[st..self.i]), 16)
                        .unwrap_or(0);
                    self.i += 1;
                    let mut b = [0u16; 2];
                    let u = char::from_u32(v).unwrap_or('\u{fffd}').encode_utf16(&mut b);
                    if u.len() == 1 {
                        Node::Char(u[0])
                    } else {
                        Node::Seq(vec![Node::Char(u[0]), Node::Char(u[1])])
                    }
                } else {
                    match self.hex(4) {
                        Some(v) => Node::Char(v),
                        None => Node::Char(b'u' as u16),
                    }
                }
            }
            b'c' => match self.peek() {
                Some(l) if (l as u8).is_ascii_alphabetic() && l < 128 => {
                    self.i += 1;
                    Node::Char(l % 32)
                }
                _ => Node::Char(b'\\' as u16),
            },
            _ => Node::Char(c),
        })
    }

    fn name_ref(&mut self, name: &str) -> usize {
        // Stored as an index into a pending list, encoded below usize::MAX.
        PENDING.with(|p| {
            let mut p = p.borrow_mut();
            p.push(name.to_string());
            p.len() - 1
        })
    }
}

thread_local! {
    static PENDING: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn resolve_names(n: &mut Node, names: &[(String, usize)], pending: &[String]) {
    match n {
        Node::Backref(i) if *i > usize::MAX / 2 => {
            let name = &pending[usize::MAX - *i];
            *i = names.iter().find(|(n, _)| n == name).map_or(0, |(_, g)| *g);
        }
        Node::Group(x, _) | Node::Look(x, _, _) | Node::Repeat(x, ..) => {
            resolve_names(x, names, pending)
        }
        Node::Alt(v) | Node::Seq(v) => v.iter_mut().for_each(|x| resolve_names(x, names, pending)),
        _ => {}
    }
}

fn fold(c: u16) -> u16 {
    if c < 128 {
        return (c as u8).to_ascii_lowercase() as u16;
    }
    char::from_u32(c as u32)
        .and_then(|ch| {
            let mut l = ch.to_lowercase();
            let x = l.next()?;
            (l.next().is_none() && (x as u32) < 0x10000).then_some(x as u16)
        })
        .unwrap_or(c)
}

impl Regex {
    pub fn new(source: &str, flags: &str) -> Result<Regex, String> {
        let mut f = [false; 6];
        for c in flags.chars() {
            let i = match c {
                'g' => 0,
                'i' => 1,
                'm' => 2,
                's' => 3,
                'y' => 4,
                'u' | 'v' => 5,
                'd' => continue,
                _ => return Err(format!("invalid regular expression flag {c}")),
            };
            if f[i] {
                return Err(format!("duplicate regular expression flag {c}"));
            }
            f[i] = true;
        }
        let s: Vec<u16> = source.encode_utf16().collect();
        PENDING.with(|p| p.borrow_mut().clear());
        let mut p = P {
            s: &s,
            i: 0,
            groups: 0,
            names: Vec::new(),
        };
        let mut node = p.alt()?;
        if p.i < s.len() {
            return p.err("unmatched )");
        }
        let pending = PENDING.with(|x| std::mem::take(&mut *x.borrow_mut()));
        resolve_names(&mut node, &p.names, &pending);
        Ok(Regex {
            node,
            groups: p.groups,
            names: p.names,
            global: f[0],
            ignore_case: f[1],
            multiline: f[2],
            dot_all: f[3],
            sticky: f[4],
            unicode: f[5],
            source: source.to_string(),
            flags: flags.to_string(),
        })
    }

    /// Captures (start, end) of the first match at or after `start`; index 0
    /// is the whole match.
    pub fn exec(&self, s: &[u16], start: usize) -> Option<Vec<Option<(usize, usize)>>> {
        let mut caps = vec![None; self.groups + 1];
        let last = if self.sticky { start } else { s.len() };
        let mut steps = 0u64;
        for st in start..=last.min(s.len()) {
            let mut m = M {
                re: self,
                s,
                caps: &mut caps,
                steps: &mut steps,
            };
            if let Some(end) = m.node(&self.node, st, &mut |_, p| Some(p)) {
                caps[0] = Some((st, end));
                return Some(caps);
            }
            if steps > 50_000_000 {
                return None;
            }
            caps.iter_mut().for_each(|c| *c = None);
        }
        None
    }
}

type K<'k> = dyn FnMut(&mut M, usize) -> Option<usize> + 'k;

struct M<'a> {
    re: &'a Regex,
    s: &'a [u16],
    caps: &'a mut Vec<Option<(usize, usize)>>,
    steps: &'a mut u64,
}

impl M<'_> {
    fn is_word(&self, i: usize) -> bool {
        self.s
            .get(i)
            .is_some_and(|&c| c < 128 && ((c as u8).is_ascii_alphanumeric() || c == b'_' as u16))
    }

    fn eq(&self, a: u16, b: u16) -> bool {
        a == b || self.re.ignore_case && fold(a) == fold(b)
    }

    fn in_class(&self, r: &[(u16, u16)], c: u16) -> bool {
        r.iter().any(|&(a, b)| (a..=b).contains(&c))
            || self.re.ignore_case && {
                let f = fold(c);
                let up = char::from_u32(c as u32)
                    .and_then(|ch| ch.to_uppercase().next())
                    .map_or(c, |u| u as u16);
                r.iter()
                    .any(|&(a, b)| (a..=b).contains(&f) || (a..=b).contains(&up))
            }
    }

    fn node(&mut self, n: &Node, i: usize, k: &mut K) -> Option<usize> {
        *self.steps += 1;
        if *self.steps > 50_000_000 {
            return None;
        }
        let s = self.s;
        match n {
            Node::Char(c) => (i < s.len() && self.eq(s[i], *c))
                .then(|| k(self, i + 1))
                .flatten(),
            Node::Any => (i < s.len()
                && (self.re.dot_all || !matches!(s[i], 10 | 13 | 0x2028 | 0x2029)))
            .then(|| k(self, i + 1))
            .flatten(),
            Node::Class(r, neg) => (i < s.len() && self.in_class(r, s[i]) != *neg)
                .then(|| k(self, i + 1))
                .flatten(),
            Node::Start => (i == 0
                || self.re.multiline && matches!(s[i - 1], 10 | 13 | 0x2028 | 0x2029))
            .then(|| k(self, i))
            .flatten(),
            Node::End => (i == s.len()
                || self.re.multiline && matches!(s[i], 10 | 13 | 0x2028 | 0x2029))
            .then(|| k(self, i))
            .flatten(),
            Node::WordB(want) => {
                let b = (i > 0 && self.is_word(i - 1)) != self.is_word(i);
                (b == *want).then(|| k(self, i)).flatten()
            }
            Node::Backref(g) => {
                let Some(Some((a, b))) = self.caps.get(*g).copied() else {
                    return k(self, i);
                };
                let len = b - a;
                (i + len <= s.len() && (0..len).all(|j| self.eq(s[a + j], s[i + j])))
                    .then(|| k(self, i + len))
                    .flatten()
            }
            Node::Group(x, None) => self.node(x, i, k),
            Node::Group(x, Some(g)) => {
                let g = *g;
                let old = self.caps[g];
                let r = self.node(x, i, &mut |m: &mut M, e| {
                    let prev = m.caps[g];
                    m.caps[g] = Some((i, e));
                    let r = k(m, e);
                    if r.is_none() {
                        m.caps[g] = prev;
                    }
                    r
                });
                if r.is_none() {
                    self.caps[g] = old;
                }
                r
            }
            Node::Look(x, neg, behind) => {
                let saved = self.caps.clone();
                let found = if *behind {
                    (0..=i).rev().any(|st| {
                        self.node(x, st, &mut |_, e| (e == i).then_some(e))
                            .is_some()
                    })
                } else {
                    self.node(x, i, &mut |_, e| Some(e)).is_some()
                };
                if found == *neg {
                    *self.caps = saved;
                    return None;
                }
                if *neg {
                    *self.caps = saved;
                }
                k(self, i)
            }
            Node::Alt(v) => {
                for x in v {
                    if let Some(r) = self.node(x, i, k) {
                        return Some(r);
                    }
                }
                None
            }
            Node::Seq(v) => self.seq(v, i, k),
            Node::Repeat(x, min, max, greedy) => {
                // A single-unit atom repeated: count matches, then back off.
                if let Node::Char(_) | Node::Any | Node::Class(..) = **x {
                    let mut n = 0u32;
                    let mut j = i;
                    while n < *max && j < s.len() && self.unit(x, s[j]) {
                        n += 1;
                        j += 1;
                    }
                    if n < *min {
                        return None;
                    }
                    if *greedy {
                        for c in (*min..=n).rev() {
                            if let Some(r) = k(self, i + c as usize) {
                                return Some(r);
                            }
                        }
                    } else {
                        for c in *min..=n {
                            if let Some(r) = k(self, i + c as usize) {
                                return Some(r);
                            }
                        }
                    }
                    return None;
                }
                self.rep(x, *min, *max, *greedy, 0, i, usize::MAX, k)
            }
        }
    }

    fn unit(&self, n: &Node, c: u16) -> bool {
        match n {
            Node::Char(x) => self.eq(c, *x),
            Node::Any => self.re.dot_all || !matches!(c, 10 | 13 | 0x2028 | 0x2029),
            Node::Class(r, neg) => self.in_class(r, c) != *neg,
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(
        &mut self,
        x: &Node,
        min: u32,
        max: u32,
        greedy: bool,
        count: u32,
        i: usize,
        last: usize,
        k: &mut K,
    ) -> Option<usize> {
        if count >= min && i == last {
            // An empty iteration ends the loop.
            return None;
        }
        let try_more = |m: &mut M, k: &mut K| -> Option<usize> {
            if count >= max {
                return None;
            }
            m.node(x, i, &mut |m: &mut M, e| {
                m.rep(x, min, max, greedy, count + 1, e, i, k)
            })
        };
        if count < min {
            return try_more(self, k);
        }
        if greedy {
            try_more(self, k).or_else(|| k(self, i))
        } else {
            k(self, i).or_else(|| try_more(self, k))
        }
    }

    fn seq(&mut self, v: &[Node], i: usize, k: &mut K) -> Option<usize> {
        match v {
            [] => k(self, i),
            [only] => self.node(only, i, k),
            [first, rest @ ..] => self.node(first, i, &mut |m: &mut M, e| m.seq(rest, e, k)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(re: &str, flags: &str, s: &str) -> Option<String> {
        let r = Regex::new(re, flags).unwrap();
        let u: Vec<u16> = s.encode_utf16().collect();
        r.exec(&u, 0).map(|c| {
            let (a, b) = c[0].unwrap();
            String::from_utf16_lossy(&u[a..b])
        })
    }

    #[test]
    fn matches() {
        assert_eq!(
            find(r"\[Channel(\d+)\]", "", "x[Channel12]").as_deref(),
            Some("[Channel12]")
        );
        assert_eq!(find("a+?b", "", "aaab").as_deref(), Some("aaab"));
        assert_eq!(
            find("(a|ab)(c|bcd)(d*)", "", "abcd").as_deref(),
            Some("abcd")
        );
        assert_eq!(find("^\\s*$", "m", "x\n  \ny").as_deref(), Some("  "));
        assert_eq!(find("HELLO", "i", "say hello").as_deref(), Some("hello"));
        assert_eq!(
            find("(?<q>['\"]).*?\\k<q>", "", "x 'a\"b' y").as_deref(),
            Some("'a\"b'")
        );
        assert_eq!(find("\\bfoo\\b", "", "foobar foo").as_deref(), Some("foo"));
        assert_eq!(find("(?<=\\$)\\d+", "", "cost $42").as_deref(), Some("42"));
        assert_eq!(find("x(?!y)", "", "xyxz").as_deref(), Some("x"));
        assert_eq!(find("[^a-c\\d]+", "", "ab12xyz").as_deref(), Some("xyz"));
        assert_eq!(find("(a*)*b", "", "aaaab").as_deref(), Some("aaaab"));
        assert_eq!(find("a{2,3}", "", "aaaa").as_deref(), Some("aaa"));
        assert_eq!(find("(\\w)\\1", "", "abccd").as_deref(), Some("cc"));
        assert!(find("a{2}", "", "a").is_none());
        assert!(Regex::new("(", "").is_err());
    }
}
