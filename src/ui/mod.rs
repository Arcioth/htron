pub mod ctron_view;
pub mod daeboard_view;
pub mod telem_view;
pub mod tunotron_view;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use gtk4::cairo::Context;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;

thread_local! {
    static CANVAS_ICE_PIXBUF: RefCell<Option<Pixbuf>> = const { RefCell::new(None) };
    static CANVAS_SURFACE_TRIED: RefCell<bool> = const { RefCell::new(false) };
}

/// Returns the user texture pack directory (`~/.config/htron/textures`).
pub fn ensure_texture_pack_dir() -> PathBuf {
    let dir = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config/htron/textures"))
        .unwrap_or_else(|_| PathBuf::from("data/textures"));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn resolve_texture_path(filename: &str) -> Option<PathBuf> {
    let user_dir = ensure_texture_pack_dir();
    let p1 = user_dir.join(filename);
    if p1.exists() {
        return Some(p1);
    }
    let rel_dir = Path::new("data/textures").join(filename);
    if rel_dir.exists() {
        return fs::canonicalize(rel_dir).ok();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let share_dir = parent.join("../share/htron/textures").join(filename);
            if share_dir.exists() {
                return fs::canonicalize(share_dir).ok();
            }
        }
    }
    let repo_dir = Path::new("/home/arcioth/Documents/htron/data/textures").join(filename);
    if repo_dir.exists() {
        return Some(repo_dir);
    }
    None
}

/// Builds dynamic CSS rules applying the 4 Ice-Serpent textures:
/// 1. `dark_bg_blurred.png`   — Blurred dark underwater iceberg for main window & deep backdrops
/// 2. `light_bar_blurred.png` — Blurred light iceberg texture for small bars, header, dock, & pills
/// 3. `snakeskin_card.png`    — Metallic ice-blue snake skin texture for cards, rail, & scale buttons
/// 4. `ice_panel.png`         — Cracked permafrost ice texture for inner panels, track rows, & buttons
pub fn build_user_texture_css() -> (String, usize) {
    CANVAS_SURFACE_TRIED.with(|t| *t.borrow_mut() = false);
    CANVAS_ICE_PIXBUF.with(|s| *s.borrow_mut() = None);

    let mut css = String::new();
    let mut count = 0usize;

    // 1. Dark blurred underwater iceberg background
    if let Some(dark_bg) = resolve_texture_path("dark_bg_blurred.png")
        .or_else(|| resolve_texture_path("background.png"))
    {
        count += 1;
        css.push_str(&format!(
            "window, .htron-window {{\n\
               background-image: linear-gradient(160deg, rgba(3, 8, 14, 0.42) 0%, rgba(4, 11, 19, 0.55) 100%), url(\"file://{}\");\n\
               background-size: cover;\n\
               background-position: center;\n\
             }}\n",
            dark_bg.display()
        ));
    }

    // 2. Light blurred iceberg texture for small bars (header, bottom dock, status strip, pills, chips)
    if let Some(light_bar) = resolve_texture_path("light_bar_blurred.png")
        .or_else(|| resolve_texture_path("header.png"))
    {
        count += 1;
        css.push_str(&format!(
            ".serpent-header, .cryo-dock {{\n\
               background-image: linear-gradient(90deg, rgba(6, 18, 30, 0.52) 0%, rgba(12, 34, 54, 0.38) 50%, rgba(6, 18, 30, 0.52) 100%), url(\"file://{0}\");\n\
               background-size: cover;\n\
               background-position: center;\n\
             }}\n\
             .status-strip {{\n\
               background-image: linear-gradient(90deg, rgba(4, 11, 19, 0.65) 0%, rgba(8, 22, 36, 0.55) 100%), url(\"file://{0}\");\n\
               background-size: cover;\n\
               background-position: center;\n\
               color: #d8f8ff;\n\
             }}\n\
             .sensor-pill, .cryo-chip, .staged-banner {{\n\
               background-image: linear-gradient(180deg, rgba(14, 38, 60, 0.48) 0%, rgba(6, 18, 30, 0.62) 100%), url(\"file://{0}\");\n\
               background-size: cover;\n\
               background-position: center;\n\
             }}\n",
            light_bar.display()
        ));
    }

    // 3. Snake skin texture for cards, navigation rail, and scale buttons
    if let Some(snake) = resolve_texture_path("snakeskin_card.png")
        .or_else(|| resolve_texture_path("snakeskin.png"))
        .or_else(|| resolve_texture_path("card.png"))
    {
        count += 1;
        css.push_str(&format!(
            ".serpent-rail {{\n\
               background-image: linear-gradient(180deg, rgba(4, 10, 18, 0.62) 0%, rgba(3, 8, 14, 0.74) 100%), url(\"file://{0}\");\n\
               background-size: 420px auto;\n\
               background-repeat: repeat;\n\
             }}\n\
             .serpent-card {{\n\
               background-image: linear-gradient(145deg, rgba(7, 18, 30, 0.64) 0%, rgba(4, 11, 19, 0.76) 100%), url(\"file://{0}\");\n\
               background-size: 520px auto;\n\
               background-repeat: repeat;\n\
             }}\n\
             .nav-scale-btn {{\n\
               background-image: linear-gradient(90deg, rgba(8, 20, 34, 0.68) 0%, rgba(5, 13, 22, 0.78) 100%), url(\"file://{0}\");\n\
               background-size: 320px auto;\n\
             }}\n\
             .nav-scale-btn-active {{\n\
               background-image: linear-gradient(90deg, rgba(0, 210, 245, 0.28) 0%, rgba(14, 58, 82, 0.52) 100%), url(\"file://{0}\");\n\
               background-size: 320px auto;\n\
             }}\n",
            snake.display()
        ));
    }

    // 4. Cracked permafrost ice texture for inner panels, track rows, and buttons
    if let Some(ice) = resolve_texture_path("ice_panel.png")
        .or_else(|| resolve_texture_path("ice_texture.png"))
    {
        count += 1;
        css.push_str(&format!(
            ".frost-panel, .track-row {{\n\
               background-image: linear-gradient(145deg, rgba(6, 16, 27, 0.64) 0%, rgba(4, 11, 19, 0.74) 100%), url(\"file://{0}\");\n\
               background-size: 480px auto;\n\
               background-repeat: repeat;\n\
             }}\n\
             button.cryo-btn {{\n\
               background-image: linear-gradient(180deg, rgba(12, 32, 50, 0.58) 0%, rgba(7, 19, 31, 0.72) 100%), url(\"file://{0}\");\n\
               background-size: 360px auto;\n\
             }}\n\
             button.cryo-btn:hover {{\n\
               background-image: linear-gradient(180deg, rgba(18, 52, 78, 0.45) 0%, rgba(10, 30, 48, 0.60) 100%), url(\"file://{0}\");\n\
               background-size: 360px auto;\n\
             }}\n",
            ice.display()
        ));
    }

    let user_dir = ensure_texture_pack_dir();
    let custom_css_path = user_dir.join("custom.css");
    if let Ok(extra) = fs::read_to_string(&custom_css_path) {
        if !extra.trim().is_empty() {
            count += 1;
            css.push('\n');
            css.push_str(&extra);
        }
    }

    (css, count)
}

