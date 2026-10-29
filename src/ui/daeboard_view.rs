use std::cell::RefCell;
use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DrawingArea, Entry, GestureClick, Label, Orientation, PolicyType,
    ScrolledWindow,
};

use crate::ffi;
use crate::ui::{draw_cryo_grid, flow_add, flow_row, responsive_box};

#[derive(Clone)]
pub struct DaeboardViewHandles {
    pub root: ScrolledWindow,
    pub refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)>,
}

struct KeyRect {
    label: &'static str,
    bind_name: &'static str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn build_keyboard_layout() -> Vec<KeyRect> {
    let mut keys = Vec::new();
    // Row 0: Esc + Function row + Delete
    let r0: &[(&str, &str, f64)] = &[
        ("ESC", "esc", 1.1),
        ("F1", "f1", 1.0),
        ("F2", "f2", 1.0),
        ("F3", "f3", 1.0),
        ("F4", "f4", 1.0),
        ("F5", "f5", 1.0),
        ("F6", "f6", 1.0),
        ("F7", "f7", 1.0),
        ("F8", "f8", 1.0),
        ("F9", "f9", 1.0),
        ("F10", "f10", 1.0),
        ("F11", "f11", 1.0),
        ("F12", "f12", 1.0),
        ("DEL", "delete", 1.3),
    ];
    // Row 1: Number row + Backspace
    let r1: &[(&str, &str, f64)] = &[
        ("`", "grave", 1.0),
        ("1", "key1", 1.0),
        ("2", "key2", 1.0),
        ("3", "key3", 1.0),
        ("4", "key4", 1.0),
        ("5", "key5", 1.0),
        ("6", "key6", 1.0),
        ("7", "key7", 1.0),
        ("8", "key8", 1.0),
        ("9", "key9", 1.0),
        ("0", "key0", 1.0),
        ("-", "minus", 1.0),
        ("=", "equal", 1.0),
        ("BKSP", "backspace", 2.4),
    ];
    // Row 2: Tab + QWERTY
    let r2: &[(&str, &str, f64)] = &[
        ("TAB", "tab", 1.5),
        ("Q", "q", 1.0),
        ("W", "w", 1.0),
        ("E", "e", 1.0),
        ("R", "r", 1.0),
        ("T", "t", 1.0),
        ("Y", "y", 1.0),
        ("U", "u", 1.0),
        ("I", "i", 1.0),
        ("O", "o", 1.0),
        ("P", "p", 1.0),
        ("[", "leftbrace", 1.0),
        ("]", "rightbrace", 1.0),
        ("\\", "backslash", 1.9),
    ];
    // Row 3: Caps + ASDF + Enter
    let r3: &[(&str, &str, f64)] = &[
        ("CAPS", "capslock", 1.8),
        ("A", "a", 1.0),
        ("S", "s", 1.0),
        ("D", "d", 1.0),
        ("F", "f", 1.0),
        ("G", "g", 1.0),
        ("H", "h", 1.0),
        ("J", "j", 1.0),
        ("K", "k", 1.0),
        ("L", "l", 1.0),
        (";", "semicolon", 1.0),
        ("'", "apostrophe", 1.0),
        ("ENTER", "enter", 2.6),
    ];
    // Row 4: Shift + ZXCV
    let r4: &[(&str, &str, f64)] = &[
        ("LSHIFT", "leftshift", 2.3),
        ("Z", "z", 1.0),
        ("X", "x", 1.0),
        ("C", "c", 1.0),
        ("V", "v", 1.0),
        ("B", "b", 1.0),
        ("N", "n", 1.0),
        ("M", "m", 1.0),
        (",", "comma", 1.0),
        (".", "dot", 1.0),
        ("/", "slash", 1.0),
        ("RSHIFT", "rightshift", 3.1),
    ];
    // Row 5: Bottom modifiers + Space
    let r5: &[(&str, &str, f64)] = &[
        ("CTRL", "leftctrl", 1.4),
        ("META", "leftmeta", 1.4),
        ("ALT", "leftalt", 1.4),
        ("SPACE", "space", 6.2),
        ("RALT", "rightalt", 1.3),
        ("RMETA", "rightmeta", 1.3),
        ("RCTRL", "rightctrl", 1.4),
    ];

