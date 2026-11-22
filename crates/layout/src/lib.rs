//! Fluid layout: a tree of modules, each with a minimum size, a growth weight and a priority.
//! [`solve`] places them in any rectangle. A group lays its children along whichever axis fits
//! (wide windows get a row, tall ones a column). When even that does not fit, the group hides
//! its lowest-priority modules until the rest fit at their minimum sizes. There are no fixed
//! breakpoints: the same tree gives a phone in portrait and a 4K desktop.

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

impl Rect {
    pub fn contains(&self, o: &Rect) -> bool {
        const E: f32 = 0.01;
        o.x >= self.x - E
            && o.y >= self.y - E
            && o.x + o.w <= self.x + self.w + E
            && o.y + o.h <= self.y + self.h + E
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        const E: f32 = 0.01;
        self.x + E < o.x + o.w
            && o.x + E < self.x + self.w
            && self.y + E < o.y + o.h
            && o.y + E < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
    /// Row when the rectangle is wider than `ratio` times its height, else column; the other
    /// axis is tried when the first does not fit.
    Auto,
}

#[derive(Clone, Debug)]
pub enum Node {
    Module {
        id: u32,
        min: Size,
        /// Share of the spare space along the group's axis.
        grow: f32,
        /// Upper bound along the group's axis.
        max: f32,
        /// Higher stays visible longer.
        priority: u8,
    },
    Group {
        axis: Axis,
        gap: f32,
        children: Vec<Node>,
        priority: u8,
    },
}

impl Node {
    pub fn module(id: u32, min_w: f32, min_h: f32, grow: f32, priority: u8) -> Self {
        Node::Module {
            id,
            min: Size { w: min_w, h: min_h },
            grow,
            max: f32::INFINITY,
            priority,
        }
    }

    pub fn group(axis: Axis, gap: f32, priority: u8, children: Vec<Node>) -> Self {
        Node::Group {
            axis,
            gap,
            children,
            priority,
        }
    }

    fn grow(&self) -> f32 {
        match self {
            Node::Module { grow, .. } => *grow,
            Node::Group { children, .. } => {
                children.iter().map(Node::grow).fold(0.0, f32::max).max(1.0)
            }
        }
    }

    fn max_along(&self, axis: Axis) -> f32 {
        match (self, axis) {
            (Node::Module { max, .. }, _) => *max,
            _ => f32::INFINITY,
        }
    }

    /// Smallest size this node can take with every child it keeps, given its hidden set.
    fn min_size(&self, hidden: &[u32]) -> Option<Size> {
        match self {
            Node::Module { id, min, .. } => (!hidden.contains(id)).then_some(*min),
            Node::Group {
                axis,
                gap,
                children,
                ..
            } => {
                let mins: Vec<Size> = children.iter().filter_map(|c| c.min_size(hidden)).collect();
                if mins.is_empty() {
                    return None;
                }
                let gaps = gap * (mins.len() - 1) as f32;
                let row = Size {
                    w: mins.iter().map(|s| s.w).sum::<f32>() + gaps,
                    h: mins.iter().map(|s| s.h).fold(0.0, f32::max),
                };
                let col = Size {
                    w: mins.iter().map(|s| s.w).fold(0.0, f32::max),
                    h: mins.iter().map(|s| s.h).sum::<f32>() + gaps,
                };
                Some(match axis {
                    Axis::Row => row,
                    Axis::Column => col,
                    // Either arrangement may be chosen; report the one with the smaller area.
                    Axis::Auto => {
                        if row.w * row.h <= col.w * col.h {
                            row
                        } else {
                            col
                        }
                    }
                })
            }
        }
    }

