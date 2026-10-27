use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DrawingArea, Entry, GestureClick, Label, Orientation, PolicyType,
    ScrolledWindow,
};

use crate::audio::{format_mmss, AudioCmd, AudioEngineHandle, BrowserEntry, PlayState};
use crate::ui::draw_cryo_grid;

#[derive(Clone)]
pub struct TunotronViewHandles {
    pub root: GtkBox,
    pub refresh_ui: Rc<dyn Fn()>,
}

pub fn build_tunotron_view(
    audio: Rc<AudioEngineHandle>,
    status_cb: Rc<dyn Fn(&str)>,
) -> TunotronViewHandles {
    let outer = GtkBox::new(Orientation::Vertical, 12);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(14);
    outer.set_margin_end(14);
    outer.set_hexpand(true);
    outer.set_vexpand(true);

    // =====================================================================
    // ROW 1: NOW PLAYING CRYO-STAGE + DSP / RADIO STUDIO (PINNED TOP)
    // =====================================================================
    let top_row = GtkBox::new(Orientation::Horizontal, 12);
    top_row.set_hexpand(true);

    let stage_card = GtkBox::new(Orientation::Vertical, 8);
    stage_card.add_css_class("serpent-card");
    stage_card.set_hexpand(true);

    let stage_hdr = GtkBox::new(Orientation::Horizontal, 8);
    let st_title = Label::new(Some("◆ TUNOTRON AUDIO DECK"));
    st_title.add_css_class("card-title");
    st_title.set_halign(Align::Start);
    st_title.set_hexpand(true);
    st_title.set_ellipsize(EllipsizeMode::End);

    let cache_badge = Label::new(Some("JAILED · 0 CACHED"));
    cache_badge.add_css_class("cryo-chip");
    stage_hdr.append(&st_title);
    stage_hdr.append(&cache_badge);
    stage_card.append(&stage_hdr);

    let track_headline = Label::new(Some("No Track Playing"));
    track_headline.add_css_class("metric-huge");
    track_headline.set_halign(Align::Start);
    track_headline.set_hexpand(true);
    track_headline.set_width_chars(10);
    track_headline.set_ellipsize(EllipsizeMode::End);

    let track_subline = Label::new(Some(
        "Click any song or ▶ button in the list below to start playback",
    ));
    track_subline.add_css_class("card-subtitle");
    track_subline.set_halign(Align::Start);
    track_subline.set_hexpand(true);
    track_subline.set_width_chars(12);
    track_subline.set_ellipsize(EllipsizeMode::End);

    stage_card.append(&track_headline);
    stage_card.append(&track_subline);

    let wave_canvas = DrawingArea::builder()
        .content_width(240)
        .content_height(78)
        .hexpand(true)
        .build();

    {
        let aud = Rc::clone(&audio);
        wave_canvas.set_draw_func(move |_, cr, w, h| {
            let wf = w as f64;
            let hf = h as f64;
            draw_cryo_grid(cr, wf, hf);

            let snap = aud.snapshot();
            let bars = 44usize;
            let pad_x = 14.0;
            let usable_w = (wf - pad_x * 2.0).max(20.0);
            let bar_w = (usable_w / bars as f64) * 0.68;
            let step_w = usable_w / bars as f64;

            let ratio = if snap.duration_sec > 0.0 {
                (snap.position_sec / snap.duration_sec).clamp(0.0, 1.0)
            } else {
                0.0
            };

            for i in 0..bars {
                let bx = pad_x + i as f64 * step_w;
                let norm = i as f64 / bars as f64;
                let phase = if snap.state == PlayState::Playing {
                    snap.position_sec * 4.2 + i as f64 * 0.45
                } else {
                    i as f64 * 0.35
                };
                let amp = if snap.state == PlayState::Playing {
                    0.22 + 0.65 * ((phase.sin().abs() * 0.6) + ((phase * 0.5).cos().abs() * 0.4))
                } else {
                    0.14 + 0.12 * (i as f64 * 0.5).sin().abs()
                };
                let bh = (amp * (hf - 26.0)).max(4.0);
                let by = (hf - 14.0 - bh) * 0.5 + 3.0;

                if norm <= ratio {
                    cr.set_source_rgba(0.0, 0.94, 1.0, 0.92);
                } else {
                    cr.set_source_rgba(0.08, 0.72, 0.65, 0.32);
                }
                cr.rectangle(bx, by, bar_w, bh);
                let _ = cr.fill();
            }

            let sb_y = hf - 9.0;
            cr.set_source_rgba(0.05, 0.14, 0.22, 0.95);
            cr.rectangle(pad_x, sb_y, usable_w, 4.0);
            let _ = cr.fill();

            cr.set_source_rgba(0.0, 0.94, 1.0, 0.95);
            cr.rectangle(pad_x, sb_y, usable_w * ratio, 4.0);
            let _ = cr.fill();
        });
    }

    // Click on wave_canvas to seek directly
    {
        let click = GestureClick::new();
        let aud = Rc::clone(&audio);
        let wc = wave_canvas.clone();
        click.connect_pressed(move |_, _, x, _| {
            let wf = wc.width() as f64;
            let pad_x = 14.0;
            let usable_w = (wf - pad_x * 2.0).max(20.0);
            let ratio = ((x - pad_x) / usable_w).clamp(0.0, 1.0);
            let dur = aud.snapshot().duration_sec;
            if dur > 0.0 {
                aud.send(AudioCmd::SeekAbsolute(ratio * dur));
            }
        });
        wave_canvas.add_controller(click);
    }

    stage_card.append(&wave_canvas);

    // Interactive Transport & Seek Controls right inside the Deck Stage
    let transport_row = GtkBox::new(Orientation::Horizontal, 6);
    let btn_st_prev = Button::with_label("⏮ Prev");
    btn_st_prev.add_css_class("cryo-btn");
    let btn_st_back10 = Button::with_label("⏪ -10s");
    btn_st_back10.add_css_class("cryo-btn");
    let btn_st_play = Button::with_label("▶ Play");
    btn_st_play.add_css_class("fang-action-btn");
    let btn_st_fwd10 = Button::with_label("+10s ⏩");
    btn_st_fwd10.add_css_class("cryo-btn");
    let btn_st_next = Button::with_label("Next ⏭");
    btn_st_next.add_css_class("cryo-btn");
    let btn_st_stop = Button::with_label("⏹ Stop");
    btn_st_stop.add_css_class("cryo-btn");

    {
        let aud = Rc::clone(&audio);
        btn_st_prev.connect_clicked(move |_| aud.send(AudioCmd::Prev));
    }
    {
        let aud = Rc::clone(&audio);
        btn_st_back10.connect_clicked(move |_| aud.send(AudioCmd::SeekRelative(-10.0)));
    }
    {
        let aud = Rc::clone(&audio);
        btn_st_play.connect_clicked(move |_| aud.send(AudioCmd::TogglePause));
    }
    {
        let aud = Rc::clone(&audio);
        btn_st_fwd10.connect_clicked(move |_| aud.send(AudioCmd::SeekRelative(10.0)));
    }
    {
        let aud = Rc::clone(&audio);
        btn_st_next.connect_clicked(move |_| aud.send(AudioCmd::Next));
    }
    {
        let aud = Rc::clone(&audio);
        btn_st_stop.connect_clicked(move |_| aud.send(AudioCmd::Stop));
    }

    let time_chip = Label::new(Some("00:00 / 00:00"));
    time_chip.add_css_class("cryo-chip");
    time_chip.set_hexpand(true);
    time_chip.set_halign(Align::End);

    transport_row.append(&btn_st_prev);
    transport_row.append(&btn_st_back10);
    transport_row.append(&btn_st_play);
    transport_row.append(&btn_st_fwd10);
    transport_row.append(&btn_st_next);
    transport_row.append(&btn_st_stop);
    transport_row.append(&time_chip);
    stage_card.append(&transport_row);

    // DSP & Stream Card (compact width so it never crowds the window)
    let dsp_card = GtkBox::new(Orientation::Vertical, 8);
    dsp_card.add_css_class("serpent-card");
    dsp_card.set_size_request(265, -1);

    let dsp_title = Label::new(Some("◆ DSP EQUALIZER & RADIO"));
    dsp_title.add_css_class("card-title");
    dsp_title.set_halign(Align::Start);
    dsp_card.append(&dsp_title);

    let eq_row1 = GtkBox::new(Orientation::Horizontal, 4);
    let eq_row2 = GtkBox::new(Orientation::Horizontal, 4);
    let eq_presets: &[(&str, &str)] = &[
        ("Flat", ""),
        ("Bass Boost", "bass=g=9:f=110:w=0.6"),
        ("Nightcore", "asetrate=48000*1.22,aresample=48000"),
        (
            "Slowed+Reverb",
            "asetrate=48000*0.84,aresample=48000,aecho=0.8:0.88:60:0.35",
        ),
        ("8D Audio", "apulsator=hz=0.125"),
        ("Crystal Treble", "treble=g=6:f=3500"),
    ];
    for (i, &(name, flt)) in eq_presets.iter().enumerate() {
        let b = Button::with_label(name);
        b.add_css_class("cryo-btn");
        let aud = Rc::clone(&audio);
        let scb = Rc::clone(&status_cb);
        let n_str = name.to_string();
        let f_str = flt.to_string();
        b.connect_clicked(move |_| {
            aud.send(AudioCmd::SetFilter {
                name: n_str.clone(),
                filter: f_str.clone(),
            });
            scb(&format!("Applied Tunotron DSP profile: {}", n_str));
        });
        if i < 3 {
            eq_row1.append(&b);
        } else {
            eq_row2.append(&b);
        }
    }
    dsp_card.append(&eq_row1);
    dsp_card.append(&eq_row2);

    let spd_row = GtkBox::new(Orientation::Horizontal, 4);
    let spd_lbl = Label::new(Some("Speed:"));
    spd_lbl.add_css_class("dim-label");
    spd_row.append(&spd_lbl);
    for spd in [0.85, 1.0, 1.15, 1.25] {
        let b = Button::with_label(&format!("{:.2}×", spd));
        b.add_css_class("cryo-btn");
        let aud = Rc::clone(&audio);
        b.connect_clicked(move |_| {
            aud.send(AudioCmd::SetSpeed(spd));
        });
        spd_row.append(&b);
    }
    dsp_card.append(&spd_row);

    let radio_lbl = Label::new(Some("LIVE RADIO STREAMS"));
    radio_lbl.add_css_class("rail-section-label");
    radio_lbl.set_halign(Align::Start);
    dsp_card.append(&radio_lbl);

    let streams_row = GtkBox::new(Orientation::Horizontal, 4);
    for (title, url) in [
        ("Groove Salad", "https://ice6.somafm.com/groovesalad-256-mp3"),
        ("Drone Zone", "https://ice6.somafm.com/dronezone-256-mp3"),
        ("DEF CON", "https://ice6.somafm.com/defcon-256-mp3"),
    ] {
        let b = Button::with_label(title);
        b.add_css_class("cryo-btn");
        let aud = Rc::clone(&audio);
        let scb = Rc::clone(&status_cb);
        let t_s = title.to_string();
        let u_s = url.to_string();
        b.connect_clicked(move |_| {
            aud.send(AudioCmd::PlayStream {
                url: u_s.clone(),
                title: t_s.clone(),
            });
            scb(&format!("Streaming live radio: {}", t_s));
        });
        streams_row.append(&b);
    }
    dsp_card.append(&streams_row);

    top_row.append(&stage_card);
    top_row.append(&dsp_card);
    outer.append(&top_row);

    // =====================================================================
    // ROW 2: JAILED FOLDER-IS-THE-PLAYLIST LIBRARY BROWSER (~/Music)
    // =====================================================================
    let lib_card = GtkBox::new(Orientation::Vertical, 8);
    lib_card.add_css_class("serpent-card");
    lib_card.set_hexpand(true);
    lib_card.set_vexpand(true);

    let nav_bar = GtkBox::new(Orientation::Horizontal, 8);
    let btn_up = Button::with_label("⬆ Parent Dir");
    btn_up.add_css_class("cryo-btn");
    {
        let aud = Rc::clone(&audio);
        btn_up.connect_clicked(move |_| {
            aud.send(AudioCmd::ParentDir);
        });
    }

    let path_lbl = Label::new(Some("~/Music"));
    path_lbl.add_css_class("card-title");
    path_lbl.set_halign(Align::Start);
    path_lbl.set_hexpand(true);
    path_lbl.set_width_chars(10);
    path_lbl.set_ellipsize(EllipsizeMode::End);

    let search_entry = Entry::builder()
        .placeholder_text("Filter folder or track...")
        .width_chars(16)
        .build();

    let btn_clear_q = Button::with_label("Clear Queue");
    btn_clear_q.add_css_class("cryo-btn");
    {
        let aud = Rc::clone(&audio);
        btn_clear_q.connect_clicked(move |_| {
            aud.send(AudioCmd::ClearQueue);
        });
    }

    nav_bar.append(&btn_up);
    nav_bar.append(&path_lbl);
    nav_bar.append(&search_entry);
    nav_bar.append(&btn_clear_q);
    lib_card.append(&nav_bar);

    let list_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let list_box = GtkBox::new(Orientation::Vertical, 4);
    list_scroll.set_child(Some(&list_box));
    lib_card.append(&list_scroll);
    outer.append(&lib_card);

    let last_rendered_sig: Rc<RefCell<(PathBuf, usize, String, Option<PathBuf>, PlayState)>> =
        Rc::new(RefCell::new((
            PathBuf::new(),
            0,
            String::new(),
            None,
            PlayState::Stopped,
        )));

    let rebuild_list = {
        let aud = Rc::clone(&audio);
        let l_box = list_box.clone();
        let s_ent = search_entry.clone();
        let sig = Rc::clone(&last_rendered_sig);
        let scb = Rc::clone(&status_cb);

        Rc::new(move |force: bool| {
            let snap = aud.snapshot();
            let query = s_ent.text().trim().to_lowercase();
            let playing_path = snap.current_track.as_ref().map(|t| t.path.clone());
            let new_sig = (
                snap.current_dir.clone(),
                snap.cached_tracks_count,
                query.clone(),
                playing_path.clone(),
                snap.state,
            );
            if !force && *sig.borrow() == new_sig {
                return;
            }
            *sig.borrow_mut() = new_sig;

            while let Some(c) = l_box.first_child() {
                l_box.remove(&c);
            }

            for (idx, entry) in snap.entries.iter().enumerate() {
                match entry {
                    BrowserEntry::ParentDir(p) => {
                        let row = GtkBox::new(Orientation::Horizontal, 10);
                        row.add_css_class("track-row");

                        let btn = Button::with_label("⬆ Open ..");
                        btn.add_css_class("cryo-btn");
                        let aud_c = Rc::clone(&aud);
                        let target = p.clone();
                        btn.connect_clicked(move |_| {
                            aud_c.send(AudioCmd::OpenDir(target.clone()));
                        });

                        let lbl = Label::new(Some("📁  .. (Parent Directory)"));
                        lbl.set_halign(Align::Start);
                        lbl.set_hexpand(true);
                        lbl.set_ellipsize(EllipsizeMode::End);

                        row.append(&btn);
                        row.append(&lbl);
                        l_box.append(&row);
                    }
                    BrowserEntry::Directory { name, path } => {
                        if !query.is_empty() && !name.to_lowercase().contains(&query) {
                            continue;
                        }
                        let row = GtkBox::new(Orientation::Horizontal, 10);
                        row.add_css_class("track-row");

                        let btn = Button::with_label("📂 Open");
                        btn.add_css_class("cryo-btn");
                        let aud_c = Rc::clone(&aud);
                        let target = path.clone();
                        btn.connect_clicked(move |_| {
                            aud_c.send(AudioCmd::OpenDir(target.clone()));
                        });

                        let lbl = Label::new(Some(&format!("📁  {}", name)));
                        lbl.add_css_class("card-title");
                        lbl.set_halign(Align::Start);
                        lbl.set_hexpand(true);
                        lbl.set_ellipsize(EllipsizeMode::End);

                        // Also allow clicking the folder label directly
                        let click = GestureClick::new();
                        let aud_row = Rc::clone(&aud);
                        let target_row = path.clone();
                        click.connect_released(move |_, _, _, _| {
                            aud_row.send(AudioCmd::OpenDir(target_row.clone()));
                        });
                        lbl.add_controller(click);

                        row.append(&btn);
                        row.append(&lbl);
                        l_box.append(&row);
                    }
                    BrowserEntry::AudioTrack(track) => {
                        if !query.is_empty()
                            && !track.title.to_lowercase().contains(&query)
                            && !track.artist.to_lowercase().contains(&query)
                            && !track.filename.to_lowercase().contains(&query)
                        {
                            continue;
                        }
                        let is_current = playing_path.as_deref() == Some(track.path.as_path());
                        let is_playing = is_current && snap.state == PlayState::Playing;

                        let row = GtkBox::new(Orientation::Horizontal, 8);
                        row.add_css_class("track-row");
                        if is_current {
                            row.add_css_class("track-row-playing");
                        }

                        // Primary Play/Pause button on the LEFT so it is never pushed off-screen
                        let play_lbl = if is_playing {
                            "⏸ Pause"
                        } else if is_current {
                            "▶ Resume"
                        } else {
                            "▶ Play"
                        };
                        let btn_play = Button::with_label(play_lbl);
                        if is_current {
                            btn_play.add_css_class("fang-action-btn");
                        } else {
                            btn_play.add_css_class("cryo-btn");
                        }
                        {
                            let aud_p = Rc::clone(&aud);
                            let scb_p = Rc::clone(&scb);
                            let t_title = track.title.clone();
                            btn_play.connect_clicked(move |_| {
                                if is_current {
                                    aud_p.send(AudioCmd::TogglePause);
                                } else {
                                    scb_p(&format!("Playing: {}", t_title));
                                    aud_p.send(AudioCmd::PlayTrackAtIndex(idx));
                                }
                            });
                        }

                        let display_str = if track.artist == "Unknown Artist" || track.artist.is_empty() {
                            track.title.clone()
                        } else {
                            format!("{} — {}", track.artist, track.title)
                        };
                        let title_lbl = Label::new(Some(&display_str));
                        title_lbl.set_halign(Align::Start);
                        title_lbl.set_hexpand(true);
                        title_lbl.set_width_chars(12);
                        title_lbl.set_ellipsize(EllipsizeMode::End);

                        // Clicking the track title also plays/toggles the track
                        {
                            let click = GestureClick::new();
                            let aud_t = Rc::clone(&aud);
                            let scb_t = Rc::clone(&scb);
                            let t_title = track.title.clone();
                            click.connect_released(move |_, _, _, _| {
                                if is_current {
                                    aud_t.send(AudioCmd::TogglePause);
                                } else {
                                    scb_t(&format!("Playing: {}", t_title));
                                    aud_t.send(AudioCmd::PlayTrackAtIndex(idx));
                                }
                            });
                            title_lbl.add_controller(click);
                        }

                        let dur_lbl = Label::new(Some(&track.duration_label));
                        dur_lbl.add_css_class("cryo-chip");

                        let btn_q = Button::with_label("+ Queue");
                        btn_q.add_css_class("cryo-btn");
                        {
                            let aud_q = Rc::clone(&aud);
                            let t_clone = std::sync::Arc::clone(track);
                            let scb_q = Rc::clone(&scb);
                            btn_q.connect_clicked(move |_| {
                                scb_q(&format!("Queued: {}", t_clone.title));
                                aud_q.send(AudioCmd::AddToQueue(std::sync::Arc::clone(&t_clone)));
                            });
                        }

                        row.append(&btn_play);
                        row.append(&title_lbl);
                        row.append(&dur_lbl);
                        row.append(&btn_q);
                        l_box.append(&row);
                    }
                }
            }
        })
    };

    {
        let rl = Rc::clone(&rebuild_list);
        search_entry.connect_changed(move |_| {
            rl(true);
        });
    }

    let refresh_ui: Rc<dyn Fn()> = {
        let aud = Rc::clone(&audio);
        let rl = Rc::clone(&rebuild_list);
        let wc = wave_canvas.clone();
        let b_st_play = btn_st_play.clone();
        let t_chip = time_chip.clone();
        Rc::new(move || {
            let snap = aud.snapshot();
            cache_badge.set_text(&format!(
                "{} CACHED · DSP: {} · Q: {}",
                snap.cached_tracks_count,
                snap.active_filter_name,
                snap.queue.len()
            ));
            path_lbl.set_text(&format!("🔒 {}", snap.current_dir.display()));

            match snap.state {
                PlayState::Playing => b_st_play.set_label("⏸ Pause"),
                PlayState::Paused => b_st_play.set_label("▶ Resume"),
                PlayState::Stopped => b_st_play.set_label("▶ Play"),
            }
            t_chip.set_text(&format!(
                "{} / {}",
                format_mmss(snap.position_sec),
                format_mmss(snap.duration_sec)
            ));

            if let Some(t) = &snap.current_track {
                track_headline.set_text(&t.title);
                track_subline.set_text(&format!(
                    "{} · {} · [{} / {}]",
                    t.artist,
                    t.album,
                    format_mmss(snap.position_sec),
                    format_mmss(snap.duration_sec)
                ));
            } else {
                track_headline.set_text("Tunotron Deck Ready");
                track_subline.set_text("Click ▶ Play on any track below or launch a SomaFM radio stream");
            }

            rl(false);
            wc.queue_draw();
        })
    };
    refresh_ui();

    TunotronViewHandles {
        root: outer,
        refresh_ui,
    }
}
