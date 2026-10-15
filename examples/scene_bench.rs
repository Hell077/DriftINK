//! Deterministic, offscreen CPU rendering benchmark; not a compositor FPS claim.
use driftink::{
    canvas::{Object, Tool},
    drawing::Editor,
    ipc::{Output, Result},
    render::Renderer,
    storage::Document,
};
use std::time::Instant;
fn main() -> Result<()> {
    let mut d = Document::empty();
    for id in 1..=1000 {
        let x = ((id % 40) as f64 - 20.0) * 256.0;
        let y = ((id / 40) as f64 - 12.0) * 256.0;
        d.objects.push(Object {
            id,
            tool: Tool::Pen,
            points: (0..32)
                .map(|i| [x + i as f64 * 4.0, y + (i as f64 * 0.3).sin() * 40.0])
                .collect(),
            color: [203, 166, 247, 255],
            width: 4.0,
            created_ms: 0,
        });
    }
    d.validate()?;
    let editor = Editor::new(d);
    let mut renderer = Renderer::new()?;
    let mut pixels = tiny_skia::Pixmap::new(2560, 1440).ok_or("pixmap allocation failed")?;
    let mut output = Output {
        name: "benchmark".into(),
        active: true,
        camera: [0.0, 0.0],
        zoom: 1.0,
        size: [2560, 1440],
    };
    let start = Instant::now();
    let frames = 120;
    for i in 0..frames {
        output.camera = [i as f64 * 20.0, i as f64 * 5.0];
        output.zoom = 0.5 + (i as f64 / frames as f64);
        renderer.draw(&mut pixels, 1, &output, &editor, false, None);
    }
    println!(
        "1000 strokes / 32000 points; 2560x1440; {frames} varying camera/zoom renders: {:.2} ms total, {:.2} ms/render",
        start.elapsed().as_secs_f64() * 1000.0,
        start.elapsed().as_secs_f64() * 1000.0 / frames as f64
    );
    Ok(())
}