    let rows = [r0, r1, r2, r3, r4, r5];
    let total_units = 15.4;
    for (ri, row) in rows.iter().enumerate() {
        let mut cur_u = 0.0;
        let y = (ri as f64) / 6.0;
        let h = 1.0 / 6.0;
        for &(label, bind_name, units) in *row {
            let x = cur_u / total_units;
            let w = units / total_units;
            keys.push(KeyRect {
                label,
                bind_name,
                x,
                y,
                w,
                h,
            });
            cur_u += units;
        }
    }
    keys
}

pub fn build_daeboard_view(status_cb: Rc<dyn Fn(&str)>) -> DaeboardViewHandles {
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

    let selected_key_idx = Rc::new(RefCell::new(0usize));
    let active_color_rgb = Rc::new(RefCell::new((0.80f64, 1.0f64, 0.99f64))); // #ccfffe default

    // =====================================================================
    // CARD 1: DAEBOARD DAEMON CONTROLLER & STATIC COLOR / BRIGHTNESS
    // =====================================================================
    let ctrl_card = GtkBox::new(Orientation::Vertical, 10);
    ctrl_card.add_css_class("serpent-card");

    let title = Label::new(Some(
        "◆ DAEBOARD KEYBOARD LED DAEMON & WRITER ARBITRATION",
    ));
    title.add_css_class("card-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_width_chars(12);
    title.set_ellipsize(EllipsizeMode::End);

    let sub = Label::new(Some(
        "Socket: /run/daeboard/daeboard.sock (SOCK_SEQPACKET) · Binds: ~/.config/ctron/daeboard.binds",
    ));
    sub.add_css_class("card-subtitle");
    sub.set_xalign(0.0);
    sub.set_hexpand(true);
    sub.set_width_chars(12);
    sub.set_ellipsize(EllipsizeMode::End);

    ctrl_card.append(&title);
    ctrl_card.append(&sub);

    let daemon_row = flow_row();
    let daemon_badge = Label::new(Some(if ffi::db_is_up() {
        "● ONLINE"
    } else {
        "○ STOPPED"
    }));
    daemon_badge.add_css_class("cryo-chip");

    let btn_start = Button::with_label("▶ Start Daemon");
    btn_start.add_css_class("cryo-btn");
    let btn_stop = Button::with_label("■ Stop");
    btn_stop.add_css_class("ember-btn");
    let btn_reload = Button::with_label("↻ Reload Binds");
    btn_reload.add_css_class("cryo-btn");

    flow_add(&daemon_row, &daemon_badge);
    flow_add(&daemon_row, &btn_start);
    flow_add(&daemon_row, &btn_stop);
    flow_add(&daemon_row, &btn_reload);
    ctrl_card.append(&daemon_row);

    // Row 1A: Static Color Swatches (wrapping FlowBox)
    let light_row = flow_row();
    let col_lbl = Label::new(Some("Resting Color:"));
    col_lbl.add_css_class("dim-label");
    flow_add(&light_row, &col_lbl);

    let kbd_canvas = DrawingArea::builder()
        .content_width(280)
        .content_height(195)
        .hexpand(true)
        .build();

    let swatches: &[(&str, &str, (f64, f64, f64))] = &[
        ("Permafrost #ccfffe", "ccfffe", (0.80, 1.00, 0.99)),
        ("Fang Cyan #00f0ff", "00f0ff", (0.00, 0.94, 1.00)),
        ("Serpent Teal #14b8a6", "14b8a6", (0.08, 0.72, 0.65)),
        ("Venom Violet #a78bfa", "a78bfa", (0.65, 0.55, 0.98)),
        ("Crimson #ff0044", "ff0044", (1.00, 0.00, 0.27)),
        ("Pure Frost #ffffff", "ffffff", (1.00, 1.00, 1.00)),
    ];

    for &(label, hex, rgb) in swatches {
        let b = Button::with_label(label);
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        let rgb_ref = Rc::clone(&active_color_rgb);
        let canvas_c = kbd_canvas.clone();
        let hex_s = hex.to_string();
        b.connect_clicked(move |_| {
            *rgb_ref.borrow_mut() = rgb;
            let ok = if ffi::db_is_up() {
                ffi::db_set_color(&hex_s)
            } else {
                ffi::set_aura_hex(&hex_s)
            };
            if ok {
                scb(&format!("Set keyboard resting color to #{}", hex_s));
            } else {
                scb(&format!("Failed to set keyboard color #{}", hex_s));
            }
            canvas_c.queue_draw();
        });
        flow_add(&light_row, &b);
    }
    ctrl_card.append(&light_row);

    // Row 1B: Keyboard Brightness
    let bright_row = flow_row();
    let br_lbl = Label::new(Some("Brightness:"));
    br_lbl.add_css_class("dim-label");
    flow_add(&bright_row, &br_lbl);

    for (bname, lvl) in [("Off (0)", 0), ("Low (1)", 1), ("Med (2)", 2), ("High (3)", 3)] {
        let b = Button::with_label(bname);
        b.add_css_class("cryo-btn");
        let scb = Rc::clone(&status_cb);
        b.connect_clicked(move |_| {
            let ok = if ffi::db_is_up() {
                ffi::db_set_brightness(lvl)
            } else {
                ffi::set_kbd(lvl)
            };
            if ok {
                scb(&format!("Keyboard brightness set to {}", bname));
            }
        });
        flow_add(&bright_row, &b);
    }

    ctrl_card.append(&bright_row);
    outer.append(&ctrl_card);

    // =====================================================================
    // CARD 2: INTERACTIVE ASUS TUF KEYBOARD RGB & BIND VISUALIZER
    // =====================================================================
    let deck_card = GtkBox::new(Orientation::Vertical, 8);
    deck_card.add_css_class("serpent-card");
    let deck_title = Label::new(Some(
        "◆ INTERACTIVE ASUS TUF KEYBOARD DECK (CLICK ANY KEY TO INSPECT OR BIND)",
    ));
    deck_title.add_css_class("card-title");
    deck_title.set_xalign(0.0);
    deck_title.set_hexpand(true);
    deck_title.set_width_chars(12);
    deck_title.set_ellipsize(EllipsizeMode::End);
    deck_card.append(&deck_title);

    let layout = Rc::new(build_keyboard_layout());
    {
        let lay = Rc::clone(&layout);
        let sel = Rc::clone(&selected_key_idx);
        let rgb_ref = Rc::clone(&active_color_rgb);
        kbd_canvas.set_draw_func(move |_, cr, w, h| {
            let wf = w as f64;
            let hf = h as f64;
            draw_cryo_grid(cr, wf, hf);

            let binds = ffi::binds_list();
            let cur_sel = *sel.borrow();
            let selected_name = binds
                .get(cur_sel)
                .map(|k| k.name.as_str())
                .unwrap_or("");
            let (ar, ag, ab) = *rgb_ref.borrow();

            let pad = 10.0;
            let kw = (wf - pad * 2.0).max(20.0);
            let kh = (hf - pad * 2.0).max(20.0);
            let font_sz = (wf / 68.0).clamp(7.5, 10.5);

            for kr in lay.iter() {
                let x = pad + kr.x * kw + 1.5;
                let y = pad + kr.y * kh + 1.5;
                let bw = (kr.w * kw - 3.0).max(4.0);
                let bh = (kr.h * kh - 3.0).max(4.0);

                let is_bound = binds
                    .iter()
                    .any(|b| b.name.eq_ignore_ascii_case(kr.bind_name));
                let is_selected = selected_name.eq_ignore_ascii_case(kr.bind_name);

                if is_selected {
                    cr.set_source_rgba(0.0, 0.94, 1.0, 0.36);
                } else if is_bound {
                    cr.set_source_rgba(0.65, 0.55, 0.98, 0.28);
                } else {
                    cr.set_source_rgba(0.04, 0.10, 0.16, 0.88);
                }
                cr.rectangle(x, y, bw, bh);
                let _ = cr.fill_preserve();

                if is_selected {
                    cr.set_source_rgba(0.0, 0.94, 1.0, 1.0);
                    cr.set_line_width(2.0);
                } else if is_bound {
                    cr.set_source_rgba(0.65, 0.55, 0.98, 0.92);
                    cr.set_line_width(1.5);
                } else {
                    cr.set_source_rgba(ar, ag, ab, 0.35);
                    cr.set_line_width(1.0);
                }
                let _ = cr.stroke();

                cr.set_font_size(font_sz);
                if is_selected || is_bound {
                    cr.set_source_rgba(0.92, 1.0, 1.0, 0.98);
                } else {
                    cr.set_source_rgba(ar, ag, ab, 0.85);
                }
                cr.move_to(x + 4.0, y + bh * 0.62);
                let _ = cr.show_text(kr.label);
            }
        });
    }

    deck_card.append(&kbd_canvas);
    outer.append(&deck_card);

    // =====================================================================
    // CARD 3: MACRO TIMELINE STUDIO (~/.config/ctron/daeboard.binds)
    // =====================================================================
    let studio_card = GtkBox::new(Orientation::Vertical, 10);
    studio_card.add_css_class("serpent-card");

    let st_hdr = GtkBox::new(Orientation::Horizontal, 8);
    let st_title = Label::new(Some("◆ MACRO TIMELINE STUDIO (DOWN / UP EDGES)"));
    st_title.add_css_class("card-title");
    st_title.set_xalign(0.0);
    st_title.set_hexpand(true);
    st_title.set_width_chars(10);
    st_title.set_ellipsize(EllipsizeMode::End);

    let btn_save_binds = Button::with_label("💾 SAVE BINDS");
    btn_save_binds.add_css_class("fang-action-btn");

    st_hdr.append(&st_title);
    st_hdr.append(&btn_save_binds);
    studio_card.append(&st_hdr);

    let bind_add_row = GtkBox::new(Orientation::Horizontal, 8);
    let new_key_entry = Entry::builder()
        .placeholder_text("Bind new key (e.g. f6, space, capslock)")
        .width_chars(12)
        .hexpand(true)
        .build();
    let btn_add_key = Button::with_label("+ Bind Key");
    btn_add_key.add_css_class("cryo-btn");
    bind_add_row.append(&new_key_entry);
    bind_add_row.append(&btn_add_key);
    studio_card.append(&bind_add_row);

    let key_tabs_row = flow_row();
    studio_card.append(&key_tabs_row);

    let down_box = GtkBox::new(Orientation::Vertical, 6);
    down_box.add_css_class("frost-panel");
    down_box.set_hexpand(true);
    let up_box = GtkBox::new(Orientation::Vertical, 6);
    up_box.add_css_class("frost-panel");
    up_box.set_hexpand(true);

    let edges_row = responsive_box(12);
    edges_row.append(&down_box);
    edges_row.append(&up_box);
    studio_card.append(&edges_row);

    let step_builder_row = GtkBox::new(Orientation::Horizontal, 8);
    let step_entry = Entry::builder()
        .placeholder_text("step (e.g. color 00f0ff 150, breathe 0, fade 2000, rest)")
        .width_chars(12)
        .hexpand(true)
        .build();
    let btn_add_down = Button::with_label("+ DOWN");
    btn_add_down.add_css_class("cryo-btn");
    let btn_add_up = Button::with_label("+ UP");
    btn_add_up.add_css_class("cryo-btn");

    step_builder_row.append(&step_entry);
    step_builder_row.append(&btn_add_down);
    step_builder_row.append(&btn_add_up);
    studio_card.append(&step_builder_row);

    let step_presets_row = flow_row();
    let presets_lbl = Label::new(Some("Presets:"));
    presets_lbl.add_css_class("dim-label");
    flow_add(&step_presets_row, &presets_lbl);

    for preset_step in [
        "color 00f0ff 160",
        "color ff0000 0",
        "brightness 0 120",
        "brightness 3 120",
        "breathe 0",
        "fade 2000",
        "rest",
    ] {
        let b = Button::with_label(preset_step);
        b.add_css_class("cryo-btn");
        let se = step_entry.clone();
        let ps = preset_step.to_string();
        b.connect_clicked(move |_| {
            se.set_text(&ps);
        });
        flow_add(&step_presets_row, &b);
    }
    studio_card.append(&step_presets_row);

    let ctron_lbl = Label::new(Some("Follower ctron command on key press:"));
    ctron_lbl.add_css_class("dim-label");
    ctron_lbl.set_xalign(0.0);
    studio_card.append(&ctron_lbl);

    let ctron_row = GtkBox::new(Orientation::Horizontal, 8);
    let ctron_entry = Entry::builder()
        .placeholder_text("e.g. profile performance  or  mode turbo")
        .width_chars(12)
        .hexpand(true)
        .build();
    let btn_set_ctron = Button::with_label("Set Key Action");
    btn_set_ctron.add_css_class("cryo-btn");
    ctron_row.append(&ctron_entry);
    ctron_row.append(&btn_set_ctron);
    studio_card.append(&ctron_row);

    outer.append(&studio_card);
    scroll.set_child(Some(&outer));

    // Rebuild Studio UI function
    let rebuild_studio: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild_fn: Rc<dyn Fn()> = {
        let kt_row = key_tabs_row.clone();
        let d_box = down_box.clone();
        let u_box = up_box.clone();
        let c_ent = ctron_entry.clone();
        let sel = Rc::clone(&selected_key_idx);
        let canvas_k = kbd_canvas.clone();
        let self_ref = Rc::clone(&rebuild_studio);

        Rc::new(move || {
            while let Some(c) = kt_row.first_child() {
                kt_row.remove(&c);
            }
            while let Some(c) = d_box.first_child() {
                d_box.remove(&c);
            }
            while let Some(c) = u_box.first_child() {
                u_box.remove(&c);
            }

            let list = ffi::binds_list();
            if list.is_empty() {
                return;
            }
            let mut cur = *sel.borrow();
            if cur >= list.len() {
                cur = 0;
                *sel.borrow_mut() = 0;
            }

            for (i, item) in list.iter().enumerate() {
                let b = Button::with_label(&format!("[{}]", item.name));
                b.add_css_class("cryo-btn");
                if i == cur {
                    b.add_css_class("cryo-btn-active");
                }
                let sel_c = Rc::clone(&sel);
                let sr = Rc::clone(&self_ref);
                b.connect_clicked(move |_| {
                    *sel_c.borrow_mut() = i;
                    if let Some(f) = sr.borrow().as_ref() {
                        f();
                    }
                });
                flow_add(&kt_row, &b);
            }

            // Delete current key button
            if list.len() > 1 {
                let b_del_key = Button::with_label("✕ Remove Key");
                b_del_key.add_css_class("ember-btn");
                let sr = Rc::clone(&self_ref);
                b_del_key.connect_clicked(move |_| {
                    ffi::binds_del_key(cur);
                    if let Some(f) = sr.borrow().as_ref() {
                        f();
                    }
                });
                flow_add(&kt_row, &b_del_key);
            }

            let active = &list[cur];
            c_ent.set_text(&active.ctron_cmd);

            let d_hdr = Label::new(Some(&format!(
                "▼ DOWN EDGE ([{}] pressed · {}/32 steps)",
                active.name,
                active.down_steps.len()
            )));
            d_hdr.add_css_class("card-title");
            d_hdr.set_xalign(0.0);
            d_hdr.set_hexpand(true);
            d_hdr.set_width_chars(10);
            d_hdr.set_ellipsize(EllipsizeMode::End);
            d_box.append(&d_hdr);

            for (si, step) in active.down_steps.iter().enumerate() {
                let r = GtkBox::new(Orientation::Horizontal, 6);
                let l = Label::new(Some(&format!("{}.  {}", si + 1, step)));
                l.add_css_class("cryo-chip");
                l.set_xalign(0.0);
                l.set_hexpand(true);
                l.set_width_chars(8);
                l.set_ellipsize(EllipsizeMode::End);
                let b_rm = Button::with_label("✕");
                b_rm.add_css_class("cryo-btn");
                let sr = Rc::clone(&self_ref);
                b_rm.connect_clicked(move |_| {
                    ffi::binds_del_step(cur, false, si);
                    if let Some(f) = sr.borrow().as_ref() {
                        f();
                    }
                });
                r.append(&l);
                r.append(&b_rm);
                d_box.append(&r);
            }

            let u_hdr = Label::new(Some(&format!(
                "▲ UP EDGE ([{}] released · {}/32 steps)",
                active.name,
                active.up_steps.len()
            )));
            u_hdr.add_css_class("card-title");
            u_hdr.set_xalign(0.0);
            u_hdr.set_hexpand(true);
            u_hdr.set_width_chars(10);
            u_hdr.set_ellipsize(EllipsizeMode::End);
            u_box.append(&u_hdr);

            for (si, step) in active.up_steps.iter().enumerate() {
                let r = GtkBox::new(Orientation::Horizontal, 6);
                let l = Label::new(Some(&format!("{}.  {}", si + 1, step)));
                l.add_css_class("venom-chip");
                l.set_xalign(0.0);
                l.set_hexpand(true);
                l.set_width_chars(8);
                l.set_ellipsize(EllipsizeMode::End);
                let b_rm = Button::with_label("✕");
                b_rm.add_css_class("cryo-btn");
                let sr = Rc::clone(&self_ref);
                b_rm.connect_clicked(move |_| {
                    ffi::binds_del_step(cur, true, si);
                    if let Some(f) = sr.borrow().as_ref() {
                        f();
                    }
                });
                r.append(&l);
                r.append(&b_rm);
                u_box.append(&r);
            }

            canvas_k.queue_draw();
        })
    };
    *rebuild_studio.borrow_mut() = Some(Rc::clone(&rebuild_fn));
    rebuild_fn();

    // Keyboard canvas click -> select or bind key
    {
        let click = GestureClick::new();
        let lay = Rc::clone(&layout);
        let sel = Rc::clone(&selected_key_idx);
        let reb = Rc::clone(&rebuild_fn);
        let canvas_c = kbd_canvas.clone();
        let scb = Rc::clone(&status_cb);
        click.connect_pressed(move |_, _, x, y| {
            let wf = canvas_c.width() as f64;
            let hf = canvas_c.height() as f64;
            let pad = 10.0;
            let kw = (wf - pad * 2.0).max(20.0);
            let kh = (hf - pad * 2.0).max(20.0);

            for kr in lay.iter() {
                let kx = pad + kr.x * kw + 1.5;
                let ky = pad + kr.y * kh + 1.5;
                let bw = (kr.w * kw - 3.0).max(4.0);
                let bh = (kr.h * kh - 3.0).max(4.0);
                if x >= kx && x <= kx + bw && y >= ky && y <= ky + bh {
                    if let Some(idx) = ffi::binds_add_key(kr.bind_name) {
                        *sel.borrow_mut() = idx;
                        scb(&format!(
                            "Selected key [{}] in Daeboard Macro Studio",
                            kr.bind_name
                        ));
                        reb();
                    }
                    break;
                }
            }
        });
        kbd_canvas.add_controller(click);
    }

    // Wire buttons
    {
        let nk = new_key_entry.clone();
        let sel = Rc::clone(&selected_key_idx);
        let reb = Rc::clone(&rebuild_fn);
        btn_add_key.connect_clicked(move |_| {
            let name = nk.text().trim().to_lowercase();
            if !name.is_empty() {
                if let Some(idx) = ffi::binds_add_key(&name) {
                    *sel.borrow_mut() = idx;
                    nk.set_text("");
                    reb();
                }
            }
        });
    }

    {
        let se = step_entry.clone();
        let sel = Rc::clone(&selected_key_idx);
        let reb = Rc::clone(&rebuild_fn);
        btn_add_down.connect_clicked(move |_| {
            let st = se.text().trim().to_string();
            if !st.is_empty() && ffi::binds_add_step(*sel.borrow(), false, &st) {
                reb();
            }
        });
    }

    {
        let se = step_entry.clone();
        let sel = Rc::clone(&selected_key_idx);
        let reb = Rc::clone(&rebuild_fn);
        btn_add_up.connect_clicked(move |_| {
            let st = se.text().trim().to_string();
            if !st.is_empty() && ffi::binds_add_step(*sel.borrow(), true, &st) {
                reb();
            }
        });
    }

    {
        let ce = ctron_entry.clone();
        let sel = Rc::clone(&selected_key_idx);
        let scb = Rc::clone(&status_cb);
        let reb = Rc::clone(&rebuild_fn);
        btn_set_ctron.connect_clicked(move |_| {
            let cmd = ce.text().trim().to_string();
            if ffi::binds_set_ctron(*sel.borrow(), &cmd) {
                scb("Updated follower ctron command for key (Save & Reload to activate)");
                reb();
            }
        });
    }

    {
        let scb = Rc::clone(&status_cb);
        let badge = daemon_badge.clone();
        btn_save_binds.connect_clicked(move |_| match ffi::binds_save_and_reload() {
            Ok(()) => {
                badge.set_text("● ONLINE");
                scb("Saved ~/.config/ctron/daeboard.binds & hot-reloaded daeboard daemon");
            }
            Err(e) => scb(&format!("Saved binds file; daemon reload note: {}", e)),
        });
    }

    {
        let scb = Rc::clone(&status_cb);
        let badge = daemon_badge.clone();
        btn_start.connect_clicked(move |_| {
            if ffi::db_start_daemon() || ffi::db_is_up() {
                badge.set_text("● ONLINE");
                scb("Started daeboard root LED daemon via systemd-run");
            } else {
                scb("Could not start daeboard daemon (check passwordless sudo systemd-run)");
            }
        });
    }

    {
        let scb = Rc::clone(&status_cb);
        let badge = daemon_badge.clone();
        btn_stop.connect_clicked(move |_| {
            if ffi::db_stop_daemon() {
                badge.set_text("○ STOPPED");
                scb("Stopped daeboard daemon (restored static keyboard color)");
            } else {
                scb("daeboard daemon is already stopped");
            }
        });
    }

    {
        let scb = Rc::clone(&status_cb);
        btn_reload.connect_clicked(move |_| match ffi::db_reload_binds() {
            Ok(()) => scb("Hot-reloaded ~/.config/ctron/daeboard.binds into daemon"),
            Err(e) => scb(&format!("daeboard reload error: {}", e)),
        });
    }

    let badge_ref = daemon_badge.clone();
    let refresh_ui: Rc<dyn Fn(&ffi::HwSnapshot)> = Rc::new(move |snap: &ffi::HwSnapshot| {
        if snap.raw.daeboard_up != 0 {
            badge_ref.set_text("● ONLINE");
        } else {
            badge_ref.set_text("○ STOPPED");
        }
    });

    DaeboardViewHandles {
        root: scroll,
        refresh_ui,
    }
}
