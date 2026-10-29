#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub u64);

impl TrackId {
    pub fn from_path(path: &Path) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_sec: f64,
    pub duration_label: String,
    pub track_number: Option<u32>,
}

impl Track {
    pub fn new(path: PathBuf) -> Self {
        let id = TrackId::from_path(&path);
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let fallback_title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.clone());

        Self {
            id,
            path,
            filename,
            title: fallback_title,
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            duration_sec: 0.0,
            duration_label: "--:--".to_string(),
            track_number: None,
        }
    }

    pub fn set_duration(&mut self, sec: f64) {
        self.duration_sec = sec;
        self.duration_label = format_mmss(sec);
    }

    pub fn is_audio_file(path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            ext.eq_ignore_ascii_case("mp3")
                || ext.eq_ignore_ascii_case("flac")
                || ext.eq_ignore_ascii_case("ogg")
                || ext.eq_ignore_ascii_case("opus")
                || ext.eq_ignore_ascii_case("m4a")
                || ext.eq_ignore_ascii_case("aac")
                || ext.eq_ignore_ascii_case("wav")
                || ext.eq_ignore_ascii_case("wma")
                || ext.eq_ignore_ascii_case("alac")
                || ext.eq_ignore_ascii_case("aiff")
        } else {
            false
        }
    }
}

pub fn format_mmss(seconds: f64) -> String {
    let total_sec = seconds.max(0.0).round() as u64;
    let mins = total_sec / 60;
    let secs = total_sec % 60;
    format!("{:02}:{:02}", mins, secs)
}

#[derive(Debug, Clone)]
pub enum BrowserEntry {
    ParentDir(PathBuf),
    Directory { name: String, path: PathBuf },
    AudioTrack(Arc<Track>),
}

/// Canonical jail resolver (Tunotron security invariant)
pub fn resolve_in_jail(canonical_root: &Path, candidate: &Path) -> Option<PathBuf> {
    let canon_cand = candidate.canonicalize().ok()?;
    if canon_cand == canonical_root || canon_cand.starts_with(canonical_root) {
        Some(canon_cand)
    } else {
        None
    }
}

pub fn read_directory(
    dir: &Path,
    root_boundary: &Path,
    cache: &HashMap<PathBuf, Arc<Track>>,
) -> Vec<BrowserEntry> {
    let mut items = Vec::new();

    if dir != root_boundary && dir.starts_with(root_boundary) {
        if let Some(parent) = dir.parent() {
            if let Some(jailed_parent) = resolve_in_jail(root_boundary, parent) {
                items.push(BrowserEntry::ParentDir(jailed_parent));
            }
        }
    }

    if let Ok(entries) = fs::read_dir(dir) {
        let mut dirs = Vec::new();
        let mut tracks: Vec<Arc<Track>> = Vec::new();

        for entry in entries.filter_map(|e| e.ok()) {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            if ft.is_dir() {
                if let Some(jailed_dir) = resolve_in_jail(root_boundary, &path) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    dirs.push(BrowserEntry::Directory {
                        name,
                        path: jailed_dir,
                    });
                }
            } else if ft.is_file() && Track::is_audio_file(&path) {
                if let Some(cached) = cache.get(&path) {
                    tracks.push(Arc::clone(cached));
                } else {
                    tracks.push(Arc::new(Track::new(path)));
                }
            }
        }

        dirs.sort_by(|a, b| match (a, b) {
            (
                BrowserEntry::Directory { name: a_name, .. },
                BrowserEntry::Directory { name: b_name, .. },
            ) => a_name
                .bytes()
                .map(|c| c.to_ascii_lowercase())
                .cmp(b_name.bytes().map(|c| c.to_ascii_lowercase())),
            _ => std::cmp::Ordering::Equal,
        });

        tracks.sort_by(|a, b| match (a.track_number, b.track_number) {
            (Some(an), Some(bn)) if an != bn => an.cmp(&bn),
            _ => a
                .filename
                .bytes()
                .map(|c| c.to_ascii_lowercase())
                .cmp(b.filename.bytes().map(|c| c.to_ascii_lowercase())),
        });

        items.extend(dirs);
        for t in tracks {
            items.push(BrowserEntry::AudioTrack(t));
        }
    }

    items
}

pub fn probe_track_metadata(path: &Path) -> Option<Track> {
    let mut track = Track::new(path.to_path_buf());
    let mut got_duration = false;

    if let Ok(probe) = Probe::open(path) {
        if let Ok(tagged_file) = probe.read() {
            let properties = tagged_file.properties();
            let duration_sec = properties.duration().as_secs_f64();
            if duration_sec > 0.0 {
                track.set_duration(duration_sec);
                got_duration = true;
            }

            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                if let Some(t) = tag.title() {
                    let s = t.trim();
                    if !s.is_empty() {
                        track.title = s.to_string();
                    }
                }
                if let Some(a) = tag.artist() {
                    let s = a.trim();
                    if !s.is_empty() {
                        track.artist = s.to_string();
                    }
                }
                if let Some(al) = tag.album() {
                    let s = al.trim();
                    if !s.is_empty() {
                        track.album = s.to_string();
                    }
                }
                track.track_number = tag.track();
            }
        }
    }

    if !got_duration {
        if let Ok(out) = std::process::Command::new(resolve_bin("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(path)
            .output()
        {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(sec) = s.trim().parse::<f64>() {
                    if sec > 0.0 {
                        track.set_duration(sec);
                    }
                }
            }
        }
    }

    Some(track)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Off,
    Track,
    All,
}

impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            LoopMode::Off => LoopMode::Track,
            LoopMode::Track => LoopMode::All,
            LoopMode::All => LoopMode::Off,
        }
    }

    pub fn display_str(self) -> &'static str {
        match self {
            LoopMode::Off => "Off",
            LoopMode::Track => "Track",
            LoopMode::All => "All",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackClock {
    anchor_pos: f64,
    anchor_at: Instant,
    playing: bool,
    speed: f64,
}

impl PlaybackClock {
    pub fn new() -> Self {
        Self {
            anchor_pos: 0.0,
            anchor_at: Instant::now(),
            playing: false,
            speed: 1.0,
        }
    }

    pub fn now(&self) -> f64 {
        if self.playing {
            self.anchor_pos + self.anchor_at.elapsed().as_secs_f64() * self.speed
        } else {
            self.anchor_pos
        }
    }

    pub fn sync(&mut self, pos: f64) {
        self.anchor_pos = pos.max(0.0);
        self.anchor_at = Instant::now();
    }

    pub fn set_playing(&mut self, playing: bool) {
        let cur = self.now();
        self.anchor_pos = cur;
        self.anchor_at = Instant::now();
        self.playing = playing;
    }

    pub fn set_speed(&mut self, speed: f64) {
        let cur = self.now();
        self.anchor_pos = cur;
        self.anchor_at = Instant::now();
        self.speed = speed.clamp(0.25, 4.0);
    }
}

#[derive(Debug, Clone)]
pub struct XorShift64(pub u64);

impl XorShift64 {
    pub fn from_entropy() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
        Self(nanos | 1)
    }

    pub fn next_bound(&mut self, max: usize) -> usize {
        if max <= 1 {
            return 0;
        }
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x as usize) % max
    }
}

/* ---- Waybar & Outbound Tunotron Status JSON -------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TunotronStatus {
    pub state: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub loop_mode: String,
    pub shuffle: bool,
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u8,
}

impl Default for TunotronStatus {
    fn default() -> Self {
        Self {
            state: "stopped".to_string(),
            title: "No Track".to_string(),
            artist: "".to_string(),
            album: "".to_string(),
            position: 0.0,
            duration: 0.0,
            volume: 100.0,
            loop_mode: "All".to_string(),
            shuffle: false,
            text: "Tunotron Stopped".to_string(),
            alt: "stopped".to_string(),
            tooltip: "No active track".to_string(),
            class: "stopped".to_string(),
            percentage: 0,
        }
    }
}

pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tunotron.sock")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tunotron-{}.sock", uid))
    }
}

pub fn default_status_file_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tunotron-status.json")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tunotron-status-{}.json", uid))
    }
}

/* ---- mpv JSON IPC Protocol ------------------------------------------ */

