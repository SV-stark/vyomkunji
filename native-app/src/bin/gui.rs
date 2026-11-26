// Signer GUI — egui/eframe 0.36 on fastframe: tray, resident shell, native
// text rendering, JSON palettes. Run: cargo run --bin gui
#![windows_subsystem = "windows"]

// Screen rendering lives in `gui/render.rs`; everything else stays in this
// crate root so the private `App` fields remain visible to all child modules.
// A bin crate root resolves `mod` relative to src/bin/, so the path is explicit
// (and avoids auto-discovering gui/render.rs as a separate binary).
#[path = "gui/render.rs"]
mod render;
#[path = "gui/state.rs"]
mod state;

use vyomkunji::{bridge, icons, pkcs11, portals, theme, timeutil};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use eframe::egui;
use fastframe_fonts::Weight;
use fastframe_shell::{Closed, Headless, Resident};
use tray_icon::menu::{Menu, MenuEvent, MenuItem as TiMenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use pkcs11::TokenManager;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use theme::{accent, accent2, border, card, card_hi, err, ok, text, text_dim, text_faint, warn};
use timeutil::{days_until, now_clock, now_iso};
use zeroize::Zeroizing;

/// Optional startup tracing: set `VYOMKUNJI_STARTUP_LOG=1` to append timing
/// lines to `%TEMP%\vyomkunji-startup.log`. The check is cheap and the feature
/// is off by default, so it does not affect normal launch time.
fn startup_log(message: &str) {
    use std::io::Write;
    if std::env::var_os("VYOMKUNJI_STARTUP_LOG").is_none() {
        return;
    }
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    let path = std::env::temp_dir().join("vyomkunji-startup.log");
    // Keep the startup log bounded: one rotated generation, 1 MiB each.
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 1024 * 1024
    {
        let rotated = std::env::temp_dir().join("vyomkunji-startup.log.1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(&path, rotated);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            file,
            "{:>8.1} ms  {}",
            start.elapsed().as_secs_f64() * 1000.0,
            message
        );
    }
}

#[derive(Clone)]
struct CertRow {
    token_label: String,
    token_serial: String,
    cn: String,
    issuer: String,
    not_after: String,
    serial: String,
    id: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Level {
    Info,
    Ok,
    Warn,
    Err,
}

impl Level {
    fn color(self) -> egui::Color32 {
        match self {
            Level::Info => text_dim(),
            Level::Ok => ok(),
            Level::Warn => warn(),
            Level::Err => err(),
        }
    }
}

struct LogEntry {
    time: String,
    level: Level,
    text: String,
}

struct Pending {
    portal: String,
    /// Browser origin/tab URL supplied by the extension. Empty for local
    /// (non-browser) requests.
    origin: String,
    bytes: usize,
    hash_hex: String,
    received: String,
    cert_cn: Option<String>,
    already_hashed: bool,
    hash: pkcs11::HashAlgorithm,
    pkcs7: bool,
    data: Vec<u8>,
    resp: Sender<serde_json::Value>,
}

const DEMO_PIN: &str = "1234";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ManualSource {
    File,
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Requests,
    Certificates,
    Sign,
    Portals,
    Activity,
    Settings,
}

impl Nav {
    fn title(self) -> &'static str {
        match self {
            Nav::Requests => "Requests",
            Nav::Certificates => "Certificates",
            Nav::Sign => "Sign manually",
            Nav::Portals => "Portals",
            Nav::Activity => "Activity",
            Nav::Settings => "Settings",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Nav::Requests => "Approve DSC requests from your browser",
            Nav::Certificates => "Certificates found on the connected token",
            Nav::Sign => "Sign a file or arbitrary text with your token",
            Nav::Portals => "Indian government portals supported by VyomKunji",
            Nav::Activity => "Audit trail of everything that happened",
            Nav::Settings => "PKCS#11 module and preferences",
        }
    }
}

type GuiRequest = (serde_json::Value, Sender<serde_json::Value>);

/// A minimal tray icon built directly on `tray-icon`, so the right-click
/// context menu is reliably shown (fastframe-tray's menu was not appearing
/// for some users).
struct VkTray {
    icon: TrayIcon,
    events: Receiver<fastframe_tray::Event>,
}

impl VkTray {
    fn events(&self) -> Vec<fastframe_tray::Event> {
        self.events.try_iter().collect()
    }
    fn set_tooltip(&self, text: impl Into<String>) {
        let _ = self.icon.set_tooltip(Some(text.into()));
    }
}

enum ConnectResult {
    Connected(String, TokenManager, serde_json::Value, String),
    Failed(String),
}

struct ConnectOutcome {
    modules: Vec<String>,
    result: ConnectResult,
}

enum SignJob {
    Manual {
        source_name: String,
        byte_len: usize,
        pkcs7: bool,
        demo: bool,
    },
    Pending(Box<Pending>),
}

struct App {
    manager: Option<(String, TokenManager)>,
    certs: Vec<CertRow>,
    tokens: serde_json::Value,
    token_signature: String,
    presence_rx: Option<Receiver<Result<String, String>>>,
    selected: usize,
    pending: Option<Pending>,
    queue: std::collections::VecDeque<Pending>,
    sign_rx: Option<Receiver<(SignJob, Result<serde_json::Value, String>)>>,
    gui_rx: Receiver<GuiRequest>,
    connect_rx: Option<Receiver<ConnectOutcome>>,
    pin: Zeroizing<String>,
    pin_set_at: Option<Instant>,
    pin_timeout: Duration,
    remember_pin: bool,
    pin_timeout_min: u32,
    demo_key: bool,
    close_to_tray: bool,
    last_token_check: Instant,
    next_auto_try: Instant,
    selected_module: String,
    module_input: String,
    available_modules: Vec<String>,
    manual_source: ManualSource,
    manual_file: Option<std::path::PathBuf>,
    manual_file_len: u64,
    manual_hash: pkcs11::HashAlgorithm,
    manual_pkcs7: bool,
    data_input: String,
    status: String,
    status_level: Level,
    log: Vec<LogEntry>,
    nav: Nav,
    signature: String,
    signature_cn: String,
    signature_pkcs7: bool,
    signature_demo: bool,
    signature_source: String,
    bridge_port: Option<u16>,
    history: Vec<String>,
    portal_counts: std::collections::HashMap<String, usize>,
    // resident-shell state
    hide_intent: bool,
    quit_requested: bool,
    wants_show: bool,
    tray: Option<VkTray>,
    waker: fastframe_shell::Waker,
    ctx: Option<egui::Context>,
    rendering: Option<fastframe_text::TextRendering>,
}

// ---------------------------------------------------------------- helpers

fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().filter(|p| !p.is_empty()).collect();
    let out: String = parts
        .iter()
        .take(2)
        .filter_map(|p| p.chars().next())
        .collect::<Vec<char>>()
        .into_iter()
        .collect();
    if out.is_empty() {
        "DSC".to_string()
    } else {
        out.to_uppercase()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Circular initials avatar.
fn avatar(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let center = rect.center();
    ui.painter()
        .circle_filled(center, size / 2.0, theme::tint(color, 46));
    ui.painter().circle_stroke(
        center,
        size / 2.0,
        egui::Stroke::new(1.0_f32, theme::tint(color, 120)),
    );
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        text,
        Weight::SemiBold.font_id(size * 0.38),
        color,
    );
}

/// Small tinted pill with a label.
fn chip(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    let font = Weight::Medium.font_id(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), color);
    let pad = egui::vec2(10.0, 4.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, theme::radius_sm(), theme::tint(color, 38));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
}

