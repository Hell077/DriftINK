//! Reversible edits and pointer gestures, independent of Wayland and rendering.
use crate::{
    canvas::{Object, Point, Scene, Tool},
    storage::{Document, timestamp},
};
use std::collections::BTreeSet;
#[derive(Clone)]
pub struct Edit {
    pub before: Vec<Object>,
    pub after: Vec<Object>,
}
enum Gesture {
    Draw(Object),
    Move {
        start: Point,
        delta: Point,
        originals: Vec<Object>,
    },
    Erase(BTreeSet<u64>),
}
pub struct Editor {
    pub scene: Scene,
    pub document_id: String,
    pub tool: Tool,
    pub color: [u8; 4],
    pub width: f64,
    pub selected: BTreeSet<u64>,
    pub dirty: bool,
    next_id: u64,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    gesture: Option<Gesture>,
}
impl Editor {
    pub fn new(document: Document) -> Self {
        let next_id = document.objects.iter().map(|o| o.id).max().unwrap_or(0) + 1;
        Self {
            scene: document.scene(),
            document_id: document.document_id,
            tool: Tool::Pen,
            color: [203, 166, 247, 255],
            width: 5.0,
            selected: BTreeSet::new(),
            dirty: false,
            next_id,
            undo: vec![],
            redo: vec![],
            gesture: None,
        }
    }
    pub fn document(&self) -> Document {
        Document {
            schema_version: 1,
            document_id: self.document_id.clone(),
            objects: self.scene.objects.values().cloned().collect(),
        }
    }
    fn apply(&mut self, edit: &Edit, forward: bool) {
        let (remove, insert) = if forward {
            (&edit.before, &edit.after)
        } else {
            (&edit.after, &edit.before)
        };
        for o in remove {
            self.scene.remove(o.id);
        }
        for o in insert {
            self.scene.insert(o.clone());
        }
        self.selected
            .retain(|id| self.scene.objects.contains_key(id));
        self.dirty = true;
    }
    pub fn commit(&mut self, edit: Edit) {
        if edit.before == edit.after {
            return;
        }
        self.apply(&edit, true);
        self.undo.push(edit);
        self.redo.clear();
        if self.undo.len() > 128 {
            self.undo.remove(0);
        }
    }
    pub fn undo(&mut self) {
        self.cancel();
        if let Some(e) = self.undo.pop() {
            self.apply(&e, false);
            self.redo.push(e);
        }
    }
    pub fn redo(&mut self) {
        self.cancel();
        if let Some(e) = self.redo.pop() {
            self.apply(&e, true);
            self.undo.push(e);
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn active(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn cancel(&mut self) {
        self.gesture = None;
    }
    pub fn select_tool(&mut self, tool: Tool) {
        self.cancel();
        self.tool = tool;
        self.selected.clear();
    }
    pub fn delete_selected(&mut self) {
        self.cancel();
        let before = self
            .selected
            .iter()
            .filter_map(|id| self.scene.objects.get(id).cloned())
            .collect();
        self.commit(Edit {
            before,
            after: vec![],
        });
        self.selected.clear();
    }
    pub fn begin(&mut self, p: Point, tolerance: f64, extend: bool) {
        self.cancel();
        match self.tool {
            Tool::Select => {
                let hit = self.scene.hit(p, tolerance);
                if let Some(id) = hit {
                    if !self.selected.contains(&id) {
                        if !extend {
                            self.selected.clear();
                        }
                        self.selected.insert(id);
                    }
                    self.gesture = Some(Gesture::Move {
                        start: p,
                        delta: [0.0, 0.0],
                        originals: self
                            .selected
                            .iter()
                            .filter_map(|id| self.scene.objects.get(id).cloned())
                            .collect(),
                    });
                } else if !extend {
                    self.selected.clear();
                }
            }
            Tool::Eraser => {
                let mut ids = BTreeSet::new();
                if let Some(id) = self.scene.hit(p, tolerance + 8.0) {
                    ids.insert(id);
                }
                self.gesture = Some(Gesture::Erase(ids));
            }
            tool => {
                let mut color = self.color;
                if tool == Tool::Marker {
                    color[3] = 90;
                }
                let points = if matches!(tool, Tool::Pen | Tool::Marker) {
                    vec![p]
                } else {
                    vec![p, p]
                };
                let width = if tool == Tool::Marker {
                    self.width * 4.0
                } else {
                    self.width
                };
                self.gesture = Some(Gesture::Draw(Object {
                    id: self.next_id,
                    tool,
                    points,
                    color,
                    width,
                    created_ms: timestamp(),
                }));
                self.selected.clear();
            }
        }
    }
    pub fn motion(&mut self, p: Point, tolerance: f64) {
        match &mut self.gesture {
            Some(Gesture::Draw(object)) => {
                if matches!(object.tool, Tool::Pen | Tool::Marker) {
                    let last = *object.points.last().expect("gesture has points");
                    if (p[0] - last[0]).hypot(p[1] - last[1]) >= tolerance / 4.0
                        && object.points.len() < 200_000
                    {
                        object.points.push(p);
                    }
                } else {
                    object.points[1] = p;
                }
            }
            Some(Gesture::Move { start, delta, .. }) => *delta = [p[0] - start[0], p[1] - start[1]],
            Some(Gesture::Erase(ids)) => {
                if let Some(id) = self.scene.hit(p, tolerance + 8.0) {
                    ids.insert(id);
                }
            }
            None => {}
        }
    }
    pub fn finish(&mut self, p: Point, tolerance: f64) {
        self.motion(p, tolerance);
        if let Some(gesture) = self.gesture.take() {
            match gesture {
                Gesture::Draw(o) => {
                    if o.validate() {
                        self.next_id += 1;
                        self.commit(Edit {
                            before: vec![],
                            after: vec![o],
                        });
                    }
                }
                Gesture::Move {
                    originals, delta, ..
                } => {
                    let after: Vec<_> = originals.iter().map(|o| o.translated(delta)).collect();
                    if after.iter().all(Object::validate) {
                        self.commit(Edit {
                            before: originals,
                            after,
                        });
                    }
                }
                Gesture::Erase(ids) => {
                    let before = ids
                        .iter()
                        .filter_map(|id| self.scene.objects.get(id).cloned())
                        .collect();
                    self.commit(Edit {
                        before,
                        after: vec![],
                    });
                }
            }
        }
    }
    pub fn visit_preview(&self, mut visit: impl FnMut(&Object, Point)) {
        match &self.gesture {
            Some(Gesture::Draw(o)) => visit(o, [0.0, 0.0]),
            Some(Gesture::Move {
                originals, delta, ..
            }) => {
                for o in originals {
                    visit(o, *delta);
                }
            }
            _ => {}
        }
    }
    pub fn preview(&self) -> Vec<Object> {
        match &self.gesture {
            Some(Gesture::Draw(o)) => vec![o.clone()],
            Some(Gesture::Move {
                originals, delta, ..
            }) => originals.iter().map(|o| o.translated(*delta)).collect(),
            _ => vec![],
        }
    }
    pub fn hidden(&self) -> BTreeSet<u64> {
        match &self.gesture {
            Some(Gesture::Move { originals, .. }) => originals.iter().map(|o| o.id).collect(),
            Some(Gesture::Erase(ids)) => ids.clone(),
            _ => BTreeSet::new(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gestures_commit_once_and_cancel_is_lossless() {
        let mut e = Editor::new(Document::empty());
        e.begin([0.0, 0.0], 1.0, false);
        e.motion([10.0, 10.0], 1.0);
        e.finish([20.0, 20.0], 1.0);
        assert_eq!(e.scene.objects.len(), 1);
        assert_eq!(e.scene.objects[&1].points.len(), 3);
        e.undo();
        assert!(e.scene.objects.is_empty());
        e.redo();
        assert_eq!(e.scene.objects.len(), 1);
        e.select_tool(Tool::Arrow);
        e.begin([0.0, 0.0], 1.0, false);
        e.cancel();
        assert_eq!(e.scene.objects.len(), 1);
        e.select_tool(Tool::Eraser);
        e.begin([10.0, 10.0], 1.0, false);
        e.finish([10.0, 10.0], 1.0);
        assert!(e.scene.objects.is_empty());
        e.undo();
        assert_eq!(e.scene.objects.len(), 1);
    }
    #[test]
    fn selection_moves_world_geometry_and_undo_restores_it() {
        let mut e = Editor::new(Document::empty());
        e.begin([0.0, 0.0], 1.0, false);
        e.finish([20.0, 20.0], 1.0);
        let original = e.scene.objects[&1].clone();
        e.select_tool(Tool::Select);
        e.begin([10.0, 10.0], 1.0, false);
        e.finish([110.0, 210.0], 1.0);
        assert_eq!(e.scene.objects[&1].points[0], [100.0, 200.0]);
        e.undo();
        assert_eq!(e.scene.objects[&1], original);
    }
    #[test]
    fn deleting_during_a_move_cannot_resurrect_the_object_on_release() {
        let mut e = Editor::new(Document::empty());
        e.begin([0.0, 0.0], 1.0, false);
        e.finish([20.0, 20.0], 1.0);
        e.select_tool(Tool::Select);
        e.begin([10.0, 10.0], 1.0, false);
        e.motion([30.0, 30.0], 1.0);
        e.delete_selected();
        e.finish([30.0, 30.0], 1.0);
        assert!(e.scene.objects.is_empty());
        e.undo();
        assert_eq!(e.scene.objects[&1].points[0], [0.0, 0.0]);
    }
}
