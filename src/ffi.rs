#![allow(dead_code)]

use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawHwSnap {
    pub model: [u8; 64],
    pub cpu: [u8; 128],
    pub bat_status: [u8; 16],
    pub bat_power_fmt: [u8; 32],
    pub active_mode: [u8; 24],
    pub latest_log: [u8; 128],

    pub is_asus: i32,
    pub has_asusctl: i32,
    pub has_armoury: i32,
    pub has_fan_curve: i32,
    pub has_fan_rpm: i32,
    pub has_nvidia_smi: i32,
    pub has_kbd_led: i32,

    pub profile: i32,
    pub epp: i32,
    pub asusd_ac: i32,
    pub asusd_bat: i32,
    pub asusd_enforced: i32,

    pub cpu_temp: i32,
    pub gpu_temp: i32,
    pub cpu_mhz_cur: i32,
    pub cpu_mhz_min: i32,
    pub cpu_mhz_max: i32,
    pub cpu_mhz_limit: i32,
    pub rpm_cpu: i32,
    pub rpm_gpu: i32,

    pub bat_pct: i32,
    pub ac_online: i32,
    pub bat_mw_known: i32,
    pub bat_mw: i32,
    pub bat_limit: i32,
    pub bat_health: i32,
    pub bat_v_mv: i32,
    pub bat_a_ma: i32,
    pub pd_n: i32,
    pub pd_in_mw: i32,

    pub ppt_spl: i32,
    pub ppt_sppt: i32,
    pub ppt_fppt: i32,
    pub ppt_mode: i32,
    pub ppt_off: i32,
    pub spl_min: i32,
    pub spl_max: i32,
    pub sppt_min: i32,
    pub sppt_max: i32,
    pub fppt_min: i32,
    pub fppt_max: i32,

    pub nv_boost: i32,
    pub nv_temp: i32,
    pub gpu_clock_lock: i32,
    pub gpu_mhz_max: i32,
    pub gpu_mw: i32,
    pub gpu_mhz: i32,
    pub gpu_util: i32,
    pub gpu_vram_used_mb: i32,
    pub gpu_vram_total_mb: i32,

    pub load1: f64,
    pub mem_total_mb: i32,
    pub mem_avail_mb: i32,
    pub mem_used_mb: i32,
    pub swap_total_mb: i32,
    pub swap_used_mb: i32,
    pub igpu_temp: i32,
    pub igpu_mhz: i32,
    pub igpu_mw: i32,
    pub ram_temp0: i32,
    pub ram_temp1: i32,
    pub nvme_temp0: i32,
    pub nvme_temp1: i32,
    pub fs_used_mb: i32,
    pub fs_total_mb: i32,

    pub panel_od: i32,
    pub cpu_boost: i32,
    pub kbd: i32,

    pub fan_cpu_n: i32,
    pub fan_cpu_t: [i32; 8],
    pub fan_cpu_p: [i32; 8],
    pub fan_gpu_n: i32,
    pub fan_gpu_t: [i32; 8],
    pub fan_gpu_p: [i32; 8],
    pub fan_cpu_on: i32,
    pub fan_gpu_on: i32,
    pub fan_staged: i32,

    pub hz_cur: i32,
    pub hz_modes: [i32; 16],
    pub hz_count: i32,

    pub mode_drifted: i32,
    pub daeboard_up: i32,
}

