use crate::{
    camera::screen_to_world,
    canvas::Tool,
    control,
    drawing::Editor,
    ipc::{self, Result, State},
    render::Renderer,
    storage,
    ui::{self, Action},
};
use calloop::{
    EventLoop, channel,
    signals::{Signal, Signals},
};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
    },
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
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};
use tiny_skia::Pixmap;
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

struct Surface {
    layer: LayerSurface,
    size: (u32, u32),
    scale: i32,
    configured: bool,
    dirty: bool,
    pending: bool,
    pixels: Option<Pixmap>,
}
struct View {
    output: wl_output::WlOutput,
    name: String,
    background: Surface,
    input: Option<Surface>,
}
enum Message {
    State(State),
    Disconnected(String),
    Control(String, mpsc::SyncSender<String>),
}
struct Pan {
    surface: wl_surface::WlSurface,
    start: [f64; 2],
    camera: [f64; 2],
    zoom: f64,
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    pool: SlotPool,
    compositor: CompositorState,
    shell: LayerShell,
    views: Vec<View>,
    state: State,
    editor: Editor,
    renderer: Renderer,
    document: PathBuf,
    editing: bool,
    top_margin: u32,
    connected: bool,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<ThemedPointer>,
    cursor: Option<CursorIcon>,
    modifiers: Modifiers,
    drawing_surface: Option<wl_surface::WlSurface>,
    pan: Option<Pan>,
    save_error: Option<String>,
    fatal: Option<String>,
    exit: bool,
    commander: ipc::Commander,
}
pub fn run(editing: bool) -> Result<()> {
    let guard = control::Guard::acquire()?;
    let document = storage::load(&guard.document)?;
    let state = ipc::snapshot()?;
    let renderer = Renderer::new()?;
    // Mask signals before spawning workers; all threads inherit this mask.
    let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        pool: SlotPool::new(1024 * 1024, &shm)?,
        shm,
        compositor,
        shell,
        views: vec![],
        state,
        editor: Editor::new(document),
        renderer,
        document: guard.document.clone(),
        editing,
        top_margin: std::env::var("DRIFTINK_TOP_MARGIN")
            .ok()
            .map(|v| v.parse::<u32>())
            .transpose()?
            .unwrap_or(48),
        connected: true,
        keyboard: None,
        pointer: None,
        cursor: None,
        modifiers: Modifiers::default(),
        drawing_surface: None,
        pan: None,
        save_error: None,
        fatal: None,
        exit: false,
        commander: ipc::Commander::new(),
    };
    queue.roundtrip(&mut app)?;
    app.reconcile(&qh)?;
    let mut event_loop = EventLoop::<App>::try_new()?;
    WaylandSource::new(conn, queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("Wayland source: {e}"))?;
    event_loop
        .handle()
        .insert_source(signals, |_, _, app| app.exit = true)
        .map_err(|e| format!("signal source: {e}"))?;
    let (sender, receiver) = channel::sync_channel::<Message>(2);
    let event_qh = qh.clone();
    event_loop
        .handle()
        .insert_source(receiver, move |event, _, app| {
            if let channel::Event::Msg(message) = event {
                match message {
                    Message::State(state) => {
                        if app.state.outputs != state.outputs
                            || app.state.fullscreen != state.fullscreen
                            || !app.connected
                        {
                            // A camera change during a gesture must not join unrelated world regions.
                            if app.editor.active() {
                                app.editor.cancel();
                                app.drawing_surface = None;
                            }
                            app.state = state;
                            app.connected = true;
                            if let Err(error) = app.reconcile(&event_qh) {
                                app.fatal = Some(error.to_string());
                            }
                            app.mark_dirty();
                        }
                    }
                    Message::Disconnected(error) => {
                        eprintln!("driftink: {error}; waiting for IPC reconnect");
                        app.connected = false;
                        app.set_editing(false, &event_qh);
                    }
                    Message::Control(command, reply) => {
                        let response = app.command(&command, &event_qh);
                        let _ = reply.send(response);
                    }
                }
            }
        })
        .map_err(|e| format!("runtime source: {e}"))?;
    let subscriber = sender.clone();
    std::thread::spawn(move || {
        loop {
            let result = (|| -> Result<()> {
                for line in ipc::connect("\"Subscribe\"")?.lines() {
                    if let Some(state) = ipc::parse(&line?)?
                        && subscriber.send(Message::State(state)).is_err()
                    {
                        return Ok(());
                    }
                }
                Err("IPC disconnected".into())
            })();
            if let Err(error) = result {
                if subscriber
                    .send(Message::Disconnected(error.to_string()))
                    .is_err()
                {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            } else {
                break;
            }
        }
    });
    let listener = guard.listener.try_clone()?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                break;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let mut command = String::new();
            if BufReader::new(&stream)
                .take(4096)
                .read_line(&mut command)
                .is_err()
            {
                continue;
            }
            let (reply, result) = mpsc::sync_channel(1);
            if sender
                .send(Message::Control(command.trim().into(), reply))
                .is_err()
            {
                break;
            }
            if let Ok(response) = result.recv_timeout(Duration::from_secs(5)) {
                let _ = writeln!(stream, "{response}");
            }
        }
    });
    eprintln!(
        "DriftInk: {} objects; {} mode; document {}",
        app.editor.scene.objects.len(),
        if editing { "drawing" } else { "navigation" },
        app.document.display()
    );
    while !app.exit && app.fatal.is_none() {
        event_loop.dispatch(None, &mut app)?;
        app.render_dirty(&qh)?;
    }
    app.editor.cancel();
    app.save()?;
    if let Some(error) = app.fatal {
        return Err(error.into());
    }
    Ok(())
}
impl App {
    fn make_surface(
        &self,
        output: &wl_output::WlOutput,
        editing: bool,
        exclusive: bool,
        scale: i32,
        qh: &QueueHandle<Self>,
    ) -> Result<Surface> {
        let surface = self.compositor.create_surface(qh);
        if !editing {
            let region = Region::new(&self.compositor)?;
            surface.set_input_region(Some(region.wl_region()));
        }
        let layer = self.shell.create_layer_surface(
            qh,
            surface,
            if editing {
                Layer::Overlay
            } else {
                Layer::Bottom
            },
            Some(if editing {
                "driftink-tools"
            } else {
                "driftink-canvas"
            }),
            Some(output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(if exclusive {
            KeyboardInteractivity::Exclusive
        } else if editing {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::None
        });
        layer.commit();
        Ok(Surface {
            layer,
            size: (1, 1),
            scale: scale.clamp(1, 4),
            configured: false,
            dirty: true,
            pending: false,
            pixels: None,
        })
    }
    fn reconcile(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let outputs: Vec<_> = self.outputs.outputs().collect();
        self.views.retain(|v| {
            outputs.contains(&v.output) && self.state.outputs.iter().any(|o| o.name == v.name)
        });
        for output in outputs {
            let Some(info) = self.outputs.info(&output) else {
                continue;
            };
            let Some(name) = info.name else {
                continue;
            };
            if !self.state.outputs.iter().any(|o| o.name == name) {
                continue;
            }
            if !self.views.iter().any(|v| v.output == output) {
                let background = self.make_surface(&output, false, false, info.scale_factor, qh)?;
                self.views.push(View {
                    output: output.clone(),
                    name: name.clone(),
                    background,
                    input: None,
                });
            }
            let idx = self
                .views
                .iter()
                .position(|v| v.output == output)
                .ok_or("view disappeared")?;
            let fullscreen = self.state.fullscreen.iter().any(|f| f.output == name);
            if self.editing && self.connected && !fullscreen {
                if self.views[idx].input.is_none() {
                    let exclusive = self
                        .state
                        .outputs
                        .iter()
                        .any(|o| o.name == name && o.active);
                    let input =
                        self.make_surface(&output, true, exclusive, info.scale_factor, qh)?;
                    self.views[idx].input = Some(input);
                }
            } else {
                self.views[idx].input = None;
            }
        }
        Ok(())
    }
    fn mark_dirty(&mut self) {
        for view in &mut self.views {
            view.background.dirty = true;
            if let Some(input) = &mut view.input {
                input.dirty = true;
            }
        }
    }
    fn set_editing(&mut self, editing: bool, qh: &QueueHandle<Self>) {
        self.editor.cancel();
        self.drawing_surface = None;
        self.pan = None;
        self.editing = editing && self.connected;
        if let Err(error) = self.reconcile(qh) {
            self.fatal = Some(error.to_string());
        }
        self.mark_dirty();
    }
    fn save(&mut self) -> Result<()> {
        if !self.editor.dirty {
            return Ok(());
        }
        match storage::save(&self.document, &self.editor.document()) {
            Ok(()) => {
                self.editor.dirty = false;
                self.save_error = None;
                Ok(())
            }
            Err(e) => {
                self.save_error = Some(e.to_string());
                eprintln!("driftink save failed: {e}");
                Err(e)
            }
        }
    }
    fn changed(&mut self) {
        let _ = self.save();
        self.mark_dirty();
    }
    fn action(&mut self, action: Action, qh: &QueueHandle<Self>) {
        match action {
            Action::Tool(tool) => self.editor.select_tool(tool),
            Action::Color(c) => self.editor.color = c,
            Action::Width(w) => self.editor.width = w,
            Action::Undo => self.editor.undo(),
            Action::Redo => self.editor.redo(),
            Action::Done => self.set_editing(false, qh),
        }
        self.changed();
    }
    fn command(&mut self, command: &str, qh: &QueueHandle<Self>) -> String {
        match command {
            "status" => {
                return format!(
                    "{}; {} objects; {}; document {}",
                    if self.editing {
                        "drawing"
                    } else {
                        "navigation"
                    },
                    self.editor.scene.objects.len(),
                    if self.save_error.is_some() {
                        "SAVE FAILED"
                    } else if self.connected {
                        "connected"
                    } else {
                        "IPC disconnected"
                    },
                    self.document.display()
                );
            }
            "enable" | "toggle" if !self.connected => {
                return "ERR compositor IPC disconnected".into();
            }
            "enable" => self.set_editing(true, qh),
            "disable" => self.set_editing(false, qh),
            "toggle" => self.set_editing(!self.editing, qh),
            "undo" => self.editor.undo(),
            "redo" => self.editor.redo(),
            "save" => {}
            "quit" => {
                if let Err(e) = self.save() {
                    return format!("ERR {e}");
                }
                self.exit = true;
            }
            _ => return "ERR unknown command".into(),
        }
        self.changed();
        if let Some(error) = &self.save_error {
            format!("ERR {error}")
        } else {
            "OK".into()
        }
    }
    fn render_dirty(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        for view in &mut self.views {
            let Some(output) = self.state.outputs.iter().find(|o| o.name == view.name) else {
                continue;
            };
            let has_input = view.input.is_some();
            for (surface, editing) in std::iter::once((&mut view.background, false))
                .chain(view.input.as_mut().map(|s| (s, true)))
            {
                if !surface.configured || !surface.dirty || surface.pending {
                    continue;
                }
                let (w, h) = surface.size;
                let pw = w
                    .checked_mul(surface.scale as u32)
                    .ok_or("buffer width overflow")?;
                let ph = h
                    .checked_mul(surface.scale as u32)
                    .ok_or("buffer height overflow")?;
                if pw == 0 || ph == 0 || u64::from(pw) * u64::from(ph) > 64_000_000 {
                    return Err("output buffer exceeds 64 million pixels".into());
                }
                if surface
                    .pixels
                    .as_ref()
                    .is_none_or(|p| p.width() != pw || p.height() != ph)
                {
                    surface.pixels =
                        Some(Pixmap::new(pw, ph).ok_or("cannot allocate output buffer")?);
                }
                let pixels = surface.pixels.as_mut().ok_or("missing pixel buffer")?;
                if !editing && has_input {
                    pixels.fill(tiny_skia::Color::TRANSPARENT);
                } else {
                    self.renderer.draw(
                        pixels,
                        surface.scale,
                        output,
                        &self.editor,
                        editing,
                        self.save_error.as_deref(),
                    );
                }
                if editing {
                    let rows = self.top_margin.min(h) * surface.scale as u32;
                    pixels.data_mut()[..(u64::from(rows) * u64::from(pw) * 4) as usize].fill(0);
                }
                let (buffer, bytes) = self.pool.create_buffer(
                    pw as i32,
                    ph as i32,
                    (pw * 4) as i32,
                    wl_shm::Format::Argb8888,
                )?;
                for (src, dst) in pixels
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(bytes.as_chunks_mut::<4>().0.iter_mut())
                {
                    dst.copy_from_slice(
                        &u32::from_be_bytes([src[3], src[0], src[1], src[2]]).to_ne_bytes(),
                    );
                }
                surface.layer.wl_surface().set_buffer_scale(surface.scale);
                buffer.attach_to(surface.layer.wl_surface())?;
                surface
                    .layer
                    .wl_surface()
                    .damage_buffer(0, 0, pw as i32, ph as i32);
                surface
                    .layer
                    .wl_surface()
                    .frame(qh, surface.layer.wl_surface().clone());
                surface.layer.commit();
                surface.pending = true;
                surface.dirty = false;
            }
        }
        Ok(())
    }
}
impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        self.each_surface(surface, |s| {
            s.scale = factor.clamp(1, 4);
            s.dirty = true;
        });
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.each_surface(surface, |s| s.pending = false);
    }
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
impl App {
    fn each_surface(&mut self, surface: &wl_surface::WlSurface, mut f: impl FnMut(&mut Surface)) {
        for view in &mut self.views {
            for s in std::iter::once(&mut view.background).chain(view.input.iter_mut()) {
                if s.layer.wl_surface() == surface {
                    f(s);
                }
            }
        }
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if let Err(e) = self.reconcile(qh) {
            self.fatal = Some(e.to_string());
        }
    }
    fn update_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if let Err(e) = self.reconcile(qh) {
            self.fatal = Some(e.to_string());
        }
        self.mark_dirty();
    }
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.editor.cancel();
        self.drawing_surface = None;
        self.views.retain(|v| v.output != output);
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.views.retain(|v| &v.background.layer != layer);
        for v in &mut self.views {
            if v.input.as_ref().is_some_and(|s| &s.layer == layer) {
                v.input = None;
            }
        }
        self.editor.cancel();
        self.drawing_surface = None;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        for view in &mut self.views {
            for (s, editing) in std::iter::once((&mut view.background, false))
                .chain(view.input.iter_mut().map(|s| (s, true)))
            {
                if &s.layer == layer {
                    if c.new_size.0 > 0 && c.new_size.1 > 0 {
                        s.size = c.new_size;
                    }
                    if editing {
                        match Region::new(&self.compositor) {
                            Ok(region) => {
                                let margin = self.top_margin.min(s.size.1);
                                region.add(
                                    0,
                                    margin as i32,
                                    s.size.0 as i32,
                                    (s.size.1 - margin) as i32,
                                );
                                s.layer
                                    .wl_surface()
                                    .set_input_region(Some(region.wl_region()));
                            }
                            Err(e) => self.fatal = Some(e.to_string()),
                        }
                    }
                    s.configured = true;
                    s.dirty = true;
                }
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
    registry_handlers![OutputState, SeatState];
}
delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);

delegate_layer!(App);
delegate_registry!(App);

#[path = "input.rs"]
mod input;
