# DriftInk

Native Rust drawing canvas for driftwm: freehand pen, marker, arrows, lines,
rectangles, ellipses and a floating **Catppuccin Mocha sidebar**. One persistent
world-space document is shared by all outputs. No compositor fork or window rule
is needed for the application.

## Start drawing

Requires Rust 1.89+ (edition 2024), driftwm with wlr-layer-shell, libxkbcommon, and
Noto Sans or DejaVu Sans installed in a standard system font directory.

```sh
cargo build --release
./target/release/driftink draw
```

The sidebar appears on each available monitor. Choose a tool and drag with the
left mouse button. Drawings autosave after completed operations. `Done` or
`Escape` returns normal application interaction; drawings then remain on the
canvas **under ordinary windows**. While editing, ink is shown above windows.
Fullscreen outputs keep normal input and hide the drawing controls.

To reopen the tools from another terminal:

```sh
./target/release/driftink enable
# Or toggle between drawing and navigation:
./target/release/driftink toggle
```

`draw` starts the process in drawing mode, or enables an existing process. `run`
starts in click-through navigation mode. Keep the starting terminal open; Ctrl+C
saves completed edits and stops the process. `quit` also stops it cleanly.

| Action | Key / mouse |
| --- | --- |
| Pen / marker | P (or B) / M |
| Select / eraser | V / E |
| Line / arrow | L / A |
| Rectangle / ellipse | R / O |
| Palette colors | 1–8 |
| Decrease / increase width | [ / ] |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Save / select all | Ctrl+S / Ctrl+A |
| Add to selection | Shift+click an object's stroke |
| Move selection | Drag a selected object's stroke |
| Delete selection | Delete or Backspace |
| Cancel active gesture | Escape |
| Return to normal apps | Escape when no gesture is active, or Done |
| Pan while drawing | Middle-button drag |
| Zoom while drawing | Mouse wheel |

Tool shortcuts use physical key positions and work with Russian layouts too.
In navigation mode, existing driftwm navigation and application shortcuts work
normally. The top 48 logical pixels are kept transparent and click-through for
Waybar, including in drawing mode. The sidebar shrinks to fit shorter viewports.

## CLI and optional keyboard binding

```sh
./target/release/driftink status
./target/release/driftink disable
./target/release/driftink undo
./target/release/driftink redo
./target/release/driftink save
./target/release/driftink quit
./target/release/driftink doctor
```

Runtime control uses a private Unix socket, never TCP. Duplicate processes cannot
edit the same document. `doctor` prints each output's logical size and camera.

For an optional compositor shortcut, merge this entry into the **existing**
`[keybindings]` table, replacing the executable path for your checkout:

```toml
"super+shift+d" = "spawn /home/alex/RustroverProjects/DriftInk/target/release/driftink draw"
```

Super+Shift+D does not conflict with this workstation's inspected overrides or
version 0.19.0 defaults. Check your own bindings before adding it. No config,
autostart entry or session service is installed automatically. You can add the
absolute executable path followed by `run` to your session autostart yourself.

## Persistence and configuration

Default document: `$XDG_DATA_HOME/driftink/canvas.json`, falling back to
`~/.local/share/driftink/canvas.json`. Geometry stores world coordinates and
widths, independent of output names and resolution. Saves use a versioned JSON
schema, geometry validation, fsync and atomic replacement. A failed save keeps
the prior file and reports the error in the sidebar and stderr; retry Ctrl+S.
A pending file, when present, stays next to the document for recovery. Corrupt or
unsupported documents are rejected without overwriting them.

| Environment | Purpose |
| --- | --- |
| `DRIFTINK_DOCUMENT` | Absolute path to another native document |
| `XDG_DATA_HOME` | Persistent document directory |
| `XDG_RUNTIME_DIR` | Wayland and private runtime socket directory |
| `WAYLAND_DISPLAY` | Target Wayland session |
| `DRIFTWM_SOCKET` | Override compositor IPC socket discovery |
| `DRIFTINK_TOP_MARGIN` | Reserved click-through strip height, default 48 logical pixels |

The control socket is `$XDG_RUNTIME_DIR/driftink/<display>.sock` (0600); its
directory is private (0700). Lock files prevent competing writers. Document
permissions on new saves are 0600. Undo history is bounded to 128 edits and lasts
for the current process. Tool/color choices are session preferences.

## Validation and current limits

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check
cargo build --release
cargo run --release --example scene_bench
```

[Testing instructions and results](docs/testing.md) cover real native pointer
input in isolated driftwm, tools, history, selection, fullscreen, autosave,
restart, IPC recovery and idle usage. Physical multi-monitor interactions,
monitor unplug/replug, fractional display scaling and sustained high-refresh
performance still need workstation acceptance testing. Output creation/removal
is handled dynamically; IPC loss disables drawing until the subscription
reconnects and the user enables it again. A Wayland disconnect terminates the
process; restart it in the new session to reload saved drawings.

Rendering uses viewport-sized CPU buffers, frame callbacks and a spatial grid.
Text, images and groups remain future tools. JSON import/export and SVG export
are not yet exposed as CLI commands; stop the process before backing up or
replacing the native document manually.

Earlier experiments are retained as `preview [seconds]` and
`poc [canvas|overlay] [seconds]`. `preview` places a timed test rectangle in the
current viewport. The canvas PoC requires the optional rule in
[examples/poc-rule.toml](examples/poc-rule.toml); the drawing app does not.

Removal: quit DriftInk, remove the build directory and any optional shortcut or
autostart entry you added. Keep the data directory to preserve your drawings.
See [architecture](docs/architecture.md), [Phase 0 evidence](docs/phase-0.md) and
[roadmap](docs/roadmap.md).
