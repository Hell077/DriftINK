# DriftInk

Independent Rust/Wayland drawing canvas for driftwm. Currently a **Phase 0
integration experiment**, not a drawing application. No drawing or persistence
engine has been implemented before the integration acceptance gate.

## Build and probe

Requires Rust (edition 2024), a running driftwm session and wlr-layer-shell.

```sh
cargo build
cargo run -- doctor
cargo run -- preview 120
cargo run -- poc overlay 15
```

`doctor` queries the compositor directly over its Unix socket and prints each
output's logical size, camera center and zoom. `DRIFTWM_SOCKET` overrides socket
discovery. No commands are executed from document or compositor data.

`preview [seconds]` places the test rectangle at the active viewport's current
world center, then keeps that world position fixed. Default lifetime: 120 seconds.
It uses a bottom layer: move a covering window aside to reveal the rectangle.
You can pan away and return or zoom to check its anchoring. Stop early with Ctrl+C.

Both `poc` probes draw a transparent 300×220 world-unit rectangle centered at (0, 0).
The origin may be outside your current viewport. Probes accept an optional
lifetime in seconds; without it, stop with Ctrl+C. Surfaces explicitly use an
empty input region and request no keyboard focus. They do not capture drawing
input. Shutdown destroys the client connection and its surfaces.

- `poc canvas`: one bounded 320×240 surface; driftwm handles world placement,
  scaling and rendering across outputs. Requires the rule in
  [examples/poc-rule.toml](examples/poc-rule.toml), followed by config reload
  **before launching** the probe. Config changes are manual and optional.
- `poc overlay`: a bottom-layer screen surface per currently connected output;
  camera subscriptions drive world-to-screen projection. Requires no rule.
  Kept below normal application windows. Fullscreen may hide it.

Never treat an unconfigured `poc canvas` as evidence of world anchoring: without
the position rule it is only an ordinary screen layer.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check
cargo build --release
```

For a disposable, nested driftwm instance with a 1280×800 `winit` output, the
canvas rule, and `[navigation] camera_speed = 1.0`:

```sh
python3 scripts/check-poc.py wayland-2
```

Requires `grim` and ImageMagick. Pass the nested display explicitly. The script
refuses outputs other than a single `winit`, temporarily changes its camera and
zoom, checks screenshot rectangle bounds for both strategies, and restores the
camera. It does not test real mouse routing or physical monitor hotplug.

When starting nested driftwm, isolate config/data/state and session files. Version
0.19.0 exports activation environment even when nested; isolate the D-Bus/systemd
bus addresses too so it cannot alter the real session's activation environment.

See [feasibility evidence](docs/phase-0.md) and [roadmap](docs/roadmap.md).
No installation, autostart or keybinding changes are made automatically. Removal:
stop the process, delete the build directory, and remove the optional rule if you
added it. No user document data is written by this experiment.
