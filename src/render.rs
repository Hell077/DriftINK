use crate::{
    camera::{screen_to_world, world_to_screen},
    canvas::{Bounds, Object, Point, Tool},
    drawing::Editor,
    ipc::{Output, Result},
    ui::{self, Action},
};
use fontdue::{Font, FontSettings, Metrics};
use std::{collections::HashMap, fs};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

pub struct Renderer {
    font: Font,
    glyphs: HashMap<(char, u16), (Metrics, Vec<u8>)>,
}
impl Renderer {
    pub fn new() -> Result<Self> {
        let bytes = [
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .iter()
        .find_map(|p| fs::read(p).ok())
        .ok_or("No UI font found (install Noto Sans or DejaVu Sans)")?;
        Ok(Self {
            font: Font::from_bytes(bytes, FontSettings::default())
                .map_err(|e| format!("font: {e}"))?,
            glyphs: HashMap::new(),
        })
    }
    pub fn draw(
        &mut self,
        pixmap: &mut Pixmap,
        scale: i32,
        output: &Output,
        editor: &Editor,
        editing: bool,
        error: Option<&str>,
    ) {
        pixmap.fill(Color::TRANSPARENT);
        let transform = Transform::from_scale(scale as f32, scale as f32);
        let p0 = screen_to_world([0.0, 0.0], output);
        let p1 = screen_to_world([output.size[0] as f64, output.size[1] as f64], output);
        let bounds = Bounds {
            min: [p0[0], p1[1]],
            max: [p1[0], p0[1]],
        };
        let hidden = editor.hidden();
        for object in editor.scene.visible(bounds) {
            if !hidden.contains(&object.id) {
                draw_object(pixmap, object, output, transform, [0.0, 0.0]);
            }
        }
        editor.visit_preview(|object, delta| draw_object(pixmap, object, output, transform, delta));
        if editing {
            for id in &editor.selected {
                if let Some(o) = editor.scene.objects.get(id) {
                    let b = o.bounds();
                    let a = world_to_screen([b.min[0], b.max[1]], output);
                    let z = world_to_screen([b.max[0], b.min[1]], output);
                    outline(
                        pixmap,
                        [
                            a[0] as f32 - 4.0,
                            a[1] as f32 - 4.0,
                            (z[0] - a[0]) as f32 + 8.0,
                            (z[1] - a[1]) as f32 + 8.0,
                        ],
                        [137, 180, 250, 255],
                        1.5,
                        transform,
                    );
                }
            }
            self.sidebar(pixmap, scale, output, editor, error);
        }
    }
    fn text(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        position: [f32; 2],
        size: f32,
        color: [u8; 4],
        transform: Transform,
    ) {
        let [x, y] = position;
        let factor = transform.sx;
        let font_size = (size * factor).round() as u16;
        let mut pen = x * factor + transform.tx;
        for ch in text.chars() {
            let (metrics, coverage) = self
                .glyphs
                .entry((ch, font_size))
                .or_insert_with(|| self.font.rasterize(ch, font_size as f32));
            let left = pen.round() as i32 + metrics.xmin;
            let top =
                (y * factor + transform.ty).round() as i32 - metrics.height as i32 - metrics.ymin;
            let w = pixmap.width() as i32;
            let h = pixmap.height() as i32;
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let xx = left + col as i32;
                    let yy = top + row as i32;
                    if xx < 0 || yy < 0 || xx >= w || yy >= h {
                        continue;
                    }
                    let alpha = coverage[row * metrics.width + col] as u32 * color[3] as u32 / 255;
                    if alpha == 0 {
                        continue;
                    }
                    let i = ((yy * w + xx) * 4) as usize;
                    let bytes = &mut pixmap.data_mut()[i..i + 4];
                    for c in 0..3 {
                        bytes[c] = (color[c] as u32 * alpha / 255
                            + bytes[c] as u32 * (255 - alpha) / 255)
                            as u8;
                    }
                    bytes[3] = (alpha + bytes[3] as u32 * (255 - alpha) / 255) as u8;
                }
            }
            pen += metrics.advance_width;
        }
    }
    fn sidebar(&mut self, p: &mut Pixmap, scale: i32, o: &Output, e: &Editor, error: Option<&str>) {
        let [x, y, ..] = ui::panel(o.size[1]);
        let f = ui::factor(o.size[1]);
        let scale = scale as f32;
        let t = Transform::from_row(
            f * scale,
            0.0,
            0.0,
            f * scale,
            x * (1.0 - f) * scale,
            y * (1.0 - f) * scale,
        );
        let w = ui::PANEL_WIDTH;
        let h = ui::PANEL_HEIGHT;
        rounded(
            p,
            [x - 4.0, y + 7.0, w + 8.0, h + 4.0],
            20.0,
            [0, 0, 0, 60],
            t,
        );
        rounded(p, [x, y, w, h], 16.0, [30, 30, 46, 250], t);
        self.text(
            p,
            "DRIFTINK",
            [x + 18.0, y + 29.0],
            17.0,
            [205, 214, 244, 255],
            t,
        );
        self.text(
            p,
            "Canvas tools",
            [x + 18.0, y + 49.0],
            11.0,
            [166, 173, 200, 255],
            t,
        );
        self.text(
            p,
            "COLOR   1-8",
            [x + 14.0, y + 408.0],
            10.0,
            [166, 173, 200, 255],
            t,
        );
        self.text(
            p,
            "STROKE WIDTH   [ / ]",
            [x + 14.0, y + 515.0],
            10.0,
            [166, 173, 200, 255],
            t,
        );
        let status = if error.is_some() {
            "Save failed - retry Ctrl+S"
        } else if e.dirty {
            "Unsaved changes"
        } else {
            "Saved automatically"
        };
        self.text(
            p,
            status,
            [x + 14.0, y + 583.0],
            9.0,
            if error.is_some() {
                [243, 139, 168, 255]
            } else {
                [166, 173, 200, 255]
            },
            t,
        );
        for b in ui::base_buttons(o.size[1]) {
            let selected = match b.action {
                Action::Tool(tool) => tool == e.tool,
                Action::Color(c) => c == e.color,
                Action::Width(w) => (w - e.width).abs() < 0.01,
                _ => false,
            };
            let disabled = matches!(b.action, Action::Undo) && !e.can_undo()
                || matches!(b.action, Action::Redo) && !e.can_redo();
            let bg = match b.action {
                Action::Color(c) => c,
                Action::Done => [203, 166, 247, 255],
                _ if selected => [69, 71, 90, 255],
                _ => [49, 50, 68, 255],
            };
            rounded(p, b.rect, 7.0, bg, t);
            if selected {
                outline(
                    p,
                    [
                        b.rect[0] - 2.0,
                        b.rect[1] - 2.0,
                        b.rect[2] + 4.0,
                        b.rect[3] + 4.0,
                    ],
                    [203, 166, 247, 255],
                    1.5,
                    t,
                );
            }
            let color = if matches!(b.action, Action::Done) {
                [30, 30, 46, 255]
            } else if disabled {
                [108, 112, 134, 255]
            } else {
                [205, 214, 244, 255]
            };
            let [bx, by, bw, ..] = b.rect;
            if let Action::Tool(tool) = b.action {
                icon(p, tool, bx + 18.0, by + 18.0, color, t);
            }
            let tx = if matches!(b.action, Action::Tool(_)) {
                bx + 40.0
            } else {
                bx + 12.0
            };
            self.text(p, b.label, [tx, by + 24.0], 13.0, color, t);
            self.text(p, b.shortcut, [bx + bw - 30.0, by + 24.0], 10.0, color, t);
        }
    }
}
fn paint(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], c[3]);
    p.anti_alias = true;
    p
}
fn rounded(p: &mut Pixmap, r: [f32; 4], radius: f32, c: [u8; 4], t: Transform) {
    let [x, y, w, h] = r;
    let mut b = PathBuilder::new();
    let k = radius.min(w / 2.0).min(h / 2.0);
    b.move_to(x + k, y);
    b.line_to(x + w - k, y);
    b.quad_to(x + w, y, x + w, y + k);
    b.line_to(x + w, y + h - k);
    b.quad_to(x + w, y + h, x + w - k, y + h);
    b.line_to(x + k, y + h);
    b.quad_to(x, y + h, x, y + h - k);
    b.line_to(x, y + k);
    b.quad_to(x, y, x + k, y);
    b.close();
    if let Some(path) = b.finish() {
        p.fill_path(&path, &paint(c), FillRule::Winding, t, None);
    }
}
fn outline(p: &mut Pixmap, r: [f32; 4], c: [u8; 4], width: f32, t: Transform) {
    if let Some(rect) = Rect::from_xywh(r[0], r[1], r[2], r[3]) {
        let mut b = PathBuilder::new();
        b.push_rect(rect);
        if let Some(path) = b.finish() {
            p.stroke_path(
                &path,
                &paint(c),
                &Stroke {
                    width,
                    ..Stroke::default()
                },
                t,
                None,
            );
        }
    }
}
fn draw_object(p: &mut Pixmap, o: &Object, output: &Output, t: Transform, delta: Point) {
    let project = |point: Point| {
        let s = world_to_screen([point[0] + delta[0], point[1] + delta[1]], output);
        [s[0] as f32, s[1] as f32]
    };
    let a = project(o.points[0]);
    let b = project(*o.points.last().expect("validated object"));
    let width = (o.width * output.zoom) as f32;
    let color = paint(o.color);
    if o.points.len() == 1 {
        if let Some(path) = PathBuilder::from_circle(a[0], a[1], (width / 2.0).max(0.25)) {
            p.fill_path(&path, &color, FillRule::Winding, t, None);
        }
        return;
    }
    let mut builder = PathBuilder::new();
    match o.tool {
        Tool::Rectangle | Tool::Ellipse => {
            if let Some(rect) = Rect::from_xywh(
                a[0].min(b[0]),
                a[1].min(b[1]),
                (b[0] - a[0]).abs(),
                (b[1] - a[1]).abs(),
            ) {
                if o.tool == Tool::Ellipse {
                    builder.push_oval(rect);
                } else {
                    builder.push_rect(rect);
                }
            } else {
                builder.move_to(a[0], a[1]);
                builder.line_to(b[0], b[1]);
            }
        }
        _ => {
            builder.move_to(a[0], a[1]);
            for point in o.points.iter().skip(1) {
                let v = project(*point);
                builder.line_to(v[0], v[1]);
            }
            if o.tool == Tool::Arrow {
                let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
                let head = ((o.width * 4.0).max(12.0) * output.zoom) as f32;
                for offset in [-0.5_f32, 0.5] {
                    builder.move_to(b[0], b[1]);
                    builder.line_to(
                        b[0] - head * (angle + offset).cos(),
                        b[1] - head * (angle + offset).sin(),
                    );
                }
            }
        }
    }
    if let Some(path) = builder.finish() {
        p.stroke_path(
            &path,
            &color,
            &Stroke {
                width: width.max(0.25),
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            },
            t,
            None,
        );
    }
}
fn icon(p: &mut Pixmap, tool: Tool, x: f32, y: f32, c: [u8; 4], t: Transform) {
    let mut b = PathBuilder::new();
    match tool {
        Tool::Select => {
            b.move_to(x - 5.0, y - 8.0);
            b.line_to(x + 6.0, y);
            b.line_to(x, y + 2.0);
            b.line_to(x - 3.0, y + 8.0);
            b.close();
        }
        Tool::Rectangle => {
            b.push_rect(Rect::from_xywh(x - 7.0, y - 6.0, 14.0, 12.0).expect("static rect"))
        }
        Tool::Ellipse => {
            b.push_oval(Rect::from_xywh(x - 7.0, y - 6.0, 14.0, 12.0).expect("static rect"))
        }
        Tool::Eraser => {
            b.move_to(x - 7.0, y + 2.0);
            b.line_to(x + 1.0, y - 7.0);
            b.line_to(x + 7.0, y - 1.0);
            b.line_to(x - 1.0, y + 7.0);
            b.close();
        }
        _ => {
            b.move_to(x - 7.0, y + 6.0);
            b.line_to(x + 6.0, y - 6.0);
            if tool == Tool::Arrow {
                b.move_to(x, y - 6.0);
                b.line_to(x + 6.0, y - 6.0);
                b.line_to(x + 6.0, y);
            }
        }
    }
    if let Some(path) = b.finish() {
        p.stroke_path(
            &path,
            &paint(c),
            &Stroke {
                width: if tool == Tool::Marker { 4.0 } else { 1.6 },
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            },
            t,
            None,
        );
    }
}
