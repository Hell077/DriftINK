# Validation

## Automated Rust checks

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
`cargo check`, and `cargo build --release` are the required pre-commit checks.
Tests cover transforms, spatial culling, reversible gestures, selection movement,
deleting during movement, IPC parsing and durable document validation.

## Native integration tests

Tests run against the installed driftwm 0.19.0 binary in a disposable Xvfb X
server. XTest events enter its `winit` backend and are delivered to real Wayland
clients. This tests actual compositor routing without touching the workstation's
mouse or installed configuration. Xvfb and libXtst are test tools only, not
application dependencies. In the development run, Xvfb was extracted under /tmp;
no package was installed system-wide.

Start Xvfb in one terminal (`Xvfb :95 -screen 0 1280x800x24 -nolisten tcp -ac`).
Create an isolated directory with this config:

```toml
autostart = []
[xwayland]
enabled = false
[navigation]
camera_speed = 1.0
[[window_rules]]
app_id = "driftink-input-probe"
position = [0, 0]
[[window_rules]]
app_id = "driftink-poc-canvas"
position = [0, 0]
```

Start the nested compositor, replacing `/tmp/driftink-test` with that directory:

```sh
env -u WAYLAND_DISPLAY DISPLAY=:95 WINIT_UNIX_BACKEND=x11 \
  XDG_CONFIG_HOME=/tmp/driftink-test XDG_DATA_HOME=/tmp/driftink-test \
  XDG_STATE_HOME=/tmp/driftink-test \
  DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/driftink-test/no-bus \
  SYSTEMD_BUS_ADDRESS=unix:path=/tmp/driftink-test/no-bus \
  driftwm --backend winit --config /tmp/driftink-test/config.toml \
  --session-file /tmp/driftink-test/session.json
```

The invalid bus addresses matter: driftwm 0.19.0 exports activation environment
even when nested. Isolating them prevents changes to the desktop activation bus.
Do not copy these bus addresses into your normal session configuration.

Use the new Wayland display printed by the nested compositor, then run tests
**sequentially** (each owns that session's control socket):

```sh
cargo build --examples
cargo build
python3 scripts/check-poc.py wayland-2
python3 scripts/check-input.py wayland-2 :95
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-editor.py wayland-2
python3 scripts/check-reconnect.py wayland-2
```

The scripts reject physical output names. XTest drivers accept only the dedicated
:95 X server and require a single compositor window. Editor tests use temporary
documents, not the user's canvas. Set `DRIFTINK_TEST_ARTIFACTS` to an absolute
directory to retain screenshots. The tests temporarily pan/zoom only the nested
compositor and restore its camera for subsequent tests.

Verified on 2026-10-15:

- Both PoCs preserve expected rectangle pixel bounds through pan/zoom.
- Both allow a click through to a native xdg-toplevel.
- Six drawing tools create the expected world-space objects with autosave.
- Sidebar undo/redo, selection drag, Ctrl+Z and erasing work through native input.
- Escape cancels a gesture, then disables editing; ordinary windows receive clicks.
- Fullscreen outputs remain interactive when drawing mode is requested.
- Restart reloads the same six saved objects and geometry.
- Losing IPC disables capture, reconnect works, and editing requires explicit enable.
- A corrupt document is rejected without modification; competing writers are refused.
- Idle usage in the nested integration run: 2 CPU ticks over 2 seconds (observed,
  not a cross-hardware guarantee).
- A resized 1024×700 logical viewport preserved pointer/world coordinates and
  kept the complete sidebar visible (screenshot inspected).

Offscreen release benchmark (`scene_bench`): 1000 strokes / 32000 points, 120
2560×1440 renders with changing camera/zoom: 476 ms total, about 3.97 ms per
render on this workstation. This excludes IPC, Wayland copying and compositor
presentation; it is **not** a 180 FPS claim.

Fractional-scale probing found that this driftwm winit backend ignores config
mode/scale overrides by design, so a config-reload attempt did not test fractional
rendering. Mathematical fractional transforms are unit tested. Physical display
scaling, monitor unplug/replug, independent two-output drawing, sustained 180 Hz
usage and actual reboot/compositor-restart recovery remain manual acceptance
checks. Do not mark those complete based on this isolated test session.
