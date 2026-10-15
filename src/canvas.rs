//! Renderer-independent world geometry and a bounded-cell spatial index.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub type Point = [f64; 2];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    Select,
    #[default]
    Pen,
    Marker,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Eraser,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub tool: Tool,
    /// World coordinates, Y up. Width is also measured in world units.
    pub points: Vec<Point>,
    pub color: [u8; 4],
    pub width: f64,
    pub created_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn intersects(self, other: Self) -> bool {
        self.min[0] <= other.max[0]
            && self.max[0] >= other.min[0]
            && self.min[1] <= other.max[1]
            && self.max[1] >= other.min[1]
    }
    pub fn contains(self, p: Point) -> bool {
        p[0] >= self.min[0] && p[0] <= self.max[0] && p[1] >= self.min[1] && p[1] <= self.max[1]
    }
    pub fn expanded(self, amount: f64) -> Self {
        Self {
            min: [self.min[0] - amount, self.min[1] - amount],
            max: [self.max[0] + amount, self.max[1] + amount],
        }
    }
}
impl Object {
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds {
            min: self.points[0],
            max: self.points[0],
        };
        for p in &self.points {
            for (axis, value) in p.iter().enumerate() {
                b.min[axis] = b.min[axis].min(*value);
                b.max[axis] = b.max[axis].max(*value);
            }
        }
        b.expanded(if self.tool == Tool::Arrow {
            self.width * 4.0 + 12.0
        } else {
            self.width / 2.0
        })
    }
    pub fn translated(&self, delta: Point) -> Self {
        let mut object = self.clone();
        for p in &mut object.points {
            p[0] += delta[0];
            p[1] += delta[1];
        }
        object
    }
    pub fn validate(&self) -> bool {
        self.id > 0
            && self.id < u64::MAX
            && self.width.is_finite()
            && (0.1..=1000.0).contains(&self.width)
            && !self.points.is_empty()
            && self.points.len() <= 200_000
            && self
                .points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() <= 1e9)
            && !matches!(self.tool, Tool::Select | Tool::Eraser)
            && (matches!(self.tool, Tool::Pen | Tool::Marker) || self.points.len() == 2)
    }
}
const CELL: f64 = 1024.0;
fn cells(b: Bounds) -> Option<Vec<(i64, i64)>> {
    let x0 = (b.min[0] / CELL).floor() as i64;
    let x1 = (b.max[0] / CELL).floor() as i64;
    let y0 = (b.min[1] / CELL).floor() as i64;
    let y1 = (b.max[1] / CELL).floor() as i64;
    if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > 4096 {
        return None;
    }
    Some(
        (x0..=x1)
            .flat_map(|x| (y0..=y1).map(move |y| (x, y)))
            .collect(),
    )
}
#[derive(Default)]
pub struct Scene {
    pub objects: BTreeMap<u64, Object>,
    grid: HashMap<(i64, i64), BTreeSet<u64>>,
    large: BTreeSet<u64>,
}
impl Scene {
    pub fn insert(&mut self, object: Object) {
        self.remove(object.id);
        if let Some(cells) = cells(object.bounds()) {
            for cell in cells {
                self.grid.entry(cell).or_default().insert(object.id);
            }
        } else {
            self.large.insert(object.id);
        }
        self.objects.insert(object.id, object);
    }
    pub fn remove(&mut self, id: u64) -> Option<Object> {
        let object = self.objects.remove(&id)?;
        if let Some(cells) = cells(object.bounds()) {
            for cell in cells {
                if let Some(ids) = self.grid.get_mut(&cell) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        self.grid.remove(&cell);
                    }
                }
            }
        }
        self.large.remove(&id);
        Some(object)
    }
    pub fn visible(&self, b: Bounds) -> Vec<&Object> {
        let mut ids = self.large.clone();
        if let Some(cells) = cells(b) {
            for cell in cells {
                if let Some(found) = self.grid.get(&cell) {
                    ids.extend(found);
                }
            }
        } else {
            ids.extend(self.objects.keys());
        }
        ids.iter()
            .filter_map(|id| self.objects.get(id))
            .filter(|o| o.bounds().intersects(b))
            .collect()
    }
    pub fn hit(&self, p: Point, tolerance: f64) -> Option<u64> {
        self.visible(Bounds { min: p, max: p }.expanded(tolerance))
            .into_iter()
            .rev()
            .find(|o| hit_object(o, p, tolerance))
            .map(|o| o.id)
    }
}
fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = d[0] * d[0] + d[1] * d[1];
    let t = if len == 0.0 {
        0.0
    } else {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len
    }
    .clamp(0.0, 1.0);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}
pub fn hit_object(o: &Object, p: Point, tolerance: f64) -> bool {
    let t = tolerance + o.width / 2.0;
    if o.points.len() == 1 {
        return segment_distance(p, o.points[0], o.points[0]) <= t;
    }
    let a = o.points[0];
    let b = *o.points.last().expect("nonempty object");
    match o.tool {
        Tool::Rectangle => {
            let corners = [a, [b[0], a[1]], b, [a[0], b[1]], a];
            corners
                .windows(2)
                .any(|s| segment_distance(p, s[0], s[1]) <= t)
        }
        Tool::Ellipse => {
            let rx = (b[0] - a[0]).abs() / 2.0;
            let ry = (b[1] - a[1]).abs() / 2.0;
            if rx < 0.001 || ry < 0.001 {
                return segment_distance(p, a, b) <= t;
            }
            let dx = p[0] - (a[0] + b[0]) / 2.0;
            let dy = p[1] - (a[1] + b[1]) / 2.0;
            ((dx / rx).hypot(dy / ry) - 1.0).abs() * rx.min(ry) <= t
        }
        _ => o
            .points
            .windows(2)
            .any(|s| segment_distance(p, s[0], s[1]) <= t),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn object(id: u64, x: f64) -> Object {
        Object {
            id,
            tool: Tool::Pen,
            points: vec![[x, 0.0], [x + 10.0, 10.0]],
            color: [255; 4],
            width: 2.0,
            created_ms: 0,
        }
    }
    #[test]
    fn index_culls_and_removes_without_duplicates() {
        let mut s = Scene::default();
        for id in 1..=1000 {
            s.insert(object(id, id as f64 * 2048.0));
        }
        let b = Bounds {
            min: [2040.0, -10.0],
            max: [2060.0, 20.0],
        };
        assert_eq!(
            s.visible(b).iter().map(|o| o.id).collect::<Vec<_>>(),
            vec![1]
        );
        s.remove(1);
        assert!(s.visible(b).is_empty());
        s.insert(object(2, -50.0));
        assert_eq!(s.hit([-45.0, 5.0], 1.0), Some(2));
    }
    #[test]
    fn huge_object_does_not_allocate_unbounded_grid() {
        let mut s = Scene::default();
        let mut o = object(1, -1e8);
        o.points.push([1e8, 1e8]);
        s.insert(o);
        assert!(s.grid.is_empty());
        assert_eq!(s.large.len(), 1);
    }
}
