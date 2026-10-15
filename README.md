# DriftInk

Native Rust drawing canvas for driftwm: freehand pen, marker, arrows, lines,
rectangles, ellipses and a floating **Catppuccin Mocha sidebar**. One persistent
world-space document is shared by all outputs. No compositor fork or window rule
is needed for the application.

## Install for your user

DriftInk currently targets **Linux / Wayland with driftwm** (validated against
0.19.0). Other compositors are not supported: layer-shell alone is insufficient
without driftwm camera IPC.

Build prerequisites: Rust 1.89+ and Cargo, a C compiler/linker, pkg-config,
libxkbcommon development files, and Python 3 for the installer. At runtime,
install Noto Sans or DejaVu Sans in a standard system font directory.
Dependencies are managed by your distribution; driftwm must already be installed.

Download/extract the source archive, or clone the repository:

```sh
git clone https://github.com/Hell077/DriftINK.git
cd DriftINK
```

From the project directory:

```sh
python3 scripts/install-user.py
driftink draw
```

The installer builds with `cargo build --release --locked`, installs the binary
under `~/.local/bin`, and adds **DriftInk** to the application menu with an icon.
Desktop files and icons go under `$XDG_DATA_HOME` or `~/.local/share`. The launcher
uses an absolute executable path and works even if `~/.local/bin` is not in the
session's PATH. Terminal users without that PATH entry can run:

```sh
~/.local/bin/driftink draw
```

No sudo, compositor edits, keybinding changes or autostart changes are needed.
The same install command updates an existing installation; atomic replacement
allows updating the binary while the previous process is running. Quit and reopen
DriftInk to use the newly installed executable.

For a custom location or a trusted prebuilt binary:

```sh
python3 scripts/install-user.py --prefix "$HOME/Applications/driftink"
python3 scripts/install-user.py --binary /absolute/path/to/driftink
```

The prebuilt binary must match the target machine's architecture and compatible
system libraries. No release binaries or distribution packages are published by
the installer. For an uninstalled development build:

```sh
cargo build --release --locked
./target/release/driftink draw
```

## Start drawing

The sidebar appears on each available monitor. Choose a tool and drag with the
left mouse button. Drawings autosave after completed operations. `Done` or
`Escape` returns normal application interaction; drawings then remain on the
canvas **under ordinary windows**. While editing, ink is shown above windows.
Fullscreen outputs keep normal input and hide the drawing controls.

To reopen the tools from another terminal:

```sh
driftink enable
# Or toggle between drawing and navigation:
driftink toggle
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
driftink status
driftink disable
driftink undo
driftink redo
driftink save
driftink quit
driftink doctor
```

Runtime control uses a private Unix socket, never TCP. Duplicate processes cannot
edit the same document. `doctor` prints each output's logical size and camera.

For an optional compositor shortcut, merge this entry into the **existing**
`[keybindings]` table in `~/.config/driftwm/config.toml`:

```toml
"super+shift+d" = "spawn ~/.local/bin/driftink draw"
```

Super+Shift+D is free in the inspected version 0.19.0 defaults, but another user
may already use it. Check existing bindings and adjust the executable path for
a custom prefix. Validate with `driftwm --check-config`, then apply using
`driftwm msg action reload-config`. No config,
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

## Uninstall

Stop the application, then run from the source directory:

```sh
driftink quit
python3 scripts/install-user.py --uninstall
```

Use the same `--prefix` and `--data-dir` overrides if you supplied them during
installation. Uninstall removes only the executable, application launcher and
icon. Drawings are kept. Remove any optional shortcut or autostart entry you
added yourself. Deleting a development checkout does not delete an installed copy.

See [architecture](docs/architecture.md), [Phase 0 evidence](docs/phase-0.md) and
[roadmap](docs/roadmap.md).

## License

Licensed under either the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
