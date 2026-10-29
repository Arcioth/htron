use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DrawingArea, Entry, GestureClick, GestureDrag, Label,
    Orientation, ScrolledWindow, Scale, PolicyType,
};

use crate::ffi;
use crate::ui::{draw_cryo_grid, draw_diamond_node, flow_add, flow_row, responsive_box};

#[derive(Clone)]
pub struct CtronViewHandles {
    pub root: ScrolledWindow,
    pub refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)>,
}

struct FanEditorState {
    editing_gpu: bool,
    selected_idx: usize,
    drag_start_x: f64,
    drag_start_y: f64,
    dragging: bool,
}

const PAD_L: f64 = 48.0;
const PAD_R: f64 = 24.0;
const PAD_T: f64 = 24.0;
const PAD_B: f64 = 34.0;
const T_MIN: f64 = 20.0;
const T_MAX: f64 = 105.0;

fn temp_to_x(temp: f64, w: f64) -> f64 {
    let plot_w = (w - PAD_L - PAD_R).max(10.0);
    PAD_L + ((temp - T_MIN) / (T_MAX - T_MIN)).clamp(0.0, 1.0) * plot_w
}

fn pwm_to_y(pwm: f64, h: f64) -> f64 {
    let plot_h = (h - PAD_T - PAD_B).max(10.0);
    PAD_T + (1.0 - (pwm / 255.0).clamp(0.0, 1.0)) * plot_h
}

fn x_to_temp(x: f64, w: f64) -> i32 {
    let plot_w = (w - PAD_L - PAD_R).max(10.0);
    let ratio = ((x - PAD_L) / plot_w).clamp(0.0, 1.0);
    (T_MIN + ratio * (T_MAX - T_MIN)).round() as i32
}

fn y_to_pwm(y: f64, h: f64) -> i32 {
    let plot_h = (h - PAD_T - PAD_B).max(10.0);
    let ratio = (1.0 - (y - PAD_T) / plot_h).clamp(0.0, 1.0);
    (ratio * 255.0).round() as i32
}

