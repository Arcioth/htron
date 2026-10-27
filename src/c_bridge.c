#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include "hw.h"
#include "control.h"
#include "fan.h"
#include "daeboard.h"
#include "cmds.h"
#include "modes.h"
#include "profile.h"
#include "settings.h"
#include "telem.h"
#include "util.h"
#include "display/display.h"

#include <ctype.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static pthread_mutex_t g_mu = PTHREAD_MUTEX_INITIALIZER;
static hw_state_t g_hw;
static bool g_inited = false;

static mode_def_t g_modes[MODES_MAX];
static int g_nmodes = 0;
static char g_active_mode[MODE_NAME_MAX] = "";
static unsigned g_mode_mask = 0;
static mode_snap_t g_mode_snap;

/* ---- daeboard.binds in-memory representation ------------------------- */
#define DBE_KEYS 24
#define DBE_STEPS 32

typedef struct {
    char name[24];
    char down[DBE_STEPS][48];
    char up[DBE_STEPS][48];
    int n_down, n_up;
    char ctron[72];
} dbe_key_t;

static dbe_key_t g_bkeys[DBE_KEYS];
static int g_nbkeys = 0;

static void b_trim(char *s) {
    char *e;
    while (*s == ' ' || *s == '\t')
        memmove(s, s + 1, strlen(s));
    e = s + strlen(s);
    while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\r' || e[-1] == '\n'))
        *--e = 0;
}

static void b_seed(void) {
    g_nbkeys = 3;
    memset(g_bkeys, 0, sizeof g_bkeys);
    snprintf(g_bkeys[0].name, sizeof g_bkeys[0].name, "leftmeta");
    snprintf(g_bkeys[0].down[0], sizeof g_bkeys[0].down[0], "breathe 0");
    g_bkeys[0].n_down = 1;
    snprintf(g_bkeys[0].up[0], sizeof g_bkeys[0].up[0], "rest");
    g_bkeys[0].n_up = 1;

    snprintf(g_bkeys[1].name, sizeof g_bkeys[1].name, "enter");
    snprintf(g_bkeys[1].down[0], sizeof g_bkeys[1].down[0], "brightness 0 120");
    snprintf(g_bkeys[1].down[1], sizeof g_bkeys[1].down[1], "brightness 3 120");
    snprintf(g_bkeys[1].down[2], sizeof g_bkeys[1].down[2], "brightness 0 120");
    snprintf(g_bkeys[1].down[3], sizeof g_bkeys[1].down[3], "brightness 3 120");
    g_bkeys[1].n_down = 4;

    snprintf(g_bkeys[2].name, sizeof g_bkeys[2].name, "backspace");
    snprintf(g_bkeys[2].down[0], sizeof g_bkeys[2].down[0], "color ff0000 0");
    g_bkeys[2].n_down = 1;
    snprintf(g_bkeys[2].up[0], sizeof g_bkeys[2].up[0], "fade 2000");
    g_bkeys[2].n_up = 1;
}

static void b_load_unlocked(void) {
    char path[512];
    FILE *f;
    char line[8192];
    dbe_key_t *cur = NULL;

    db_binds_path(path, (int)sizeof path);
    f = fopen(path, "r");
    g_nbkeys = 0;
    memset(g_bkeys, 0, sizeof g_bkeys);
    if (!f) {
        b_seed();
        return;
    }
    while (fgets(line, sizeof line, f)) {
        char *s = line;
        char *lb;
        b_trim(s);
        if (!*s || *s == '#')
            continue;
        if (*s == '[') {
            char *e = strchr(s, ']');
            if (!e || g_nbkeys >= DBE_KEYS)
                continue;
            *e = 0;
            cur = &g_bkeys[g_nbkeys++];
            memset(cur, 0, sizeof *cur);
            snprintf(cur->name, sizeof cur->name, "%.*s", (int)sizeof cur->name - 1, s + 1);
            continue;
        }
        if (!cur)
            continue;
        lb = strchr(s, '=');
        if (!lb)
            continue;
        *lb = 0;
        b_trim(s);
        b_trim(lb + 1);
        if (!strcmp(s, "ctron")) {
            snprintf(cur->ctron, sizeof cur->ctron, "%s", lb + 1);
        } else if (!strcmp(s, "down") || !strcmp(s, "up")) {
            char *list = lb + 1;
            int up = !strcmp(s, "up");
            char *dst = up ? cur->up[0] : cur->down[0];
            int *pn = up ? &cur->n_up : &cur->n_down;
            while (*list && *pn < DBE_STEPS) {
                char *comma = strchr(list, ',');
                char *slot = dst + (*pn) * 48;
                int n;
                if (comma)
                    *comma = 0;
                b_trim(list);
                n = (int)strlen(list);
                if (n > 47)
                    n = 47;
                memcpy(slot, list, (size_t)n);
                slot[n] = 0;
                if (slot[0])
                    (*pn)++;
                if (!comma)
                    break;
                list = comma + 1;
            }
        }
    }
    fclose(f);
    if (g_nbkeys == 0)
        b_seed();
}

