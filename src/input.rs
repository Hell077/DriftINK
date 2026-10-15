//! Wayland seat events translated into shared world-space editor actions.
use super::*;
impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => self.fatal = Some(format!("keyboard: {e}")),
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.compositor.create_surface(qh),
                ThemeSpec::default(),
            ) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => self.fatal = Some(format!("pointer: {e}")),
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if capability == Capability::Pointer
            && let Some(p) = self.pointer.take()
        {
            drop(p);
            self.editor.cancel();
            self.drawing_surface = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let Some(view) = self.views.iter().find(|v| {
                v.input
                    .as_ref()
                    .is_some_and(|s| s.layer.wl_surface() == &event.surface)
            }) else {
                continue;
            };
            let Some(output) = self
                .state
                .outputs
                .iter()
                .find(|o| o.name == view.name)
                .cloned()
            else {
                continue;
            };
            if !self.editing || !self.connected {
                continue;
            }
            let point = [event.position.0, event.position.1];
            if matches!(event.kind, PointerEventKind::Enter { .. }) {
                self.cursor = None;
            }
            if !matches!(event.kind, PointerEventKind::Leave { .. }) {
                let cursor = if ui::contains(ui::panel(output.size[1]), point) {
                    CursorIcon::Default
                } else if self.pan.is_some() {
                    CursorIcon::Grabbing
                } else if self.editor.tool == Tool::Select {
                    CursorIcon::Move
                } else {
                    CursorIcon::Crosshair
                };
                if self.cursor != Some(cursor)
                    && let Some(pointer) = &self.pointer
                    && pointer.set_cursor(conn, cursor).is_ok()
                {
                    self.cursor = Some(cursor);
                }
            }
            let world = screen_to_world(point, &output);
            let tolerance = 6.0 / output.zoom;
            match event.kind {
                PointerEventKind::Press { button: 272, .. } => {
                    if let Some(action) = ui::hit(output.size[1], point) {
                        self.action(action, qh);
                    } else if !ui::contains(ui::panel(output.size[1]), point) {
                        self.editor.begin(world, tolerance, self.modifiers.shift);
                        self.drawing_surface = Some(event.surface.clone());
                        self.mark_dirty();
                    }
                }
                PointerEventKind::Press { button: 274, .. } => {
                    self.editor.cancel();
                    self.drawing_surface = None;
                    self.pan = Some(Pan {
                        surface: event.surface.clone(),
                        start: point,
                        camera: output.camera,
                        zoom: output.zoom,
                    });
                }
                PointerEventKind::Motion { .. } => {
                    if let Some(pan) = &self.pan {
                        if pan.surface == event.surface {
                            self.commander.send(serde_json::json!({"Camera":[pan.camera[0]-(point[0]-pan.start[0])/pan.zoom,pan.camera[1]+(point[1]-pan.start[1])/pan.zoom]}));
                        }
                    } else if self.drawing_surface.as_ref() == Some(&event.surface) {
                        self.editor.motion(world, tolerance);
                        self.mark_dirty();
                    }
                }
                PointerEventKind::Release { button: 272, .. } => {
                    if self.drawing_surface.as_ref() == Some(&event.surface) {
                        self.editor.finish(world, tolerance);
                        self.drawing_surface = None;
                        self.changed();
                    }
                }
                PointerEventKind::Release { button: 274, .. } => self.pan = None,
                PointerEventKind::Axis { vertical, .. } => {
                    self.editor.cancel();
                    self.drawing_surface = None;
                    let amount = vertical.absolute;
                    if amount != 0.0 {
                        self.commander.send(serde_json::json!({"Zoom":(output.zoom*(-amount*0.006).exp()).clamp(0.05,8.0)}));
                    }
                }
                _ => {}
            }
        }
    }
}
impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.modifiers = Modifiers::default();
    }
    fn press_key(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if !self.editing {
            return;
        }
        let key = event.raw_code;
        // evdev physical keys keep shortcuts usable with Russian and other layouts.
        if key == 1 {
            if self.editor.active() {
                self.editor.cancel();
                self.drawing_surface = None;
                self.mark_dirty();
            } else {
                self.set_editing(false, qh);
            }
            return;
        }
        if self.modifiers.ctrl {
            match key {
                44 if self.modifiers.shift => self.editor.redo(),
                44 => self.editor.undo(),
                21 => self.editor.redo(),
                31 => {}
                30 => self
                    .editor
                    .selected
                    .extend(self.editor.scene.objects.keys()),
                _ => return,
            }
            self.changed();
            return;
        }
        if key == 111 || key == 14 {
            self.editor.delete_selected();
            self.changed();
            return;
        }
        if (2..=9).contains(&key) {
            self.action(Action::Color(ui::COLORS[(key - 2) as usize]), qh);
            return;
        }
        if key == 26 || key == 27 {
            let widths = [2.0, 5.0, 10.0];
            let current = widths
                .iter()
                .position(|w| *w >= self.editor.width)
                .unwrap_or(2);
            let index = if key == 26 {
                current.saturating_sub(1)
            } else {
                (current + 1).min(2)
            };
            self.action(Action::Width(widths[index]), qh);
            return;
        }
        let tool = match key {
            47 => Tool::Select,
            25 | 48 => Tool::Pen,
            50 => Tool::Marker,
            38 => Tool::Line,
            30 => Tool::Arrow,
            19 => Tool::Rectangle,
            24 => Tool::Ellipse,
            18 => Tool::Eraser,
            _ => return,
        };
        self.action(Action::Tool(tool), qh);
    }
    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.modifiers = modifiers;
    }
}

delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