#[derive(Debug, Serialize)]
struct MpvRequest {
    command: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MpvIncoming {
    Event(MpvEvent),
    Response(MpvResponse),
}

#[derive(Debug, Deserialize)]
struct MpvResponse {
    error: String,
    request_id: Option<u64>,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event")]
enum MpvEvent {
    #[serde(rename = "property-change")]
    PropertyChange {
        id: u64,
        name: String,
        data: Option<serde_json::Value>,
    },
    #[serde(rename = "end-file")]
    EndFile {
        reason: Option<String>,
    },
    #[serde(rename = "file-loaded")]
    FileLoaded,
    #[serde(rename = "playback-restart")]
    PlaybackRestart,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone)]
pub enum AudioCmd {
    OpenDir(PathBuf),
    ParentDir,
    PlayTrackAtIndex(usize),
    PlayPath(PathBuf),
    PlayStream { url: String, title: String },
    TogglePause,
    Stop,
    Next,
    Prev,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(f64),
    SetSpeed(f64),
    ToggleMute,
    CycleLoop,
    ToggleShuffle,
    SetFilter { name: String, filter: String },
    AddToQueue(Arc<Track>),
    ClearQueue,
}

#[derive(Debug, Clone)]
pub struct AudioUiSnapshot {
    pub root_dir: PathBuf,
    pub current_dir: PathBuf,
    pub entries: Vec<BrowserEntry>,
    pub playlist: Vec<Arc<Track>>,
    pub queue: Vec<Arc<Track>>,
    pub current_track: Option<Arc<Track>>,
    pub state: PlayState,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub volume: f64,
    pub speed: f64,
    pub muted: bool,
    pub loop_mode: LoopMode,
    pub shuffle: bool,
    pub active_filter_name: String,
    pub cached_tracks_count: usize,
}

impl AudioUiSnapshot {
    pub fn to_tunotron_status(&self) -> TunotronStatus {
        let state_str = match self.state {
            PlayState::Playing => "playing",
            PlayState::Paused => "paused",
            PlayState::Stopped => "stopped",
        };
        let (title, artist, album) = if let Some(t) = &self.current_track {
            (t.title.clone(), t.artist.clone(), t.album.clone())
        } else {
            ("No Track".to_string(), "".to_string(), "".to_string())
        };
        let pos = self.position_sec;
        let dur = self.duration_sec;
        let percentage = if dur > 0.0 {
            ((pos / dur) * 100.0).clamp(0.0, 100.0) as u8
        } else {
            0
        };
        let text = if self.current_track.is_some() {
            if artist.is_empty() || artist == "Unknown Artist" {
                title.clone()
            } else {
                format!("{} — {}", artist, title)
            }
        } else {
            "Tunotron Stopped".to_string()
        };
        let tooltip = if self.current_track.is_some() {
            format!(
                "{}\nAlbum: {}\n[{} / {}]",
                text,
                if album.is_empty() { "Unknown" } else { &album },
                format_mmss(pos),
                format_mmss(dur)
            )
        } else {
            "No active track".to_string()
        };

        TunotronStatus {
            state: state_str.to_string(),
            title,
            artist,
            album,
            position: (pos * 10.0).round() / 10.0,
            duration: (dur * 10.0).round() / 10.0,
            volume: self.volume,
            loop_mode: self.loop_mode.display_str().to_string(),
            shuffle: self.shuffle,
            text,
            alt: state_str.to_string(),
            tooltip,
            class: state_str.to_string(),
            percentage,
        }
    }
}

struct MpvGuard {
    child: Option<Child>,
    socket_path: PathBuf,
}

impl Drop for MpvGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.start_kill();
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Resolves external audio helper binaries reliably on NixOS & standard Linux
/// even when launched from `.desktop` runners with minimal `PATH`.
fn resolve_bin(name: &str) -> String {
    let nix_sys = format!("/run/current-system/sw/bin/{}", name);
    if Path::new(&nix_sys).exists() {
        return nix_sys;
    }
    if let Ok(home) = std::env::var("HOME") {
        let nix_prof = format!("{}/.nix-profile/bin/{}", home, name);
        if Path::new(&nix_prof).exists() {
            return nix_prof;
        }
    }
    name.to_string()
}

fn effective_clock_speed(filter_name: &str, base_speed: f64) -> f64 {
    let lower = filter_name.to_lowercase();
    if lower.contains("nightcore") {
        1.22
    } else if lower.contains("slowed") || lower.contains("vapor") {
        0.84
    } else {
        base_speed
    }
}

/// Native PipeWire / ffplay fallback audio backend when `mpv` is not installed on NixOS.
struct FfplayPlayer {
    child: Option< std::process::Child >,
    active_source: Option<String>,
    paused: bool,
}

impl FfplayPlayer {
    fn new() -> Self {
        Self {
            child: None,
            active_source: None,
            paused: false,
        }
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let pid = c.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGCONT);
                libc::kill(pid, libc::SIGKILL);
            }
            let _ = c.wait();
        }
        self.active_source = None;
        self.paused = false;
    }

    fn dsp_filter_for_name(name: &str, raw_filter: &str, speed: f64) -> String {
        let mut parts: Vec<String> = Vec::new();
        let lower = name.to_lowercase();
        if lower.contains("bass") {
            parts.push("bass=g=9:f=110:w=0.6".to_string());
        } else if lower.contains("nightcore") {
            parts.push("asetrate=48000*1.22,aresample=48000".to_string());
        } else if lower.contains("slowed") || lower.contains("vapor") {
            parts.push("asetrate=48000*0.84,aresample=48000,aecho=0.8:0.88:60:0.35".to_string());
        } else if lower.contains("8d") {
            parts.push("apulsator=hz=0.125".to_string());
        } else if lower.contains("treble") || lower.contains("crystal") {
            parts.push("treble=g=6:f=3500".to_string());
        } else if !raw_filter.trim().is_empty() {
            let cleaned = raw_filter.trim().trim_start_matches("lavfi=[").trim_end_matches(']');
            if !cleaned.is_empty() {
                parts.push(cleaned.to_string());
            }
        }

        if (speed - 1.0).abs() > 0.03 && !lower.contains("nightcore") && !lower.contains("slowed") {
            parts.push(format!("atempo={:.2}", speed.clamp(0.5, 2.0)));
        }

        parts.join(",")
    }

    fn play(
        &mut self,
        source: &str,
        start_sec: f64,
        volume: f64,
        muted: bool,
        speed: f64,
        filter_name: &str,
        raw_filter: &str,
    ) {
        self.stop();
        self.active_source = Some(source.to_string());
        self.paused = false;

        let parent_pid = unsafe { libc::getpid() };
        let vol_int = if muted {
            0
        } else {
            volume.round().clamp(0.0, 100.0) as i32
        };

        let mut cmd = std::process::Command::new(resolve_bin("ffplay"));
        cmd.args([
            "-nodisp",
            "-autoexit",
            "-vn",
            "-loglevel",
            "quiet",
            "-volume",
            &vol_int.to_string(),
        ]);

        if start_sec > 0.15 {
            cmd.args(["-ss", &format!("{:.2}", start_sec)]);
        }

        let af = Self::dsp_filter_for_name(filter_name, raw_filter, speed);
        if !af.is_empty() {
            cmd.args(["-af", &af]);
        }

        cmd.arg(source);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(move || {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                if libc::getppid() != parent_pid {
                    libc::_exit(1);
                }
                Ok(())
            });
        }

        if let Ok(child) = cmd.spawn() {
            self.child = Some(child);
        }
    }

    fn set_paused(&mut self, pause: bool) {
        if let Some(c) = &self.child {
            let pid = c.id() as i32;
            unsafe {
                if pause {
                    libc::kill(pid, libc::SIGSTOP);
                } else {
                    libc::kill(pid, libc::SIGCONT);
                }
            }
            self.paused = pause;
        }
    }

    fn set_volume_live(&self, volume: f64, muted: bool) {
        if let Some(c) = &self.child {
            let pid_str = c.id().to_string();
            let frac = if muted {
                0.0
            } else {
                (volume / 100.0).clamp(0.0, 1.0)
            };
            let _ = std::process::Command::new(resolve_bin("wpctl"))
                .args(["set-volume", "-p", &pid_str, &format!("{:.2}", frac)])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }

    /// Returns true if the active `ffplay` process finished naturally (EOF).
    fn check_eof(&mut self) -> bool {
        if self.paused {
            return false;
        }
        if let Some(c) = &mut self.child {
            if let Ok(Some(_status)) = c.try_wait() {
                self.child = None;
                return true;
            }
        }
        false
    }
}

impl Drop for FfplayPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn spawn_mpv() -> Result<(MpvGuard, UnixStream)> {
    let uid = unsafe { libc::getuid() };
    let pid = std::process::id();
    let socket_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let socket_path = socket_dir.join(format!("htron-mpv-{}-{}.sock", uid, pid));
    let _ = fs::remove_file(&socket_path);

    let parent_pid = unsafe { libc::getpid() };
    let mut cmd = Command::new(resolve_bin("mpv"));
    cmd.args([
        "--no-video",
        "--audio-display=no",
        "--vo=null",
        "--no-terminal",
        "--input-terminal=no",
        "--idle=yes",
        "--keep-open=no",
        "--no-osc",
        "--no-osd-bar",
        "--load-scripts=no",
        "--no-config",
        "--volume=100",
        "--volume-max=100",
        "--ao=pipewire,pulse,alsa",
        "--msg-level=all=no",
    ]);
    cmd.arg(format!("--input-ipc-server={}", socket_path.display()));
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.kill_on_drop(true);

    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent_pid {
                libc::_exit(1);
            }
            Ok(())
        });
    }

    let mut child = cmd.spawn().context("Failed to spawn headless mpv")?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("mpv exited prematurely: {}", status);
        }
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                return Ok((
                    MpvGuard {
                        child: Some(child),
                        socket_path,
                    },
                    stream,
                ));
            }
            Err(_) if start.elapsed() < Duration::from_secs(3) => {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            Err(e) => anyhow::bail!("timeout connecting to mpv socket: {}", e),
        }
    }
}

