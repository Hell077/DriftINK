# Roadmap

| Phase | Status | Evidence / remaining work |
| --- | --- | --- |
| 0 — feasibility | Complete | Installed-version research; both rendering strategies; pan/zoom screenshots; native click-through proof; no compositor patch |
| 1 — foundation | Implemented; physical lifecycle checks pending | Event loop, signals, private control socket, document lock, IPC reconnect and dynamic views; Wayland disconnect requires restart |
| 2 — coordinates | Implemented; physical scale checks pending | Independent output cameras, bidirectional f64 transforms and scaled buffers; fractional physical display acceptance remains |
| 3 — pen MVP | Implemented; workstation acceptance pending | Pen, drawing/navigation modes, sidebar and Escape verified via native pointer input |
| 4 — infinite rendering | Initial implementation | Spatial grid, overflow handling, viewport buffers, frame scheduling and 1000-stroke benchmark; sustained navigation profiling remains |
| 5 — persistence | Implemented; reboot check pending | Validated versioned JSON, atomic replacement, autosave, restart restoration and corrupt-file protection |
| 6 — editing | Initial tools implemented | Undo/redo, eraser, selection, movement, deletion, line, arrow, rectangle, ellipse and marker; text/images/groups remain future work |
| 7 — UI | Sidebar implemented | Catppuccin Mocha, tool/color/width controls, history buttons, keyboard shortcuts, responsive height and Done |
| 8 — CLI/integration | Core implemented | run/draw/enable/disable/toggle/status/save/undo/redo/quit/doctor; optional binding documented; import/export and autostart installer pending |
| 9 — multi-monitor | Workstation acceptance pending | Both physical outputs expose usable cameras; per-output surfaces verified; drawing across monitors, fullscreen IDE and hotplug still need manual testing |
| 10 — quality | Automated checks passing | Unit tests and native isolated integration; resource observations recorded; manual acceptance remains |
| 11 — packaging/docs | Local build documented | Usage, architecture, configuration, troubleshooting, testing and removal provided; packaging/release not published |

The application is now usable for drawing with a sidebar. The full MVP acceptance
claim remains conditional on the physical-workstation tests above; advanced tools
and packaging are deliberately tracked separately.