pub fn build_ctron_view(status_cb: Rc<dyn Fn(&str)>) -> CtronViewHandles {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let outer = GtkBox::new(Orientation::Vertical, 16);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(18);
    outer.set_margin_end(18);

    // =====================================================================
    // ROW 1: CRYO-OPHIDIAN 8-POINT DUAL FAN CURVE STUDIO + QUICK MODES
    // =====================================================================
    let top_row = responsive_box(16);

    let fan_card = GtkBox::new(Orientation::Vertical, 10);
    fan_card.add_css_class("serpent-card");
    fan_card.set_hexpand(true);

    let fan_hdr = GtkBox::new(Orientation::Horizontal, 8);
    let fan_title_box = GtkBox::new(Orientation::Vertical, 2);
    let fan_title = Label::new(Some("◆ CRYO-OPHIDIAN DUAL 8-POINT FAN CURVE STUDIO"));
    fan_title.add_css_class("card-title");
    fan_title.set_xalign(0.0);
    fan_title.set_hexpand(true);
    fan_title.set_width_chars(10);
    fan_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let fan_sub = Label::new(Some(
        "Drag diamond nodes to stage temperature/PWM curves · Verified EC commit",
    ));
    fan_sub.add_css_class("card-subtitle");
    fan_sub.set_xalign(0.0);
    fan_sub.set_hexpand(true);
    fan_sub.set_width_chars(10);
    fan_sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    fan_title_box.append(&fan_title);
    fan_title_box.append(&fan_sub);
    fan_title_box.set_hexpand(true);

    let staged_badge = Label::new(Some("EC SYNCED"));
    staged_badge.add_css_class("cryo-chip");

    let btn_edit_cpu = Button::with_label("CPU Curve");
    btn_edit_cpu.add_css_class("cryo-btn");
    btn_edit_cpu.add_css_class("cryo-btn-active");

    let btn_edit_gpu = Button::with_label("GPU Curve");
    btn_edit_gpu.add_css_class("cryo-btn");

    fan_hdr.append(&fan_title_box);
    fan_hdr.append(&staged_badge);
    fan_hdr.append(&btn_edit_cpu);
    fan_hdr.append(&btn_edit_gpu);
    fan_card.append(&fan_hdr);

    let editor_state = Rc::new(RefCell::new(FanEditorState {
        editing_gpu: false,
        selected_idx: 0,
        drag_start_x: 0.0,
        drag_start_y: 0.0,
        dragging: false,
    }));

    let fan_canvas = DrawingArea::builder()
        .content_width(340)
        .content_height(260)
        .hexpand(true)
        .vexpand(true)
        .build();

    let ed_draw = Rc::clone(&editor_state);
    fan_canvas.set_draw_func(move |_area, cr, w, h| {
        let wf = w as f64;
        let hf = h as f64;
        draw_cryo_grid(cr, wf, hf);

        let snap = ffi::get_snapshot(false);
        let st = ed_draw.borrow();

        // Axis guide lines (25%, 50%, 75%, 100% PWM and 30..100°C)
        cr.set_font_size(10.0);
        for pct in [0, 25, 50, 75, 100] {
            let pwm = (pct as f64) * 2.55;
            let y = pwm_to_y(pwm, hf);
            cr.set_source_rgba(0.49, 0.98, 1.0, 0.12);
            cr.set_line_width(1.0);
            cr.move_to(PAD_L, y);
            cr.line_to(wf - PAD_R, y);
            let _ = cr.stroke();

            cr.set_source_rgba(0.55, 0.78, 0.86, 0.85);
            cr.move_to(8.0, y + 3.5);
            let _ = cr.show_text(&format!("{:>3}%", pct));
        }

        for t in (20..=100).step_by(10) {
            let x = temp_to_x(t as f64, wf);
            cr.set_source_rgba(0.49, 0.98, 1.0, 0.10);
            cr.move_to(x, PAD_T);
            cr.line_to(x, hf - PAD_B);
            let _ = cr.stroke();

            cr.set_source_rgba(0.55, 0.78, 0.86, 0.85);
            cr.move_to(x - 12.0, hf - 12.0);
            let _ = cr.show_text(&format!("{}°C", t));
        }

        // Helper closure to draw a fan curve
        let draw_curve = |pts: &[(i32, i32)], r: f64, g: f64, b: f64, is_active: bool| {
            if pts.is_empty() {
                return;
            }
            // Translucent fill under curve
            let grad = gtk4::cairo::LinearGradient::new(0.0, PAD_T, 0.0, hf - PAD_B);
            grad.add_color_stop_rgba(0.0, r, g, b, if is_active { 0.28 } else { 0.10 });
            grad.add_color_stop_rgba(1.0, r, g, b, 0.01);

            let first_x = temp_to_x(pts[0].0 as f64, wf);
            let last_x = temp_to_x(pts[pts.len() - 1].0 as f64, wf);
            cr.move_to(first_x, hf - PAD_B);
            for &(t, p) in pts {
                cr.line_to(temp_to_x(t as f64, wf), pwm_to_y(p as f64, hf));
            }
            cr.line_to(last_x, hf - PAD_B);
            cr.close_path();
            let _ = cr.set_source(&grad);
            let _ = cr.fill();

            // Stroke line
            cr.set_source_rgba(r, g, b, if is_active { 0.95 } else { 0.45 });
            cr.set_line_width(if is_active { 2.6 } else { 1.6 });
            for (i, &(t, p)) in pts.iter().enumerate() {
                let x = temp_to_x(t as f64, wf);
                let y = pwm_to_y(p as f64, hf);
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();

            // Diamond scale nodes
            for (i, &(t, p)) in pts.iter().enumerate() {
                let x = temp_to_x(t as f64, wf);
                let y = pwm_to_y(p as f64, hf);
                let selected = is_active && st.selected_idx == i;
                draw_diamond_node(
                    cr,
                    x,
                    y,
                    if selected { 6.5 } else { 4.8 },
                    r,
                    g,
                    b,
                    selected,
                );
                if selected {
                    let pct = ((p as f64 / 255.0) * 100.0).round() as i32;
                    cr.set_source_rgba(0.85, 1.0, 1.0, 0.95);
                    cr.move_to((x - 24.0).max(PAD_L), (y - 12.0).max(14.0));
                    let _ = cr.show_text(&format!("P{}: {}°C · {}%", i + 1, t, pct));
                }
            }
        };

        let cpu_pts = snap.cpu_points();
        let gpu_pts = snap.gpu_points();

        if st.editing_gpu {
            draw_curve(&cpu_pts, 0.0, 0.94, 1.0, false);
            draw_curve(&gpu_pts, 0.65, 0.55, 0.98, true);
        } else {
            draw_curve(&gpu_pts, 0.65, 0.55, 0.98, false);
            draw_curve(&cpu_pts, 0.0, 0.94, 1.0, true);
        }

        // Live CPU & GPU temperature laser markers
        if snap.raw.cpu_temp > 0 {
            let cx = temp_to_x(snap.raw.cpu_temp as f64, wf);
            cr.set_source_rgba(0.0, 0.94, 1.0, 0.55);
            cr.set_line_width(1.2);
            cr.move_to(cx, PAD_T);
            cr.line_to(cx, hf - PAD_B);
            let _ = cr.stroke();
            cr.move_to(cx + 4.0, PAD_T + 12.0);
            let _ = cr.show_text(&format!("CPU {}°C", snap.raw.cpu_temp));
        }
        if snap.raw.gpu_temp > 0 {
            let gx = temp_to_x(snap.raw.gpu_temp as f64, wf);
            cr.set_source_rgba(0.65, 0.55, 0.98, 0.55);
            cr.set_line_width(1.2);
            cr.move_to(gx, PAD_T);
            cr.line_to(gx, hf - PAD_B);
            let _ = cr.stroke();
            cr.move_to(gx + 4.0, PAD_T + 24.0);
            let _ = cr.show_text(&format!("GPU {}°C", snap.raw.gpu_temp));
        }
    });

    // Click & Drag controllers on Fan Canvas
    {
        let click = GestureClick::new();
        let ed = Rc::clone(&editor_state);
        let canvas_ref = fan_canvas.clone();
        click.connect_pressed(move |_, _, x, y| {
            let wf = canvas_ref.width() as f64;
            let hf = canvas_ref.height() as f64;
            let snap = ffi::get_snapshot(false);
            let mut st = ed.borrow_mut();
            let pts = if st.editing_gpu {
                snap.gpu_points()
            } else {
                snap.cpu_points()
            };
            let mut best_i = st.selected_idx;
            let mut best_d = f64::MAX;
            for (i, &(t, p)) in pts.iter().enumerate() {
                let px = temp_to_x(t as f64, wf);
                let py = pwm_to_y(p as f64, hf);
                let d = (px - x).hypot(py - y);
                if d < best_d {
                    best_d = d;
                    best_i = i;
                }
            }
            st.selected_idx = best_i;
            canvas_ref.queue_draw();
        });
        fan_canvas.add_controller(click);

        let drag = GestureDrag::new();
        let ed_b = Rc::clone(&editor_state);
        let canvas_b = fan_canvas.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let wf = canvas_b.width() as f64;
            let hf = canvas_b.height() as f64;
            let snap = ffi::get_snapshot(false);
            let mut st = ed_b.borrow_mut();
            let pts = if st.editing_gpu {
                snap.gpu_points()
            } else {
                snap.cpu_points()
            };
            let mut best_i = st.selected_idx;
            let mut best_d = f64::MAX;
            for (i, &(t, p)) in pts.iter().enumerate() {
                let px = temp_to_x(t as f64, wf);
                let py = pwm_to_y(p as f64, hf);
                let d = (px - x).hypot(py - y);
                if d < best_d {
                    best_d = d;
                    best_i = i;
                }
            }
            st.selected_idx = best_i;
            st.drag_start_x = x;
            st.drag_start_y = y;
            st.dragging = best_d < 36.0;
            canvas_b.queue_draw();
        });

        let ed_u = Rc::clone(&editor_state);
        let canvas_u = fan_canvas.clone();
        let badge_u = staged_badge.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let mut st = ed_u.borrow_mut();
            if !st.dragging {
                return;
            }
            let wf = canvas_u.width() as f64;
            let hf = canvas_u.height() as f64;
            let nx = st.drag_start_x + dx;
            let ny = st.drag_start_y + dy;
            let new_t = x_to_temp(nx, wf);
            let new_p = y_to_pwm(ny, hf);
            if let Some(new_idx) = ffi::fan_set_point(st.editing_gpu, st.selected_idx, new_t, new_p) {
                st.selected_idx = new_idx;
                badge_u.set_text("● STAGED (UNWRITTEN)");
                badge_u.add_css_class("staged-banner");
            }
            canvas_u.queue_draw();
        });
        fan_canvas.add_controller(drag);
    }

    {
        let ed = Rc::clone(&editor_state);
        let c_btn = btn_edit_cpu.clone();
        let g_btn = btn_edit_gpu.clone();
        let canvas_c = fan_canvas.clone();
        btn_edit_cpu.connect_clicked(move |_| {
            ed.borrow_mut().editing_gpu = false;
            c_btn.add_css_class("cryo-btn-active");
            g_btn.remove_css_class("cryo-btn-active");
            canvas_c.queue_draw();
        });

        let ed2 = Rc::clone(&editor_state);
        let c_btn2 = btn_edit_cpu.clone();
        let g_btn2 = btn_edit_gpu.clone();
        let canvas_g = fan_canvas.clone();
        btn_edit_gpu.connect_clicked(move |_| {
            ed2.borrow_mut().editing_gpu = true;
            g_btn2.add_css_class("cryo-btn-active");
            c_btn2.remove_css_class("cryo-btn-active");
            canvas_g.queue_draw();
        });
    }

    fan_card.append(&fan_canvas);

    // Fan Preset & Write Action Bar (Split into 2 compact rows so it never forces wide min-width)
    let fan_bar = flow_row();
    let lbl_presets = Label::new(Some("Presets:"));
    lbl_presets.add_css_class("dim-label");
    flow_add(&fan_bar, &lbl_presets);

    for (label, idx) in [
        ("Quiet", 0),
        ("Balanced", 1),
        ("Perf", 2),
        ("Live EC", 3),
    ] {
        let b = Button::with_label(label);
        b.add_css_class("cryo-btn");
        let canvas_p = fan_canvas.clone();
        let badge_p = staged_badge.clone();
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            if ffi::fan_load_preset(idx) {
                badge_p.set_text("● STAGED PRESET");
                badge_p.add_css_class("staged-banner");
                scb(&format!("Loaded '{}' fan curve into staging buffer", label));
                canvas_p.queue_draw();
            }
        });
        flow_add(&fan_bar, &b);
    }

    for (label, q_idx) in [("Stock", 0), ("Silent", 1), ("Cool", 2), ("FullSpeed", 3)] {
        let b = Button::with_label(label);
        b.add_css_class("cryo-btn");
        let canvas_q = fan_canvas.clone();
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            if ffi::fan_quick_preset(q_idx) {
                scb(&format!("Applied quick fan preset '{}'", label));
            } else {
                scb(&format!("Quick fan preset '{}' failed (check privileges)", label));
            }
            canvas_q.queue_draw();
        });
        flow_add(&fan_bar, &b);
    }
    fan_card.append(&fan_bar);

    let fan_bar2 = GtkBox::new(Orientation::Horizontal, 8);
    let btn_fan_enable = Button::with_label("Custom Curve: ON");
    btn_fan_enable.add_css_class("cryo-btn");
    {
        let scb = Rc::clone(&status_cb);
        let btn_clone = btn_fan_enable.clone();
        btn_fan_enable.connect_clicked(move |_| {
            let snap = ffi::get_snapshot(false);
            let next_on = snap.raw.fan_cpu_on == 0 || snap.raw.fan_gpu_on == 0;
            if ffi::fan_set_enabled(next_on, next_on) {
                btn_clone.set_label(if next_on {
                    "Custom Curve: ON"
                } else {
                    "Custom Curve: OFF"
                });
                scb(if next_on {
                    "Enabled custom EC fan curves (CPU + GPU)"
                } else {
                    "Reverted to factory automatic fan curves"
                });
            }
        });
    }
    fan_bar2.append(&btn_fan_enable);

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    fan_bar2.append(&spacer);

    let btn_write_ec = Button::with_label("⚡ WRITE CURVES TO EC");
    btn_write_ec.add_css_class("fang-action-btn");
    {
        let canvas_w = fan_canvas.clone();
        let badge_w = staged_badge.clone();
        let scb = Rc::clone(&status_cb);
        btn_write_ec.connect_clicked(move |_| {
            if ffi::fan_write_ec() {
                badge_w.set_text("✓ EC VERIFIED");
                badge_w.remove_css_class("staged-banner");
                scb("Fan curves written to EC & verified by read-back");
            } else {
                scb("EC fan write failed or read-back mismatch (check asusctl / sysfs)");
            }
            canvas_w.queue_draw();
        });
    }
    fan_bar2.append(&btn_write_ec);
    fan_card.append(&fan_bar2);

    // Right Column in Row 1: MODES & HARDWARE SNAPSHOTS
    let modes_card = GtkBox::new(Orientation::Vertical, 8);
    modes_card.add_css_class("serpent-card");
    modes_card.set_size_request(250, -1);

    let m_title = Label::new(Some("◆ MODES & SNAPSHOTS"));
    m_title.add_css_class("card-title");
    m_title.set_xalign(0.0);
    m_title.set_hexpand(true);
    m_title.set_width_chars(8);
    m_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let m_sub = Label::new(Some("~/.config/ctron/modes.ini & profiles/*.ctr"));
    m_sub.add_css_class("card-subtitle");
    m_sub.set_xalign(0.0);
    m_sub.set_hexpand(true);
    m_sub.set_width_chars(8);
    m_sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    modes_card.append(&m_title);
    modes_card.append(&m_sub);

    let modes_list_box = GtkBox::new(Orientation::Vertical, 6);
    modes_card.append(&modes_list_box);

    let snap_hdr = Label::new(Some("HARDWARE SNAPSHOTS (.ctr)"));
    snap_hdr.add_css_class("rail-section-label");
    snap_hdr.set_halign(Align::Start);
    modes_card.append(&snap_hdr);

    let snap_entry_row = GtkBox::new(Orientation::Horizontal, 6);
    let snap_name_entry = Entry::builder()
        .placeholder_text("snapshot name")
        .width_chars(10)
        .hexpand(true)
        .build();
    let btn_snap_save = Button::with_label("Capture");
    btn_snap_save.add_css_class("cryo-btn");
    snap_entry_row.append(&snap_name_entry);
    snap_entry_row.append(&btn_snap_save);
    modes_card.append(&snap_entry_row);

    let snaps_list_box = GtkBox::new(Orientation::Vertical, 6);
    modes_card.append(&snaps_list_box);

    let rebuild_modes_and_snaps: Rc<dyn Fn()> = {
        let m_box = modes_list_box.clone();
        let s_box = snaps_list_box.clone();
        let scb = Rc::clone(&status_cb);
        let canvas_m = fan_canvas.clone();
        Rc::new(move || {
            while let Some(c) = m_box.first_child() {
                m_box.remove(&c);
            }
            for mode in ffi::list_modes() {
                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.add_css_class("frost-panel");
                let info = GtkBox::new(Orientation::Vertical, 2);
                info.set_hexpand(true);
                let nm = Label::new(Some(&format!("⚡ {}", mode.name.to_uppercase())));
                nm.set_xalign(0.0);
                nm.set_hexpand(true);
                nm.add_css_class("card-title");
                nm.set_width_chars(8);
                nm.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                let st = Label::new(Some(&mode.steps));
                st.set_xalign(0.0);
                st.set_hexpand(true);
                st.add_css_class("dim-label");
                st.set_width_chars(8);
                st.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                info.append(&nm);
                info.append(&st);

                let btn_apply = Button::with_label("Apply");
                btn_apply.add_css_class("cryo-btn");
                let mname = mode.name.clone();
                let scb2 = Rc::clone(&scb);
                let cv = canvas_m.clone();
                btn_apply.connect_clicked(move |_| {
                    match ffi::apply_mode(&mname) {
                        Ok(()) => scb2(&format!("Applied mode [{}]", mname)),
                        Err(e) => scb2(&format!("Mode [{}] error: {}", mname, e)),
                    }
                    cv.queue_draw();
                });

                row.append(&info);
                row.append(&btn_apply);
                m_box.append(&row);
            }

            while let Some(c) = s_box.first_child() {
                s_box.remove(&c);
            }
            let profiles = ffi::list_profiles();
            if profiles.is_empty() {
                let empty = Label::new(Some("No saved .ctr snapshots yet"));
                empty.add_css_class("dim-label");
                empty.set_halign(Align::Start);
                s_box.append(&empty);
            } else {
                for p in profiles.into_iter().take(5) {
                    let row = GtkBox::new(Orientation::Horizontal, 6);
                    row.add_css_class("frost-panel");
                    let vb = GtkBox::new(Orientation::Vertical, 1);
                    vb.set_hexpand(true);
                    let l1 = Label::new(Some(&format!("{} ({})", p.name, p.diff)));
                    l1.set_xalign(0.0);
                    l1.set_hexpand(true);
                    l1.set_width_chars(8);
                    l1.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    let l2 = Label::new(Some(&p.summary));
                    l2.add_css_class("dim-label");
                    l2.set_xalign(0.0);
                    l2.set_hexpand(true);
                    l2.set_width_chars(8);
                    l2.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    l2.set_tooltip_text(Some(&p.summary));
                    vb.append(&l1);
                    vb.append(&l2);

                    let b_app = Button::with_label("Load");
                    b_app.add_css_class("cryo-btn");
                    let pname = p.name.clone();
                    let scb3 = Rc::clone(&scb);
                    let cv = canvas_m.clone();
                    b_app.connect_clicked(move |_| {
                        match ffi::apply_profile(&pname) {
                            Ok(()) => scb3(&format!("Applied snapshot '{}'", pname)),
                            Err(e) => scb3(&format!("Snapshot error: {}", e)),
                        }
                        cv.queue_draw();
                    });

                    row.append(&vb);
                    row.append(&b_app);
                    s_box.append(&row);
                }
            }
        })
    };
    rebuild_modes_and_snaps();

    {
        let entry_c = snap_name_entry.clone();
        let reb = Rc::clone(&rebuild_modes_and_snaps);
        let scb = Rc::clone(&status_cb);
        btn_snap_save.connect_clicked(move |_| {
            let name = entry_c.text().to_string();
            if ffi::capture_profile(&name, "captured from htron") {
                scb("Captured hardware snapshot (.ctr)");
                entry_c.set_text("");
                reb();
            } else {
                scb("Failed to capture hardware snapshot");
            }
        });
    }

    top_row.append(&fan_card);
    top_row.append(&modes_card);
    outer.append(&top_row);

    // =====================================================================
    // ROW 2: POWER / PPT / CPU / GPU / DISPLAY / BATTERY CONTROL MATRIX
    // =====================================================================
    let mid_row = responsive_box(16);

    // --- Card A: Platform Profile, EPP & CPU Ceiling ---
    let cpu_card = GtkBox::new(Orientation::Vertical, 10);
    cpu_card.add_css_class("serpent-card");
    cpu_card.set_hexpand(true);

    let cpu_hdr = Label::new(Some("◆ PLATFORM PROFILE, EPP & CPU FREQUENCY"));
    cpu_hdr.add_css_class("card-title");
    cpu_hdr.set_xalign(0.0);
    cpu_hdr.set_hexpand(true);
    cpu_hdr.set_width_chars(10);
    cpu_hdr.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    cpu_card.append(&cpu_hdr);

    let prof_row = flow_row();
    let prof_lbl = Label::new(Some("Platform Profile:"));
    prof_lbl.add_css_class("dim-label");
    flow_add(&prof_row, &prof_lbl);

    let mut prof_btns = Vec::new();
    for (name, idx) in [("Quiet", 0), ("Balanced", 1), ("Performance", 2)] {
        let b = Button::with_label(name);
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            if ffi::set_profile(idx) {
                scb(&format!("Platform profile set to {}", name));
            } else {
                scb(&format!("Failed to set platform profile {}", name));
            }
        });
        flow_add(&prof_row, &b);
        prof_btns.push((idx, b));
    }
    cpu_card.append(&prof_row);

    let epp_row = flow_row();
    let epp_lbl = Label::new(Some("EPP Governor:"));
    epp_lbl.add_css_class("dim-label");
    flow_add(&epp_row, &epp_lbl);
    let mut epp_btns = Vec::new();
    for (name, idx) in [
        ("power", 0),
        ("bal_power", 1),
        ("bal_perf", 2),
        ("performance", 3),
    ] {
        let b = Button::with_label(name);
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            if ffi::set_epp(idx) {
                scb(&format!("EPP set to {}", name));
            } else {
                scb(&format!("Failed to set EPP {}", name));
            }
        });
        flow_add(&epp_row, &b);
        epp_btns.push((idx, b));
    }
    cpu_card.append(&epp_row);

    // CPU MHz Ceiling Slider + Boost
    let init_snap = ffi::get_snapshot(false);
    let max_hw_mhz = init_snap.raw.cpu_mhz_max.max(4500) as f64;
    let min_hw_mhz = init_snap.raw.cpu_mhz_min.max(1200) as f64;

    let freq_row = GtkBox::new(Orientation::Horizontal, 8);
    let freq_lbl = Label::new(Some(&format!(
        "CPU Ceiling: {} MHz",
        init_snap.raw.cpu_mhz_limit
    )));
    freq_lbl.set_xalign(0.0);
    freq_lbl.set_hexpand(true);
    freq_lbl.set_width_chars(10);
    freq_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let freq_scale = Scale::with_range(Orientation::Horizontal, min_hw_mhz, max_hw_mhz, 50.0);
    freq_scale.set_value(init_snap.raw.cpu_mhz_limit.max(2000) as f64);
    freq_scale.set_hexpand(true);
    freq_scale.set_size_request(140, -1);
    freq_scale.set_draw_value(true);
    freq_scale.set_value_pos(gtk4::PositionType::Right);
    freq_scale.set_digits(0);

    let btn_apply_freq = Button::with_label("Set MHz");
    btn_apply_freq.add_css_class("cryo-btn");
    {
        let sc = freq_scale.clone();
        let scb = Rc::clone(&status_cb);
        btn_apply_freq.connect_clicked(move |_| {
            let mhz = sc.value().round() as i32;
            if ffi::set_cpu_max_mhz(mhz) {
                scb(&format!("Applied all-core CPU ceiling: {} MHz", mhz));
            } else {
                scb("Failed to set CPU frequency ceiling");
            }
        });
    }

    let btn_cpu_boost = Button::with_label(if init_snap.raw.cpu_boost != 0 {
        "CPU Boost: ON"
    } else {
        "CPU Boost: OFF"
    });
    btn_cpu_boost.add_css_class("cryo-btn");
    {
        let scb = Rc::clone(&status_cb);
        let b_clone = btn_cpu_boost.clone();
        btn_cpu_boost.connect_clicked(move |_| {
            let cur = ffi::get_snapshot(false).raw.cpu_boost != 0;
            let next = !cur;
            if ffi::set_cpu_boost(next) {
                b_clone.set_label(if next { "CPU Boost: ON" } else { "CPU Boost: OFF" });
                scb(if next {
                    "Enabled CPU turbo boost"
                } else {
                    "Disabled CPU turbo boost"
                });
            }
        });
    }

    cpu_card.append(&freq_lbl);
    freq_row.append(&freq_scale);
    freq_row.append(&btn_apply_freq);
    freq_row.append(&btn_cpu_boost);
    cpu_card.append(&freq_row);

    // --- Card B: PPT Watt Limits & NVIDIA dGPU Controls ---
    let ppt_card = GtkBox::new(Orientation::Vertical, 10);
    ppt_card.add_css_class("serpent-card");
    ppt_card.set_hexpand(true);

    let ppt_hdr = Label::new(Some("◆ PPT WATTAGE (SPL / SPPT / FPPT) & NVIDIA dGPU"));
    ppt_hdr.add_css_class("card-title");
    ppt_hdr.set_halign(Align::Start);
    ppt_hdr.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    ppt_card.append(&ppt_hdr);

    let ppt_preset_row = flow_row();
    let ppt_p_lbl = Label::new(Some("PPT Quick:"));
    ppt_p_lbl.add_css_class("dim-label");
    flow_add(&ppt_preset_row, &ppt_p_lbl);

    for token in ["Q45", "B60", "P80", "off", "on"] {
        let b = Button::with_label(token);
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        let tok = token.to_string();
        b.connect_clicked(move |_| match ffi::run_ctron_cmd("ppt", &tok) {
            Ok(()) => scb(&format!("Applied PPT preset '{}'", tok)),
            Err(e) => scb(&format!("PPT '{}': {}", tok, e)),
        });
        flow_add(&ppt_preset_row, &b);
    }
    ppt_card.append(&ppt_preset_row);

    let spl_scale = Scale::with_range(Orientation::Horizontal, 15.0, 120.0, 1.0);
    spl_scale.set_value(if init_snap.raw.ppt_spl > 0 {
        init_snap.raw.ppt_spl as f64
    } else {
        45.0
    });
    spl_scale.set_hexpand(true);

    let sppt_scale = Scale::with_range(Orientation::Horizontal, 15.0, 130.0, 1.0);
    sppt_scale.set_value(if init_snap.raw.ppt_sppt > 0 {
        init_snap.raw.ppt_sppt as f64
    } else {
        60.0
    });
    sppt_scale.set_hexpand(true);

    let fppt_scale = Scale::with_range(Orientation::Horizontal, 15.0, 140.0, 1.0);
    fppt_scale.set_value(if init_snap.raw.ppt_fppt > 0 {
        init_snap.raw.ppt_fppt as f64
    } else {
        80.0
    });
    fppt_scale.set_hexpand(true);

    let btn_apply_ppt = Button::with_label("Write PPT");
    btn_apply_ppt.add_css_class("cryo-btn");
    {
        let s1 = spl_scale.clone();
        let s2 = sppt_scale.clone();
        let s3 = fppt_scale.clone();
        let scb = Rc::clone(&status_cb);
        btn_apply_ppt.connect_clicked(move |_| {
            let spl = s1.value().round() as i32;
            let sppt = s2.value().round().max(spl as f64) as i32;
            let fppt = s3.value().round().max(sppt as f64) as i32;
            s2.set_value(sppt as f64);
            s3.set_value(fppt as f64);
            if ffi::set_ppt(spl, sppt, fppt) {
                scb(&format!("Wrote PPT: {}/{}/{} W (SPL/SPPT/FPPT)", spl, sppt, fppt));
            } else {
                scb("PPT write failed (check ryzenadj / asus-armoury sudoers)");
            }
        });
    }
    // One labelled slider per limit (sustained / slow / fast) with live watt readout
    for (name, sc) in [
        ("SPL", &spl_scale),
        ("SPPT", &sppt_scale),
        ("FPPT", &fppt_scale),
    ] {
        let r = GtkBox::new(Orientation::Horizontal, 8);
        let l = Label::new(Some(name));
        l.add_css_class("dim-label");
        l.set_width_chars(5);
        l.set_xalign(0.0);
        sc.set_draw_value(true);
        sc.set_value_pos(gtk4::PositionType::Right);
        sc.set_digits(0);
        sc.set_size_request(140, -1);
        r.append(&l);
        r.append(sc);
        ppt_card.append(&r);
    }
    btn_apply_ppt.set_halign(Align::End);
    ppt_card.append(&btn_apply_ppt);

    // NVIDIA GPU Clock Lock & Dynamic Boost
    let nv_row = GtkBox::new(Orientation::Horizontal, 8);
    let nv_lbl = Label::new(Some("dGPU Clock Lock:"));
    nv_lbl.add_css_class("dim-label");
    let gpu_max_clock = init_snap.raw.gpu_mhz_max.max(2500) as f64;
    let nv_clock_scale = Scale::with_range(Orientation::Horizontal, 600.0, gpu_max_clock, 15.0);
    nv_clock_scale.set_value(1800.0);
    nv_clock_scale.set_hexpand(true);
    nv_clock_scale.set_size_request(140, -1);
    nv_clock_scale.set_draw_value(true);
    nv_clock_scale.set_value_pos(gtk4::PositionType::Right);
    nv_clock_scale.set_digits(0);

    let btn_lock_gpu = Button::with_label("Lock MHz");
    btn_lock_gpu.add_css_class("cryo-btn");
    {
        let sc = nv_clock_scale.clone();
        let scb = Rc::clone(&status_cb);
        btn_lock_gpu.connect_clicked(move |_| {
            let mhz = sc.value().round() as i32;
            if ffi::set_gpu_clock(mhz) {
                scb(&format!("Locked NVIDIA core clock to ≤ {} MHz", mhz));
            } else {
                scb("NVIDIA clock lock failed (needs sudoers for nvidia-smi -lgc)");
            }
        });
    }

    let btn_reset_gpu = Button::with_label("Reset Clock");
    btn_reset_gpu.add_css_class("cryo-btn");
    {
        let scb = Rc::clone(&status_cb);
        btn_reset_gpu.connect_clicked(move |_| {
            if ffi::set_gpu_clock(0) {
                scb("Reset NVIDIA GPU core clock lock to driver default");
            }
        });
    }
    nv_lbl.set_halign(Align::Start);
    ppt_card.append(&nv_lbl);
    nv_row.append(&nv_clock_scale);
    nv_row.append(&btn_lock_gpu);
    nv_row.append(&btn_reset_gpu);
    ppt_card.append(&nv_row);

    // --- Card C: Display (Hyprland Hz + Panel OD) & Battery Charge Limit ---
    let disp_card = GtkBox::new(Orientation::Vertical, 10);
    disp_card.add_css_class("serpent-card");
    disp_card.set_hexpand(true);

    let disp_hdr = Label::new(Some("◆ HYPRLAND DISPLAY REFRESH & BATTERY HEALTH"));
    disp_hdr.add_css_class("card-title");
    disp_hdr.set_halign(Align::Start);
    disp_hdr.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    disp_card.append(&disp_hdr);

    let hz_row = flow_row();
    let hz_lbl = Label::new(Some(&format!("Refresh Rate ({} Hz):", init_snap.raw.hz_cur)));
    hz_lbl.add_css_class("dim-label");
    flow_add(&hz_row, &hz_lbl);

    let modes_hz = {
        let m = init_snap.supported_hz();
        if m.is_empty() {
            vec![60, 144]
        } else {
            m
        }
    };
    for hz in modes_hz {
        let b = Button::with_label(&format!("{} Hz", hz));
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            if ffi::set_hz(hz) {
                scb(&format!("Verified Hyprland display refresh rate: {} Hz", hz));
            } else {
                scb(&format!("Failed to switch display to {} Hz", hz));
            }
        });
        flow_add(&hz_row, &b);
    }

    let btn_od = Button::with_label(if init_snap.raw.panel_od != 0 {
        "Panel Overdrive: ON"
    } else {
        "Panel Overdrive: OFF"
    });
    btn_od.add_css_class("cryo-btn");
    {
        let scb = Rc::clone(&status_cb);
        let b_clone = btn_od.clone();
        btn_od.connect_clicked(move |_| {
            let cur = ffi::get_snapshot(false).raw.panel_od != 0;
            let next = !cur;
            if ffi::set_panel_od(next) {
                b_clone.set_label(if next {
                    "Panel Overdrive: ON"
                } else {
                    "Panel Overdrive: OFF"
                });
                scb(if next {
                    "Enabled panel overdrive"
                } else {
                    "Disabled panel overdrive"
                });
            }
        });
    }
    flow_add(&hz_row, &btn_od);
    disp_card.append(&hz_row);

    // Battery Charge Limit
    let bat_row = GtkBox::new(Orientation::Horizontal, 8);
    let bat_lbl = Label::new(Some(&format!(
        "Charge Ceiling ({}%):",
        init_snap.raw.bat_limit.max(80)
    )));
    bat_lbl.add_css_class("dim-label");
    bat_lbl.set_halign(Align::Start);
    bat_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let bat_scale = Scale::with_range(Orientation::Horizontal, 20.0, 100.0, 5.0);
    bat_scale.set_value(init_snap.raw.bat_limit.max(80) as f64);
    bat_scale.set_hexpand(true);
    bat_scale.set_size_request(140, -1);
    bat_scale.set_draw_value(true);
    bat_scale.set_value_pos(gtk4::PositionType::Right);
    bat_scale.set_digits(0);

    let btn_set_bat = Button::with_label("Set Limit");
    btn_set_bat.add_css_class("cryo-btn");
    {
        let sc = bat_scale.clone();
        let scb = Rc::clone(&status_cb);
        btn_set_bat.connect_clicked(move |_| {
            let pct = sc.value().round() as i32;
            if ffi::set_battery_limit(pct) {
                scb(&format!("Set battery charge threshold to {}%", pct));
            } else {
                scb("Failed to set battery charge threshold");
            }
        });
    }

    let btn_oneshot = Button::with_label("⚡ One-Shot 100%");
    btn_oneshot.add_css_class("cryo-btn");
    {
        let scb = Rc::clone(&status_cb);
        btn_oneshot.connect_clicked(move |_| {
            if ffi::trigger_battery_oneshot() {
                scb("Triggered one-shot 100% full battery charge");
            } else {
                scb("One-shot battery charge command failed");
            }
        });
    }

    disp_card.append(&bat_lbl);
    bat_row.append(&bat_scale);
    bat_row.append(&btn_set_bat);
    bat_row.append(&btn_oneshot);
    disp_card.append(&bat_row);

    mid_row.append(&cpu_card);
    mid_row.append(&ppt_card);
    outer.append(&mid_row);
    outer.append(&disp_card);
    scroll.set_child(Some(&outer));

    let canvas_refresh = fan_canvas.clone();
    let refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)> = Rc::new(move |snap: &ffi::HwSnapshot| {
        for (idx, btn) in &prof_btns {
            if snap.raw.profile == *idx {
                btn.add_css_class("cryo-btn-active");
            } else {
                btn.remove_css_class("cryo-btn-active");
            }
        }
        for (idx, btn) in &epp_btns {
            if snap.raw.epp == *idx {
                btn.add_css_class("cryo-btn-active");
            } else {
                btn.remove_css_class("cryo-btn-active");
            }
        }
        freq_lbl.set_text(&format!(
            "CPU Ceiling: {} MHz (Live {} MHz)",
            snap.raw.cpu_mhz_limit, snap.raw.cpu_mhz_cur
        ));
        hz_lbl.set_text(&format!("Refresh Rate ({} Hz):", snap.raw.hz_cur));
        bat_lbl.set_text(&format!(
            "Charge Ceiling ({}% · {} · {}):",
            snap.raw.bat_limit, snap.bat_status, snap.bat_power_fmt
        ));
        canvas_refresh.queue_draw();
    });

    CtronViewHandles {
        root: scroll,
        refresh_ui,
    }
}