/// A labelled key/value row.
fn kv(ui: &mut egui::Ui, k: &str, v: &str, mono: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(k).color(text_faint()).size(12.0));
        let family = if mono {
            egui::FontFamily::Monospace
        } else {
            egui::FontFamily::Proportional
        };
        ui.label(
            egui::RichText::new(v)
                .family(family)
                .color(text())
                .size(12.5),
        );
    });
}

// ---------------------------------------------------------------- startup

struct Window {
    app: fastframe_shell::Held<App>,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }
}

fn native_options() -> eframe::NativeOptions {
    let icon = std::sync::Arc::new(egui::IconData {
        rgba: icons::app_icon_rgba(32),
        width: 32,
        height: 32,
    });
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1020.0, 680.0])
            .with_min_inner_size([860.0, 560.0])
            .with_icon(icon)
            .with_title("VyomKunji — DSC Token"),
        ..Default::default()
    }
}

/// Opt-in local crash log. Set `VYOMKUNJI_CRASH_LOG=1` to append panic details
/// to `%LOCALAPPDATA%\VyomKunji\crash.log` (falling back to the temp folder).
/// Nothing is transmitted anywhere; the file is only for local diagnosis.
fn install_crash_hook() {
    if std::env::var_os("VYOMKUNJI_CRASH_LOG").is_none() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut root = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        root.push("VyomKunji");
        let _ = std::fs::create_dir_all(&root);
        let path = root.join("crash.log");
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "(unknown location)".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(non-string panic payload)".to_string());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write;
            let _ = writeln!(file, "[{timestamp}] panic at {location}: {message}");
            if std::env::var_os("RUST_BACKTRACE").is_some() {
                let _ = writeln!(file, "{}", std::backtrace::Backtrace::force_capture());
            }
        }
        previous(info);
    }));
}

fn main() -> Result<(), eframe::Error> {
    install_crash_hook();
    startup_log("main() entered");
    // Single-instance guard: if a resident VyomKunji is already listening on
    // the local bridge, ask it to show its window and exit cleanly.
    if bridge::request_gui(
        &serde_json::json!({ "type": "show" }),
        Duration::from_millis(500),
    )
    .is_some_and(|v| v.get("ok").and_then(|o| o.as_bool()) == Some(true))
    {
        return Ok(());
    }
    let waker = fastframe_shell::Waker::default();
    let app = App::new(&waker);
    fastframe_shell::Shell::new(app, &waker)
        .idle(fastframe_tray::idle)
        .run(|lease| {
            eframe::run_native(
                "VyomKunji",
                native_options(),
                Box::new(move |cc| {
                    let mut app = lease.take(&cc.egui_ctx);
                    app.attach(&cc.egui_ctx);
                    Ok(Box::new(Window { app }))
                }),
            )
        })
}