/// Draws the cracked permafrost ice texture inside a Cairo canvas (fan curves, telemetry, keyboard, waveform).
pub fn draw_cryo_grid(cr: &Context, w: f64, h: f64) {
    CANVAS_SURFACE_TRIED.with(|tried| {
        if !*tried.borrow() {
            *tried.borrow_mut() = true;
            if let Some(path) = resolve_texture_path("canvas_ice.png")
                .or_else(|| resolve_texture_path("ice_panel.png"))
            {
                if let Ok(pb) = Pixbuf::from_file(path) {
                    CANVAS_ICE_PIXBUF.with(|s| *s.borrow_mut() = Some(pb));
                }
            }
        }
    });

    let mut painted_tex = false;
    CANVAS_ICE_PIXBUF.with(|s| {
        if let Some(pb) = s.borrow().as_ref() {
            let sw = pb.width().max(1) as f64;
            let sh = pb.height().max(1) as f64;
            let _ = cr.save();
            let sx = (w / sw).max(h / sh);
            cr.scale(sx, sx);
            cr.set_source_pixbuf(pb, 0.0, 0.0);
            let _ = cr.paint();
            let _ = cr.restore();

            // Dark abyssal glaze so chart curves & keyboard legends stay razor-sharp
            cr.set_source_rgba(0.015, 0.04, 0.07, 0.58);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();
            painted_tex = true;
        }
    });

    if !painted_tex {
        let pat = gtk4::cairo::LinearGradient::new(0.0, 0.0, w, h);
        pat.add_color_stop_rgba(0.0, 0.02, 0.045, 0.075, 0.98);
        pat.add_color_stop_rgba(0.55, 0.035, 0.075, 0.12, 0.98);
        pat.add_color_stop_rgba(1.0, 0.015, 0.035, 0.06, 0.99);
        let _ = cr.set_source(&pat);
        cr.rectangle(0.0, 0.0, w, h);
        let _ = cr.fill();
    }

    // Soft horizontal reference guides only (no sci-fi square grid)
    cr.set_source_rgba(0.65, 0.96, 1.0, 0.05);
    cr.set_line_width(1.0);
    for frac in [0.25, 0.50, 0.75] {
        let gy = h * frac;
        cr.move_to(0.0, gy);
        cr.line_to(w, gy);
    }
    let _ = cr.stroke();

    // Outer icy specular border
    cr.set_source_rgba(0.55, 0.95, 1.0, 0.24);
    cr.set_line_width(1.2);
    cr.rectangle(0.6, 0.6, (w - 1.2).max(1.0), (h - 1.2).max(1.0));
    let _ = cr.stroke();
}

