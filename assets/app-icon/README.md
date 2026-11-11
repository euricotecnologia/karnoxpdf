# KarnoxPDF app icon

<img src="hicolor/128x128/apps/com.karnox.karnoxpdf.png" alt="KarnoxPDF app icon: a white K inside a circuit ring on a blue tile" width="128">

**Source:** [`img/icone1.png`](../../img/icone1.png), the project owner's logo: a white "K" with circuit lines inside a
broken ring, on a blue rounded tile. It is slightly off square (1036 × 1032), so every size is rendered from it centred
on a transparent square.

**Provenance:** original artwork by the project owner, licence in [LICENSE.txt](LICENSE.txt) (`MIT OR Apache-2.0`, like the repo).

## Files

Everything below is rendered by `cargo xtask app-icons` (Rust, no external tools). The outputs are committed, so
building and packaging never need to run it. After running it, update the `sha256` values in `ATTRIBUTION.toml`, then
`cargo xtask assets --write`.

| File | What it is |
|---|---|
| `karnoxpdf-1024.png` | 1024 px on Apple's grid (an 824 px body on a transparent canvas); also the runtime Dock icon on macOS |
| `karnoxpdf.icns` | macOS icon (16–1024 px, PNG entries) |
| `karnoxpdf.ico` | Windows icon (16–256 px, PNG entries), embedded in `karnoxpdf.exe` by `apps/karnoxpdf/build.rs` |
| `hicolor/<n>x<n>/apps/com.karnox.karnoxpdf.png` | Linux hicolor theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |

Where it shows: `apps/karnoxpdf/src/main.rs` sets the window icon (Dock, taskbar, Alt-Tab, launcher) and the
Wayland app id `com.karnox.karnoxpdf`; `packaging/linux/com.karnox.karnoxpdf.desktop` names the icon for launchers;
`packaging/macos/package.sh` copies the `.icns` into the app bundle.
