//! One shared layout for sidebar rendering and hit testing (logical pixels).
use crate::canvas::Tool;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Tool(Tool),
    Color([u8; 4]),
    Width(f64),
    Undo,
    Redo,
    Done,
}
#[derive(Clone)]
pub struct Button {
    pub rect: [f32; 4],
    pub label: &'static str,
    pub shortcut: &'static str,
    pub action: Action,
}
pub const COLORS: [[u8; 4]; 8] = [
    [203, 166, 247, 255],
    [137, 180, 250, 255],
    [166, 227, 161, 255],
    [249, 226, 175, 255],
    [250, 179, 135, 255],
    [243, 139, 168, 255],
    [205, 214, 244, 255],
    [69, 71, 90, 255],
];
pub const PANEL_WIDTH: f32 = 192.0;
pub const PANEL_HEIGHT: f32 = 702.0;
pub fn factor(height: u32) -> f32 {
    ((height.saturating_sub(100)) as f32 / PANEL_HEIGHT).clamp(0.3, 1.0)
}
pub fn panel(height: u32) -> [f32; 4] {
    let f = factor(height);
    [
        20.0,
        ((height as f32 - PANEL_HEIGHT * f) / 2.0).max(50.0),
        PANEL_WIDTH * f,
        PANEL_HEIGHT * f,
    ]
}
pub fn contains(rect: [f32; 4], point: [f64; 2]) -> bool {
    point[0] >= rect[0] as f64
        && point[0] < (rect[0] + rect[2]) as f64
        && point[1] >= rect[1] as f64
        && point[1] < (rect[1] + rect[3]) as f64
}
pub fn base_buttons(height: u32) -> Vec<Button> {
    let [x, y, ..] = panel(height);
    let mut result = Vec::new();
    for (i, (tool, label, key)) in [
        (Tool::Select, "Select", "V"),
        (Tool::Pen, "Pen", "P"),
        (Tool::Marker, "Marker", "M"),
        (Tool::Line, "Line", "L"),
        (Tool::Arrow, "Arrow", "A"),
        (Tool::Rectangle, "Rectangle", "R"),
        (Tool::Ellipse, "Ellipse", "O"),
        (Tool::Eraser, "Eraser", "E"),
    ]
    .into_iter()
    .enumerate()
    {
        result.push(Button {
            rect: [x + 10.0, y + 72.0 + i as f32 * 40.0, 172.0, 36.0],
            label,
            shortcut: key,
            action: Action::Tool(tool),
        });
    }
    for (i, color) in COLORS.into_iter().enumerate() {
        result.push(Button {
            rect: [
                x + 14.0 + (i % 4) as f32 * 42.0,
                y + 420.0 + (i / 4) as f32 * 36.0,
                30.0,
                28.0,
            ],
            label: "",
            shortcut: "",
            action: Action::Color(color),
        });
    }
    for (i, (width, label)) in [(2.0, "2"), (5.0, "5"), (10.0, "10")]
        .into_iter()
        .enumerate()
    {
        result.push(Button {
            rect: [x + 12.0 + i as f32 * 57.0, y + 530.0, 53.0, 36.0],
            label,
            shortcut: "",
            action: Action::Width(width),
        });
    }
    result.push(Button {
        rect: [x + 12.0, y + 590.0, 82.0, 36.0],
        label: "Undo",
        shortcut: "",
        action: Action::Undo,
    });
    result.push(Button {
        rect: [x + 100.0, y + 590.0, 80.0, 36.0],
        label: "Redo",
        shortcut: "",
        action: Action::Redo,
    });
    result.push(Button {
        rect: [x + 12.0, y + 644.0, 168.0, 40.0],
        label: "Done",
        shortcut: "Esc",
        action: Action::Done,
    });
    result
}
pub fn buttons(height: u32) -> Vec<Button> {
    let [x, y, ..] = panel(height);
    let f = factor(height);
    base_buttons(height)
        .into_iter()
        .map(|mut b| {
            b.rect = [
                x + (b.rect[0] - x) * f,
                y + (b.rect[1] - y) * f,
                b.rect[2] * f,
                b.rect[3] * f,
            ];
            b
        })
        .collect()
}
pub fn hit(height: u32, point: [f64; 2]) -> Option<Action> {
    buttons(height)
        .into_iter()
        .find(|b| contains(b.rect, point))
        .map(|b| b.action)
}
