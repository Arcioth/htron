use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, DrawingArea, Label, Orientation, PolicyType, ScrolledWindow};

use crate::ffi;
use crate::ui::{draw_cryo_grid, responsive_box};

#[derive(Clone)]
pub struct TelemViewHandles {
    pub root: ScrolledWindow,
    pub refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)>,
}

fn draw_spark_area(
    cr: &gtk4::cairo::Context,
    w: f64,
    h: f64,
    samples: &[f64],
    fixed_min: Option<f64>,
    fixed_max: Option<f64>,
    r: f64,
    g: f64,
    b: f64,
) {
    draw_cryo_grid(cr, w, h);
    let valid: Vec<f64> = samples.iter().copied().filter(|&v| v >= 0.0).collect();
    if valid.len() < 2 {
        cr.set_source_rgba(0.49, 0.98, 1.0, 0.45);
        cr.set_font_size(11.0);
        cr.move_to(14.0, h * 0.5);
        let _ = cr.show_text("Collecting cryo-telemetry samples...");
        return;
    }

    let mut mn = fixed_min.unwrap_or_else(|| valid.iter().cloned().fold(f64::INFINITY, f64::min));
    let mut mx =
        fixed_max.unwrap_or_else(|| valid.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    if (mx - mn).abs() < 1e-3 {
        mn -= 1.0;
        mx += 1.0;
    }

    let pad_x = 10.0;
    let pad_y = 12.0;
    let pw = (w - pad_x * 2.0).max(10.0);
    let ph = (h - pad_y * 2.0).max(10.0);

    let grad = gtk4::cairo::LinearGradient::new(0.0, pad_y, 0.0, h - pad_y);
    grad.add_color_stop_rgba(0.0, r, g, b, 0.36);
    grad.add_color_stop_rgba(1.0, r, g, b, 0.02);

    let n = valid.len();
    cr.move_to(pad_x, h - pad_y);
    for (i, &v) in valid.iter().enumerate() {
        let x = pad_x + (i as f64 / (n - 1) as f64) * pw;
        let ratio = ((v - mn) / (mx - mn)).clamp(0.0, 1.0);
        let y = pad_y + (1.0 - ratio) * ph;
        cr.line_to(x, y);
    }
    cr.line_to(pad_x + pw, h - pad_y);
    cr.close_path();
    let _ = cr.set_source(&grad);
    let _ = cr.fill();

    cr.set_source_rgba(r, g, b, 0.95);
    cr.set_line_width(2.2);
    for (i, &v) in valid.iter().enumerate() {
        let x = pad_x + (i as f64 / (n - 1) as f64) * pw;
        let ratio = ((v - mn) / (mx - mn)).clamp(0.0, 1.0);
        let y = pad_y + (1.0 - ratio) * ph;
        if i == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    let _ = cr.stroke();
}

pub fn build_telem_view() -> TelemViewHandles {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let outer = GtkBox::new(Orientation::Vertical, 14);
    outer.set_margin_top(14);
    outer.set_margin_bottom(14);
    outer.set_margin_start(14);
    outer.set_margin_end(14);

    let banner = GtkBox::new(Orientation::Vertical, 4);
    banner.add_css_class("serpent-card");
    let b_title = Label::new(Some(
        "◆ 60-SECOND ROLLING CRYO-TELEMETRY MATRIX (READ-ONLY · ZERO WRITES)",
    ));
    b_title.add_css_class("card-title");
    b_title.set_hexpand(true);
    b_title.set_xalign(0.0);
    b_title.set_width_chars(12);
    b_title.set_ellipsize(EllipsizeMode::End);

    let b_meta = Label::new(Some("--"));
    b_meta.add_css_class("card-subtitle");
    b_meta.set_hexpand(true);
    b_meta.set_xalign(0.0);
    b_meta.set_width_chars(12);
    b_meta.set_ellipsize(EllipsizeMode::End);
    banner.append(&b_title);
    banner.append(&b_meta);
    outer.append(&banner);

    // Helper to build a telemetry card
    let make_card = |title: &str,
                     series_idx: i32,
                     fixed_min: Option<f64>,
                     fixed_max: Option<f64>,
                     (r, g, b): (f64, f64, f64)| {
        let card = GtkBox::new(Orientation::Vertical, 8);
        card.add_css_class("serpent-card");
        card.set_hexpand(true);

        let hdr = GtkBox::new(Orientation::Horizontal, 8);
        let t_lbl = Label::new(Some(title));
        t_lbl.add_css_class("card-title");
        t_lbl.set_xalign(0.0);
        t_lbl.set_hexpand(true);
        t_lbl.set_width_chars(10);
        t_lbl.set_ellipsize(EllipsizeMode::End);

        let val_lbl = Label::new(Some("--"));
        val_lbl.add_css_class("cryo-chip");
        hdr.append(&t_lbl);
        hdr.append(&val_lbl);

        let sub_lbl = Label::new(Some("--"));
        sub_lbl.add_css_class("card-subtitle");
        sub_lbl.set_xalign(0.0);
        sub_lbl.set_hexpand(true);
        sub_lbl.set_width_chars(10);
        sub_lbl.set_ellipsize(EllipsizeMode::End);

        let area = DrawingArea::builder()
            .content_width(220)
            .content_height(130)
            .hexpand(true)
            .build();

        area.set_draw_func(move |_, cr, w, h| {
            let data = ffi::read_telem_series(series_idx);
            draw_spark_area(
                cr,
                w as f64,
                h as f64,
                &data.samples,
                fixed_min,
                fixed_max,
                r,
                g,
                b,
            );
        });

        card.append(&hdr);
        card.append(&sub_lbl);
        card.append(&area);
        (card, val_lbl, sub_lbl, area)
    };

    let (c_cpu, v_cpu, s_cpu, a_cpu) = make_card(
        "◆ CPU THERMAL & CLOCK",
        ffi::TS_CPU_TEMP,
        Some(25.0),
        Some(100.0),
        (0.0, 0.94, 1.0),
    );
    let (c_gpu, v_gpu, s_gpu, a_gpu) = make_card(
        "◆ dGPU (NVIDIA) THERMAL & POWER",
        ffi::TS_GPU_TEMP,
        Some(25.0),
        Some(95.0),
        (0.65, 0.55, 0.98),
    );
    let (c_fan, v_fan, s_fan, a_fan) = make_card(
        "◆ COOLING TURBINES (CPU / GPU RPM)",
        ffi::TS_FAN_CPU,
        Some(0.0),
        Some(5800.0),
        (0.08, 0.72, 0.65),
    );
    let (c_igpu, v_igpu, s_igpu, a_igpu) = make_card(
        "◆ iGPU / SOC CLOCK & DRAW",
        ffi::TS_CPU_MHZ,
        None,
        None,
        (0.22, 0.74, 0.97),
    );
    let (c_mem, v_mem, s_mem, a_mem) = make_card(
        "◆ MEMORY & SPD5118 THERMAL",
        ffi::TS_MEM_USED,
        Some(0.0),
        None,
        (0.80, 1.0, 0.99),
    );
    let (c_bat, v_bat, s_bat, a_bat) = make_card(
        "◆ BATTERY / USB-C PD & NVMe STORAGE",
        ffi::TS_BAT_MW,
        Some(0.0),
        None,
        (0.98, 0.44, 0.52),
    );

    let row1 = responsive_box(14);
    row1.append(&c_cpu);
    row1.append(&c_gpu);

    let row2 = responsive_box(14);
    row2.append(&c_fan);
    row2.append(&c_igpu);

    let row3 = responsive_box(14);
    row3.append(&c_mem);
    row3.append(&c_bat);

    outer.append(&row1);
    outer.append(&row2);
    outer.append(&row3);
    scroll.set_child(Some(&outer));

    let refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)> = Rc::new(move |snap: &ffi::HwSnapshot| {
        b_meta.set_text(&format!(
            "{} · {} · Profile: {}",
            snap.model,
            snap.cpu,
            snap.profile_name()
        ));

        let cpu_t = if snap.raw.cpu_temp > 0 {
            format!("{} °C", snap.raw.cpu_temp)
        } else {
            "--".to_string()
        };
        v_cpu.set_text(&format!("{} · {} MHz", cpu_t, snap.raw.cpu_mhz_cur));
        s_cpu.set_text(&format!(
            "Ceiling: {} MHz · Load1: {:.2} · Boost: {}",
            snap.raw.cpu_mhz_limit,
            snap.raw.load1,
            if snap.raw.cpu_boost != 0 { "ON" } else { "OFF" }
        ));

        let gpu_t = if snap.raw.gpu_temp > 0 {
            format!("{} °C", snap.raw.gpu_temp)
        } else {
            "--".to_string()
        };
        v_gpu.set_text(&format!(
            "{} · {:.1} W · {}%",
            gpu_t,
            snap.raw.gpu_mw as f64 / 1000.0,
            snap.raw.gpu_util.max(0)
        ));
        s_gpu.set_text(&format!(
            "Clock: {} MHz · VRAM: {}/{} MB · DynBoost: {}W / {}°C",
            snap.raw.gpu_mhz.max(0),
            snap.raw.gpu_vram_used_mb.max(0),
            snap.raw.gpu_vram_total_mb.max(0),
            snap.raw.nv_boost,
            snap.raw.nv_temp
        ));

        v_fan.set_text(&format!(
            "CPU {} · GPU {} RPM",
            snap.raw.rpm_cpu.max(0),
            snap.raw.rpm_gpu.max(0)
        ));
        s_fan.set_text(&format!(
            "Custom Curves: CPU {} / GPU {}",
            if snap.raw.fan_cpu_on != 0 { "ON" } else { "OFF" },
            if snap.raw.fan_gpu_on != 0 { "ON" } else { "OFF" }
        ));

        let igpu_suffix = if snap.raw.igpu_temp > 0 {
            format!(" · iGPU {} °C", snap.raw.igpu_temp)
        } else {
            String::new()
        };
        v_igpu.set_text(&format!("{} MHz{}", snap.raw.cpu_mhz_cur, igpu_suffix));
        s_igpu.set_text(&format!(
            "EPP: {} · Display: {} Hz",
            snap.epp_name(),
            snap.raw.hz_cur
        ));

        v_mem.set_text(&format!(
            "{} / {} MB",
            snap.raw.mem_used_mb, snap.raw.mem_total_mb
        ));
        let dimm_str = if snap.raw.ram_temp0 > 0 || snap.raw.ram_temp1 > 0 {
            format!(
                "DIMM Temps: {}/{} °C · ",
                snap.raw.ram_temp0.max(0),
                snap.raw.ram_temp1.max(0)
            )
        } else {
            String::new()
        };
        s_mem.set_text(&format!(
            "{}Swap: {}/{} MB",
            dimm_str, snap.raw.swap_used_mb, snap.raw.swap_total_mb
        ));

        v_bat.set_text(&format!("{}% · {}", snap.raw.bat_pct, snap.bat_power_fmt));
        let nvme_str = if snap.raw.nvme_temp0 > 0 {
            format!(" · NVMe: {} °C", snap.raw.nvme_temp0)
        } else {
            String::new()
        };
        s_bat.set_text(&format!(
            "Status: {} (Limit {}%){} · Disk /: {}/{} MB",
            snap.bat_status,
            snap.raw.bat_limit,
            nvme_str,
            snap.raw.fs_used_mb,
            snap.raw.fs_total_mb
        ));

        a_cpu.queue_draw();
        a_gpu.queue_draw();
        a_fan.queue_draw();
        a_igpu.queue_draw();
        a_mem.queue_draw();
        a_bat.queue_draw();
    });

    TelemViewHandles {
        root: scroll,
        refresh_ui,
    }
}
