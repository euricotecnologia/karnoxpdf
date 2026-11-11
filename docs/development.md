# Developing KarnoxPDF

Working instructions for agents and contributors are in `AGENTS.md`. This page
collects what the desktop app reads from its environment and where it writes its diagnostics.

## Logs

The desktop app writes its `log` records to standard error and to `logs/karnoxpdf.log` in its
settings folder, next to `app.ron`: Linux and FreeBSD `~/.local/share/karnoxpdf/logs/` (or
`$XDG_DATA_HOME/karnoxpdf/logs/`), macOS `~/Library/Application Support/KarnoxPDF/logs/`, Windows
`%APPDATA%\KarnoxPDF\data\logs\`. A start from a desktop menu, Finder or the Start menu has no
terminal, so this file is what to attach to a bug report: a failed autosave, a page that would not
render, a render worker that could not start and the report of an internal error all land there.
Each launch moves the previous log to `karnoxpdf.1.log` (and that one to `karnoxpdf.2.log`), so the
log of a run that crashed survives the next start. The file stops growing at 16 MiB. `--version`
writes no file.

By default KarnoxPDF's own crates (`karnoxpdf*`) log at `info` and everything else at `warn`.
`RUST_LOG` replaces that with env_logger-style directives, for example `RUST_LOG=debug`,
`RUST_LOG=warn,karnoxpdf_render=trace` or `RUST_LOG=info,wgpu_core=warn`; a directive ending in `*`
covers every target starting with it (`karnoxpdf*=debug`). The logger is
`apps/karnoxpdf/src/logging.rs`. It never records the control-channel token or document passwords.

## Environment variables

| Variable | Effect |
|---|---|
| `RUST_LOG` | Log levels for standard error and the log file (see [Logs](#logs)) |
| `RUST_BACKTRACE` | `1` adds a backtrace to the report of an internal error |
| `XDG_DATA_HOME` | Linux/FreeBSD: base of the settings folder (`karnoxpdf/`), the log folder and crash recovery |
| `WGPU_POWER_PREF` | GPU choice; by default KarnoxPDF prefers the low-power (integrated) GPU |
| `WGPU_BACKEND` | Graphics backend; by default Windows uses Direct3D 12, falling back to OpenGL |
| `CRAFT_FONTS_DIR` | Build time: a folder of fonts to embed (Japanese fonts), optional |
