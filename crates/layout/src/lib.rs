#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

const EPS: f32 = 0.01;

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn hit(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x - EPS
            && o.y >= self.y - EPS
            && o.x + o.w <= self.x + self.w + EPS
            && o.y + o.h <= self.y + self.h + EPS
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x + EPS < o.x + o.w
            && o.x + EPS < self.x + self.w
            && self.y + EPS < o.y + o.h
            && o.y + EPS < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
    Auto,
}

pub type Id = u8;
pub const MAX_MODULES: usize = 64;
pub const AUTO_ROW_RATIO: f32 = 1.2;

#[derive(Clone, Debug)]
pub enum Node {
    Module { id: Id, min: Size, grow: f32, max: f32, priority: u8 },
    Group { axis: Axis, gap: f32, priority: u8, children: Vec<Node> },
}

impl Node {
    pub fn module(id: Id, min_w: f32, min_h: f32, grow: f32, priority: u8) -> Self {
        Node::Module { id, min: Size { w: min_w, h: min_h }, grow, max: f32::INFINITY, priority }
    }

    pub fn group(axis: Axis, gap: f32, priority: u8, children: Vec<Node>) -> Self {
        Node::Group { axis, gap, priority, children }
    }
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Module { id: Id, min: Size, max: f32 },
    Group { axis: Axis, gap: f32 },
}

#[derive(Clone, Copy, Debug)]
struct Flat {
    kind: Kind,
    grow: f32,
    end: u32,
}

#[derive(Debug)]
pub struct Layout {
    nodes: Box<[Flat]>,
    hide_order: Box<[Id]>,
    mins: Box<[Option<Size>]>,
    scratch: Vec<f32>,
}

impl Layout {
    pub fn new(root: &Node) -> Self {
        let mut nodes = Vec::new();
        let mut prio = Vec::new();
        flatten(root, u8::MAX, &mut nodes, &mut prio);
        prio.sort_unstable_by_key(|&(p, id)| (p, std::cmp::Reverse(id)));
        let mut seen = 0u64;
        for &(_, id) in &prio {
            assert!(usize::from(id) < MAX_MODULES, "module id {id} out of range");
            assert!(seen & (1 << id) == 0, "module id {id} used twice");
            seen |= 1 << id;
        }
        let len = nodes.len();
        Self {
            nodes: nodes.into(),
            hide_order: prio.into_iter().map(|(_, id)| id).collect(),
            mins: vec![None; len].into(),
            scratch: Vec::with_capacity(len),
        }
    }

    pub fn solve(&mut self, area: Rect, out: &mut Vec<(Id, Rect)>) -> u64 {
        let mut hidden = 0u64;
        for i in 0..=self.hide_order.len() {
            out.clear();
            self.measure(hidden);
            if self.place(0, area, out) {
                return hidden;
            }
            if let Some(&id) = self.hide_order.get(i) {
                hidden |= 1 << id;
            }
        }
        out.clear();
        hidden
    }

    fn next(&self, c: usize) -> usize {
        self.nodes[c].end as usize
    }

