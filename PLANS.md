# Architecture & Design Plan (`htron`)

## Overview
`htron` is a native Wayland `.desktop` application written in Rust + GTK4/Libadwaita + Cairo with an embedded C11 hardware backend (`c_src/`).

## Modules
1. **`c_src/` & `src/ffi.rs`**: Safe Rust wrapper around the `ctron` C11 hardware control and telemetry layer. Preserves the strict hardware safety invariant: periodic UI polling is 100% read-only; hardware/EC writes happen only on explicit user actions.
2. **`src/audio.rs` & `src/ui/tunotron_view.rs`**: Background audio engine supporting `ffplay` (with `wpctl` PipeWire per-PID volume and `SIGSTOP`/`SIGCONT` pause/resume) and `mpv` JSON IPC, `lofty` metadata caching, and jailed `~/Music` directory browsing.
3. **`src/ui/ctron_view.rs`**: Interactive Cairo 8-point dual fan curve editor, platform profiles, EPP governor, CPU frequency ceiling, PPT sliders, NVIDIA dGPU clock lock, Hyprland refresh rate switcher, and battery charge limit controls.
4. **`src/ui/daeboard_view.rs`**: ASUS TUF keyboard RGB lighting studio, interactive Cairo keyboard layout, and `~/.config/ctron/daeboard.binds` macro editor.
5. **`src/ui/telem_view.rs`**: 60-second rolling Cairo sparkline graphs for CPU, dGPU, fans, iGPU, memory, and battery/NVMe.