/// Draws a diamond-shaped serpent scale node at (x, y)
pub fn draw_diamond_node(
    cr: &Context,
    x: f64,
    y: f64,
    radius: f64,
    r: f64,
    g: f64,
    b: f64,
    active: bool,
) {
    if active {
        cr.set_source_rgba(r, g, b, 0.32);
        cr.move_to(x, y - radius * 1.8);
        cr.line_to(x + radius * 1.8, y);
        cr.line_to(x, y + radius * 1.8);
        cr.line_to(x - radius * 1.8, y);
        cr.close_path();
        let _ = cr.fill();
    }

    cr.set_source_rgba(r, g, b, 0.95);
    cr.move_to(x, y - radius);
    cr.line_to(x + radius, y);
    cr.line_to(x, y + radius);
    cr.line_to(x - radius, y);
    cr.close_path();
    let _ = cr.fill_preserve();

    cr.set_source_rgba(0.85, 1.0, 1.0, 0.9);
    cr.set_line_width(1.3);
    let _ = cr.stroke();
}

/// Draws the artistic Ice Serpent emblem on the header bar
pub fn draw_serpent_emblem(cr: &Context, w: f64, h: f64, phase: f64) {
    let cx = w * 0.5;
    let cy = h * 0.5;
    let r = w.min(h) * 0.44;

    // Organic diamond-scale crest
    cr.set_source_rgba(0.04, 0.11, 0.18, 0.95);
    cr.move_to(cx, cy - r);
    cr.line_to(cx + r * 0.86, cy);
    cr.line_to(cx, cy + r);
    cr.line_to(cx - r * 0.86, cy);
    cr.close_path();
    let _ = cr.fill_preserve();

    let glow = 0.65 + 0.35 * (phase.sin());
    cr.set_source_rgba(0.55, 0.95, 1.0, glow);
    cr.set_line_width(1.6);
    let _ = cr.stroke();

    // Serpent head & fangs
    cr.set_source_rgba(0.84, 0.98, 1.0, 0.96);
    cr.move_to(cx, cy - r * 0.68);
    cr.line_to(cx + r * 0.45, cy - r * 0.20);
    cr.line_to(cx + r * 0.22, cy + r * 0.55);
    cr.line_to(cx + r * 0.10, cy + r * 0.12);
    cr.line_to(cx, cy + r * 0.32);
    cr.line_to(cx - r * 0.10, cy + r * 0.12);
    cr.line_to(cx - r * 0.22, cy + r * 0.55);
    cr.line_to(cx - r * 0.45, cy - r * 0.20);
    cr.close_path();
    let _ = cr.fill();

    // Glowing serpent eyes
    cr.set_source_rgba(0.02, 0.05, 0.09, 1.0);
    cr.arc(cx - r * 0.18, cy - r * 0.12, 2.2, 0.0, std::f64::consts::TAU);
    cr.arc(cx + r * 0.18, cy - r * 0.12, 2.2, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();

    cr.set_source_rgba(0.45, 0.96, 1.0, 1.0);
    cr.arc(cx - r * 0.18, cy - r * 0.12, 1.2, 0.0, std::f64::consts::TAU);
    cr.arc(cx + r * 0.18, cy - r * 0.12, 1.2, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
}
