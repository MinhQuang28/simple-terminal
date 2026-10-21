use eframe::egui::{Rect, pos2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::SplitDir;

/// Direction to move focus between panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// Binary-or-wider split tree of a tab's panes; sizes are fractions of the parent split.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PaneLayout {
    Leaf(Uuid),
    Split { dir: SplitDir, children: Vec<PaneLayout>, sizes: Vec<f32> },
}

/// A draggable divider between two children of a split.
pub struct Divider {
    pub rect: Rect,
    pub dir: SplitDir,
    /// Child indices from the root down to the split that owns the divider.
    pub path: Vec<usize>,
    /// The divider sits after this child.
    pub index: usize,
}

impl PaneLayout {
    pub fn from_flat(panes: &[Uuid], dir: SplitDir) -> Self {
        match panes {
            [single] => Self::Leaf(*single),
            _ => Self::Split {
                dir,
                children: panes.iter().map(|p| Self::Leaf(*p)).collect(),
                sizes: vec![1.0 / panes.len().max(1) as f32; panes.len()],
            },
        }
    }

    #[cfg(test)]
    pub fn leaves(&self) -> Vec<Uuid> {
        match self {
            Self::Leaf(id) => vec![*id],
            Self::Split { children, .. } => children.iter().flat_map(Self::leaves).collect(),
        }
    }

    /// Puts `new` next to `target` in direction `dir`; joins an existing split in that direction.
    pub fn split(&mut self, target: Uuid, dir: SplitDir, new: Uuid) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                *self = Self::Split {
                    dir,
                    children: vec![Self::Leaf(target), Self::Leaf(new)],
                    sizes: vec![0.5; 2],
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { dir: own, children, sizes } => {
                let at = children.iter().position(|c| matches!(c, Self::Leaf(id) if *id == target));
                match at {
                    Some(i) if *own == dir => {
                        let share = sizes[i] / 2.0;
                        sizes[i] = share;
                        sizes.insert(i + 1, share);
                        children.insert(i + 1, Self::Leaf(new));
                        true
                    }
                    _ => children.iter_mut().any(|c| c.split(target, dir, new)),
                }
            }
        }
    }

    /// Removes a pane, giving its space to its siblings; returns whether it was found.
    pub fn remove(&mut self, id: Uuid) -> bool {
        let Self::Split { children, sizes, .. } = self else { return false };
        if let Some(i) = children.iter().position(|c| matches!(c, Self::Leaf(leaf) if *leaf == id)) {
            children.remove(i);
            let freed = sizes.remove(i);
            let total: f32 = sizes.iter().sum();
            for size in sizes.iter_mut() {
                *size += freed * (*size / total.max(f32::EPSILON));
            }
            if children.len() == 1 {
                *self = children.remove(0);
            }
            return true;
        }
        children.iter_mut().any(|c| c.remove(id))
    }

    /// Screen rect of every pane, with `gap` points between siblings.
    pub fn rects(&self, area: Rect, gap: f32) -> Vec<(Uuid, Rect)> {
        let mut out = Vec::new();
        self.walk(area, gap, &mut Vec::new(), &mut out, &mut Vec::new());
        out
    }

    pub fn dividers(&self, area: Rect, gap: f32) -> Vec<Divider> {
        let mut dividers = Vec::new();
        self.walk(area, gap, &mut Vec::new(), &mut Vec::new(), &mut dividers);
        dividers
    }

    fn walk(
        &self,
        area: Rect,
        gap: f32,
        path: &mut Vec<usize>,
        out: &mut Vec<(Uuid, Rect)>,
        dividers: &mut Vec<Divider>,
    ) {
        let Self::Split { dir, children, sizes } = self else {
            if let Self::Leaf(id) = self {
                out.push((*id, area));
            }
            return;
        };
        let total = sizes.iter().sum::<f32>().max(f32::EPSILON);
        let extent = match dir {
            SplitDir::Right => area.width(),
            SplitDir::Down => area.height(),
        } - gap * (children.len() - 1) as f32;
        let mut offset = 0.0;
        for (i, (child, size)) in children.iter().zip(sizes).enumerate() {
            let length = extent * size / total;
            let rect = match dir {
                SplitDir::Right => Rect::from_min_max(
                    pos2(area.left() + offset, area.top()),
                    pos2(area.left() + offset + length, area.bottom()),
                ),
                SplitDir::Down => Rect::from_min_max(
                    pos2(area.left(), area.top() + offset),
                    pos2(area.right(), area.top() + offset + length),
                ),
            };
            path.push(i);
            child.walk(rect, gap, path, out, dividers);
            path.pop();
            offset += length + gap;
            if i + 1 < children.len() {
                let divider = match dir {
                    SplitDir::Right => Rect::from_min_max(
                        pos2(rect.right() - 2.0, rect.top()),
                        pos2(rect.right() + gap + 2.0, rect.bottom()),
                    ),
                    SplitDir::Down => Rect::from_min_max(
                        pos2(rect.left(), rect.bottom() - 2.0),
                        pos2(rect.right(), rect.bottom() + gap + 2.0),
                    ),
                };
                dividers.push(Divider { rect: divider, dir: *dir, path: path.clone(), index: i });
            }
        }
    }

    /// Moves the divider after `index` in the split at `path` by `delta`, a fraction of the split.
    pub fn resize(&mut self, path: &[usize], index: usize, delta: f32) {
        let Self::Split { children, sizes, .. } = self else { return };
        match path.split_first() {
            Some((first, rest)) => {
                if let Some(child) = children.get_mut(*first) {
                    child.resize(rest, index, delta);
                }
            }
            None => {
                if index + 1 >= sizes.len() {
                    return;
                }
                let total: f32 = sizes.iter().sum();
                let min = total * 0.1;
                let delta = delta.clamp(min - sizes[index], sizes[index + 1] - min);
                sizes[index] += delta;
                sizes[index + 1] -= delta;
            }
        }
    }

    /// The pane beside `from` in direction `nav`, judged by geometry.
    pub fn neighbor(&self, from: Uuid, nav: Nav) -> Option<Uuid> {
        let rects = self.rects(Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 1000.0)), 0.0);
        let (_, origin) = rects.iter().find(|(id, _)| *id == from)?;
        rects
            .iter()
            .filter(|(id, _)| *id != from)
            .filter(|(_, r)| match nav {
                Nav::Left => r.right() <= origin.left() + 0.5 && r.y_range().intersects(origin.y_range()),
                Nav::Right => r.left() >= origin.right() - 0.5 && r.y_range().intersects(origin.y_range()),
                Nav::Up => r.bottom() <= origin.top() + 0.5 && r.x_range().intersects(origin.x_range()),
                Nav::Down => r.top() >= origin.bottom() - 0.5 && r.x_range().intersects(origin.x_range()),
            })
            .min_by(|(_, a), (_, b)| {
                let distance = |r: &Rect| match nav {
                    Nav::Left => origin.left() - r.right(),
                    Nav::Right => r.left() - origin.right(),
                    Nav::Up => origin.top() - r.bottom(),
                    Nav::Down => r.top() - origin.bottom(),
                };
                distance(a).total_cmp(&distance(b))
            })
            .map(|(id, _)| *id)
    }
}

#[cfg(test)]
#[path = "pane_layout_tests.rs"]
mod tests;
