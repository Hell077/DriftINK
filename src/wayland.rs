use crate::{
    camera::world_to_screen,
    ipc::{self, Result, State},
};
use calloop::{EventLoop, channel};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::{
    io::BufRead,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};

struct View {
    layer: LayerSurface,
    output: String,
    size: (u32, u32),
    configured: bool,
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    shm: Shm,
    pool: SlotPool,
    views: Vec<View>,
    state: State,
    canvas: bool,
    world_center: [f64; 2],
    error: Option<String>,
    exit: bool,
}
pub fn run(canvas: bool, seconds: Option<u64>, preview: bool) -> Result<()> {
    let state = ipc::snapshot()?;
    let world_center = if preview {
        state
            .outputs
            .iter()
            .find(|o| o.active)
            .ok_or("no active output")?
            .camera
    } else {
        [0.0, 0.0]
    };
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        pool: SlotPool::new(320 * 240 * 4, &shm)?,
        shm,
        views: vec![],
        state,
        canvas,
        world_center,
        error: None,
        exit: false,
    };
    queue.roundtrip(&mut app)?;
    for output in app.outputs.outputs() {
        let info = app
            .outputs
            .info(&output)
            .ok_or("missing output information")?;
        let name = info.name.ok_or("output has no name")?;
        if !app.state.outputs.iter().any(|o| o.name == name) {
            continue;
        }
        let surface = compositor.create_surface(&qh);
        let region = smithay_client_toolkit::compositor::Region::new(&compositor)?;
        surface.set_input_region(Some(region.wl_region()));
        drop(region);
        let layer = shell.create_layer_surface(
            &qh,
            surface,
            Layer::Bottom,
            Some(if canvas {
                "driftink-poc-canvas"
            } else {
                "driftink-poc-overlay"
            }),
            Some(&output),
        );
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1);
        if canvas {
            layer.set_size(320, 240);
        } else {
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_size(0, 0);
        }
        layer.commit();
        app.views.push(View {
            layer,
            output: name,
            size: (320, 240),
            configured: false,
        });
        // One canvas surface is shared across all compositor viewports.
        if canvas {
            break;
        }
    }
    if app.views.is_empty() {
        return Err("no matching Wayland / IPC outputs".into());
    }
    let mut event_loop = EventLoop::<App>::try_new()?;
    WaylandSource::new(conn, queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("Wayland event source: {e}"))?;
    let (sender, receiver) = channel::sync_channel::<std::result::Result<State, String>>(2);
    event_loop
        .handle()
        .insert_source(receiver, |event, _, app| {
            if let channel::Event::Msg(message) = event {
                match message {
                    Ok(state) => {
                        if state.outputs != app.state.outputs {
                            app.state = state;
                            if !app.canvas {
                                app.draw_all();
                            }
                        }
                    }
                    Err(error) => app.error = Some(error),
                }
            }
        })
        .map_err(|e| format!("IPC event source: {e}"))?;
    std::thread::spawn(move || {
        let result = (|| -> Result<()> {
            for line in ipc::connect("\"Subscribe\"")?.lines() {
                if let Some(state) = ipc::parse(&line?)?
                    && sender.send(Ok(state)).is_err()
                {
                    return Ok(());
                }
            }
            Err("driftwm IPC disconnected".into())
        })();
        if let Err(error) = result {
            let _ = sender.send(Err(error.to_string()));
        }
    });
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));
    eprintln!(
        "Phase 0 {} surface active; world rectangle center {:?}; input region empty",
        if canvas { "canvas" } else { "overlay" },
        world_center
    );
    while !app.exit && app.error.is_none() {
        let timeout = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        if timeout == Some(Duration::ZERO) {
            break;
        }
        event_loop.dispatch(timeout, &mut app)?;
    }
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}
impl App {
    fn draw_all(&mut self) {
        for i in 0..self.views.len() {
            if let Err(error) = self.draw(i) {
                self.error = Some(error.to_string());
                break;
            }
        }
    }
    fn draw(&mut self, i: usize) -> Result<()> {
        let view = &self.views[i];
        if !view.configured {
            return Ok(());
        }
        let (w, h) = view.size;
        if w == 0 || h == 0 || w > 16384 || h > 16384 {
            return Err("unsupported surface size".into());
        }
        let (center, zoom) = if self.canvas {
            ([160.0, 120.0], 1.0)
        } else {
            let Some(output) = self.state.outputs.iter().find(|o| o.name == view.output) else {
                return Ok(());
            };
            (world_to_screen(self.world_center, output), output.zoom)
        };
        let (buffer, pixels) = self.pool.create_buffer(
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
        )?;
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let dx = ((index % w as usize) as f64 + 0.5 - center[0]).abs() / zoom;
            let dy = ((index / w as usize) as f64 + 0.5 - center[1]).abs() / zoom;
            let inside = dx <= 150.0 && dy <= 110.0 && (dx >= 144.0 || dy >= 104.0);
            pixel.copy_from_slice(&if inside { 0xffcba6f7_u32 } else { 0 }.to_ne_bytes());
        }
        buffer.attach_to(view.layer.wl_surface())?;
        view.layer
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        view.layer.commit();
        Ok(())
    }
}
impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.error = Some("output removed; restart Phase 0 probe".into());
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(i) = self.views.iter().position(|v| &v.layer == layer) {
            self.views[i].size = (
                if c.new_size.0 == 0 { 320 } else { c.new_size.0 },
                if c.new_size.1 == 0 { 240 } else { c.new_size.1 },
            );
            eprintln!(
                "configured {}: {}x{} logical pixels",
                self.views[i].output, self.views[i].size.0, self.views[i].size.1
            );
            self.views[i].configured = true;
            if let Err(e) = self.draw(i) {
                self.error = Some(e.to_string());
            }
        }
    }
}
impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}
delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_layer!(App);
delegate_registry!(App);
