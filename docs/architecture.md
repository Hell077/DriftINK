# Architecture

One native Rust process owns one logical document. No compositor patch or
custom protocol is required. The initial implementation uses per-output bottom
layer surfaces for persisted ink and temporary overlay surfaces for editing.
Both display the same world-space scene through independent IPC output cameras.
Only editing surfaces request input; destroying them restores normal routing.
The top margin is explicitly excluded from their input regions.

| Module | Responsibility |
| --- | --- |
| `app` | Event loop, surfaces, output lifecycle, mode transitions and autosave |
| `input` | Wayland seat events, tool shortcuts and pointer gestures |
| `canvas` | Validated geometry, object bounds and uniform-grid visibility index |
| `camera` | Logical pixel ↔ Y-up world transforms using f64 |
| `drawing` | Gesture state and reversible edits; no Wayland types |
| `render` | Anti-aliased tiny-skia paths, selection and cached fontdue glyphs |
| `ui` | Shared sidebar layout and hit targets |
| `storage` | Schema validation and durable same-directory atomic saves |
| `ipc` | driftwm snapshots/subscriptions and coalesced navigation requests |
| `control` | Private process control socket and exclusive document lock |
| `wayland` | Original Phase 0 experiments |

The main calloop loop receives Wayland events, bounded worker messages and Unix
signals. Subscription reads reconnect in a worker; camera commands are coalesced
in another worker on a persistent connection. No compositor CLI subprocess is
started per event or frame. Socket replies and persistence errors are surfaced
without putting network services or shell execution in the document model.

Rendering is event driven and limited by compositor frame callbacks. Logical
viewport dimensions are distinct from physical buffer dimensions; integer
Wayland buffer scale provides supersampling when the compositor advertises it.
Pixmap storage is reused. The slot pool reuses released shared-memory buffers.
The 1024-world-unit grid culls offscreen objects; objects spanning more than
4096 cells use a separate overflow set. Very large viewports fall back to a
bounds scan, without allocating a giant grid or framebuffer.

While editing, the bottom surface is transparent and the overlay renders the
scene plus sidebar, avoiding doubled marker opacity. In navigation mode only the
bottom surface renders. Fullscreen inventory suppresses editing surfaces on the
corresponding output. Camera changes cancel incomplete strokes instead of
joining points from inconsistent projections. Completed edits save atomically;
undo/redo stores changed objects rather than full-document snapshots.

IPC disconnection removes editing surfaces. Reconnection restores camera data,
not input capture; enabling tools is explicit. Wayland output events reconcile
views with IPC names, and a newly connected output renders the existing scene.
Document data has no output identifiers. A compositor restart requires restarting
the Wayland client; its document is then restored from disk.
