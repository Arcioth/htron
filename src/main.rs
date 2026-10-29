mod audio;
mod ffi;
mod ui;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, DrawingArea, Label, MenuButton, Orientation,
    Popover, Scale, Stack, StackTransitionType, STYLE_PROVIDER_PRIORITY_APPLICATION,
    STYLE_PROVIDER_PRIORITY_USER,
};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::audio::{format_mmss, AudioCmd, PlayState};
use crate::ui::build_user_texture_css;
use crate::ui::ctron_view::build_ctron_view;
use crate::ui::daeboard_view::build_daeboard_view;
use crate::ui::draw_serpent_emblem;
use crate::ui::telem_view::build_telem_view;
use crate::ui::tunotron_view::build_tunotron_view;

const APP_ID: &str = "dev.arcioth.htron";

fn print_headless_status() {
    ffi::init_core();
    let s = ffi::get_snapshot(true);
    println!("htron v0.2.0 — {} ({})", s.model, s.cpu);
    println!(
        "  Profile / EPP : {} / {}",
        s.profile_name(),
        s.epp_name()
    );
    println!(
        "  CPU           : {} °C · {} MHz (limit {} MHz, boost {})",
        s.raw.cpu_temp,
        s.raw.cpu_mhz_cur,
        s.raw.cpu_mhz_limit,
        if s.raw.cpu_boost != 0 { "on" } else { "off" }
    );
    println!(
        "  dGPU          : {} °C · {} MHz · {:.2} W · util {}%",
        s.raw.gpu_temp,
        s.raw.gpu_mhz,
        s.raw.gpu_mw as f64 / 1000.0,
        s.raw.gpu_util
    );
    println!(
        "  Fans          : {} / {} RPM (cpu/gpu) · custom {}/{}",
        s.raw.rpm_cpu,
        s.raw.rpm_gpu,
        if s.raw.fan_cpu_on != 0 { "on" } else { "off" },
        if s.raw.fan_gpu_on != 0 { "on" } else { "off" }
    );
    println!(
        "  Battery       : {}% {} ({}) · limit {}%",
        s.raw.bat_pct, s.bat_status, s.bat_power_fmt, s.raw.bat_limit
    );
    println!(
        "  Display       : {} Hz · Panel OD {}",
        s.raw.hz_cur,
        if s.raw.panel_od != 0 { "on" } else { "off" }
    );
    println!(
        "  Daeboard      : {} · Keyboard LED: {}",
        if s.raw.daeboard_up != 0 {
            "online (pong)"
        } else {
            "stopped"
        },
        s.kbd_name()
    );
}

fn ensure_nixos_runtime_env() {
    let extra_schemas = [
        "/nix/store/8g01hy2crkmnjb5v1b6nmgbfrs6y4h39-gtk4-4.22.4/share/gsettings-schemas/gtk4-4.22.4",
        "/nix/store/sbfjpm25ddfzvbqs1p6v72mik7kfvqbw-gtk4-4.22.4/share/gsettings-schemas/gtk4-4.22.4",
        "/nix/store/f0fiqn9dnivd4ihirkij2vmbbgflykxw-gsettings-desktop-schemas-50.1/share/gsettings-schemas/gsettings-desktop-schemas-50.1",
        "/run/current-system/sw/share",
    ];
    let mut dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for p in extra_schemas {
        if std::path::Path::new(p).exists() && !dirs.contains(p) {
            if !dirs.is_empty() {
                dirs.push(':');
            }
            dirs.push_str(p);
        }
    }
    std::env::set_var("XDG_DATA_DIRS", dirs);
}

