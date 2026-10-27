# htron — Cryo-Ophidian Hardware & Audio Suite

`htron` (`dev.arcioth.htron`) is a native Wayland desktop application for Hyprland (Rust + GTK4/Libadwaita + Cairo + C11 FFI) that unifies:

1. **Ctron Hardware & Fan Control (`ctron`)** — ASUS TUF laptop hardware control, interactive 8-point CPU/GPU EC fan curve studio, platform profiles (`quiet` / `balanced` / `performance`), EPP governor, CPU frequency ceiling & boost toggle, PPT wattage sliders (`SPL` / `SPPT` / `FPPT`), NVIDIA dGPU clock lock, Hyprland display refresh rate (`60Hz` / `144Hz`) + Panel Overdrive, and battery charge threshold management.
2. **Cryo Telemetry (`telem`)** — 60-second rolling Cairo telemetry graphs (CPU, dGPU, cooling turbines, iGPU/SoC, DDR5 SPD5118 RAM, and battery/NVMe).
3. **Daeboard RGB & Macro Studio (`daeboard`)** — Live `daeboard` UNIX socket daemon control, interactive ASUS TUF RGB lighting & brightness controls, and visual keyboard + multi-step macro bind editor (`~/.config/ctron/daeboard.binds`).
4. **Tunotron Audio Deck (`tunotron`)** — Native audio player (`ffplay` / `mpv` IPC + PipeWire `wpctl` volume + `lofty` ID3/FLAC tag caching), jailed `~/Music` folder-is-the-playlist browser, interactive Cairo waveform seek bar, DSP equalizer presets, SomaFM live radio streams, persistent bottom dock, and Waybar status sink (`~/.cache/tunotron/waybar.json`).

---

## Ice-Serpent Custom Texture Pack (`~/.config/htron/textures/`)

`htron` ships with the **Ice Serpent (Cryo-Ophidian)** theme and supports live-reloadable textures in `~/.config/htron/textures/` (or `data/textures/`):

- `dark_bg_blurred.png` — Blurred dark underwater iceberg texture for main window backdrop
- `light_bar_blurred.png` — Blurred light iceberg texture for top header, bottom dock, status strip, and sensor pills
- `snakeskin_card.png` — Snake-skin texture for cards, navigation rail, and scale buttons
- `ice_panel.png` / `canvas_ice.png` — Cracked permafrost ice texture for inner panels, track rows, buttons, and Cairo canvases

Click the **`🎨`** button in the top-right header bar at any time to hot-reload textures without restarting.

---

## Building & Installing (NixOS / Linux)

```bash
nix-shell -p rustc cargo pkg-config gcc gtk4 libadwaita --run "cargo build --release"
install -Dm755 target/release/htron ~/.local/bin/htron
install -Dm644 data/htron.desktop ~/.local/share/applications/htron.desktop
install -Dm644 data/htron.svg ~/.local/share/icons/hicolor/scalable/apps/htron.svg
```

### CLI Options

```bash
htron                        # Launch the GTK4 desktop suite
htron --view ctron           # Open directly on Ctron Hardware & Fan Control
htron --view telem           # Open directly on 60s Cryo Telemetry
htron --view daeboard        # Open directly on Daeboard RGB & Macro Studio
htron --view tunotron        # Open directly on Tunotron Audio Deck
htron --status               # Print headless hardware telemetry snapshot
htron --mode turbo           # Apply a ctron shortcut mode (silent|quiet|balanced|performance|turbo)
```