    fn measure(&mut self, hidden: u64) {
        for i in (0..self.nodes.len()).rev() {
            self.mins[i] = match self.nodes[i].kind {
                Kind::Module { id, min, .. } => (hidden & (1 << id) == 0).then_some(min),
                Kind::Group { axis, gap } => {
                    let (mut n, mut sw, mut sh, mut mw, mut mh) = (0u32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
                    let mut c = i + 1;
                    while c < self.next(i) {
                        if let Some(m) = self.mins[c] {
                            n += 1;
                            sw += m.w;
                            sh += m.h;
                            mw = mw.max(m.w);
                            mh = mh.max(m.h);
                        }
                        c = self.next(c);
                    }
                    (n > 0).then(|| {
                        let gaps = gap * (n - 1) as f32;
                        let row = Size { w: sw + gaps, h: mh };
                        let col = Size { w: mw, h: sh + gaps };
                        match axis {
                            Axis::Row => row,
                            Axis::Column => col,
                            Axis::Auto if row.w * row.h <= col.w * col.h => row,
                            Axis::Auto => col,
                        }
                    })
                }
            };
        }
    }

    fn place(&mut self, i: usize, area: Rect, out: &mut Vec<(Id, Rect)>) -> bool {
        let Some(min) = self.mins[i] else { return true };
        match self.nodes[i].kind {
            Kind::Module { id, .. } => {
                let ok = min.w <= area.w + EPS && min.h <= area.h + EPS;
                if ok {
                    out.push((id, area));
                }
                ok
            }
            Kind::Group { axis, gap } => {
                let first = match axis {
                    Axis::Auto if area.w > area.h * AUTO_ROW_RATIO => Axis::Row,
                    Axis::Auto => Axis::Column,
                    fixed => fixed,
                };
                if self.line(i, first, gap, area, out) {
                    return true;
                }
                let other = if first == Axis::Row { Axis::Column } else { Axis::Row };
                axis == Axis::Auto && self.line(i, other, gap, area, out)
            }
        }
    }

    fn line(&mut self, i: usize, axis: Axis, gap: f32, area: Rect, out: &mut Vec<(Id, Rect)>) -> bool {
        let row = axis == Axis::Row;
        let (main, cross) = if row { (area.w, area.h) } else { (area.h, area.w) };
        let end = self.next(i);
        let base = self.scratch.len();
        let mut used = 0.0;
        let mut weight = 0.0;
        let mut c = i + 1;
        while c < end {
            if let Some(m) = self.mins[c] {
                let (along, across) = if row { (m.w, m.h) } else { (m.h, m.w) };
                if across > cross + EPS {
                    self.scratch.truncate(base);
                    return false;
                }
                used += along;
                weight += self.nodes[c].grow;
                self.scratch.push(along);
            }
            c = self.next(c);
        }
        let n = self.scratch.len() - base;
        let mut spare = main - used - gap * n.saturating_sub(1) as f32;
        if spare < -EPS {
            self.scratch.truncate(base);
            return false;
        }

        while spare > EPS && weight > 0.0 {
            let (mut given, mut open) = (0.0, 0.0);
            let (mut c, mut k) = (i + 1, base);
            while c < end {
                if self.mins[c].is_some() {
                    let f = self.nodes[c];
                    let cap = match f.kind {
                        Kind::Module { max, .. } => max,
                        Kind::Group { .. } => f32::INFINITY,
                    };
                    let s = &mut self.scratch[k];
                    k += 1;
                    if f.grow > 0.0 && *s < cap - EPS {
                        let add = (spare * f.grow / weight).min(cap - *s);
                        *s += add;
                        given += add;
                        if *s < cap - EPS {
                            open += f.grow;
                        }
                    }
                }
                c = self.next(c);
            }
            spare -= given;
            weight = open;
            if given <= EPS {
                break;
            }
        }

        let mark = out.len();
        let mut pos = if row { area.x } else { area.y } + spare.max(0.0) * 0.5;
        let (mut c, mut k) = (i + 1, base);
        let mut ok = true;
        while c < end {
            if self.mins[c].is_some() {
                let len = self.scratch[k];
                k += 1;
                let r = if row { Rect::new(pos, area.y, len, area.h) } else { Rect::new(area.x, pos, area.w, len) };
                if !self.place(c, r, out) {
                    ok = false;
                    break;
                }
                pos += len + gap;
            }
            c = self.next(c);
        }
        self.scratch.truncate(base);
        if !ok {
            out.truncate(mark);
        }
        ok
    }
}

fn flatten(node: &Node, inherited: u8, nodes: &mut Vec<Flat>, prio: &mut Vec<(u8, Id)>) -> f32 {
    let at = nodes.len();
    match node {
        Node::Module { id, min, grow, max, priority } => {
            nodes.push(Flat { kind: Kind::Module { id: *id, min: *min, max: *max }, grow: *grow, end: 0 });
            prio.push(((*priority).min(inherited), *id));
        }
        Node::Group { axis, gap, priority, children } => {
            nodes.push(Flat { kind: Kind::Group { axis: *axis, gap: *gap }, grow: 0.0, end: 0 });
            let p = (*priority).min(inherited);
            nodes[at].grow = children.iter().map(|c| flatten(c, p, nodes, prio)).fold(1.0, f32::max);
        }
    }
    nodes[at].end = nodes.len() as u32;
    nodes[at].grow
}

#[cfg(test)]
mod tests;