fn main() {
    ensure_nixos_runtime_env();
    let args: Vec<String> = std::env::args().collect();
    let mut initial_tab = "ctron".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--status" | "-s" => {
                print_headless_status();
                return;
            }
            "--mode" if i + 1 < args.len() => {
                ffi::init_core();
                match ffi::apply_mode(&args[i + 1]) {
                    Ok(()) => {
                        println!("htron: applied mode '{}'", args[i + 1]);
                        return;
                    }
                    Err(e) => {
                        eprintln!("htron: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "--view" | "--tab" if i + 1 < args.len() => {
                initial_tab = args[i + 1].clone();
                i += 1;
            }
            "--help" | "-h" => {
                println!("Usage: htron [--status] [--mode <name>] [--view ctron|telem|daeboard|tunotron]");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    ffi::init_core();

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    let start_tab = initial_tab.clone();
    app.connect_activate(move |app| {
        let base_provider = CssProvider::new();
        base_provider.load_from_data(include_str!("style.css"));

        let user_tex_provider = CssProvider::new();
        let (init_tex_css, init_tex_cnt) = build_user_texture_css();
        user_tex_provider.load_from_data(&init_tex_css);

        if let Some(display) = Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &base_provider,
                STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            gtk4::style_context_add_provider_for_display(
                &display,
                &user_tex_provider,
                STYLE_PROVIDER_PRIORITY_USER,
            );
        }

        let music_dir = std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Music"))
            .unwrap_or_else(|_| PathBuf::from("/home/arcioth/Music"));
        let audio = Rc::new(audio::start_audio_engine(music_dir));

        let root_vbox = GtkBox::new(Orientation::Vertical, 0);

        // Status strip at very bottom
        let status_strip = GtkBox::new(Orientation::Horizontal, 10);
        status_strip.add_css_class("status-strip");
        let status_lbl = Label::new(Some(&format!(
            "◆ htron v0.2.0 Ready — Cryo-Ophidian Hardware & Audio Suite{}",
            if init_tex_cnt > 0 {
                format!(" ({} custom textures active)", init_tex_cnt)
            } else {
                String::new()
            }
        )));
        status_lbl.set_xalign(0.0);
        status_lbl.set_hexpand(true);
        status_lbl.set_width_chars(12);
        status_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let hw_ident_lbl = Label::new(Some(""));
        hw_ident_lbl.set_xalign(1.0);
        hw_ident_lbl.set_max_width_chars(38);
        hw_ident_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        status_strip.append(&status_lbl);
        status_strip.append(&hw_ident_lbl);

        let status_cb: Rc<dyn Fn(&str)> = {
            let sl = status_lbl.clone();
            Rc::new(move |msg: &str| {
                sl.set_text(&format!("◆ {}", msg));
            })
        };

        // =================================================================
        // TOP HEADER BAR (ICE SERPENT CREST + LIVE SENSOR PILLS + QUICK MODES)
        // =================================================================
        let header = GtkBox::new(Orientation::Horizontal, 6);
        header.add_css_class("serpent-header");

        let crest_canvas = DrawingArea::builder()
            .content_width(34)
            .content_height(34)
            .build();
        let crest_phase = Rc::new(RefCell::new(0.0f64));
        {
            let cp = Rc::clone(&crest_phase);
            crest_canvas.set_draw_func(move |_, cr, w, h| {
                draw_serpent_emblem(cr, w as f64, h as f64, *cp.borrow());
            });
        }

        let brand_box = GtkBox::new(Orientation::Vertical, 0);
        let brand_t = Label::new(Some("HTRON"));
        brand_t.add_css_class("brand-title");
        brand_t.set_halign(Align::Start);
        let brand_s = Label::new(Some("CRYO-OPHIDIAN SUITE"));
        brand_s.add_css_class("brand-subtitle");
        brand_s.set_halign(Align::Start);
        brand_box.append(&brand_t);
        brand_box.append(&brand_s);

        header.append(&crest_canvas);
        header.append(&brand_box);

        let hdr_spacer = GtkBox::new(Orientation::Horizontal, 0);
        hdr_spacer.set_hexpand(true);
        header.append(&hdr_spacer);

        let pill_cpu = Label::new(Some("CPU --°C"));
        pill_cpu.add_css_class("sensor-pill");
        let pill_gpu = Label::new(Some("GPU --°C"));
        pill_gpu.add_css_class("sensor-pill");
        pill_gpu.add_css_class("sensor-pill-venom");
        let pill_fan = Label::new(Some("--/-- RPM"));
        pill_fan.add_css_class("sensor-pill");
        let pill_bat = Label::new(Some("BAT --%"));
        pill_bat.add_css_class("sensor-pill");

        header.append(&pill_cpu);
        header.append(&pill_gpu);
        header.append(&pill_fan);
        header.append(&pill_bat);

        // Quick ctron modes live in a compact popover so the header never
        // forces the window wider than a half-screen Hyprland tile.
        let mode_menu = MenuButton::new();
        mode_menu.set_label("⚡ Mode");
        mode_menu.add_css_class("cryo-menu");
        mode_menu.set_tooltip_text(Some("Apply a ctron shortcut mode (~/.config/ctron/modes.ini)"));
        let mode_pop = Popover::new();
        mode_pop.add_css_class("cryo-popover");
        let mode_pop_box = GtkBox::new(Orientation::Vertical, 6);
        for (label, mname) in [
            ("🌙  Silent", "silent"),
            ("🍃  Quiet", "quiet"),
            ("⚖  Balanced", "balanced"),
            ("🔥  Performance", "performance"),
            ("⚡  Turbo", "turbo"),
        ] {
            let b = Button::with_label(label);
            b.add_css_class("cryo-btn");
            let scb = Rc::clone(&status_cb);
            let mn = mname.to_string();
            let pop = mode_pop.clone();
            b.connect_clicked(move |_| {
                pop.popdown();
                match ffi::apply_mode(&mn) {
                    Ok(()) => scb(&format!("Switched to mode [{}]", mn)),
                    Err(e) => scb(&format!("Mode [{}]: {}", mn, e)),
                }
            });
            mode_pop_box.append(&b);
        }
        mode_pop.set_child(Some(&mode_pop_box));
        mode_menu.set_popover(Some(&mode_pop));
        header.append(&mode_menu);

        // Live User Texture Pack Reload Button (~/.config/htron/textures/)
        let btn_reload_tex = Button::with_label("🎨");
        btn_reload_tex.add_css_class("cryo-btn");
        btn_reload_tex.set_tooltip_text(Some(
            "Reload custom textures from ~/.config/htron/textures/",
        ));
        {
            let utp = user_tex_provider.clone();
            let scb = Rc::clone(&status_cb);
            btn_reload_tex.connect_clicked(move |_| {
                let (css, count) = build_user_texture_css();
                utp.load_from_data(&css);
                if count > 0 {
                    scb(&format!(
                        "Applied {} custom texture override(s) from ~/.config/htron/textures/",
                        count
                    ));
                } else {
                    scb("No custom textures found in ~/.config/htron/textures/");
                }
            });
        }
        header.append(&btn_reload_tex);

        root_vbox.append(&header);

        // =================================================================
        // MIDDLE WORKSPACE: LEFT SERPENT RAIL + MAIN VIEW STACK
        // =================================================================
        let mid_hbox = GtkBox::new(Orientation::Horizontal, 0);
        mid_hbox.set_hexpand(true);
        mid_hbox.set_vexpand(true);

        let rail = GtkBox::new(Orientation::Vertical, 4);
        rail.add_css_class("serpent-rail");
        rail.set_size_request(168, -1);

        let rail_lbl1 = Label::new(Some("SYSTEM & HARDWARE"));
        rail_lbl1.add_css_class("rail-section-label");
        rail_lbl1.set_halign(Align::Start);
        rail.append(&rail_lbl1);

        let btn_nav_ctron = Button::with_label("◆  Ctron Control");
        btn_nav_ctron.add_css_class("nav-scale-btn");
        btn_nav_ctron.add_css_class("nav-scale-btn-active");

        let btn_nav_telem = Button::with_label("◈  Cryo Telemetry");
        btn_nav_telem.add_css_class("nav-scale-btn");

        let btn_nav_daeboard = Button::with_label("❖  Daeboard Studio");
        btn_nav_daeboard.add_css_class("nav-scale-btn");

        rail.append(&btn_nav_ctron);
        rail.append(&btn_nav_telem);
        rail.append(&btn_nav_daeboard);

        let rail_lbl2 = Label::new(Some("AUDIO & MEDIA"));
        rail_lbl2.add_css_class("rail-section-label");
        rail_lbl2.set_halign(Align::Start);
        rail.append(&rail_lbl2);

        let btn_nav_tuno = Button::with_label("♪  Tunotron Deck");
        btn_nav_tuno.add_css_class("nav-scale-btn");
        rail.append(&btn_nav_tuno);

        let rail_spacer = GtkBox::new(Orientation::Vertical, 0);
        rail_spacer.set_vexpand(true);
        rail.append(&rail_spacer);

        let dae_rail_badge = Label::new(Some("DAEBOARD: CHECKING"));
        dae_rail_badge.add_css_class("cryo-chip");
        rail.append(&dae_rail_badge);

        // Build Views
        let stack = Stack::builder()
            .transition_type(StackTransitionType::Crossfade)
            .transition_duration(160)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .hexpand(true)
            .vexpand(true)
            .build();

        let nav_buttons = vec![
            ("ctron", btn_nav_ctron.clone()),
            ("telem", btn_nav_telem.clone()),
            ("daeboard", btn_nav_daeboard.clone()),
            ("tunotron", btn_nav_tuno.clone()),
        ];

        let ctron_v = build_ctron_view(Rc::clone(&status_cb));
        let telem_v = build_telem_view();
        let daeboard_v = build_daeboard_view(Rc::clone(&status_cb));
        let tuno_v = build_tunotron_view(Rc::clone(&audio), Rc::clone(&status_cb));

        stack.add_named(&ctron_v.root, Some("ctron"));
        stack.add_named(&telem_v.root, Some("telem"));
        stack.add_named(&daeboard_v.root, Some("daeboard"));
        stack.add_named(&tuno_v.root, Some("tunotron"));

        let do_switch_tab: Rc<dyn Fn(&str)> = {
            let st_c = stack.clone();
            let all_btns = nav_buttons.clone();
            Rc::new(move |target: &str| {
                st_c.set_visible_child_name(target);
                for (n, b) in &all_btns {
                    if *n == target {
                        b.add_css_class("nav-scale-btn-active");
                    } else {
                        b.remove_css_class("nav-scale-btn-active");
                    }
                }
            })
        };

        for (name, btn) in &nav_buttons {
            let sw = Rc::clone(&do_switch_tab);
            let target = (*name).to_string();
            btn.connect_clicked(move |_| {
                sw(&target);
            });
        }

        if ["ctron", "telem", "daeboard", "tunotron"].contains(&start_tab.as_str()) {
            do_switch_tab(&start_tab);
        }

        mid_hbox.append(&rail);
        mid_hbox.append(&stack);
        root_vbox.append(&mid_hbox);

        // =================================================================
        // PERSISTENT BOTTOM CRYO-PLAYER DOCK (TUNOTRON)
        // =================================================================
        let dock = GtkBox::new(Orientation::Horizontal, 8);
        dock.add_css_class("cryo-dock");

        let btn_prev = Button::with_label("⏮");
        btn_prev.add_css_class("cryo-btn");
        let btn_play = Button::with_label("▶ Play");
        btn_play.add_css_class("fang-action-btn");
        let btn_next = Button::with_label("⏭");
        btn_next.add_css_class("cryo-btn");
        let btn_stop = Button::with_label("⏹");
        btn_stop.add_css_class("cryo-btn");

        {
            let aud = Rc::clone(&audio);
            btn_prev.connect_clicked(move |_| aud.send(AudioCmd::Prev));
        }
        {
            let aud = Rc::clone(&audio);
            btn_play.connect_clicked(move |_| aud.send(AudioCmd::TogglePause));
        }
        {
            let aud = Rc::clone(&audio);
            btn_next.connect_clicked(move |_| aud.send(AudioCmd::Next));
        }
        {
            let aud = Rc::clone(&audio);
            btn_stop.connect_clicked(move |_| aud.send(AudioCmd::Stop));
        }

        dock.append(&btn_prev);
        dock.append(&btn_play);
        dock.append(&btn_next);
        dock.append(&btn_stop);

        let dock_track_lbl =
            Label::new(Some("♪ Tunotron Ready — Open Tunotron Deck to play ~/Music"));
        dock_track_lbl.set_xalign(0.0);
        dock_track_lbl.set_hexpand(true);
        dock_track_lbl.set_width_chars(10);
        dock_track_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        dock.append(&dock_track_lbl);

        let dock_time_lbl = Label::new(Some("00:00 / 00:00"));
        dock_time_lbl.add_css_class("cryo-chip");
        dock.append(&dock_time_lbl);

        let btn_loop = Button::with_label("🔁 All");
        btn_loop.add_css_class("cryo-btn");
        btn_loop.set_tooltip_text(Some("Cycle loop mode"));
        {
            let aud = Rc::clone(&audio);
            btn_loop.connect_clicked(move |_| aud.send(AudioCmd::CycleLoop));
        }

        let btn_shuf = Button::with_label("🔀 Off");
        btn_shuf.add_css_class("cryo-btn");
        btn_shuf.set_tooltip_text(Some("Toggle shuffle"));
        {
            let aud = Rc::clone(&audio);
            btn_shuf.connect_clicked(move |_| aud.send(AudioCmd::ToggleShuffle));
        }

        let vol_lbl = Label::new(Some("🔊"));
        vol_lbl.add_css_class("dim-label");
        let vol_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
        vol_scale.set_value(100.0);
        vol_scale.set_size_request(80, -1);
        {
            let aud = Rc::clone(&audio);
            vol_scale.connect_value_changed(move |s| {
                aud.send(AudioCmd::SetVolume(s.value()));
            });
        }

        dock.append(&btn_loop);
        dock.append(&btn_shuf);
        dock.append(&vol_lbl);
        dock.append(&vol_scale);

        root_vbox.append(&dock);
        root_vbox.append(&status_strip);

        // =================================================================
        // PERIODIC READ-ONLY TELEMETRY & AUDIO SYNC (250 ms)
        // =================================================================
        let tick_counter = Rc::new(RefCell::new(0u64));
        {
            let cp = Rc::clone(&crest_phase);
            let crest_c = crest_canvas.clone();
            let p_cpu = pill_cpu.clone();
            let p_gpu = pill_gpu.clone();
            let p_fan = pill_fan.clone();
            let p_bat = pill_bat.clone();
            let d_badge = dae_rail_badge.clone();
            let hw_id = hw_ident_lbl.clone();
            let st_view = stack.clone();
            let ctron_cb = Rc::clone(&ctron_v.refresh_ui);
            let telem_cb = Rc::clone(&telem_v.refresh_ui);
            let dae_cb = Rc::clone(&daeboard_v.refresh_ui);
            let tuno_cb = Rc::clone(&tuno_v.refresh_ui);
            let aud = Rc::clone(&audio);
            let d_track = dock_track_lbl.clone();
            let d_time = dock_time_lbl.clone();
            let b_play = btn_play.clone();
            let b_loop = btn_loop.clone();
            let b_shuf = btn_shuf.clone();
            let tc = Rc::clone(&tick_counter);

            gtk4::glib::timeout_add_local(Duration::from_millis(250), move || {
                let mut t = tc.borrow_mut();
                *t += 1;
                let cur_tick = *t;
                drop(t);

                *cp.borrow_mut() += 0.14;
                crest_c.queue_draw();

                // Read-only fast sensor poll (never writes hardware)
                ffi::refresh_fast();
                let probe_db = cur_tick % 8 == 1;
                let snap = ffi::get_snapshot(probe_db);

                p_cpu.set_text(&format!("CPU {}°C", snap.raw.cpu_temp.max(0)));
                if snap.raw.cpu_temp >= 82 {
                    p_cpu.add_css_class("sensor-pill-hot");
                } else {
                    p_cpu.remove_css_class("sensor-pill-hot");
                }

                p_gpu.set_text(&format!("dGPU {}°C", snap.raw.gpu_temp.max(0)));
                p_fan.set_text(&format!(
                    "{}/{} RPM",
                    snap.raw.rpm_cpu.max(0),
                    snap.raw.rpm_gpu.max(0)
                ));
                p_bat.set_text(&format!("BAT {}%", snap.raw.bat_pct));

                if probe_db {
                    d_badge.set_text(if snap.raw.daeboard_up != 0 {
                        "● DAEBOARD: ONLINE"
                    } else {
                        "○ DAEBOARD: STOPPED"
                    });
                }

                hw_id.set_text(&format!(
                    "{} · Profile: {} · {} Hz",
                    snap.model,
                    snap.profile_name(),
                    snap.raw.hz_cur
                ));

                // Update visible workspace tab
                if let Some(vis) = st_view.visible_child_name() {
                    match vis.as_str() {
                        "ctron" => ctron_cb(&snap),
                        "telem" => telem_cb(&snap),
                        "daeboard" => {
                            if probe_db {
                                dae_cb(&snap);
                            }
                        }
                        "tunotron" => tuno_cb(),
                        _ => {}
                    }
                }

                // Update bottom Tunotron dock
                let asnap = aud.snapshot();
                match asnap.state {
                    PlayState::Playing => b_play.set_label("⏸ Pause"),
                    PlayState::Paused => b_play.set_label("▶ Resume"),
                    PlayState::Stopped => b_play.set_label("▶ Play"),
                }
                if let Some(tr) = &asnap.current_track {
                    d_track.set_text(&format!("♪  {} — {}  ({})", tr.artist, tr.title, tr.album));
                } else {
                    d_track.set_text("♪  Tunotron Stopped — Select any track in Tunotron Deck");
                }
                d_time.set_text(&format!(
                    "{} / {}",
                    format_mmss(asnap.position_sec),
                    format_mmss(asnap.duration_sec)
                ));
                b_loop.set_label(&format!("🔁 {}", asnap.loop_mode.display_str()));
                b_shuf.set_label(if asnap.shuffle { "🔀 On" } else { "🔀 Off" });

                gtk4::glib::ControlFlow::Continue
            });
        }

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("htron — Cryo-Ophidian Hardware & Audio Suite")
            .default_width(1100)
            .default_height(720)
            .width_request(720)
            .height_request(480)
            .content(&root_vbox)
            .build();
        window.add_css_class("htron-window");

        // Narrow layout (half-screen tiles): stack side-by-side cards vertically.
        let narrow = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 1250px")
                .expect("valid breakpoint condition"),
        );
        let vertical = Orientation::Vertical.to_value();
        for b in ui::take_responsive_boxes() {
            narrow.add_setter(&b, "orientation", Some(&vertical));
        }
        window.add_breakpoint(narrow);

        window.present();
    });

    app.run_with_args::<&str>(&[]);
}
