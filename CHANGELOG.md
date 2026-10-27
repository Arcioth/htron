# Changelog

## [0.2.0] - 2026-10-27
### Added
- Applied custom Ice-Serpent texture pack (`dark_bg_blurred.png`, `light_bar_blurred.png`, `snakeskin_card.png`, `ice_panel.png`, and `canvas_ice.png`) across GTK4 widgets and Cairo canvases.
- Added native `FfplayPlayer` audio backend (`ffplay` + `wpctl` real-time PipeWire volume + `ffprobe` duration + `SIGSTOP`/`SIGCONT` instant pause/resume) alongside `mpv` IPC in `src/audio.rs`.
- Added left-aligned `▶ Play / ⏸ Pause` buttons and click-to-play track titles in `Tunotron Deck` (`src/ui/tunotron_view.rs`), plus transport controls inside the top Now Playing stage.
- Vendored the C11 hardware backend into `c_src/` so the repository compiles standalone.

### Fixed
- Fixed right-side horizontal window clipping on scaled Hyprland displays by disabling homogeneous `gtk4::Stack` sizing (`hhomogeneous(false)`) and splitting wide button bars into compact rows.

## [0.1.0] - 2026-10-27
### Added
- Initial native Wayland GTK4/Libadwaita desktop suite unifying `ctron`, `daeboard`, and `tunotron`.