extern "C" {
    fn htron_core_init();
    fn htron_core_refresh_fast();
    fn htron_core_refresh_live();
    fn htron_core_get_snap(out: *mut RawHwSnap, probe_daeboard: c_int);

    fn htron_set_profile(p: c_int) -> c_int;
    fn htron_set_asusd_auto(ac: c_int, bat: c_int) -> c_int;
    fn htron_set_epp(e: c_int) -> c_int;
    fn htron_set_cpu_max_mhz(mhz: c_int) -> c_int;
    fn htron_set_cpu_boost(on: c_int) -> c_int;
    fn htron_set_ppt(spl: c_int, sppt: c_int, fppt: c_int) -> c_int;
    fn htron_set_nv_boost(watts: c_int) -> c_int;
    fn htron_set_nv_temp(celsius: c_int) -> c_int;
    fn htron_set_gpu_clock(mhz: c_int) -> c_int;
    fn htron_set_panel_od(on: c_int) -> c_int;
    fn htron_set_battery_limit(pct: c_int) -> c_int;
    fn htron_battery_oneshot() -> c_int;
    fn htron_set_hz(hz: c_int) -> c_int;
    fn htron_set_kbd(lvl: c_int) -> c_int;
    fn htron_set_aura(effect_idx: c_int, color_idx: c_int) -> c_int;
    fn htron_set_aura_hex(hex: *const c_char) -> c_int;
    fn htron_cmd_run(key: *const c_char, val: *const c_char, err: *mut c_char, errn: c_int) -> c_int;

    fn htron_fan_set_point(is_gpu: c_int, idx: c_int, temp: c_int, pwm: c_int) -> c_int;
    fn htron_fan_load_preset(preset_idx: c_int) -> c_int;
    fn htron_fan_quick_preset(preset: c_int) -> c_int;
    fn htron_fan_set_enabled(cpu_on: c_int, gpu_on: c_int) -> c_int;
    fn htron_fan_write_ec() -> c_int;

    fn htron_modes_count() -> c_int;
    fn htron_mode_get(idx: c_int, name: *mut c_char, nn: c_int, steps: *mut c_char, sn: c_int) -> c_int;
    fn htron_mode_apply_name(name: *const c_char, err: *mut c_char, errn: c_int) -> c_int;
    fn htron_mode_save_item(name: *const c_char, steps: *const c_char) -> c_int;
    fn htron_mode_delete_item(name: *const c_char) -> c_int;

    fn htron_profiles_count() -> c_int;
    fn htron_profile_get(
        idx: c_int,
        name: *mut c_char,
        nn: c_int,
        summary: *mut c_char,
        sn: c_int,
        diff: *mut c_char,
        dn: c_int,
        note: *mut c_char,
        noten: c_int,
    ) -> c_int;
    fn htron_profile_capture(name: *const c_char, note: *const c_char) -> c_int;
    fn htron_profile_apply(name: *const c_char, err: *mut c_char, errn: c_int) -> c_int;
    fn htron_profile_remove(name: *const c_char) -> c_int;

    fn htron_telem_series_read(
        series_idx: c_int,
        out_samples: *mut c_double,
        max_samples: c_int,
        out_min: *mut c_double,
        out_max: *mut c_double,
        out_last: *mut c_double,
    ) -> c_int;

    fn htron_db_is_up() -> c_int;
    fn htron_db_start_daemon() -> c_int;
    fn htron_db_stop_daemon() -> c_int;
    fn htron_db_reload_binds(err: *mut c_char, errn: c_int) -> c_int;
    fn htron_db_color(hex: *const c_char) -> c_int;
    fn htron_db_brightness(level: c_int) -> c_int;

    fn htron_binds_reload_disk();
    fn htron_binds_key_count() -> c_int;
    fn htron_binds_key_info(
        idx: c_int,
        name: *mut c_char,
        nn: c_int,
        ctron_cmd: *mut c_char,
        cn: c_int,
        n_down: *mut c_int,
        n_up: *mut c_int,
    ) -> c_int;
    fn htron_binds_step_get(key_idx: c_int, is_up: c_int, step_idx: c_int, out: *mut c_char, outn: c_int) -> c_int;
    fn htron_binds_step_add(key_idx: c_int, is_up: c_int, step: *const c_char) -> c_int;
    fn htron_binds_step_del(key_idx: c_int, is_up: c_int, step_idx: c_int) -> c_int;
    fn htron_binds_set_ctron(key_idx: c_int, cmd: *const c_char) -> c_int;
    fn htron_binds_add_key(name: *const c_char) -> c_int;
    fn htron_binds_del_key(key_idx: c_int) -> c_int;
    fn htron_binds_save_and_reload(err: *mut c_char, errn: c_int) -> c_int;
}