    fn ids(&self, out: &mut Vec<(u8, u32)>, inherited: u8) {
        match self {
            Node::Module { id, priority, .. } => out.push(((*priority).min(inherited), *id)),
            Node::Group {
                children, priority, ..
            } => children
                .iter()
                .for_each(|c| c.ids(out, (*priority).min(inherited))),
        }
    }
}

/// Aspect ratio above which an [`Axis::Auto`] group starts as a row.
pub const AUTO_ROW_RATIO: f32 = 1.2;

/// Places `root` in `area`. Returns the visible modules and their rectangles; hidden modules are
/// absent. Rectangles never overlap and never leave `area`.
pub fn solve(root: &Node, area: Rect) -> Vec<(u32, Rect)> {
    let mut order = Vec::new();
    root.ids(&mut order, u8::MAX);
    // Lowest priority first; later ids lose ties so the tree order is the tie-break.
    order.sort_by_key(|&(p, id)| (p, std::cmp::Reverse(id)));
    let mut hidden = Vec::new();
    for &(_, id) in order.iter() {
        let mut out = Vec::new();
        if place(root, area, &hidden, &mut out) {
            return out;
        }
        hidden.push(id);
    }
    Vec::new()
}

fn fits(min: Size, area: Rect) -> bool {
    min.w <= area.w + 0.01 && min.h <= area.h + 0.01
}

fn place(node: &Node, area: Rect, hidden: &[u32], out: &mut Vec<(u32, Rect)>) -> bool {
    match node {
        Node::Module { id, min, .. } => {
            if hidden.contains(id) {
                return true;
            }
            if !fits(*min, area) {
                return false;
            }
            out.push((*id, area));
            true
        }
        Node::Group {
            axis,
            gap,
            children,
            ..
        } => {
            let axes: &[Axis] = match axis {
                Axis::Auto if area.w > area.h * AUTO_ROW_RATIO => &[Axis::Row, Axis::Column],
                Axis::Auto => &[Axis::Column, Axis::Row],
                Axis::Row => &[Axis::Row],
                Axis::Column => &[Axis::Column],
            };
            axes.iter().any(|&a| {
                let mark = out.len();
                let ok = place_line(a, *gap, children, area, hidden, out);
                if !ok {
                    out.truncate(mark);
                }
                ok
            })
        }
    }
}

fn place_line(
    axis: Axis,
    gap: f32,
    children: &[Node],
    area: Rect,
    hidden: &[u32],
    out: &mut Vec<(u32, Rect)>,
) -> bool {
    let row = axis == Axis::Row;
    let kids: Vec<(&Node, Size)> = children
        .iter()
        .filter_map(|c| c.min_size(hidden).map(|m| (c, m)))
        .collect();
    if kids.is_empty() {
        return true;
    }
    let (main, cross) = if row {
        (area.w, area.h)
    } else {
        (area.h, area.w)
    };
    let along = |s: Size| if row { s.w } else { s.h };
    let across = |s: Size| if row { s.h } else { s.w };
    let gaps = gap * (kids.len() - 1) as f32;
    if kids.iter().any(|(_, m)| across(*m) > cross + 0.01) {
        return false;
    }
    let mut sizes: Vec<f32> = kids.iter().map(|(_, m)| along(*m)).collect();
    let mut spare = main - gaps - sizes.iter().sum::<f32>();
    if spare < -0.01 {
        return false;
    }
    // Hand out spare space by weight; children that reach their maximum drop out and the rest
    // share what is left.
    let mut open: Vec<usize> = (0..kids.len())
        .filter(|&i| kids[i].0.grow() > 0.0)
        .collect();
    while spare > 0.01 && !open.is_empty() {
        let total: f32 = open.iter().map(|&i| kids[i].0.grow()).sum();
        let mut used = 0.0;
        open.retain(|&i| {
            let cap = kids[i].0.max_along(axis);
            let add = (spare * kids[i].0.grow() / total).min(cap - sizes[i]);
            sizes[i] += add;
            used += add;
            sizes[i] < cap - 0.01
        });
        spare -= used;
        if used <= 0.01 {
            break;
        }
    }
    // Space nobody could take centres the line.
    let mut pos = (if row { area.x } else { area.y }) + spare.max(0.0) / 2.0;
    for ((node, _), len) in kids.iter().zip(sizes) {
        let r = if row {
            Rect {
                x: pos,
                y: area.y,
                w: len,
                h: area.h,
            }
        } else {
            Rect {
                x: area.x,
                y: pos,
                w: area.w,
                h: len,
            }
        };
        if !place(node, r, hidden, out) {
            return false;
        }
        pos += len + gap;
    }
    true
}

#[cfg(test)]
mod tests;