static long mono_ms_now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long)ts.tv_sec * 1000L + (long)(ts.tv_nsec / 1000000L);
}

static void sample_telem_unlocked(hw_state_t *hw) {
    static long prev_ms = -1;
    static long long prev_rd = -1, prev_wr = -1;
    long now_ms = mono_ms_now();

    if (!telem_tick(now_ms))
        return;

    double rd_bs = -1, wr_bs = -1;
    if (hw->disk_rd_sectors >= 0 && prev_rd >= 0 && prev_ms > 0 && now_ms > prev_ms) {
        double dt = (double)(now_ms - prev_ms) / 1000.0;
        long long d = hw->disk_rd_sectors - prev_rd;
        if (d >= 0)
            rd_bs = (double)d * 512.0 / dt;
        d = hw->disk_wr_sectors - prev_wr;
        if (d >= 0)
            wr_bs = (double)d * 512.0 / dt;
    }
    prev_rd = hw->disk_rd_sectors;
    prev_wr = hw->disk_wr_sectors;
    prev_ms = now_ms;

    telem_push(TS_CPU_TEMP, hw->cpu_temp);
    telem_push(TS_CPU_MHZ, hw->cpu_mhz_cur);
    telem_push(TS_CPU_LOAD, hw->load1);
    telem_push(TS_GPU_TEMP, hw->gpu_temp);
    telem_push(TS_GPU_MHZ, hw->gpu_mhz);
    telem_push(TS_GPU_MW, hw->gpu_mw);
    telem_push(TS_GPU_UTIL, hw->gpu_util);
    telem_push(TS_IGPU_TEMP, hw->igpu_temp);
    telem_push(TS_IGPU_MHZ, hw->igpu_mhz);
    telem_push(TS_IGPU_MW, hw->igpu_mw);
    telem_push(TS_FAN_CPU, hw->rpm_cpu);
    telem_push(TS_FAN_GPU, hw->rpm_gpu);
    telem_push(TS_MEM_USED, hw->mem_used_mb);
    telem_push(TS_DISK_RD, rd_bs);
    telem_push(TS_DISK_WR, wr_bs);
    telem_push(TS_BAT_MW, hw->bat_mw_known ? (hw->bat_mw < 0 ? -hw->bat_mw : hw->bat_mw) : -1);
    telem_push(TS_BAT_PCT, hw->bat_pct);
    telem_push(TS_PD_MW, hw->pd_in_mw > 0 ? hw->pd_in_mw : -1);
}

