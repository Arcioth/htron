# Agent Guidelines for `htron`

1. **Hardware Safety Invariant**: Never write to EC, sysfs, `asusctl`, `ryzenadj`, or `nvidia-smi` inside the periodic UI poll timer (`ffi::refresh_fast()`). All hardware mutations must only occur inside explicit user button handlers.
2. **Music Directory Jail**: `src/audio.rs` must keep all file browsing canonicalized within `~/Music`.
3. **Layout Responsiveness**: Keep `gtk4::Stack` with `hhomogeneous(false)` and use `set_width_chars` + `EllipsizeMode::End` on expanding labels so the window fits cleanly inside scaled Hyprland displays.