fn cbuf_to_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[derive(Debug, Clone)]
pub struct HwSnapshot {
    pub raw: RawHwSnap,
    pub model: String,
    pub cpu: String,
    pub bat_status: String,
    pub bat_power_fmt: String,
    pub active_mode: String,
    pub latest_log: String,
}

impl HwSnapshot {
    pub fn profile_name(&self) -> &'static str {
        match self.raw.profile {
            0 => "Quiet",
            1 => "Balanced",
            2 => "Performance",
            _ => "Unknown",
        }
    }

    pub fn epp_name(&self) -> &'static str {
        match self.raw.epp {
            0 => "power",
            1 => "balance_power",
            2 => "balance_performance",
            3 => "performance",
            _ => "--",
        }
    }

    pub fn kbd_name(&self) -> &'static str {
        match self.raw.kbd {
            0 => "Off",
            1 => "Low",
            2 => "Med",
            3 => "High",
            _ => "--",
        }
    }

    pub fn cpu_points(&self) -> Vec<(i32, i32)> {
        let n = self.raw.fan_cpu_n.clamp(0, 8) as usize;
        (0..n)
            .map(|i| (self.raw.fan_cpu_t[i], self.raw.fan_cpu_p[i]))
            .collect()
    }

    pub fn gpu_points(&self) -> Vec<(i32, i32)> {
        let n = self.raw.fan_gpu_n.clamp(0, 8) as usize;
        (0..n)
            .map(|i| (self.raw.fan_gpu_t[i], self.raw.fan_gpu_p[i]))
            .collect()
    }

    pub fn supported_hz(&self) -> Vec<i32> {
        let n = self.raw.hz_count.clamp(0, 16) as usize;
        self.raw.hz_modes[..n].to_vec()
    }
}

#[derive(Debug, Clone)]
pub struct ModeItem {
    pub name: String,
    pub steps: String,
}