/* Flat, ABI-stable snapshot struct exported to Rust */
typedef struct {
    char model[64];
    char cpu[128];
    char bat_status[16];
    char bat_power_fmt[32];
    char active_mode[24];
    char latest_log[128];

    int32_t is_asus;
    int32_t has_asusctl;
    int32_t has_armoury;
    int32_t has_fan_curve;
    int32_t has_fan_rpm;
    int32_t has_nvidia_smi;
    int32_t has_kbd_led;

    int32_t profile;
    int32_t epp;
    int32_t asusd_ac;
    int32_t asusd_bat;
    int32_t asusd_enforced;

    int32_t cpu_temp;
    int32_t gpu_temp;
    int32_t cpu_mhz_cur;
    int32_t cpu_mhz_min;
    int32_t cpu_mhz_max;
    int32_t cpu_mhz_limit;
    int32_t rpm_cpu;
    int32_t rpm_gpu;

    int32_t bat_pct;
    int32_t ac_online;
    int32_t bat_mw_known;
    int32_t bat_mw;
    int32_t bat_limit;
    int32_t bat_health;
    int32_t bat_v_mv;
    int32_t bat_a_ma;
    int32_t pd_n;
    int32_t pd_in_mw;

    int32_t ppt_spl;
    int32_t ppt_sppt;
    int32_t ppt_fppt;
    int32_t ppt_mode;
    int32_t ppt_off;
    int32_t spl_min, spl_max;
    int32_t sppt_min, sppt_max;
    int32_t fppt_min, fppt_max;

    int32_t nv_boost;
    int32_t nv_temp;
    int32_t gpu_clock_lock;
    int32_t gpu_mhz_max;
    int32_t gpu_mw;
    int32_t gpu_mhz;
    int32_t gpu_util;
    int32_t gpu_vram_used_mb;
    int32_t gpu_vram_total_mb;

    double load1;
    int32_t mem_total_mb;
    int32_t mem_avail_mb;
    int32_t mem_used_mb;
    int32_t swap_total_mb;
    int32_t swap_used_mb;
    int32_t igpu_temp;
    int32_t igpu_mhz;
    int32_t igpu_mw;
    int32_t ram_temp0;
    int32_t ram_temp1;
    int32_t nvme_temp0;
    int32_t nvme_temp1;
    int32_t fs_used_mb;
    int32_t fs_total_mb;

    int32_t panel_od;
    int32_t cpu_boost;
    int32_t kbd;

    int32_t fan_cpu_n;
    int32_t fan_cpu_t[8];
    int32_t fan_cpu_p[8];
    int32_t fan_gpu_n;
    int32_t fan_gpu_t[8];
    int32_t fan_gpu_p[8];
    int32_t fan_cpu_on;
    int32_t fan_gpu_on;
    int32_t fan_staged;

    int32_t hz_cur;
    int32_t hz_modes[16];
    int32_t hz_count;

    int32_t mode_drifted;
    int32_t daeboard_up;
} htron_hw_snap_t;

void htron_core_init(void) {
    pthread_mutex_lock(&g_mu);
    if (!g_inited) {
        telem_reset();
        hw_init(&g_hw);
        settings_load(&g_hw);
        hw_refresh_live(&g_hw);
        sample_telem_unlocked(&g_hw);
        g_nmodes = modes_load(g_modes, MODES_MAX);
        b_load_unlocked();
        g_inited = true;
    }
    pthread_mutex_unlock(&g_mu);
}

void htron_core_refresh_fast(void) {
    pthread_mutex_lock(&g_mu);
    if (g_inited) {
        hw_refresh_fast(&g_hw);
        sample_telem_unlocked(&g_hw);
    }
    pthread_mutex_unlock(&g_mu);
}

void htron_core_refresh_live(void) {
    pthread_mutex_lock(&g_mu);
    if (g_inited) {
        hw_refresh_live(&g_hw);
        sample_telem_unlocked(&g_hw);
    }
    pthread_mutex_unlock(&g_mu);
}