pub struct AudioEngineHandle {
    pub cmd_tx: mpsc::UnboundedSender<AudioCmd>,
    pub state: Arc<RwLock<AudioUiSnapshot>>,
}

impl AudioEngineHandle {
    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn snapshot(&self) -> AudioUiSnapshot {
        self.state.read().unwrap().clone()
    }
}

pub fn start_audio_engine(music_root: PathBuf) -> AudioEngineHandle {
    let canonical_root = music_root
        .canonicalize()
        .unwrap_or_else(|_| music_root.clone());

    let mut metadata_cache: HashMap<PathBuf, Arc<Track>> = HashMap::new();
    let initial_entries = read_directory(&canonical_root, &canonical_root, &metadata_cache);

    let initial_snap = AudioUiSnapshot {
        root_dir: canonical_root.clone(),
        current_dir: canonical_root.clone(),
        entries: initial_entries.clone(),
        playlist: Vec::new(),
        queue: Vec::new(),
        current_track: None,
        state: PlayState::Stopped,
        position_sec: 0.0,
        duration_sec: 0.0,
        volume: 100.0,
        speed: 1.0,
        muted: false,
        loop_mode: LoopMode::All,
        shuffle: false,
        active_filter_name: "Flat".to_string(),
        cached_tracks_count: 0,
    };

    let state = Arc::new(RwLock::new(initial_snap));
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<AudioCmd>();
    let (patch_tx, mut patch_rx) = mpsc::unbounded_channel::<Vec<Track>>();

    // Spawn background metadata scanner for initial folder
    let to_scan: Vec<PathBuf> = initial_entries
        .iter()
        .filter_map(|e| match e {
            BrowserEntry::AudioTrack(t) => Some(t.path.clone()),
            _ => None,
        })
        .collect();
    if !to_scan.is_empty() {
        let ptx = patch_tx.clone();
        std::thread::spawn(move || {
            let mut batch = Vec::new();
            for p in to_scan {
                if let Some(t) = probe_track_metadata(&p) {
                    batch.push(t);
                    if batch.len() >= 25 {
                        let _ = ptx.send(std::mem::take(&mut batch));
                    }
                }
            }
            if !batch.is_empty() {
                let _ = ptx.send(batch);
            }
        });
    }

    let state_clone = Arc::clone(&state);
    let cmd_tx_ipc = cmd_tx.clone();
    let cmd_tx_loop = cmd_tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to build Tokio audio runtime");

        rt.block_on(async move {
            let (broadcast_tx, _) = broadcast::channel::<TunotronStatus>(16);

            // Spawn Tunotron IPC socket server for Waybar / CLI compatibility if socket is free
            let sock_path = default_socket_path();
            let b_tx_server = broadcast_tx.clone();
            let state_server = Arc::clone(&state_clone);
            let cmd_tx_server = cmd_tx_ipc.clone();
            tokio::spawn(async move {
                // Only bind if no existing tunotron TUI is actively listening
                if UnixStream::connect(&sock_path).await.is_err() {
                    let _ = fs::remove_file(&sock_path);
                    if let Ok(listener) = UnixListener::bind(&sock_path) {
                        while let Ok((stream, _)) = listener.accept().await {
                            let s_ref = Arc::clone(&state_server);
                            let mut b_rx = b_tx_server.subscribe();
                            let c_tx = cmd_tx_server.clone();
                            tokio::spawn(async move {
                                let (reader, mut writer) = stream.into_split();
                                let mut lines = BufReader::new(reader).lines();
                                while let Ok(Some(line)) = lines.next_line().await {
                                    let l = line.trim();
                                    if l.contains("\"subscribe\"") {
                                        let cur = s_ref.read().unwrap().to_tunotron_status();
                                        if let Ok(j) = serde_json::to_string(&cur) {
                                            let _ = writer.write_all(format!("{}\n", j).as_bytes()).await;
                                        }
                                        while let Ok(upd) = b_rx.recv().await {
                                            if let Ok(j) = serde_json::to_string(&upd) {
                                                if writer.write_all(format!("{}\n", j).as_bytes()).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                        break;
                                    } else if l.contains("\"status\"") {
                                        let cur = s_ref.read().unwrap().to_tunotron_status();
                                        let resp = serde_json::json!({ "ok": true, "status": cur });
                                        let _ = writer.write_all(format!("{}\n", resp).as_bytes()).await;
                                    } else if l.contains("\"toggle\"") || l.contains("\"play_pause\"") {
                                        let _ = c_tx.send(AudioCmd::TogglePause);
                                        let _ = writer.write_all(b"{\"ok\":true}\n").await;
                                    } else if l.contains("\"next\"") {
                                        let _ = c_tx.send(AudioCmd::Next);
                                        let _ = writer.write_all(b"{\"ok\":true}\n").await;
                                    } else if l.contains("\"prev\"") {
                                        let _ = c_tx.send(AudioCmd::Prev);
                                        let _ = writer.write_all(b"{\"ok\":true}\n").await;
                                    } else if l.contains("\"stop\"") {
                                        let _ = c_tx.send(AudioCmd::Stop);
                                        let _ = writer.write_all(b"{\"ok\":true}\n").await;
                                    }
                                }
                            });
                        }
                    }
                }
            });

            let (_mpv_guard, mut mpv_writer, mut mpv_reader) = if let Ok((guard, stream)) = spawn_mpv().await {
                let (r, w) = tokio::io::split(stream);
                (
                    Some(guard),
                    Some(FramedWrite::new(w, LinesCodec::new())),
                    Some(FramedRead::new(r, LinesCodec::new())),
                )
            } else {
                (None, None, None)
            };

            if let Some(w) = mpv_writer.as_mut() {
                let obs = serde_json::json!({ "command": ["observe_property", 1, "duration"] });
                let _ = w.send(obs.to_string()).await;
            }

            let mut clock = PlaybackClock::new();
            let mut ffplay = FfplayPlayer::new();
            let mut active_raw_filter = String::new();
            let mut rng = XorShift64::from_entropy();
            let mut current_index: Option<usize> = None;
            let mut shuffle_deck: Vec<usize> = Vec::new();
            let mut shuffle_pos: usize = 0;
            let mut history: Vec<usize> = Vec::new();

            let mut drift_interval = tokio::time::interval(Duration::from_secs(5));
            let mut ui_tick = tokio::time::interval(Duration::from_millis(250));

            let sync_waybar = |snap: &AudioUiSnapshot| {
                let st = snap.to_tunotron_status();
                let _ = broadcast_tx.send(st.clone());
                if let Ok(j) = serde_json::to_string(&st) {
                    let _ = fs::write(default_status_file_path(), j);
                }
            };

            loop {
                tokio::select! {
                    _ = drift_interval.tick() => {
                        let is_playing = state_clone.read().unwrap().state == PlayState::Playing;
                        if is_playing {
                            if let Some(w) = mpv_writer.as_mut() {
                                let req = MpvRequest {
                                    command: vec!["get_property".into(), "time-pos".into()],
                                    request_id: Some(42),
                                };
                                if let Ok(s) = serde_json::to_string(&req) {
                                    let _ = w.send(s).await;
                                }
                            }
                        }
                    }
                    _ = ui_tick.tick() => {
                        let mut hit_eof = false;
                        {
                            let mut w = state_clone.write().unwrap();
                            if w.state == PlayState::Playing {
                                w.position_sec = clock.now().min(w.duration_sec.max(clock.now()));
                                if mpv_writer.is_none() && ffplay.check_eof() {
                                    hit_eof = true;
                                }
                            }
                        }
                        if hit_eof {
                            let _ = cmd_tx_loop.send(AudioCmd::Next);
                        }
                    }
                    Some(batch) = patch_rx.recv() => {
                        let mut w = state_clone.write().unwrap();
                        for t in batch {
                            let arc_t = Arc::new(t.clone());
                            if metadata_cache.len() < 10_000 {
                                metadata_cache.insert(t.path.clone(), Arc::clone(&arc_t));
                            }
                            for entry in &mut w.entries {
                                if let BrowserEntry::AudioTrack(existing) = entry {
                                    if existing.path == t.path {
                                        *existing = Arc::clone(&arc_t);
                                    }
                                }
                            }
                            for pl_t in &mut w.playlist {
                                if pl_t.path == t.path {
                                    *pl_t = Arc::clone(&arc_t);
                                }
                            }
                            if let Some(cur) = &mut w.current_track {
                                if cur.path == t.path {
                                    *cur = Arc::clone(&arc_t);
                                    if t.duration_sec > 0.0 {
                                        w.duration_sec = t.duration_sec;
                                    }
                                }
                            }
                        }
                        w.cached_tracks_count = metadata_cache.len();
                    }
                    Some(Ok(line)) = async {
                        match mpv_reader.as_mut() {
                            Some(r) => r.next().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        if let Ok(incoming) = serde_json::from_str::<MpvIncoming>(&line) {
                            match incoming {
                                MpvIncoming::Response(resp) => {
                                    if resp.request_id == Some(42) && resp.error == "success" {
                                        if let Some(pos) = resp.data.and_then(|v| v.as_f64()) {
                                            clock.sync(pos);
                                            state_clone.write().unwrap().position_sec = pos;
                                        }
                                    }
                                }
                                MpvIncoming::Event(ev) => match ev {
                                    MpvEvent::PropertyChange { name, data, .. } if name == "duration" => {
                                        if let Some(dur) = data.and_then(|v| v.as_f64()) {
                                            let mut w = state_clone.write().unwrap();
                                            w.duration_sec = dur;
                                            sync_waybar(&w);
                                        }
                                    }
                                    MpvEvent::PlaybackRestart => {
                                        let w = state_clone.write().unwrap();
                                        if w.state == PlayState::Playing {
                                            clock.set_playing(true);
                                        }
                                        sync_waybar(&w);
                                    }
                                    MpvEvent::EndFile { reason } => {
                                        if reason.as_deref() == Some("eof") {
                                            let _ = cmd_tx_loop.send(AudioCmd::Next);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Some(cmd) = cmd_rx.recv() => {
                        match cmd {
                            AudioCmd::OpenDir(target) => {
                                let root = state_clone.read().unwrap().root_dir.clone();
                                if let Some(jailed) = resolve_in_jail(&root, &target) {
                                    let entries = read_directory(&jailed, &root, &metadata_cache);
                                    let uncached: Vec<PathBuf> = entries
                                        .iter()
                                        .filter_map(|e| match e {
                                            BrowserEntry::AudioTrack(t) if !metadata_cache.contains_key(&t.path) => Some(t.path.clone()),
                                            _ => None,
                                        })
                                        .collect();
                                    {
                                        let mut w = state_clone.write().unwrap();
                                        w.current_dir = jailed;
                                        w.entries = entries;
                                    }
                                    if !uncached.is_empty() {
                                        let ptx = patch_tx.clone();
                                        std::thread::spawn(move || {
                                            let mut batch = Vec::new();
                                            for p in uncached {
                                                if let Some(t) = probe_track_metadata(&p) {
                                                    batch.push(t);
                                                    if batch.len() >= 25 {
                                                        let _ = ptx.send(std::mem::take(&mut batch));
                                                    }
                                                }
                                            }
                                            if !batch.is_empty() {
                                                let _ = ptx.send(batch);
                                            }
                                        });
                                    }
                                }
                            }
                            AudioCmd::ParentDir => {
                                let (cur, root) = {
                                    let r = state_clone.read().unwrap();
                                    (r.current_dir.clone(), r.root_dir.clone())
                                };
                                if cur != root {
                                    if let Some(p) = cur.parent() {
                                        let _ = cmd_tx_loop.send(AudioCmd::OpenDir(p.to_path_buf()));
                                    }
                                }
                            }
                            AudioCmd::PlayTrackAtIndex(entry_idx) => {
                                let mut chosen_track: Option<Arc<Track>> = None;
                                let mut folder_tracks: Vec<Arc<Track>> = Vec::new();
                                {
                                    let r = state_clone.read().unwrap();
                                    for e in &r.entries {
                                        if let BrowserEntry::AudioTrack(t) = e {
                                            folder_tracks.push(Arc::clone(t));
                                        }
                                    }
                                    if let Some(BrowserEntry::AudioTrack(t)) = r.entries.get(entry_idx) {
                                        chosen_track = Some(Arc::clone(t));
                                    }
                                }
                                if let Some(track) = chosen_track {
                                    let pl_idx = folder_tracks.iter().position(|t| t.path == track.path).unwrap_or(0);
                                    current_index = Some(pl_idx);
                                    history.push(pl_idx);

                                    // Rebuild Fisher-Yates deck if shuffle is active
                                    let n = folder_tracks.len();
                                    shuffle_deck = (0..n).collect();
                                    for i in (1..n).rev() {
                                        let j = rng.next_bound(i + 1);
                                        shuffle_deck.swap(i, j);
                                    }
                                    if let Some(pos) = shuffle_deck.iter().position(|&x| x == pl_idx) {
                                        shuffle_deck.swap(0, pos);
                                    }
                                    shuffle_pos = 0;

                                    if let Some(w) = mpv_writer.as_mut() {
                                        let req = MpvRequest {
                                            command: vec!["loadfile".into(), track.path.to_string_lossy().into(), "replace".into()],
                                            request_id: None,
                                        };
                                        let _ = w.send(serde_json::to_string(&req).unwrap()).await;
                                        let unp = MpvRequest {
                                            command: vec!["set_property".into(), "pause".into(), false.into()],
                                            request_id: None,
                                        };
                                        let _ = w.send(serde_json::to_string(&unp).unwrap()).await;
                                    } else {
                                        let (vol, muted, spd, fname) = {
                                            let r = state_clone.read().unwrap();
                                            (r.volume, r.muted, r.speed, r.active_filter_name.clone())
                                        };
                                        ffplay.play(
                                            &track.path.to_string_lossy(),
                                            0.0,
                                            vol,
                                            muted,
                                            spd,
                                            &fname,
                                            &active_raw_filter,
                                        );
                                    }
                                    clock.sync(0.0);
                                    clock.set_playing(true);

                                    let mut w = state_clone.write().unwrap();
                                    w.playlist = folder_tracks;
                                    w.duration_sec = track.duration_sec;
                                    w.position_sec = 0.0;
                                    w.current_track = Some(track);
                                    w.state = PlayState::Playing;
                                    sync_waybar(&w);
                                }
                            }
                            AudioCmd::PlayPath(path) => {
                                let root = state_clone.read().unwrap().root_dir.clone();
                                if let Some(jailed) = resolve_in_jail(&root, &path) {
                                    let track = metadata_cache
                                        .get(&jailed)
                                        .cloned()
                                        .unwrap_or_else(|| Arc::new(probe_track_metadata(&jailed).unwrap_or_else(|| Track::new(jailed.clone()))));
                                    if let Some(w) = mpv_writer.as_mut() {
                                        let req = MpvRequest {
                                            command: vec!["loadfile".into(), track.path.to_string_lossy().into(), "replace".into()],
                                            request_id: None,
                                        };
                                        let _ = w.send(serde_json::to_string(&req).unwrap()).await;
                                        let unp = MpvRequest {
                                            command: vec!["set_property".into(), "pause".into(), false.into()],
                                            request_id: None,
                                        };
                                        let _ = w.send(serde_json::to_string(&unp).unwrap()).await;
                                    } else {
                                        let (vol, muted, spd, fname) = {
                                            let r = state_clone.read().unwrap();
                                            (r.volume, r.muted, r.speed, r.active_filter_name.clone())
                                        };
                                        ffplay.play(
                                            &track.path.to_string_lossy(),
                                            0.0,
                                            vol,
                                            muted,
                                            spd,
                                            &fname,
                                            &active_raw_filter,
                                        );
                                    }
                                    clock.sync(0.0);
                                    clock.set_playing(true);
                                    let mut w = state_clone.write().unwrap();
                                    w.duration_sec = track.duration_sec;
                                    w.position_sec = 0.0;
                                    w.current_track = Some(track);
                                    w.state = PlayState::Playing;
                                    sync_waybar(&w);
                                }
                            }
                            AudioCmd::PlayStream { url, title } => {
                                if let Some(w) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["loadfile".into(), url.clone().into(), "replace".into()],
                                        request_id: None,
                                    };
                                    let _ = w.send(serde_json::to_string(&req).unwrap()).await;
                                    let unp = MpvRequest {
                                        command: vec!["set_property".into(), "pause".into(), false.into()],
                                        request_id: None,
                                    };
                                    let _ = w.send(serde_json::to_string(&unp).unwrap()).await;
                                } else {
                                    let (vol, muted, spd, fname) = {
                                        let r = state_clone.read().unwrap();
                                        (r.volume, r.muted, r.speed, r.active_filter_name.clone())
                                    };
                                    ffplay.play(&url, 0.0, vol, muted, spd, &fname, &active_raw_filter);
                                }
                                let mut stream_track = Track::new(PathBuf::from(&url));
                                stream_track.title = title;
                                stream_track.artist = "Live Stream".to_string();
                                stream_track.album = "Tunotron Radio".to_string();
                                clock.sync(0.0);
                                clock.set_playing(true);
                                let mut w = state_clone.write().unwrap();
                                w.current_track = Some(Arc::new(stream_track));
                                w.duration_sec = 0.0;
                                w.position_sec = 0.0;
                                w.state = PlayState::Playing;
                                sync_waybar(&w);
                            }
                            AudioCmd::TogglePause => {
                                let mut w = state_clone.write().unwrap();
                                match w.state {
                                    PlayState::Playing => {
                                        if let Some(mw) = mpv_writer.as_mut() {
                                            let req = MpvRequest {
                                                command: vec!["set_property".into(), "pause".into(), true.into()],
                                                request_id: None,
                                            };
                                            let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                        } else {
                                            ffplay.set_paused(true);
                                        }
                                        clock.set_playing(false);
                                        w.position_sec = clock.now();
                                        w.state = PlayState::Paused;
                                        sync_waybar(&w);
                                    }
                                    PlayState::Paused => {
                                        if let Some(mw) = mpv_writer.as_mut() {
                                            let req = MpvRequest {
                                                command: vec!["set_property".into(), "pause".into(), false.into()],
                                                request_id: None,
                                            };
                                            let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                        } else {
                                            ffplay.set_paused(false);
                                        }
                                        clock.set_playing(true);
                                        w.state = PlayState::Playing;
                                        sync_waybar(&w);
                                    }
                                    PlayState::Stopped => {
                                        // Play first track in current directory if stopped
                                        if let Some(pos) = w.entries.iter().position(|e| matches!(e, BrowserEntry::AudioTrack(_))) {
                                            drop(w);
                                            let _ = cmd_tx_loop.send(AudioCmd::PlayTrackAtIndex(pos));
                                        }
                                    }
                                }
                            }
                            AudioCmd::Stop => {
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["stop".into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else {
                                    ffplay.stop();
                                }
                                clock.sync(0.0);
                                clock.set_playing(false);
                                let mut w = state_clone.write().unwrap();
                                w.state = PlayState::Stopped;
                                w.position_sec = 0.0;
                                sync_waybar(&w);
                            }
                            AudioCmd::Next => {
                                let mut next_track: Option<Arc<Track>> = None;
                                {
                                    let mut w = state_clone.write().unwrap();
                                    if !w.queue.is_empty() {
                                        next_track = Some(w.queue.remove(0));
                                    } else if !w.playlist.is_empty() {
                                        let len = w.playlist.len();
                                        if w.loop_mode == LoopMode::Track {
                                            if let Some(idx) = current_index {
                                                next_track = w.playlist.get(idx).cloned();
                                            }
                                        } else if w.shuffle {
                                            if shuffle_deck.len() != len {
                                                shuffle_deck = (0..len).collect();
                                            }
                                            if shuffle_pos + 1 < shuffle_deck.len() {
                                                shuffle_pos += 1;
                                                let idx = shuffle_deck[shuffle_pos];
                                                current_index = Some(idx);
                                                history.push(idx);
                                                next_track = w.playlist.get(idx).cloned();
                                            } else if w.loop_mode == LoopMode::All {
                                                for i in (1..len).rev() {
                                                    let j = rng.next_bound(i + 1);
                                                    shuffle_deck.swap(i, j);
                                                }
                                                shuffle_pos = 0;
                                                let idx = shuffle_deck[0];
                                                current_index = Some(idx);
                                                history.push(idx);
                                                next_track = w.playlist.get(idx).cloned();
                                            }
                                        } else if let Some(idx) = current_index {
                                            if idx + 1 < len {
                                                current_index = Some(idx + 1);
                                                history.push(idx + 1);
                                                next_track = w.playlist.get(idx + 1).cloned();
                                            } else if w.loop_mode == LoopMode::All {
                                                current_index = Some(0);
                                                history.push(0);
                                                next_track = w.playlist.get(0).cloned();
                                            }
                                        }
                                    }
                                }
                                if let Some(track) = next_track {
                                    if let Some(mw) = mpv_writer.as_mut() {
                                        let req = MpvRequest {
                                            command: vec!["loadfile".into(), track.path.to_string_lossy().into(), "replace".into()],
                                            request_id: None,
                                        };
                                        let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                        let unp = MpvRequest {
                                            command: vec!["set_property".into(), "pause".into(), false.into()],
                                            request_id: None,
                                        };
                                        let _ = mw.send(serde_json::to_string(&unp).unwrap()).await;
                                    } else {
                                        let (vol, muted, spd, fname) = {
                                            let r = state_clone.read().unwrap();
                                            (r.volume, r.muted, r.speed, r.active_filter_name.clone())
                                        };
                                        ffplay.play(
                                            &track.path.to_string_lossy(),
                                            0.0,
                                            vol,
                                            muted,
                                            spd,
                                            &fname,
                                            &active_raw_filter,
                                        );
                                    }
                                    clock.sync(0.0);
                                    clock.set_playing(true);
                                    let mut w = state_clone.write().unwrap();
                                    w.duration_sec = track.duration_sec;
                                    w.position_sec = 0.0;
                                    w.current_track = Some(track);
                                    w.state = PlayState::Playing;
                                    sync_waybar(&w);
                                }
                            }
                            AudioCmd::Prev => {
                                let mut prev_track: Option<Arc<Track>> = None;
                                {
                                    let w = state_clone.read().unwrap();
                                    if !w.playlist.is_empty() {
                                        if history.len() > 1 {
                                            history.pop();
                                            if let Some(&idx) = history.last() {
                                                current_index = Some(idx);
                                                prev_track = w.playlist.get(idx).cloned();
                                            }
                                        } else if let Some(idx) = current_index {
                                            let n_idx = if idx > 0 { idx - 1 } else { w.playlist.len() - 1 };
                                            current_index = Some(n_idx);
                                            prev_track = w.playlist.get(n_idx).cloned();
                                        }
                                    }
                                }
                                if let Some(track) = prev_track {
                                    if let Some(mw) = mpv_writer.as_mut() {
                                        let req = MpvRequest {
                                            command: vec!["loadfile".into(), track.path.to_string_lossy().into(), "replace".into()],
                                            request_id: None,
                                        };
                                        let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                    } else {
                                        let (vol, muted, spd, fname) = {
                                            let r = state_clone.read().unwrap();
                                            (r.volume, r.muted, r.speed, r.active_filter_name.clone())
                                        };
                                        ffplay.play(
                                            &track.path.to_string_lossy(),
                                            0.0,
                                            vol,
                                            muted,
                                            spd,
                                            &fname,
                                            &active_raw_filter,
                                        );
                                    }
                                    clock.sync(0.0);
                                    clock.set_playing(true);
                                    let mut w = state_clone.write().unwrap();
                                    w.duration_sec = track.duration_sec;
                                    w.position_sec = 0.0;
                                    w.current_track = Some(track);
                                    w.state = PlayState::Playing;
                                    sync_waybar(&w);
                                }
                            }
                            AudioCmd::SeekRelative(delta) => {
                                let mut w = state_clone.write().unwrap();
                                let new_pos = (clock.now() + delta).clamp(0.0, w.duration_sec.max(0.0));
                                clock.sync(new_pos);
                                w.position_sec = new_pos;
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["seek".into(), new_pos.into(), "absolute+exact".into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else if let Some(src) = ffplay.active_source.clone() {
                                    let was_paused = w.state == PlayState::Paused;
                                    ffplay.play(
                                        &src,
                                        new_pos,
                                        w.volume,
                                        w.muted,
                                        w.speed,
                                        &w.active_filter_name,
                                        &active_raw_filter,
                                    );
                                    if was_paused {
                                        ffplay.set_paused(true);
                                    }
                                }
                                sync_waybar(&w);
                            }
                            AudioCmd::SeekAbsolute(pos) => {
                                let mut w = state_clone.write().unwrap();
                                let clamped = pos.clamp(0.0, w.duration_sec.max(pos));
                                clock.sync(clamped);
                                w.position_sec = clamped;
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["seek".into(), clamped.into(), "absolute+exact".into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else if let Some(src) = ffplay.active_source.clone() {
                                    let was_paused = w.state == PlayState::Paused;
                                    ffplay.play(
                                        &src,
                                        clamped,
                                        w.volume,
                                        w.muted,
                                        w.speed,
                                        &w.active_filter_name,
                                        &active_raw_filter,
                                    );
                                    if was_paused {
                                        ffplay.set_paused(true);
                                    }
                                }
                                sync_waybar(&w);
                            }
                            AudioCmd::SetVolume(vol) => {
                                let v = vol.clamp(0.0, 100.0);
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["set_property".into(), "volume".into(), v.into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else {
                                    let muted = state_clone.read().unwrap().muted;
                                    ffplay.set_volume_live(v, muted);
                                }
                                let mut w = state_clone.write().unwrap();
                                w.volume = v;
                                sync_waybar(&w);
                            }
                            AudioCmd::SetSpeed(spd) => {
                                let s = spd.clamp(0.5, 2.0);
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["set_property".into(), "speed".into(), s.into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else if let Some(src) = ffplay.active_source.clone() {
                                    let cur_pos = clock.now();
                                    let r = state_clone.read().unwrap();
                                    let was_paused = r.state == PlayState::Paused;
                                    ffplay.play(
                                        &src,
                                        cur_pos,
                                        r.volume,
                                        r.muted,
                                        s,
                                        &r.active_filter_name,
                                        &active_raw_filter,
                                    );
                                    if was_paused {
                                        ffplay.set_paused(true);
                                    }
                                }
                                clock.set_speed(effective_clock_speed(
                                    &state_clone.read().unwrap().active_filter_name,
                                    s,
                                ));
                                state_clone.write().unwrap().speed = s;
                            }
                            AudioCmd::ToggleMute => {
                                let mut w = state_clone.write().unwrap();
                                w.muted = !w.muted;
                                let m = w.muted;
                                let v = w.volume;
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["set_property".into(), "mute".into(), m.into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else {
                                    ffplay.set_volume_live(v, m);
                                }
                            }
                            AudioCmd::CycleLoop => {
                                let mut w = state_clone.write().unwrap();
                                w.loop_mode = w.loop_mode.next();
                                sync_waybar(&w);
                            }
                            AudioCmd::ToggleShuffle => {
                                let mut w = state_clone.write().unwrap();
                                w.shuffle = !w.shuffle;
                                if w.shuffle && !w.playlist.is_empty() {
                                    let n = w.playlist.len();
                                    shuffle_deck = (0..n).collect();
                                    for i in (1..n).rev() {
                                        let j = rng.next_bound(i + 1);
                                        shuffle_deck.swap(i, j);
                                    }
                                    shuffle_pos = 0;
                                }
                                sync_waybar(&w);
                            }
                            AudioCmd::SetFilter { name, filter } => {
                                active_raw_filter = filter.clone();
                                if let Some(mw) = mpv_writer.as_mut() {
                                    let req = MpvRequest {
                                        command: vec!["set_property".into(), "af".into(), filter.into()],
                                        request_id: None,
                                    };
                                    let _ = mw.send(serde_json::to_string(&req).unwrap()).await;
                                } else if let Some(src) = ffplay.active_source.clone() {
                                    let cur_pos = clock.now();
                                    let r = state_clone.read().unwrap();
                                    let was_paused = r.state == PlayState::Paused;
                                    ffplay.play(
                                        &src,
                                        cur_pos,
                                        r.volume,
                                        r.muted,
                                        r.speed,
                                        &name,
                                        &active_raw_filter,
                                    );
                                    if was_paused {
                                        ffplay.set_paused(true);
                                    }
                                }
                                clock.set_speed(effective_clock_speed(
                                    &name,
                                    state_clone.read().unwrap().speed,
                                ));
                                state_clone.write().unwrap().active_filter_name = name;
                            }
                            AudioCmd::AddToQueue(track) => {
                                state_clone.write().unwrap().queue.push(track);
                            }
                            AudioCmd::ClearQueue => {
                                state_clone.write().unwrap().queue.clear();
                            }
                        }
                    }
                }
            }
        });
    });

    AudioEngineHandle { cmd_tx, state }
}