#[derive(Debug, Clone)]
pub struct ProfileItem {
    pub name: String,
    pub summary: String,
    pub diff: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct BindKeyItem {
    pub name: String,
    pub ctron_cmd: String,
    pub down_steps: Vec<String>,
    pub up_steps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TelemSeriesData {
    pub samples: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub last: f64,
}

pub const TS_CPU_TEMP: i32 = 0;
pub const TS_CPU_MHZ: i32 = 1;
pub const TS_CPU_LOAD: i32 = 2;
pub const TS_GPU_TEMP: i32 = 3;
pub const TS_GPU_MHZ: i32 = 4;
pub const TS_GPU_MW: i32 = 5;
pub const TS_GPU_UTIL: i32 = 6;
pub const TS_IGPU_TEMP: i32 = 7;
pub const TS_IGPU_MHZ: i32 = 8;
pub const TS_IGPU_MW: i32 = 9;
pub const TS_FAN_CPU: i32 = 10;
pub const TS_FAN_GPU: i32 = 11;
pub const TS_MEM_USED: i32 = 12;
pub const TS_DISK_RD: i32 = 13;
pub const TS_DISK_WR: i32 = 14;
pub const TS_BAT_MW: i32 = 15;
pub const TS_BAT_PCT: i32 = 16;
pub const TS_PD_MW: i32 = 17;

pub fn init_core() {
    unsafe { htron_core_init() };
}

pub fn refresh_fast() {
    unsafe { htron_core_refresh_fast() };
}

pub fn refresh_live() {
    unsafe { htron_core_refresh_live() };
}

pub fn get_snapshot(probe_daeboard: bool) -> HwSnapshot {
    let mut raw: RawHwSnap = unsafe { std::mem::zeroed() };
    unsafe { htron_core_get_snap(&mut raw, if probe_daeboard { 1 } else { 0 }) };
    HwSnapshot {
        model: cbuf_to_string(&raw.model),
        cpu: cbuf_to_string(&raw.cpu),
        bat_status: cbuf_to_string(&raw.bat_status),
        bat_power_fmt: cbuf_to_string(&raw.bat_power_fmt),
        active_mode: cbuf_to_string(&raw.active_mode),
        latest_log: cbuf_to_string(&raw.latest_log),
        raw,
    }
}

pub fn set_profile(p: i32) -> bool {
    unsafe { htron_set_profile(p) == 0 }
}

pub fn set_asusd_auto(ac: i32, bat: i32) -> bool {
    unsafe { htron_set_asusd_auto(ac, bat) == 0 }
}

pub fn set_epp(e: i32) -> bool {
    unsafe { htron_set_epp(e) == 0 }
}

pub fn set_cpu_max_mhz(mhz: i32) -> bool {
    unsafe { htron_set_cpu_max_mhz(mhz) == 0 }
}

pub fn set_cpu_boost(on: bool) -> bool {
    unsafe { htron_set_cpu_boost(if on { 1 } else { 0 }) == 0 }
}

pub fn set_ppt(spl: i32, sppt: i32, fppt: i32) -> bool {
    unsafe { htron_set_ppt(spl, sppt, fppt) == 0 }
}

pub fn set_nv_boost(watts: i32) -> bool {
    unsafe { htron_set_nv_boost(watts) == 0 }
}

pub fn set_nv_temp(celsius: i32) -> bool {
    unsafe { htron_set_nv_temp(celsius) == 0 }
}

pub fn set_gpu_clock(mhz: i32) -> bool {
    unsafe { htron_set_gpu_clock(mhz) == 0 }
}

pub fn set_panel_od(on: bool) -> bool {
    unsafe { htron_set_panel_od(if on { 1 } else { 0 }) == 0 }
}

pub fn set_battery_limit(pct: i32) -> bool {
    unsafe { htron_set_battery_limit(pct) == 0 }
}

pub fn trigger_battery_oneshot() -> bool {
    unsafe { htron_battery_oneshot() == 0 }
}

pub fn set_hz(hz: i32) -> bool {
    unsafe { htron_set_hz(hz) == 0 }
}

pub fn set_kbd(lvl: i32) -> bool {
    unsafe { htron_set_kbd(lvl) == 0 }
}

pub fn set_aura_hex(hex: &str) -> bool {
    if let Ok(c) = CString::new(hex.trim_start_matches('#')) {
        unsafe { htron_set_aura_hex(c.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn run_ctron_cmd(key: &str, val: &str) -> Result<(), String> {
    let ck = CString::new(key).map_err(|e| e.to_string())?;
    let cv = CString::new(val).map_err(|e| e.to_string())?;
    let mut err_buf = [0u8; 160];
    let rc = unsafe {
        htron_cmd_run(
            ck.as_ptr(),
            cv.as_ptr(),
            err_buf.as_mut_ptr() as *mut c_char,
            err_buf.len() as c_int,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        let msg = cbuf_to_string(&err_buf);
        Err(if msg.is_empty() {
            format!("{} {} failed", key, val)
        } else {
            msg
        })
    }
}

pub fn fan_set_point(is_gpu: bool, idx: usize, temp: i32, pwm: i32) -> Option<usize> {
    let res = unsafe {
        htron_fan_set_point(if is_gpu { 1 } else { 0 }, idx as c_int, temp, pwm)
    };
    if res >= 0 {
        Some(res as usize)
    } else {
        None
    }
}

pub fn fan_load_preset(preset_idx: i32) -> bool {
    unsafe { htron_fan_load_preset(preset_idx) == 0 }
}

pub fn fan_quick_preset(preset: i32) -> bool {
    unsafe { htron_fan_quick_preset(preset) == 0 }
}

pub fn fan_set_enabled(cpu_on: bool, gpu_on: bool) -> bool {
    unsafe { htron_fan_set_enabled(if cpu_on { 1 } else { 0 }, if gpu_on { 1 } else { 0 }) == 0 }
}

pub fn fan_write_ec() -> bool {
    unsafe { htron_fan_write_ec() == 0 }
}

pub fn list_modes() -> Vec<ModeItem> {
    let n = unsafe { htron_modes_count() };
    let mut out = Vec::new();
    for i in 0..n {
        let mut nb = [0u8; 32];
        let mut sb = [0u8; 256];
        if unsafe {
            htron_mode_get(
                i,
                nb.as_mut_ptr() as *mut c_char,
                nb.len() as c_int,
                sb.as_mut_ptr() as *mut c_char,
                sb.len() as c_int,
            )
        } == 0
        {
            out.push(ModeItem {
                name: cbuf_to_string(&nb),
                steps: cbuf_to_string(&sb),
            });
        }
    }
    out
}

pub fn apply_mode(name: &str) -> Result<(), String> {
    let cn = CString::new(name).map_err(|e| e.to_string())?;
    let mut err_buf = [0u8; 160];
    let rc = unsafe {
        htron_mode_apply_name(
            cn.as_ptr(),
            err_buf.as_mut_ptr() as *mut c_char,
            err_buf.len() as c_int,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        let msg = cbuf_to_string(&err_buf);
        Err(if msg.is_empty() {
            format!("Failed to apply mode '{}'", name)
        } else {
            msg
        })
    }
}

pub fn save_mode(name: &str, steps: &str) -> bool {
    if let (Ok(cn), Ok(cs)) = (CString::new(name), CString::new(steps)) {
        unsafe { htron_mode_save_item(cn.as_ptr(), cs.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn delete_mode(name: &str) -> bool {
    if let Ok(cn) = CString::new(name) {
        unsafe { htron_mode_delete_item(cn.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn list_profiles() -> Vec<ProfileItem> {
    let n = unsafe { htron_profiles_count() };
    let mut out = Vec::new();
    for i in 0..n {
        let mut nb = [0u8; 64];
        let mut sb = [0u8; 160];
        let mut db = [0u8; 160];
        let mut noteb = [0u8; 96];
        if unsafe {
            htron_profile_get(
                i,
                nb.as_mut_ptr() as *mut c_char,
                nb.len() as c_int,
                sb.as_mut_ptr() as *mut c_char,
                sb.len() as c_int,
                db.as_mut_ptr() as *mut c_char,
                db.len() as c_int,
                noteb.as_mut_ptr() as *mut c_char,
                noteb.len() as c_int,
            )
        } == 0
        {
            out.push(ProfileItem {
                name: cbuf_to_string(&nb),
                summary: cbuf_to_string(&sb),
                diff: cbuf_to_string(&db),
                note: cbuf_to_string(&noteb),
            });
        }
    }
    out
}

pub fn capture_profile(name: &str, note: &str) -> bool {
    if let (Ok(cn), Ok(cno)) = (CString::new(name), CString::new(note)) {
        unsafe { htron_profile_capture(cn.as_ptr(), cno.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn apply_profile(name: &str) -> Result<(), String> {
    let cn = CString::new(name).map_err(|e| e.to_string())?;
    let mut err_buf = [0u8; 160];
    let rc = unsafe {
        htron_profile_apply(
            cn.as_ptr(),
            err_buf.as_mut_ptr() as *mut c_char,
            err_buf.len() as c_int,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        let msg = cbuf_to_string(&err_buf);
        Err(if msg.is_empty() {
            format!("Failed to apply snapshot '{}'", name)
        } else {
            msg
        })
    }
}

pub fn delete_profile(name: &str) -> bool {
    if let Ok(cn) = CString::new(name) {
        unsafe { htron_profile_remove(cn.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn read_telem_series(series_idx: i32) -> TelemSeriesData {
    let mut buf = [0.0f64; 240];
    let mut mn = 0.0f64;
    let mut mx = 0.0f64;
    let mut last = -1.0f64;
    let n = unsafe {
        htron_telem_series_read(
            series_idx,
            buf.as_mut_ptr(),
            240,
            &mut mn,
            &mut mx,
            &mut last,
        )
    };
    let take = n.clamp(0, 240) as usize;
    TelemSeriesData {
        samples: buf[..take].to_vec(),
        min: mn,
        max: mx,
        last,
    }
}

/* ---- Daeboard & Binds Wrappers -------------------------------------- */

pub fn db_is_up() -> bool {
    unsafe { htron_db_is_up() != 0 }
}

pub fn db_start_daemon() -> bool {
    unsafe { htron_db_start_daemon() == 0 }
}

pub fn db_stop_daemon() -> bool {
    unsafe { htron_db_stop_daemon() == 0 }
}

pub fn db_reload_binds() -> Result<(), String> {
    let mut err = [0u8; 96];
    let rc = unsafe { htron_db_reload_binds(err.as_mut_ptr() as *mut c_char, err.len() as c_int) };
    if rc == 0 {
        Ok(())
    } else {
        let s = cbuf_to_string(&err);
        Err(if s.is_empty() { "reload failed".into() } else { s })
    }
}

pub fn db_set_color(hex: &str) -> bool {
    if let Ok(c) = CString::new(hex.trim_start_matches('#')) {
        unsafe { htron_db_color(c.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn db_set_brightness(level: i32) -> bool {
    unsafe { htron_db_brightness(level.clamp(0, 3)) == 0 }
}

pub fn binds_reload_from_disk() {
    unsafe { htron_binds_reload_disk() };
}

pub fn binds_list() -> Vec<BindKeyItem> {
    let n = unsafe { htron_binds_key_count() };
    let mut out = Vec::new();
    for k in 0..n {
        let mut name_buf = [0u8; 32];
        let mut ctron_buf = [0u8; 96];
        let mut n_down = 0i32;
        let mut n_up = 0i32;
        if unsafe {
            htron_binds_key_info(
                k,
                name_buf.as_mut_ptr() as *mut c_char,
                name_buf.len() as c_int,
                ctron_buf.as_mut_ptr() as *mut c_char,
                ctron_buf.len() as c_int,
                &mut n_down,
                &mut n_up,
            )
        } == 0
        {
            let mut down_steps = Vec::new();
            for s in 0..n_down {
                let mut sb = [0u8; 64];
                if unsafe {
                    htron_binds_step_get(k, 0, s, sb.as_mut_ptr() as *mut c_char, sb.len() as c_int)
                } == 0
                {
                    down_steps.push(cbuf_to_string(&sb));
                }
            }
            let mut up_steps = Vec::new();
            for s in 0..n_up {
                let mut sb = [0u8; 64];
                if unsafe {
                    htron_binds_step_get(k, 1, s, sb.as_mut_ptr() as *mut c_char, sb.len() as c_int)
                } == 0
                {
                    up_steps.push(cbuf_to_string(&sb));
                }
            }
            out.push(BindKeyItem {
                name: cbuf_to_string(&name_buf),
                ctron_cmd: cbuf_to_string(&ctron_buf),
                down_steps,
                up_steps,
            });
        }
    }
    out
}

pub fn binds_add_step(key_idx: usize, is_up: bool, step: &str) -> bool {
    if let Ok(cs) = CString::new(step) {
        unsafe { htron_binds_step_add(key_idx as c_int, if is_up { 1 } else { 0 }, cs.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn binds_del_step(key_idx: usize, is_up: bool, step_idx: usize) -> bool {
    unsafe {
        htron_binds_step_del(
            key_idx as c_int,
            if is_up { 1 } else { 0 },
            step_idx as c_int,
        ) == 0
    }
}

pub fn binds_set_ctron(key_idx: usize, cmd: &str) -> bool {
    if let Ok(cs) = CString::new(cmd) {
        unsafe { htron_binds_set_ctron(key_idx as c_int, cs.as_ptr()) == 0 }
    } else {
        false
    }
}

pub fn binds_add_key(name: &str) -> Option<usize> {
    if let Ok(cs) = CString::new(name) {
        let rc = unsafe { htron_binds_add_key(cs.as_ptr()) };
        if rc >= 0 {
            Some(rc as usize)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn binds_del_key(key_idx: usize) -> bool {
    unsafe { htron_binds_del_key(key_idx as c_int) == 0 }
}

pub fn binds_save_and_reload() -> Result<(), String> {
    let mut err = [0u8; 96];
    let rc = unsafe {
        htron_binds_save_and_reload(err.as_mut_ptr() as *mut c_char, err.len() as c_int)
    };
    if rc == 0 {
        Ok(())
    } else {
        let s = cbuf_to_string(&err);
        Err(if s.is_empty() {
            "saved, but daemon reload failed".into()
        } else {
            s
        })
    }
}