void htron_core_get_snap(htron_hw_snap_t *out, int probe_daeboard) {
    if (!out)
        return;
    pthread_mutex_lock(&g_mu);
    memset(out, 0, sizeof *out);
    snprintf(out->model, sizeof out->model, "%s", g_hw.model);
    snprintf(out->cpu, sizeof out->cpu, "%s", g_hw.cpu);
    snprintf(out->bat_status, sizeof out->bat_status, "%s", g_hw.bat_status);
    hw_fmt_power(out->bat_power_fmt, sizeof out->bat_power_fmt, g_hw.bat_mw_known, g_hw.bat_mw);
    snprintf(out->active_mode, sizeof out->active_mode, "%s", g_active_mode);
    if (ut_log_count() > 0) {
        const char *l = ut_log_get(0);
        if (l)
            snprintf(out->latest_log, sizeof out->latest_log, "%s", l);
    }

    out->is_asus = g_hw.is_asus ? 1 : 0;
    out->has_asusctl = g_hw.has_asusctl ? 1 : 0;
    out->has_armoury = g_hw.has_armoury ? 1 : 0;
    out->has_fan_curve = g_hw.has_fan_curve ? 1 : 0;
    out->has_fan_rpm = g_hw.has_fan_rpm ? 1 : 0;
    out->has_nvidia_smi = g_hw.has_nvidia_smi ? 1 : 0;
    out->has_kbd_led = g_hw.has_kbd_led ? 1 : 0;

    out->profile = (int32_t)g_hw.profile;
    out->epp = (int32_t)g_hw.epp;
    out->asusd_ac = g_hw.asusd_ac;
    out->asusd_bat = g_hw.asusd_bat;
    out->asusd_enforced = hw_asusd_enforced(&g_hw);

    out->cpu_temp = g_hw.cpu_temp;
    out->gpu_temp = g_hw.gpu_temp;
    out->cpu_mhz_cur = g_hw.cpu_mhz_cur;
    out->cpu_mhz_min = g_hw.cpu_mhz_min;
    out->cpu_mhz_max = g_hw.cpu_mhz_max;
    out->cpu_mhz_limit = g_hw.cpu_mhz_limit;
    out->rpm_cpu = g_hw.rpm_cpu;
    out->rpm_gpu = g_hw.rpm_gpu;

    out->bat_pct = g_hw.bat_pct;
    out->ac_online = g_hw.ac_online ? 1 : 0;
    out->bat_mw_known = g_hw.bat_mw_known ? 1 : 0;
    out->bat_mw = g_hw.bat_mw;
    out->bat_limit = g_hw.bat_limit;
    out->bat_health = g_hw.bat_health;
    out->bat_v_mv = g_hw.bat_v_mv;
    out->bat_a_ma = g_hw.bat_a_ma;
    out->pd_n = g_hw.pd_n;
    out->pd_in_mw = g_hw.pd_in_mw;

    out->ppt_spl = g_hw.ppt_spl;
    out->ppt_sppt = g_hw.ppt_sppt;
    out->ppt_fppt = g_hw.ppt_fppt;
    out->ppt_mode = g_hw.ppt_mode;
    out->ppt_off = g_hw.ppt_off ? 1 : 0;
    {
        int smin = 15, smax = 120, pmin = 15, pmax = 130, fmin = 15, fmax = 140;
        ctrl_ppt_limits(&g_hw, &smin, &smax, &pmin, &pmax, &fmin, &fmax);
        out->spl_min = smin;
        out->spl_max = smax;
        out->sppt_min = pmin;
        out->sppt_max = pmax;
        out->fppt_min = fmin;
        out->fppt_max = fmax;
    }

    out->nv_boost = g_hw.nv_boost;
    out->nv_temp = g_hw.nv_temp;
    out->gpu_clock_lock = g_hw.gpu_clock_lock;
    out->gpu_mhz_max = g_hw.gpu_mhz_max;
    out->gpu_mw = g_hw.gpu_mw;
    out->gpu_mhz = g_hw.gpu_mhz;
    out->gpu_util = g_hw.gpu_util;
    out->gpu_vram_used_mb = g_hw.gpu_vram_used_mb;
    out->gpu_vram_total_mb = g_hw.gpu_vram_total_mb;

    out->load1 = g_hw.load1;
    out->mem_total_mb = g_hw.mem_total_mb;
    out->mem_avail_mb = g_hw.mem_avail_mb;
    out->mem_used_mb = g_hw.mem_used_mb;
    out->swap_total_mb = g_hw.swap_total_mb;
    out->swap_used_mb = g_hw.swap_used_mb;
    out->igpu_temp = g_hw.igpu_temp;
    out->igpu_mhz = g_hw.igpu_mhz;
    out->igpu_mw = g_hw.igpu_mw;
    out->ram_temp0 = g_hw.ram_temp_n >= 1 ? g_hw.ram_temp[0] : -1;
    out->ram_temp1 = g_hw.ram_temp_n >= 2 ? g_hw.ram_temp[1] : -1;
    out->nvme_temp0 = g_hw.nvme_temp_n >= 1 ? g_hw.nvme_temp[0] : -1;
    out->nvme_temp1 = g_hw.nvme_temp_n >= 2 ? g_hw.nvme_temp[1] : -1;
    out->fs_used_mb = g_hw.fs_used_mb;
    out->fs_total_mb = g_hw.fs_total_mb;

    out->panel_od = g_hw.panel_od ? 1 : 0;
    out->cpu_boost = g_hw.cpu_boost ? 1 : 0;
    out->kbd = (int32_t)g_hw.kbd;

    out->fan_cpu_n = g_hw.fan_cpu.n;
    for (int i = 0; i < 8; i++) {
        out->fan_cpu_t[i] = g_hw.fan_cpu.temp_c[i];
        out->fan_cpu_p[i] = g_hw.fan_cpu.pwm[i];
    }
    out->fan_gpu_n = g_hw.fan_gpu.n;
    for (int i = 0; i < 8; i++) {
        out->fan_gpu_t[i] = g_hw.fan_gpu.temp_c[i];
        out->fan_gpu_p[i] = g_hw.fan_gpu.pwm[i];
    }
    out->fan_cpu_on = g_hw.fan_cpu_on ? 1 : 0;
    out->fan_gpu_on = g_hw.fan_gpu_on ? 1 : 0;
    out->fan_staged = g_hw.fan_staged ? 1 : 0;

    out->hz_cur = g_hw.hz_cur;
    out->hz_count = g_hw.hz_count;
    for (int i = 0; i < 16 && i < g_hw.hz_count; i++) {
        out->hz_modes[i] = g_hw.hz_modes[i];
    }

    if (g_active_mode[0] && g_mode_mask) {
        out->mode_drifted = mode_drift_count(&g_hw, g_mode_mask, &g_mode_snap) > 0 ? 1 : 0;
    } else {
        out->mode_drifted = 0;
    }
    pthread_mutex_unlock(&g_mu);

    if (probe_daeboard) {
        out->daeboard_up = db_up() ? 1 : 0;
    }
}

