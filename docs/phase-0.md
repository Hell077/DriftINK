# Phase 0 feasibility — 2026-10-15

## Installed version and evidence

- `/usr/bin/driftwm --version`: 0.19.0; pacman: 0.19.0-1.
- Local package recipe downloads tag `v0.19.0`. Inspected that tag's source,
  not current main. Package recipe uses `sha256sums=('SKIP')`; this establishes
  version correspondence, not reproducible binary provenance.
- Read-only IPC query succeeded in the actual session: HDMI-A-1 1920×1080 and
  DP-1 2560×1440, with separate camera centers and zoom. Current configured
  scales are 1.0; fractional scaling is not validated.
- Existing keyboard overrides include Alt+Shift language switching,
  Super+Space launcher and Super+Shift+S screenshot. Super+Shift+D is not in the
  local overrides; default bindings must also be checked before assigning it.
  No bindings were installed.

Version-pinned references:

- [IPC](https://github.com/malbiruk/driftwm/blob/v0.19.0/docs/ipc.md)
- [Window rules](https://github.com/malbiruk/driftwm/blob/v0.19.0/docs/window-rules.md)
- [Configuration](https://github.com/malbiruk/driftwm/blob/v0.19.0/docs/config.md)
- [Layer handler](https://github.com/malbiruk/driftwm/blob/v0.19.0/src/handlers/layer_shell.rs)
- [Rendering](https://github.com/malbiruk/driftwm/blob/v0.19.0/src/render/layers.rs)

The socket uses newline-delimited JSON; `"State"` returns `Ok.State` and
`"Subscribe"` acknowledges then streams `State` events. No frame-by-frame CLI
subprocesses are used. World coordinates are centers with Y pointing up.
Logical output size and zoom define projection; physical buffer scale is a
separate concern. The initial probe uses buffer scale 1 and may appear softer
on a scaled display.

## Strategy comparison

| Property | Canvas-positioned surface | Per-output screen layer |
| --- | --- | --- |
| Placement | Namespace-matched rule with `position` | Client projects using each output camera |
| Camera animation | Native compositor transform | IPC/render scheduling can lag a frame |
| Allocation | Bounded rectangle, future chunks | Full logical viewport per output |
| Multiple outputs | One surface rendered across viewports by compositor | One surface per output |
| Existing config | Requires opt-in position rule | No rule needed |
| Stacking | Canvas layers under normal windows | Bottom layer under normal windows |
| Input in this probe | Explicit empty wl_region, no keyboard | Same |
| Fullscreen | Canvas may show under translucent fullscreen | Screen layers hidden by fullscreen policy |

Source inspection: `handlers/layer_shell.rs` promotes a matching namespace with
`position` into `canvas_layers`; no custom protocol or compositor patch is
needed. `render/layers.rs` projects these surfaces into visible viewports.
Persistent geometry can therefore remain independent of monitor resolution.

Initial choice: strategy A for bounded persisted drawing chunks, with a later
screen-space input surface for editing. This is provisional until real-session
input and multi-monitor acceptance tests pass. Strategy B remains an experiment
and fallback; do not assume IPC synchronization is visually exact at 180 Hz.

## Verification and limitations

The nested compositor uses an isolated temporary config with one position rule,
no autostart, Xwayland disabled, separate state/data/session paths, and unreachable
D-Bus/systemd bus addresses. The installed compositor binary is unchanged.

A native Wayland screenshot showed the purple transparent rectangle centered on
the nested viewport. IPC reported canvas position [0,0], size [320,240]. Automated
screenshot measurements are recorded below after the test run.

The probe renders only on configure and relevant camera/output changes; it does
not request a continuous frame callback. IPC updates use a bounded channel. It
exits with a diagnostic on IPC loss or output removal; reconnect and hotplug
recovery belong to Phase 1. Newly attached outputs require a restart. Multiple
probe launches are not prevented yet. The CPU renderer, rough rectangle edges,
and full-surface redraw are intentionally restricted to this experiment.

**Not yet verified:** real mouse clicks passing to normal apps, real desktop
pan/zoom on both physical outputs with the canvas rule, independent zoom,
fractional scaling, fullscreen transitions, hotplug, and sustained performance.
An empty input region establishes the client-side contract; unit tests and
screenshots do not establish real input-routing acceptance.

Phase 0 remains pending until those integration acceptance checks succeed.
No full drawing engine should be implemented before that gate.

### Completed probe runs

`python3 scripts/check-poc.py wayland-2` passed all four screenshot assertions
with the installed nested compositor (1280×800 output, camera_speed=1):

| Strategy | Camera center | Zoom | Rectangle pixel bounds |
| --- | --- | --- | --- |
| Canvas | (0,0) | 1.0 | 300×220 at (490,290) |
| Canvas | (100,50) | 0.5 | 150×110 at (515,370) |
| Overlay | (0,0) | 1.0 | 300×220 at (490,290) |
| Overlay | (100,50) | 0.5 | 150×110 at (515,370) |

The check allows compositor frames to settle; it does not measure animation
latency. Initial fixed-delay attempts failed while animation or rendering was
still settling. The final check retries screenshots up to a deadline. A separate
real-session `poc overlay 2` received configure events for both HDMI-A-1
(1920×1080) and DP-1 (2560×1440), then exited with status 0. The compositor's
`layers` inventory deduplicates namespaces, so it cannot count these surfaces.

Automated Rust checks passed: fmt, Clippy with warnings denied, 2 unit tests,
check, and release build. No real-session config was modified. The optional
canvas rule is prepared for review in `examples/poc-rule.toml`.