/* ---- Explicit User Control Actions ---------------------------------- */

int htron_set_profile(int p) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_profile(&g_hw, (hw_profile_t)p);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_asusd_auto(int ac, int bat) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_asusd_auto(&g_hw, ac, bat);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_epp(int e) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_epp(&g_hw, (hw_epp_t)e);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_cpu_max_mhz(int mhz) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_cpu_max_mhz(&g_hw, mhz);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_cpu_boost(int on) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_cpu_boost(&g_hw, on != 0);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_ppt(int spl, int sppt, int fppt) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_ppt(&g_hw, spl, sppt, fppt);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_nv_boost(int watts) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_nv_boost(&g_hw, watts);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_nv_temp(int celsius) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_nv_temp(&g_hw, celsius);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_gpu_clock(int mhz) {
    int rc;
    pthread_mutex_lock(&g_mu);
    if (mhz <= 0)
        rc = ctrl_gpu_clock_reset(&g_hw);
    else
        rc = ctrl_gpu_clock_lock(&g_hw, mhz);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_panel_od(int on) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_panel_od(&g_hw, on != 0);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_battery_limit(int pct) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_battery_limit(&g_hw, pct);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_battery_oneshot(void) {
    return ctrl_battery_oneshot();
}

int htron_set_hz(int hz) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_hz(&g_hw, hz);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_kbd(int lvl) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_kbd(&g_hw, (hw_kbd_t)lvl);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_aura(int effect_idx, int color_idx) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_aura(&g_hw, effect_idx, color_idx);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_set_aura_hex(const char *hex) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_set_aura_hex(&g_hw, hex);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_cmd_run(const char *key, const char *val, char *err, int errn) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = cmd_run(&g_hw, key, val, err, (size_t)errn);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

/* ---- Fan Curve Operations ------------------------------------------- */

int htron_fan_set_point(int is_gpu, int idx, int temp, int pwm) {
    int new_idx;
    pthread_mutex_lock(&g_mu);
    fan_curve_t *fc = is_gpu ? &g_hw.fan_gpu : &g_hw.fan_cpu;
    new_idx = fan_set_point(fc, idx, temp, pwm);
    if (new_idx >= 0)
        g_hw.fan_staged = true;
    pthread_mutex_unlock(&g_mu);
    return new_idx;
}

int htron_fan_load_preset(int preset_idx) {
    int rc;
    fan_preset_info_t info;
    pthread_mutex_lock(&g_mu);
    rc = fan_preset_get(&g_hw, (fan_preset_t)preset_idx, &g_hw.fan_cpu, &g_hw.fan_gpu, &info);
    if (rc == 0)
        g_hw.fan_staged = true;
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_fan_quick_preset(int preset) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_fan_preset(&g_hw, preset);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_fan_set_enabled(int cpu_on, int gpu_on) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_fan_set_enabled(&g_hw, cpu_on != 0, gpu_on != 0);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_fan_write_ec(void) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = ctrl_fan_write(&g_hw);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

/* ---- Modes & Snapshots ---------------------------------------------- */

int htron_modes_count(void) {
    int n;
    pthread_mutex_lock(&g_mu);
    g_nmodes = modes_load(g_modes, MODES_MAX);
    n = g_nmodes;
    pthread_mutex_unlock(&g_mu);
    return n;
}

int htron_mode_get(int idx, char *name, int nn, char *steps, int sn) {
    pthread_mutex_lock(&g_mu);
    if (idx < 0 || idx >= g_nmodes) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    if (name && nn > 0)
        snprintf(name, (size_t)nn, "%s", g_modes[idx].name);
    if (steps && sn > 0)
        snprintf(steps, (size_t)sn, "%s", g_modes[idx].steps);
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_mode_apply_name(const char *name, char *err, int errn) {
    int rc = -1;
    pthread_mutex_lock(&g_mu);
    g_nmodes = modes_load(g_modes, MODES_MAX);
    int idx = mode_find(g_modes, g_nmodes, name);
    if (idx < 0) {
        if (err && errn > 0)
            snprintf(err, (size_t)errn, "unknown mode '%s'", name ? name : "");
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    rc = mode_apply(&g_hw, g_modes[idx].steps, err, (size_t)errn);
    if (rc == 0) {
        snprintf(g_active_mode, sizeof g_active_mode, "%s", g_modes[idx].name);
        unsigned want = mode_touch_mask(g_modes[idx].steps);
        g_mode_mask = mode_snapshot(&g_hw, want, &g_mode_snap);
        settings_save(&g_hw);
    }
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_mode_save_item(const char *name, const char *steps) {
    int rc = -1;
    if (!name || !name[0] || !steps || !steps[0])
        return -1;
    pthread_mutex_lock(&g_mu);
    g_nmodes = modes_load(g_modes, MODES_MAX);
    int idx = mode_find(g_modes, g_nmodes, name);
    if (idx >= 0) {
        snprintf(g_modes[idx].steps, sizeof g_modes[idx].steps, "%s", steps);
        rc = modes_save(g_modes, g_nmodes);
    } else if (g_nmodes < MODES_MAX) {
        snprintf(g_modes[g_nmodes].name, sizeof g_modes[g_nmodes].name, "%s", name);
        snprintf(g_modes[g_nmodes].steps, sizeof g_modes[g_nmodes].steps, "%s", steps);
        g_nmodes++;
        rc = modes_save(g_modes, g_nmodes);
    }
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_mode_delete_item(const char *name) {
    int rc = -1;
    pthread_mutex_lock(&g_mu);
    g_nmodes = modes_load(g_modes, MODES_MAX);
    int idx = mode_find(g_modes, g_nmodes, name);
    if (idx >= 0) {
        for (int i = idx; i + 1 < g_nmodes; i++)
            g_modes[i] = g_modes[i + 1];
        g_nmodes--;
        rc = modes_save(g_modes, g_nmodes);
    }
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_profiles_count(void) {
    char list[MAX_PROFILES][PROFILE_NAME_MAX];
    int n;
    pthread_mutex_lock(&g_mu);
    n = profile_list(list, MAX_PROFILES);
    pthread_mutex_unlock(&g_mu);
    return n;
}

int htron_profile_get(int idx, char *name, int nn, char *summary, int sn, char *diff, int dn, char *note, int noten) {
    char list[MAX_PROFILES][PROFILE_NAME_MAX];
    pthread_mutex_lock(&g_mu);
    int n = profile_list(list, MAX_PROFILES);
    if (idx < 0 || idx >= n) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    if (name && nn > 0)
        snprintf(name, (size_t)nn, "%s", list[idx]);
    if (summary && sn > 0)
        profile_summary(list[idx], summary, (size_t)sn);
    profile_meta_t m;
    if (profile_meta_parse(list[idx], &m) == 0) {
        if (diff && dn > 0)
            profile_meta_diff(&g_hw, &m, diff, (size_t)dn);
        if (note && noten > 0)
            snprintf(note, (size_t)noten, "%s", m.note);
    } else {
        if (diff && dn > 0)
            diff[0] = 0;
        if (note && noten > 0)
            note[0] = 0;
    }
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_profile_capture(const char *name, const char *note) {
    char clean[PROFILE_NAME_MAX];
    int rc;
    pthread_mutex_lock(&g_mu);
    if (!name || !name[0])
        profile_gen_name(clean, sizeof clean);
    else
        profile_sanitize_name(name, clean, sizeof clean);
    rc = profile_export(clean, &g_hw);
    if (rc == 0 && note && note[0])
        profile_set_note(clean, note);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_profile_apply(const char *name, char *err, int errn) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = profile_import(name, &g_hw, err, (size_t)errn);
    if (rc == 0)
        settings_save(&g_hw);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

int htron_profile_remove(const char *name) {
    int rc;
    pthread_mutex_lock(&g_mu);
    rc = profile_delete(name);
    pthread_mutex_unlock(&g_mu);
    return rc;
}

/* ---- Telemetry Ring Extraction -------------------------------------- */

int htron_telem_series_read(int series_idx, double *out_samples, int max_samples,
                            double *out_min, double *out_max, double *out_last) {
    pthread_mutex_lock(&g_mu);
    int cnt = telem_count(series_idx);
    if (cnt <= 0) {
        if (out_min) *out_min = 0.0;
        if (out_max) *out_max = 0.0;
        if (out_last) *out_last = -1.0;
        pthread_mutex_unlock(&g_mu);
        return 0;
    }
    int take = cnt < max_samples ? cnt : max_samples;
    for (int i = 0; i < take; i++) {
        int back = (take - 1) - i;
        double v = -1.0;
        telem_at(series_idx, back, &v);
        out_samples[i] = v;
    }
    double mn = 0.0, mx = 0.0;
    telem_minmax(series_idx, &mn, &mx);
    if (out_min) *out_min = mn;
    if (out_max) *out_max = mx;
    if (out_last) *out_last = telem_last(series_idx);
    pthread_mutex_unlock(&g_mu);
    return take;
}

/* ---- Daeboard & daeboard.binds Studio ------------------------------- */

int htron_db_is_up(void) {
    return db_up();
}

int htron_db_start_daemon(void) {
    return db_start();
}

int htron_db_stop_daemon(void) {
    return db_quit();
}

int htron_db_reload_binds(char *err, int errn) {
    return db_reload(err, errn);
}

int htron_db_color(const char *hex) {
    return db_set_color(hex);
}

int htron_db_brightness(int level) {
    return db_set_brightness(level);
}

void htron_binds_reload_disk(void) {
    pthread_mutex_lock(&g_mu);
    b_load_unlocked();
    pthread_mutex_unlock(&g_mu);
}

int htron_binds_key_count(void) {
    int n;
    pthread_mutex_lock(&g_mu);
    n = g_nbkeys;
    pthread_mutex_unlock(&g_mu);
    return n;
}

int htron_binds_key_info(int idx, char *name, int nn, char *ctron_cmd, int cn, int *n_down, int *n_up) {
    pthread_mutex_lock(&g_mu);
    if (idx < 0 || idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    if (name && nn > 0)
        snprintf(name, (size_t)nn, "%s", g_bkeys[idx].name);
    if (ctron_cmd && cn > 0)
        snprintf(ctron_cmd, (size_t)cn, "%s", g_bkeys[idx].ctron);
    if (n_down)
        *n_down = g_bkeys[idx].n_down;
    if (n_up)
        *n_up = g_bkeys[idx].n_up;
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_step_get(int key_idx, int is_up, int step_idx, char *out, int outn) {
    pthread_mutex_lock(&g_mu);
    if (key_idx < 0 || key_idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    int n = is_up ? g_bkeys[key_idx].n_up : g_bkeys[key_idx].n_down;
    if (step_idx < 0 || step_idx >= n) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    const char *src = is_up ? g_bkeys[key_idx].up[step_idx] : g_bkeys[key_idx].down[step_idx];
    if (out && outn > 0)
        snprintf(out, (size_t)outn, "%s", src);
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_step_add(int key_idx, int is_up, const char *step) {
    if (!step || !step[0])
        return -1;
    pthread_mutex_lock(&g_mu);
    if (key_idx < 0 || key_idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    int *pn = is_up ? &g_bkeys[key_idx].n_up : &g_bkeys[key_idx].n_down;
    if (*pn >= DBE_STEPS) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    char *dst = is_up ? g_bkeys[key_idx].up[*pn] : g_bkeys[key_idx].down[*pn];
    snprintf(dst, 48, "%s", step);
    (*pn)++;
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_step_del(int key_idx, int is_up, int step_idx) {
    pthread_mutex_lock(&g_mu);
    if (key_idx < 0 || key_idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    int *pn = is_up ? &g_bkeys[key_idx].n_up : &g_bkeys[key_idx].n_down;
    if (step_idx < 0 || step_idx >= *pn) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    for (int i = step_idx; i + 1 < *pn; i++) {
        char *dst = is_up ? g_bkeys[key_idx].up[i] : g_bkeys[key_idx].down[i];
        const char *src = is_up ? g_bkeys[key_idx].up[i + 1] : g_bkeys[key_idx].down[i + 1];
        memcpy(dst, src, 48);
    }
    (*pn)--;
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_set_ctron(int key_idx, const char *cmd) {
    pthread_mutex_lock(&g_mu);
    if (key_idx < 0 || key_idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    snprintf(g_bkeys[key_idx].ctron, sizeof g_bkeys[key_idx].ctron, "%s", cmd ? cmd : "");
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_add_key(const char *name) {
    if (!name || !name[0])
        return -1;
    pthread_mutex_lock(&g_mu);
    for (int i = 0; i < g_nbkeys; i++) {
        if (!strcasecmp(g_bkeys[i].name, name)) {
            pthread_mutex_unlock(&g_mu);
            return i;
        }
    }
    if (g_nbkeys >= DBE_KEYS) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    int idx = g_nbkeys++;
    memset(&g_bkeys[idx], 0, sizeof g_bkeys[idx]);
    snprintf(g_bkeys[idx].name, sizeof g_bkeys[idx].name, "%s", name);
    snprintf(g_bkeys[idx].down[0], sizeof g_bkeys[idx].down[0], "color 00f0ff 200");
    g_bkeys[idx].n_down = 1;
    pthread_mutex_unlock(&g_mu);
    return idx;
}

int htron_binds_del_key(int key_idx) {
    pthread_mutex_lock(&g_mu);
    if (key_idx < 0 || key_idx >= g_nbkeys) {
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    for (int i = key_idx; i + 1 < g_nbkeys; i++)
        g_bkeys[i] = g_bkeys[i + 1];
    g_nbkeys--;
    pthread_mutex_unlock(&g_mu);
    return 0;
}

int htron_binds_save_and_reload(char *err, int errn) {
    char path[512];
    FILE *f;

    pthread_mutex_lock(&g_mu);
    db_binds_path(path, (int)sizeof path);
    f = fopen(path, "w");
    if (!f) {
        if (err && errn > 0)
            snprintf(err, (size_t)errn, "cannot write %s", path);
        pthread_mutex_unlock(&g_mu);
        return -1;
    }
    for (int i = 0; i < g_nbkeys; i++) {
        fprintf(f, "[%s]\n", g_bkeys[i].name);
        if (g_bkeys[i].n_down) {
            fprintf(f, "down =");
            for (int s = 0; s < g_bkeys[i].n_down; s++)
                fprintf(f, "%s %s", s ? "," : "", g_bkeys[i].down[s]);
            fprintf(f, "\n");
        }
        if (g_bkeys[i].n_up) {
            fprintf(f, "up =");
            for (int s = 0; s < g_bkeys[i].n_up; s++)
                fprintf(f, "%s %s", s ? "," : "", g_bkeys[i].up[s]);
            fprintf(f, "\n");
        }
        if (g_bkeys[i].ctron[0])
            fprintf(f, "ctron = %s\n", g_bkeys[i].ctron);
        fprintf(f, "\n");
    }
    fclose(f);
    pthread_mutex_unlock(&g_mu);

    if (!db_up())
        db_start();
    return db_reload(err, errn);
}
