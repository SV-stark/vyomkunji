// Signer GUI — egui/eframe 0.36 on fastframe: tray, resident shell, native
// text rendering, JSON palettes. Run: cargo run --bin gui
#![windows_subsystem = "windows"]
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

/// Build certificate rows from a `list_tokens` JSON payload.
fn cert_rows_from_tokens(tokens: &serde_json::Value) -> Vec<CertRow> {
    let mut rows = Vec::new();
    if let Some(arr) = tokens.as_array() {
        for t in arr {
            let tl = t["label"].as_str().unwrap_or("").to_string();
            let ts = t["serial"].as_str().unwrap_or("").to_string();
            for c in t["certs"].as_array().cloned().unwrap_or_default() {
                rows.push(CertRow {
                    token_label: tl.clone(),
                    token_serial: ts.clone(),
                    cn: c["cn"].as_str().unwrap_or("").to_string(),
                    issuer: c["issuer"].as_str().unwrap_or("").to_string(),
                    not_after: c["notAfter"].as_str().unwrap_or("").to_string(),
                    serial: c["serial"].as_str().unwrap_or("").to_string(),
                    id: c["id"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    rows
}

// ---------------------------------------------------------------- app

fn append_activity_file(level: Level, text: &str) {
    use std::io::Write;
    let mut root = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    root.push("VyomKunji");
    let _ = std::fs::create_dir_all(&root);
    let path = root.join("activity.log");
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 1024 * 1024
    {
        let rotated = root.join("activity.log.1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(&path, rotated);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let tag = match level {
            Level::Info => "INFO",
            Level::Ok => "OK",
            Level::Warn => "WARN",
            Level::Err => "ERR",
        };
        let _ = writeln!(file, "{}  [{tag}]  {text}", now_iso());
    }
}

fn demo_date_in(days: i64) -> String {
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86400)
        .unwrap_or(0);
    let (y, m, d) = timeutil::civil_from_days(today + days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn demo_tokens() -> serde_json::Value {
    let cert = |id: &str, cn: &str, org: &str, from: i64, to: i64, serial: &str| {
        serde_json::json!({
            "id": B64.encode(id),
            "label": cn,
            "cn": cn,
            "subject": format!("C=IN, O={org}, CN={cn}"),
            "issuer": "C=IN, O=Demo Certifying Authority, CN=Demo Sub CA for Class 3 Individual",
            "notBefore": demo_date_in(from),
            "notAfter": demo_date_in(to),
            "serial": serial,
            "der": "",
        })
    };
    serde_json::json!([{
        "id": "slot-demo",
        "label": "Demo USB key",
        "serial": "DEMO0001",
        "certs": [
            cert("demo-1", "ASHA RAO", "Personal", -488, 242, "1A2B3C4D5E6F7081"),
            cert("demo-2", "SAMPLE TRADERS PVT LTD", "Sample Traders Pvt Ltd", -760, -30, "0F1E2D3C4B5A6978"),
        ],
    }])
}

fn demo_sign(
    pin: &str,
    data: &[u8],
    opts: &pkcs11::SignOptions<'_>,
) -> Result<serde_json::Value, String> {
    if pin != DEMO_PIN {
        return Err(format!(
            "login failed: wrong PIN (the demo key's PIN is {DEMO_PIN})"
        ));
    }
    let digest = if opts.already_hashed {
        data.to_vec()
    } else {
        opts.hash.digest(data)
    };
    let mut fake = b"DEMO-SIGNATURE-NOT-VALID:".to_vec();
    fake.extend_from_slice(&digest);
    let cn = opts.cert_cn.unwrap_or("ASHA RAO").to_string();
    let mut out = serde_json::json!({
        "cert": "",
        "certId": B64.encode("demo-1"),
        "certCN": cn,
        "hash": opts.hash.name(),
        "keyType": "RSA",
    });
    if opts.pkcs7 {
        out["pkcs7"] = serde_json::json!(B64.encode(&fake));
        out["mechanism"] = serde_json::json!("RSA_PKCS_CMS");
        out["chain"] = serde_json::json!([]);
    } else {
        out["signature"] = serde_json::json!(B64.encode(&fake));
        out["mechanism"] = serde_json::json!(format!("{}_RSA_PKCS", opts.hash.name()));
    }
    Ok(out)
}

impl App {
    fn new(waker: &fastframe_shell::Waker) -> Self {
        startup_log("App::new begin");
        let (tx, rx) = std::sync::mpsc::channel::<GuiRequest>();
        let bridge = std::thread::spawn(move || bridge::serve_gui(tx).ok())
            .join()
            .ok()
            .flatten();
        let (bridge_port, _bridge_token) = match bridge {
            Some((port, token)) => (Some(port), Some(token)),
            None => (None, None),
        };

        let cfg = pkcs11::read_config();
        let get_bool = |k: &str, d: bool| cfg.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        let dark = get_bool("darkMode", true);
        theme::set_base(if dark {
            fastframe_theme::Base::Dark
        } else {
            fastframe_theme::Base::Light
        });
        let demo_flag = std::env::args().any(|a| a == "--demo");
        let demo_env = std::env::var_os("VYOMKUNJI_MOCK").is_some();
        let demo_key = demo_flag || demo_env || get_bool("demoKey", false);
        let pin_timeout_min = cfg
            .get("pinTimeoutMin")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                std::env::var("VYOMKUNJI_PIN_TIMEOUT_MIN")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(5)
            .clamp(1, 60) as u32;
        let configured = pkcs11::configured_module_path().unwrap_or_default();
        let mut app = Self {
            manager: None,
            certs: Vec::new(),
            tokens: serde_json::Value::Null,
            token_signature: String::new(),
            presence_rx: None,
            selected: 0,
            pending: None,
            queue: std::collections::VecDeque::new(),
            sign_rx: None,
            gui_rx: rx,
            connect_rx: None,
            pin: Zeroizing::new(String::new()),
            pin_set_at: None,
            pin_timeout: Duration::from_secs(pin_timeout_min as u64 * 60),
            remember_pin: get_bool("rememberPin", false),
            pin_timeout_min,
            demo_key,
            close_to_tray: get_bool("closeToTray", true),
            last_token_check: Instant::now(),
            next_auto_try: Instant::now(),
            selected_module: configured.clone(),
            module_input: configured,
            available_modules: Vec::new(),
            manual_source: ManualSource::Text,
            manual_file: None,
            manual_file_len: 0,
            manual_hash: pkcs11::HashAlgorithm::Sha256,
            manual_pkcs7: true,
            data_input: "VYOMKUNJI-TEST-PAYLOAD".to_string(),
            status: "Starting…".to_string(),
            status_level: Level::Info,
            log: Vec::new(),
            nav: Nav::Requests,
            signature: String::new(),
            signature_cn: String::new(),
            signature_pkcs7: false,
            signature_demo: false,
            signature_source: String::new(),
            bridge_port,
            history: Vec::new(),
            portal_counts: std::collections::HashMap::new(),
            hide_intent: false,
            quit_requested: false,
            wants_show: false,
            tray: None,
            waker: waker.clone(),
            ctx: None,
            rendering: None,
        };

        // The tray is what keeps the app reachable when the window is closed.
        // Built directly on tray-icon so the right-click context menu shows.
        let wake_for_menu = waker.clone();
        let wake_for_click = waker.clone();
        let (tx, rx) = std::sync::mpsc::channel::<fastframe_tray::Event>();

        // Install process-wide handlers before building the icon.
        let tx_menu = tx.clone();
        let wake_menu = wake_for_menu.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id = event.id.0.as_str();
            let ev = match id {
                "show" => Some(fastframe_tray::Event::Menu("show")),
                "requests" => Some(fastframe_tray::Event::Menu("requests")),
                "quit" => Some(fastframe_tray::Event::Menu("quit")),
                _ => None,
            };
            if let Some(ev) = ev
                && tx_menu.send(ev).is_ok()
            {
                wake_menu.wake();
            }
        }));
        let tx_click = tx.clone();
        let wake_click = wake_for_click.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && tx_click.send(fastframe_tray::Event::Show).is_ok()
            {
                wake_click.wake();
            }
        }));

        let icon_size = 32usize;
        let icon = Icon::from_rgba(
            icons::app_icon_rgba(icon_size),
            icon_size as u32,
            icon_size as u32,
        )
        .ok();
        let menu = Menu::new();
        let show_item = TiMenuItem::with_id("show", "Show VyomKunji", true, None);
        let req_item = TiMenuItem::with_id("requests", "Pending requests", true, None);
        let quit_item = TiMenuItem::with_id("quit", "Quit VyomKunji", true, None);
        let _ = menu.append(&show_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&req_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let tray = icon.and_then(|ic| {
            TrayIconBuilder::new()
                .with_icon(ic)
                .with_tooltip("VyomKunji")
                .with_menu(Box::new(menu.clone()))
                .with_menu_on_left_click(false)
                .with_menu_on_right_click(true)
                .build()
                .ok()
                .map(|icn| VkTray {
                    icon: icn,
                    events: rx,
                })
        });
        app.tray = tray;

        app.push_log(Level::Info, "VyomKunji started".to_string());
        startup_log("App::new done (window not created yet)");
        app.connect_async();
        startup_log("App::new returned");
        app
    }

    /// Called for every window: fonts, the desktop's text rendering, theme.
    fn attach(&mut self, ctx: &egui::Context) {
        startup_log("first frame (attach)");
        self.ctx = Some(ctx.clone());

        // Fonts: Inter for the interface, with the desktop's hinting and
        // antialiasing so the text matches the rest of the machine.
        let rendering = fastframe_text::detect();
        let mut fonts = fastframe_fonts::FontSetup::default().definitions();
        rendering.apply_to(&mut fonts);
        ctx.set_fonts(fonts);
        self.rendering = Some(rendering);

        // Palette: the user's own if there is one, our defaults otherwise.
        let (mut dark, mut light) = theme::palettes();
        if let Some(custom) = theme::load_user_palette() {
            match custom.base {
                fastframe_theme::Base::Dark => dark = custom,
                fastframe_theme::Base::Light => light = custom,
            }
        }
        theme::install(dark, light, theme::active_base());
        self.apply_theme(ctx);
    }

    /// (Re)apply the palette and the desktop's text rendering.
    fn apply_theme(&mut self, ctx: &egui::Context) {
        theme::apply(ctx);
        if let Some(rendering) = &self.rendering {
            ctx.all_styles_mut(|style| rendering.apply_to_visuals(&mut style.visuals));
        }
    }

    fn push_log(&mut self, level: Level, text: String) {
        append_activity_file(level, &text);
        let time = now_clock();
        self.log.insert(0, LogEntry { time, level, text });
        if self.log.len() > 500 {
            self.log.truncate(500);
        }
    }

    fn set_status(&mut self, level: Level, text: String) {
        self.status_level = level;
        self.status = text.clone();
        self.push_log(level, text);
    }

    /// Ask for a token connection without blocking the UI. The PKCS#11
    /// subsystem can take seconds to enumerate drivers and probe tokens, so it
    /// must never run on the path to the first frame.
    fn connect(&mut self) {
        pkcs11::clear_unresponsive();
        self.connect_async();
    }

    fn connect_async(&mut self) {
        if self.demo_key {
            let tokens = demo_tokens();
            self.certs = cert_rows_from_tokens(&tokens);
            self.tokens = tokens;
            self.manager = None;
            self.token_signature = "DEMO0001".to_string();
            self.selected = 0;
            self.set_status(
                Level::Warn,
                format!(
                    "Demo key active · {} certificate(s) (PIN {DEMO_PIN})",
                    self.certs.len()
                ),
            );
            return;
        }
        if self.connect_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<ConnectOutcome>();
        self.connect_rx = Some(rx);
        self.set_status(Level::Info, "Detecting token…".to_string());
        startup_log("token detection started (background)");
        let waker = self.waker.clone();
        std::thread::spawn(move || {
            let modules = pkcs11::discover_modules();
            let result = match TokenManager::auto_connected(pkcs11::PROBE_TIMEOUT) {
                Ok((module, tm, tokens)) => {
                    let signature = tm.token_signature().unwrap_or_default();
                    ConnectResult::Connected(module, tm, tokens, signature)
                }
                Err(e) => ConnectResult::Failed(e),
            };
            let _ = tx.send(ConnectOutcome { modules, result });
            // Wake the UI so it polls the result even when the window is idle.
            waker.wake();
        });
    }

    fn poll_connect(&mut self) {
        let outcome = match self.connect_rx.as_ref().map(|rx| rx.try_recv()) {
            None => return,
            Some(Ok(outcome)) => {
                self.connect_rx = None;
                outcome
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => return,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.connect_rx = None;
                return;
            }
        };

        if !outcome.modules.is_empty() {
            self.available_modules = outcome.modules;
        }
        startup_log("token detection finished");

        match outcome.result {
            ConnectResult::Connected(module, tm, tokens, signature) => {
                let rows = cert_rows_from_tokens(&tokens);
                let n = rows.len();
                self.certs = rows;
                self.tokens = tokens.clone();
                self.selected = 0;
                self.token_signature = signature;
                pkcs11::save_last_module(&module);
                // Keep the manager even when no token is inserted: the presence
                // probe is cheap and will pick the token up on reinsertion.
                self.manager = Some((module, tm));
                if n == 0 {
                    self.set_status(Level::Warn, "No token inserted".to_string());
                } else {
                    self.set_status(Level::Ok, format!("Token connected · {} certificate(s)", n));
                }
            }
            ConnectResult::Failed(e) => {
                self.manager = None;
                self.certs.clear();
                self.tokens = serde_json::Value::Null;
                self.token_signature.clear();
                self.selected = 0;
                self.set_status(Level::Err, format!("Token unavailable: {}", e));
            }
        }
    }

    /// Hot-plug aware refresh. Only slot presence is checked on the timer;
    /// reading certificates takes seconds on some tokens, so a changed
    /// presence/serial triggers a background re-connect instead.
    fn refresh_token(&mut self) {
        if self.demo_key {
            return;
        }
        // Throttle.
        if self.last_token_check.elapsed() < Duration::from_secs(3) {
            return;
        }
        self.last_token_check = Instant::now();

        if let Some((_module, tm)) = &self.manager {
            // Probe presence on a worker: even the cheap slot query blocks for
            // ~150 ms on some drivers, which is visible as a stutter if done
            // on the UI thread every few seconds.
            if self.presence_rx.is_some() {
                return;
            }
            let tm = tm.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.presence_rx = Some(rx);
            let waker = self.waker.clone();
            std::thread::spawn(move || {
                let signature = tm.token_signature();
                let _ = tx.send(signature);
                waker.wake();
            });
        } else if self.connect_rx.is_none() && Instant::now() >= self.next_auto_try {
            // Re-discover a module in the background, at most every 10 s.
            self.next_auto_try = Instant::now() + Duration::from_secs(10);
            self.connect_async();
        }
    }

    /// Apply the result of a background token-presence probe.
    fn poll_presence(&mut self) {
        let result = match self.presence_rx.as_ref().map(|rx| rx.try_recv()) {
            None => return,
            Some(Ok(result)) => {
                self.presence_rx = None;
                result
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => return,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.presence_rx = None;
                return;
            }
        };

        match result {
            Ok(signature) => {
                if signature == self.token_signature {
                    // No change since the last probe.
                } else if signature.is_empty() {
                    // Token removed. Keep the manager: the presence probe is
                    // cheap, so reinsertion is picked up without re-initialising
                    // every driver.
                    self.certs.clear();
                    self.tokens = serde_json::Value::Null;
                    self.token_signature.clear();
                    self.selected = 0;
                    self.set_status(Level::Warn, "Token removed".to_string());
                } else {
                    // A different token is in the reader: reload certificates
                    // in the background rather than blocking the UI.
                    self.certs.clear();
                    self.selected = 0;
                    self.token_signature = signature;
                    if self.connect_rx.is_none() {
                        self.connect_async();
                    }
                }
            }
            Err(_) => {
                self.manager = None;
                self.certs.clear();
                self.token_signature.clear();
                self.selected = 0;
                self.set_status(Level::Warn, "Token disconnected".to_string());
            }
        }
    }

    fn current_cert(&self) -> Option<&CertRow> {
        self.certs.get(self.selected)
    }

    fn sign_manual(&mut self) {
        if self.sign_rx.is_some() {
            return;
        }
        if !self.demo_key && self.manager.is_none() {
            self.set_status(Level::Err, "No token connected".to_string());
            return;
        }
        if self.pin.is_empty() {
            self.set_status(Level::Warn, "PIN required".to_string());
            return;
        }
        let (data, source_name) = match self.manual_source {
            ManualSource::File => {
                let Some(path) = &self.manual_file else {
                    self.set_status(Level::Warn, "Choose a file first".to_string());
                    return;
                };
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".to_string());
                        (bytes, name)
                    }
                    Err(e) => {
                        self.set_status(Level::Err, format!("Could not read file: {e}"));
                        return;
                    }
                }
            }
            ManualSource::Text => (self.data_input.as_bytes().to_vec(), "text".to_string()),
        };
        let byte_len = data.len();
        let cn = self.current_cert().map(|c| c.cn.clone());
        let pin = Zeroizing::new((*self.pin).clone());
        let hash = self.manual_hash;
        let pkcs7 = self.manual_pkcs7;
        let demo = self.demo_key;
        let tm = self.manager.as_ref().map(|(_, tm)| tm.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        self.sign_rx = Some(rx);
        self.set_status(Level::Info, "Signing on token…".to_string());
        let waker = self.waker.clone();
        std::thread::spawn(move || {
            let opts = pkcs11::SignOptions {
                cert_cn: cn.as_deref(),
                already_hashed: false,
                hash,
                pkcs7,
            };
            let result = if demo {
                demo_sign(&pin, &data, &opts)
            } else if let Some(tm) = tm {
                tm.sign_with(&pin, &data, &opts)
            } else {
                Err("No token connected".to_string())
            };
            let _ = tx.send((
                SignJob::Manual {
                    source_name,
                    byte_len,
                    pkcs7,
                    demo,
                },
                result,
            ));
            waker.wake();
        });
    }

    fn accept_request(&mut self, req: serde_json::Value, resp: Sender<serde_json::Value>) {
        let rtype = req.get("type").and_then(|v| v.as_str()).unwrap_or("sign");

        if rtype == "status" {
            let _ = resp.send(serde_json::json!({
                "ok": true,
                "module": self.manager.as_ref().map(|(m, _)| m.clone()),
                "token_connected": (self.manager.is_some() || self.demo_key) && !self.certs.is_empty(),
                "cert_selected": !self.certs.is_empty(),
            }));
            return;
        }

        if rtype == "listTokens" {
            let tokens_val = if self.tokens.is_null() {
                serde_json::json!([])
            } else {
                self.tokens.clone()
            };
            let _ = resp.send(serde_json::json!({
                "ok": true,
                "module": self.manager.as_ref().map(|(m, _)| m.clone()),
                "token_connected": (self.manager.is_some() || self.demo_key) && !self.certs.is_empty(),
                "tokens": tokens_val,
            }));
            return;
        }

        if rtype == "selectCert" {
            let cn_target = req
                .get("certCN")
                .and_then(|v| v.as_str())
                .or_else(|| req.get("certId").and_then(|v| v.as_str()))
                .or_else(|| {
                    req.get("payload")
                        .and_then(|p| p.get("certCN"))
                        .and_then(|v| v.as_str())
                });
            if let Some(target) = cn_target
                && let Some(idx) = self.certs.iter().position(|c| {
                    c.cn.eq_ignore_ascii_case(target) || c.id == target || c.serial == target
                })
            {
                self.selected = idx;
            }
            let _ = resp.send(serde_json::json!({ "ok": true }));
            return;
        }

        if rtype == "ping" {
            let _ = resp.send(serde_json::json!({
                "ok": true,
                "version": env!("CARGO_PKG_VERSION")
            }));
            return;
        }

        if rtype == "show" {
            self.wants_show = true;
            self.waker.wake();
            if let Some(ctx) = &self.ctx {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            }
            let _ = resp.send(serde_json::json!({ "ok": true }));
            return;
        }

        let portal = req
            .get("portal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown portal")
            .to_string();
        // The background script attaches the real tab URL. Treat anything else
        // as a local request and show it as such in the approval card.
        let origin = req
            .get("origin")
            .and_then(|v| v.as_str())
            .or_else(|| {
                req.get("payload")
                    .and_then(|p| p.get("origin"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("")
            .chars()
            .take(512)
            .collect::<String>();
        let payload = req.get("payload");
        let data_b64 = payload
            .and_then(|p| p.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let bytes = B64.decode(data_b64).unwrap_or_default();
        let already_hashed = payload
            .and_then(|p| p.get("alreadyHashed"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let hash = pkcs11::HashAlgorithm::parse(
            payload
                .and_then(|p| p.get("hashAlg"))
                .and_then(|v| v.as_str())
                .unwrap_or("SHA256"),
        );
        let pkcs7 = payload
            .and_then(|p| p.get("pkcs7"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let hash_hex = if already_hashed && bytes.len() == hash.digest_len() {
            hex(&bytes)
        } else {
            hex(&hash.digest(&bytes))
        };
        let cert_cn = payload
            .and_then(|p| p.get("certCN"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        self.push_log(
            Level::Warn,
            if origin.is_empty() {
                format!("Request received from {}", portal)
            } else {
                format!("Request received from {} ({})", portal, origin)
            },
        );
        self.history
            .insert(0, format!("{}  ·  {}", now_clock(), portal));
        if self.history.len() > 50 {
            self.history.truncate(50);
        }

        let item = Pending {
            portal: portal.clone(),
            origin,
            bytes: bytes.len(),
            hash_hex,
            received: now_clock(),
            cert_cn,
            already_hashed,
            hash,
            pkcs7,
            data: bytes,
            resp,
        };

        if self.pending.is_some() {
            self.queue.push_back(item);
            if let Some(tray) = self.tray.as_mut() {
                tray.set_tooltip(format!(
                    "VyomKunji\nWaiting for approval (+{} queued)",
                    self.queue.len()
                ));
            }
            self.waker.wake();
            return;
        }

        self.pending = Some(item);
        self.nav = Nav::Requests;
        self.wants_show = true;
        if self.manager.is_none() {
            self.connect_async();
        }
        if let Some(tray) = self.tray.as_mut() {
            tray.set_tooltip(format!("VyomKunji\nWaiting for approval: {}", portal));
        }
        self.waker.wake();
    }

    fn promote_next_pending(&mut self) {
        if self.pending.is_none()
            && let Some(next) = self.queue.pop_front()
        {
            let portal = next.portal.clone();
            self.pending = Some(next);
            if let Some(tray) = self.tray.as_mut() {
                tray.set_tooltip(format!("VyomKunji\nWaiting for approval: {portal}"));
            }
        } else if self.pending.is_none()
            && let Some(tray) = self.tray.as_mut()
        {
            tray.set_tooltip("VyomKunji");
        }
    }

    fn approve_pending(&mut self) {
        if self.sign_rx.is_some() {
            return;
        }
        let pending = match self.pending.take() {
            Some(p) => p,
            None => return,
        };
        if self.pin.is_empty() {
            self.set_status(Level::Warn, "Enter the token PIN to approve".to_string());
            self.pending = Some(pending);
            return;
        }
        if !self.demo_key && self.manager.is_none() {
            let _ = pending.resp.send(serde_json::json!({
                "ok": false,
                "error": {"code": "NO_TOKEN", "message": "No token connected"}
            }));
            self.promote_next_pending();
            return;
        }
        let data = pending.data.clone();
        let cert_cn = pending.cert_cn.clone();
        let already_hashed = pending.already_hashed;
        let hash = pending.hash;
        let pkcs7 = pending.pkcs7;
        let pin = Zeroizing::new((*self.pin).clone());
        let demo = self.demo_key;
        let tm = self.manager.as_ref().map(|(_, tm)| tm.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        self.sign_rx = Some(rx);
        self.set_status(Level::Info, format!("Signing for {}…", pending.portal));
        let waker = self.waker.clone();
        std::thread::spawn(move || {
            let opts = pkcs11::SignOptions {
                cert_cn: cert_cn.as_deref(),
                already_hashed,
                hash,
                pkcs7,
            };
            let result = if demo {
                demo_sign(&pin, &data, &opts)
            } else if let Some(tm) = tm {
                tm.sign_with(&pin, &data, &opts)
            } else {
                Err("No token connected".to_string())
            };
            let _ = tx.send((SignJob::Pending(Box::new(pending)), result));
            waker.wake();
        });
    }

    fn poll_sign(&mut self) {
        let (job, result) = match self.sign_rx.as_ref().map(|rx| rx.try_recv()) {
            None => return,
            Some(Ok(v)) => {
                self.sign_rx = None;
                v
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => return,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.sign_rx = None;
                return;
            }
        };

        match job {
            SignJob::Manual {
                source_name,
                byte_len,
                pkcs7,
                demo,
            } => match result {
                Ok(r) => {
                    let key = if pkcs7 { "pkcs7" } else { "signature" };
                    self.signature = r[key].as_str().unwrap_or("").to_string();
                    self.signature_cn = r["certCN"].as_str().unwrap_or("").to_string();
                    self.signature_pkcs7 = pkcs7;
                    self.signature_demo = demo;
                    self.signature_source = source_name;
                    self.set_status(
                        Level::Ok,
                        format!("Signed {} bytes with {}", byte_len, self.signature_cn),
                    );
                    if self.remember_pin {
                        self.pin_set_at = Some(Instant::now());
                    } else {
                        self.pin = Zeroizing::new(String::new());
                        self.pin_set_at = None;
                    }
                }
                Err(e) => {
                    self.signature.clear();
                    self.set_status(Level::Err, format!("Sign failed: {}", e));
                }
            },
            SignJob::Pending(pending) => {
                match result {
                    Ok(r) => {
                        let sig = r["signature"].as_str().unwrap_or("").to_string();
                        let cert = r["cert"].as_str().unwrap_or("").to_string();
                        let cn = r["certCN"].as_str().unwrap_or("").to_string();
                        self.signature = sig.clone();
                        self.signature_cn = cn.clone();
                        self.signature_pkcs7 = pending.pkcs7;
                        self.signature_demo = self.demo_key;
                        self.signature_source = pending.portal.clone();
                        if let Some(p) = portals::find(&pending.portal, &pending.origin) {
                            *self.portal_counts.entry(p.name.to_string()).or_insert(0) += 1;
                        }
                        let mut response = serde_json::json!({
                            "ok": true,
                            "signature": sig,
                            "cert": cert,
                            "certId": r["certId"],
                            "certCN": cn,
                            "signedAt": now_iso(),
                        });
                        for key in ["pkcs7", "chain", "mechanism", "hash", "keyType"] {
                            if let Some(value) = r.get(key) {
                                response[key] = value.clone();
                            }
                        }
                        let _ = pending.resp.send(response);
                        self.set_status(
                            Level::Ok,
                            format!("Approved & signed for {}", pending.portal),
                        );
                        if self.remember_pin {
                            self.pin_set_at = Some(Instant::now());
                        } else {
                            self.pin = Zeroizing::new(String::new());
                            self.pin_set_at = None;
                        }
                    }
                    Err(e) => {
                        let _ = pending.resp.send(serde_json::json!({
                            "ok": false,
                            "error": {"code": "SIGN_FAILED", "message": e}
                        }));
                        self.set_status(Level::Err, format!("Sign failed: {}", e));
                    }
                }
                self.promote_next_pending();
            }
        }
    }

    fn cancel(&mut self) {
        if let Some(p) = self.pending.take() {
            let _ = p.resp.send(serde_json::json!({
                "ok": false,
                "error": {"code": "USER_CANCELLED", "message": "Cancelled by user"}
            }));
            self.set_status(Level::Warn, format!("Request from {} cancelled", p.portal));
        }
        self.promote_next_pending();
    }

    /// Take requests that arrived since the last frame.
    fn pump_requests(&mut self) {
        while let Ok((req, resp)) = self.gui_rx.try_recv() {
            self.accept_request(req, resp);
        }
        self.poll_sign();
        self.poll_connect();
        self.poll_presence();
        self.refresh_token();
    }

    /// Handle tray clicks.
    fn drain_tray(&mut self) {
        let Some(tray) = &self.tray else { return };
        for event in tray.events() {
            match event {
                fastframe_tray::Event::Menu("quit") => {
                    self.quit_requested = true;
                    self.wants_show = false;
                }
                fastframe_tray::Event::Menu("show")
                | fastframe_tray::Event::Toggle
                | fastframe_tray::Event::Show => {
                    self.wants_show = true;
                    if let Some(ctx) = &self.ctx {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Informational,
                        ));
                    }
                }
                fastframe_tray::Event::Menu("requests") => {
                    self.nav = Nav::Requests;
                    self.wants_show = true;
                }
                _ => {}
            }
        }
    }
}

// ------------------------------------------------- resident shell (fastframe)

impl Resident for App {
    fn closed(&self) -> Closed {
        if !self.quit_requested && self.hide_intent {
            Closed::Hide
        } else {
            Closed::Quit
        }
    }

    fn window_gone(&mut self) {
        self.hide_intent = false;
        self.wants_show = false;
        self.ctx = None;
    }

    fn headless_frame(&mut self, ctx: &egui::Context) -> Headless {
        self.ctx = Some(ctx.clone());
        self.pump_requests();
        self.drain_tray();
        if self.quit_requested {
            return Headless::Quit;
        }
        if self.wants_show {
            self.wants_show = false;
            return Headless::Show;
        }
        Headless::Wait
    }

    fn start_hidden(&mut self) -> bool {
        false
    }

    fn shutdown(&mut self) {
        self.push_log(Level::Info, "VyomKunji stopped".to_string());
        if let Some(port) = self.bridge_port {
            bridge::clear_endpoint_if_port(port);
        }
    }
}

// ---------------------------------------------------------------- render

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Closing the window keeps the app alive in the tray when close_to_tray is enabled.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit_requested
            && self.close_to_tray
        {
            self.hide_intent = true;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.pump_requests();
        self.drain_tray();

        // Idle auto-lock: clear the in-memory PIN after the configured timeout.
        if !self.pin.is_empty() {
            let expired = self
                .pin_set_at
                .map(|t| t.elapsed() >= self.pin_timeout)
                .unwrap_or(false);
            if expired {
                self.pin = Zeroizing::new(String::new());
                self.pin_set_at = None;
                self.set_status(Level::Info, "PIN cleared after idle timeout".to_string());
            }
        }

        // TEST MODE: auto-connect and auto-approve when VYOMKUNJI_GUI_TEST_PIN (or SIGNER_GUI_TEST_PIN) is set.
        if let Ok(pin) = std::env::var("VYOMKUNJI_GUI_TEST_PIN")
            .or_else(|_| std::env::var("SIGNER_GUI_TEST_PIN"))
        {
            if self.manager.is_none() {
                self.connect();
            }
            self.pin = Zeroizing::new(pin);
            if self.pending.is_some() {
                self.approve_pending();
            }
        }

        egui::Panel::top("top")
            .exact_size(60.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(1.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::left("rail")
            .exact_size(238.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(0.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(12, 16)),
            )
            .show(ui, |ui| self.left_rail(ui));

        egui::Panel::bottom("status")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(egui::Stroke::new(1.0_f32, border()))
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ui, |ui| match self.nav {
                Nav::Requests => self.view_requests(ui),
                Nav::Certificates => self.view_certificates(ui),
                Nav::Sign => self.view_sign(ui),
                Nav::Portals => self.view_portals(ui),
                Nav::Activity => self.view_activity(ui),
                Nav::Settings => self.view_settings(ui),
            });

        let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc_pressed && self.pending.is_some() {
            self.cancel();
        }
    }
}

impl App {
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 10.0, accent());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "V",
                Weight::Bold.font_id(19.0),
                egui::Color32::WHITE,
            );

            ui.vertical(|ui| {
                ui.add_space(1.0);
                ui.label(
                    egui::RichText::new("VyomKunji")
                        .font(Weight::SemiBold.font_id(17.0))
                        .color(text()),
                );
                ui.label(
                    egui::RichText::new("India G2C · DSC signing")
                        .font(Weight::Regular.font_id(11.0))
                        .color(text_faint()),
                );
            });

            ui.add_space(6.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    self.quit_requested = true;
                    if let Some(ctx) = &self.ctx {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                if ui.button("Refresh token").clicked() {
                    self.connect();
                }
                let is_dark = theme::active_base() == fastframe_theme::Base::Dark;
                if ui
                    .button(if is_dark { "Light mode" } else { "Dark mode" })
                    .clicked()
                {
                    let next_dark = !is_dark;
                    theme::set_base(if next_dark {
                        fastframe_theme::Base::Dark
                    } else {
                        fastframe_theme::Base::Light
                    });
                    let _ = pkcs11::save_config_key("darkMode", serde_json::json!(next_dark));
                    let ctx = ui.ctx().clone();
                    self.apply_theme(&ctx);
                }
                let (dot_col, label) = match (self.demo_key, &self.manager, self.pending.is_some())
                {
                    (_, _, true) => (warn(), "Awaiting approval"),
                    (true, _, false) => (warn(), "Demo key"),
                    (false, Some(_), false) => (ok(), "Ready"),
                    (false, None, false) => (err(), "No token"),
                };
                let font = Weight::Medium.font_id(11.5);
                let galley = ui
                    .painter()
                    .layout_no_wrap(label.to_string(), font.clone(), dot_col);
                let size = galley.size() + egui::vec2(34.0, 10.0);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(dot_col, 34));
                ui.painter().circle_filled(
                    rect.left_center() + egui::vec2(12.0, 0.0),
                    4.0,
                    dot_col,
                );
                ui.painter().text(
                    rect.left_center() + egui::vec2(22.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    dot_col,
                );
            });
        });
    }

    fn left_rail(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("WORKSPACE")
                .font(Weight::SemiBold.font_id(10.0))
                .color(text_faint()),
        );
        ui.add_space(6.0);

        let items = [
            (Nav::Requests, self.pending.is_some()),
            (Nav::Certificates, false),
            (Nav::Sign, false),
            (Nav::Portals, false),
            (Nav::Activity, false),
            (Nav::Settings, false),
        ];
        for (nav, badge) in items {
            let selected = self.nav == nav;
            let (rect, resp) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
            if selected {
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(accent(), 40));
                let bar = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(0.0, 9.0),
                    egui::vec2(3.0, 20.0),
                );
                ui.painter().rect_filled(bar, 2.0, accent());
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, theme::radius_sm(), theme::tint(text_dim(), 22));
            }
            let col = if selected { text() } else { text_dim() };
            ui.painter().text(
                rect.left_center() + egui::vec2(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                nav.title(),
                Weight::Medium.font_id(13.0),
                col,
            );
            if badge {
                ui.painter().circle_filled(
                    rect.right_center() + egui::vec2(-14.0, 0.0),
                    4.0,
                    warn(),
                );
            }
            if resp.clicked() {
                self.nav = nav;
            }
        }

        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("TOKEN")
                .font(Weight::SemiBold.font_id(10.0))
                .color(text_faint()),
        );
        ui.add_space(6.0);

        let (label, serial, connected) = match self.certs.first() {
            Some(c) => (
                c.token_label.clone(),
                c.token_serial.clone(),
                self.manager.is_some(),
            ),
            None => ("—".to_string(), "—".to_string(), false),
        };
        theme::card_frame(Some(theme::tint(text_dim(), 14))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let col = if connected { ok() } else { err() };
                ui.painter().circle_filled(
                    ui.min_rect().left_center() + egui::vec2(6.0, 0.0),
                    4.0,
                    col,
                );
                ui.label(egui::RichText::new(&label).size(12.5).color(text()));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(&serial)
                    .family(egui::FontFamily::Monospace)
                    .size(10.5)
                    .color(text_faint()),
            );
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if let Some(p) = self.bridge_port {
                ui.label(
                    egui::RichText::new(format!("bridge 127.0.0.1:{}", p))
                        .family(egui::FontFamily::Monospace)
                        .size(10.0)
                        .color(text_faint()),
                );
            }
            ui.label(
                egui::RichText::new("VyomKunji 0.3.0")
                    .font(Weight::Regular.font_id(10.0))
                    .color(text_faint()),
            );
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let col = self.status_level.color();
            ui.painter().circle_filled(
                ui.min_rect().left_center() + egui::vec2(5.0, 0.0),
                4.0,
                col,
            );
            ui.label(
                egui::RichText::new(&self.status)
                    .size(12.0)
                    .color(text_dim()),
            );
        });
    }

    fn view_header(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(self.nav.title())
                .font(Weight::Bold.font_id(23.0))
                .color(text()),
        );
        ui.label(
            egui::RichText::new(self.nav.hint())
                .size(12.0)
                .color(text_faint()),
        );
        ui.add_space(14.0);
    }

    fn view_requests(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        let info = self.pending.as_ref().map(|p| {
            (
                p.portal.clone(),
                p.origin.clone(),
                p.bytes,
                p.hash_hex.clone(),
                p.received.clone(),
                p.cert_cn.clone(),
                p.hash.name().to_string(),
                p.pkcs7,
            )
        });

        match info {
            Some((portal, origin, bytes, hash_hex, received, cert_cn, hash_name, pkcs7)) => {
                theme::card_frame(None).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Signing request")
                                .font(Weight::SemiBold.font_id(17.0))
                                .color(text()),
                        );
                        if let Some(p) = portals::find(&portal, &origin) {
                            chip(ui, p.name, accent());
                        } else {
                            chip(ui, &portal, accent());
                        }
                        if !self.queue.is_empty() {
                            chip(ui, &format!("+{} queued", self.queue.len()), warn());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("received {}", received))
                                    .size(11.0)
                                    .color(text_faint()),
                            );
                        });
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    egui::Grid::new("reqmeta")
                        .num_columns(2)
                        .spacing([28.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Payload")
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(format!("{} bytes", bytes))
                                    .size(12.5)
                                    .color(text()),
                            );
                            ui.end_row();
                            ui.label(egui::RichText::new("Origin").color(text_faint()).size(12.0));
                            if origin.is_empty() {
                                ui.label(
                                    egui::RichText::new("Local request")
                                        .size(12.5)
                                        .color(warn()),
                                );
                            } else {
                                let trusted = portals::origin_trusted(&origin);
                                ui.label(
                                    egui::RichText::new(if trusted {
                                        origin.clone()
                                    } else {
                                        format!("{} (unverified)", origin)
                                    })
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.0)
                                    .color(if trusted {
                                        text_dim()
                                    } else {
                                        err()
                                    }),
                                );
                            }
                            ui.end_row();
                            ui.label(
                                egui::RichText::new(&hash_name)
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(&hash_hex)
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.0)
                                    .color(text_dim()),
                            );
                            ui.end_row();
                            ui.label(egui::RichText::new("Output").color(text_faint()).size(12.0));
                            ui.label(
                                egui::RichText::new(if pkcs7 {
                                    "Detached PKCS#7 (CMS) with chain"
                                } else {
                                    "Raw token signature"
                                })
                                .size(12.5)
                                .color(text()),
                            );
                            ui.end_row();
                            ui.label(
                                egui::RichText::new("Certificate")
                                    .color(text_faint())
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(
                                    cert_cn.clone().unwrap_or_else(|| "auto-select".to_string()),
                                )
                                .size(12.5)
                                .color(text()),
                            );
                            ui.end_row();
                        });

                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Token PIN")
                            .color(text_faint())
                            .size(12.0),
                    );
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut *self.pin)
                            .password(true)
                            .hint_text("Enter PIN to approve")
                            .desired_width(f32::INFINITY),
                    );
                    if r.changed() {
                        self.pin_set_at = if self.pin.is_empty() {
                            None
                        } else {
                            Some(Instant::now())
                        };
                    }

                    ui.add_space(14.0);
                    let signing = self.sign_rx.is_some();
                    ui.horizontal(|ui| {
                        let approve = ui.add_enabled(
                            !signing,
                            egui::Button::new(
                                egui::RichText::new(if signing {
                                    "Signing…"
                                } else {
                                    "Approve & Sign"
                                })
                                .font(Weight::SemiBold.font_id(13.5))
                                .color(egui::Color32::WHITE),
                            )
                            .fill(accent())
                            .min_size(egui::vec2(190.0, 40.0)),
                        );
                        if approve.clicked() {
                            self.approve_pending();
                        }
                        let cancel = ui.add_enabled(
                            !signing,
                            egui::Button::new("Cancel").min_size(egui::vec2(120.0, 40.0)),
                        );
                        if cancel.clicked() {
                            self.cancel();
                        }
                        if signing {
                            ui.add(egui::Spinner::new());
                        } else {
                            ui.label(
                                egui::RichText::new("Esc to cancel")
                                    .size(11.0)
                                    .color(text_faint()),
                            );
                        }
                    });
                });
            }
            None => {
                theme::card_frame(None).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 26.0, theme::tint(ok(), 34));
                        ui.painter().circle_stroke(
                            rect.center(),
                            26.0,
                            egui::Stroke::new(1.0_f32, theme::tint(ok(), 110)),
                        );
                        let c = rect.center();
                        let check = vec![
                            c + egui::vec2(-7.0, 0.0),
                            c + egui::vec2(-2.5, 5.0),
                            c + egui::vec2(7.0, -6.0),
                        ];
                        ui.painter()
                            .add(egui::Shape::line(check, egui::Stroke::new(2.5_f32, ok())));
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("All clear")
                                .font(Weight::SemiBold.font_id(18.0))
                                .color(text()),
                        );
                        ui.label(
                            egui::RichText::new("No pending signing requests")
                                .size(12.5)
                                .color(text_dim()),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "When a portal asks for a DSC signature it will appear here for approval.",
                            )
                            .size(12.0)
                            .color(text_faint()),
                        );
                        ui.add_space(14.0);
                    });
                });

                if !self.history.is_empty() {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Recent requests")
                            .font(Weight::SemiBold.font_id(13.0))
                            .color(text()),
                    );
                    ui.add_space(6.0);
                    theme::card_frame(None).show(ui, |ui| {
                        for h in self.history.iter().take(8) {
                            ui.label(
                                egui::RichText::new(h)
                                    .family(egui::FontFamily::Monospace)
                                    .size(11.5)
                                    .color(text_dim()),
                            );
                        }
                    });
                }
            }
        }
    }

    fn view_certificates(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        if self.certs.is_empty() {
            theme::card_frame(None).show(ui, |ui| {
                ui.label("No certificates found. Connect a DSC token and press Refresh.");
            });
            return;
        }

        let mut columns = 2;
        if ui.available_width() > 900.0 {
            columns = 3;
        }
        let card_w = (ui.available_width() - 16.0 * (columns as f32 - 1.0)) / columns as f32;

        egui::Grid::new("certgrid")
            .num_columns(columns)
            .spacing(egui::vec2(16.0, 16.0))
            .show(ui, |ui| {
                for (i, c) in self.certs.iter().enumerate() {
                    let selected = self.selected == i;
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(card_w, 128.0), egui::Sense::click());

                    let fill = if selected {
                        theme::tint(accent(), 30)
                    } else if resp.hovered() {
                        card_hi()
                    } else {
                        card()
                    };
                    ui.painter().rect_filled(rect, theme::radius(), fill);
                    ui.painter().rect_stroke(
                        rect,
                        theme::radius(),
                        egui::Stroke::new(1.0_f32, if selected { accent() } else { border() }),
                        egui::StrokeKind::Middle,
                    );

                    let pad = 14.0;
                    let top = rect.left_top() + egui::vec2(pad, pad);
                    let av = egui::Rect::from_min_size(top, egui::vec2(38.0, 38.0));
                    let name_col = if selected { accent() } else { accent2() };
                    ui.painter()
                        .circle_filled(av.center(), 19.0, theme::tint(name_col, 46));
                    ui.painter().text(
                        av.center(),
                        egui::Align2::CENTER_CENTER,
                        initials(&c.cn),
                        Weight::SemiBold.font_id(14.0),
                        name_col,
                    );

                    ui.painter().text(
                        av.right_top() + egui::vec2(12.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        if c.cn.is_empty() { "(unnamed)" } else { &c.cn },
                        Weight::SemiBold.font_id(13.5),
                        text(),
                    );
                    ui.painter().text(
                        av.right_top() + egui::vec2(12.0, 20.0),
                        egui::Align2::LEFT_TOP,
                        c.token_label.clone(),
                        Weight::Regular.font_id(10.5),
                        text_faint(),
                    );

                    let (exp_txt, exp_col) = match days_until(&c.not_after) {
                        Some(d) if d < 0 => (format!("expired {}d ago", -d), err()),
                        Some(d) if d < 30 => (format!("expires in {}d", d), warn()),
                        Some(d) => (format!("{}d left", d), ok()),
                        None => ("expiry unknown".to_string(), text_dim()),
                    };
                    let chip_font = Weight::Medium.font_id(10.5);
                    let galley =
                        ui.painter()
                            .layout_no_wrap(exp_txt.clone(), chip_font.clone(), exp_col);
                    let chip_size = galley.size() + egui::vec2(18.0, 8.0);
                    let chip_rect = egui::Rect::from_min_size(
                        rect.left_bottom() + egui::vec2(pad, -pad - chip_size.y),
                        chip_size,
                    );
                    ui.painter().rect_filled(
                        chip_rect,
                        theme::radius_sm(),
                        theme::tint(exp_col, 36),
                    );
                    ui.painter().text(
                        chip_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        exp_txt,
                        chip_font,
                        exp_col,
                    );

                    if selected {
                        ui.painter().circle_filled(
                            rect.right_top() + egui::vec2(-14.0, 14.0),
                            5.0,
                            accent(),
                        );
                    }

                    if resp.clicked() {
                        self.selected = i;
                    }
                    ui.end_row();
                }
            });

        ui.add_space(18.0);
        if let Some(c) = self.current_cert() {
            theme::card_frame(None).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let name = self
                        .current_cert()
                        .map(|c| c.cn.clone())
                        .unwrap_or_default();
                    avatar(ui, &initials(&name), 34.0, accent2());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Certificate details")
                                .font(Weight::SemiBold.font_id(14.0))
                                .color(text()),
                        );
                        ui.label(
                            egui::RichText::new(if name.is_empty() {
                                "—".to_string()
                            } else {
                                name
                            })
                            .size(11.5)
                            .color(text_faint()),
                        );
                    });
                });
                ui.add_space(10.0);
                egui::Grid::new("certmeta")
                    .num_columns(2)
                    .spacing([28.0, 7.0])
                    .show(ui, |ui| {
                        kv(ui, "Subject", &c.cn, false);
                        ui.end_row();
                        kv(ui, "Issuer", &c.issuer, false);
                        ui.end_row();
                        kv(ui, "Serial", &c.serial, true);
                        ui.end_row();
                        kv(ui, "Valid until", &c.not_after, true);
                        ui.end_row();
                        kv(ui, "Token", &c.token_serial, true);
                        ui.end_row();
                        kv(ui, "CKA ID", &c.id, true);
                        ui.end_row();
                    });
            });
        }
    }

    fn view_sign(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        theme::card_frame(None).show(ui, |ui| {
            ui.label(
                egui::RichText::new("Manual signing")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.label(
                egui::RichText::new("Sign a file or arbitrary text with your connected token.")
                    .size(12.0)
                    .color(text_faint()),
            );
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                ui.radio_value(&mut self.manual_source, ManualSource::File, "A file");
                ui.radio_value(&mut self.manual_source, ManualSource::Text, "Some text");
            });
            ui.add_space(8.0);

            match self.manual_source {
                ManualSource::File => {
                    ui.horizontal(|ui| {
                        if ui.button("Choose file…").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .set_title("Choose file to sign")
                                .pick_file()
                        {
                            self.manual_file_len =
                                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                            self.manual_file = Some(path);
                        }
                        match &self.manual_file {
                            Some(p) => {
                                let name = p
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                ui.label(egui::RichText::new(name).size(12.5).color(text()));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "({} bytes)",
                                        self.manual_file_len
                                    ))
                                    .size(11.5)
                                    .color(text_faint()),
                                );
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new(
                                        "No file chosen (PDF, XML, JSON, anything)",
                                    )
                                    .size(12.0)
                                    .color(text_faint()),
                                );
                            }
                        }
                    });
                }
                ManualSource::Text => {
                    ui.label(
                        egui::RichText::new("Data to sign")
                            .color(text_faint())
                            .size(12.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.data_input)
                            .desired_width(f32::INFINITY)
                            .hint_text("text to sign"),
                    );
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Format").color(text_faint()).size(12.0));
                ui.radio_value(&mut self.manual_pkcs7, true, "Detached PKCS#7 (.p7s)");
                ui.radio_value(&mut self.manual_pkcs7, false, "Raw signature (.sig)");
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Hash").color(text_faint()).size(12.0));
                egui::ComboBox::from_id_salt("manual_hash")
                    .selected_text(self.manual_hash.name())
                    .show_ui(ui, |ui| {
                        for h in [
                            pkcs11::HashAlgorithm::Sha256,
                            pkcs11::HashAlgorithm::Sha384,
                            pkcs11::HashAlgorithm::Sha512,
                            pkcs11::HashAlgorithm::Sha1,
                        ] {
                            ui.selectable_value(&mut self.manual_hash, h, h.name());
                        }
                    });
            });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Token PIN")
                    .color(text_faint())
                    .size(12.0),
            );
            let r = ui.add(
                egui::TextEdit::singleline(&mut *self.pin)
                    .password(true)
                    .hint_text("PIN")
                    .desired_width(f32::INFINITY),
            );
            if r.changed() {
                self.pin_set_at = if self.pin.is_empty() {
                    None
                } else {
                    Some(Instant::now())
                };
            }

            ui.add_space(14.0);
            let signing = self.sign_rx.is_some();
            ui.horizontal(|ui| {
                let b = ui.add_enabled(
                    !signing,
                    egui::Button::new(
                        egui::RichText::new(if signing {
                            "Signing…"
                        } else {
                            "Sign with selected certificate"
                        })
                        .font(Weight::SemiBold.font_id(13.0))
                        .color(egui::Color32::WHITE),
                    )
                    .fill(accent())
                    .min_size(egui::vec2(200.0, 38.0)),
                );
                if b.clicked() {
                    self.sign_manual();
                }
                if signing {
                    ui.add(egui::Spinner::new());
                } else if let Some(c) = self.current_cert() {
                    ui.label(
                        egui::RichText::new(format!("using {}", c.cn))
                            .size(11.5)
                            .color(text_dim()),
                    );
                }
            });
        });

        if !self.signature.is_empty() {
            ui.add_space(16.0);
            theme::card_frame(None).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Signature")
                            .font(Weight::SemiBold.font_id(14.0))
                            .color(text()),
                    );
                    chip(ui, &self.signature_cn, accent2());
                    if !self.signature_source.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.signature_source)
                                .size(11.5)
                                .color(text_faint()),
                        );
                    }
                });
                if self.signature_demo {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Made with the demo key: this is not a valid signature.",
                        )
                        .size(12.0)
                        .color(warn()),
                    );
                }
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.signature)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let save_label = if self.signature_pkcs7 {
                        "Save .p7s…"
                    } else {
                        "Save .sig…"
                    };
                    if ui.button(save_label).clicked()
                        && let Ok(bytes) = B64.decode(&self.signature)
                    {
                        let ext = if self.signature_pkcs7 { "p7s" } else { "sig" };
                        let default_name = match &self.manual_file {
                            Some(p) => format!(
                                "{}.{ext}",
                                p.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "signature".to_string())
                            ),
                            None => format!("signature.{ext}"),
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Save signature")
                            .set_file_name(&default_name)
                            .save_file()
                        {
                            match std::fs::write(&path, bytes) {
                                Ok(()) => self.set_status(
                                    Level::Ok,
                                    format!("Saved signature to {}", path.display()),
                                ),
                                Err(e) => {
                                    self.set_status(Level::Err, format!("Could not save: {e}"))
                                }
                            }
                        }
                    }
                    if ui.button("Copy signature").clicked() {
                        ui.ctx().copy_text(self.signature.clone());
                    }
                });
            });
        }
    }

    fn view_portals(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        ui.label(
            egui::RichText::new(
                "The browser extension catches each portal's DSC sign button and localhost signer calls, and sends them here. No Java and no vendor utility are needed.",
            )
            .size(12.5)
            .color(text_dim()),
        );
        ui.add_space(12.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width();
            let cols = if width > 760.0 {
                3
            } else if width > 480.0 {
                2
            } else {
                1
            };
            let card_w = ((width - 14.0 * (cols as f32 - 1.0)) / cols as f32).max(200.0);
            for row in portals::PORTALS.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for portal in row {
                        let signed = self.portal_counts.get(portal.name).copied().unwrap_or(0);
                        theme::card_frame(None).show(ui, |ui| {
                            ui.set_width(card_w - 30.0);
                            ui.set_min_height(104.0);
                            ui.horizontal(|ui| {
                                avatar(
                                    ui,
                                    portal.short,
                                    34.0,
                                    if portal.filings { accent() } else { accent2() },
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(portal.name)
                                            .font(Weight::SemiBold.font_id(13.5))
                                            .color(text()),
                                    );
                                    ui.label(
                                        egui::RichText::new(portal.domains.join(", "))
                                            .family(egui::FontFamily::Monospace)
                                            .size(10.5)
                                            .color(text_faint()),
                                    );
                                });
                            });
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(portal.what)
                                    .size(12.0)
                                    .color(text_dim()),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                chip(
                                    ui,
                                    if portal.filings {
                                        "Citizen filings"
                                    } else {
                                        "Procurement & trade"
                                    },
                                    if portal.filings { accent() } else { accent2() },
                                );
                                if signed > 0 {
                                    chip(ui, &format!("{signed} signed"), ok());
                                }
                            });
                        });
                    }
                });
                ui.add_space(10.0);
            }
        });
    }

    /// Build a privacy-conscious diagnostics blob for support requests. It
    /// deliberately excludes PINs, signatures, payload bytes, and certificate
    /// bodies; only versions, module paths, token metadata, and recent log text
    /// are included.
    fn diagnostics_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("VyomKunji {}\n", env!("CARGO_PKG_VERSION")));
        out.push_str(&format!(
            "OS: {} / {}\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        out.push_str(&format!(
            "Module: {}\n",
            self.manager
                .as_ref()
                .map(|(m, _)| m.as_str())
                .unwrap_or("(none)")
        ));
        out.push_str(&format!(
            "Browser bridge port: {}\n",
            self.bridge_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "(not listening)".to_string())
        ));
        out.push_str(&format!("Certificates: {}\n", self.certs.len()));
        if let Some(cert) = self.certs.first() {
            out.push_str(&format!("Token: {}\n", cert.token_label));
        }
        out.push_str(&format!("Pending request: {}\n", self.pending.is_some()));
        out.push_str(&format!(
            "Status: {:?} — {}\n",
            self.status_level, self.status
        ));
        out.push_str("\nRecent log:\n");
        for entry in self.log.iter().rev().take(50) {
            out.push_str(&format!("{}  {}\n", entry.time, entry.text));
        }
        // Keep local usernames out of the copied text even when a custom PKCS#11
        // module path happens to live under the user profile.
        for (variable, placeholder) in [
            ("USERPROFILE", "%USERPROFILE%"),
            ("LOCALAPPDATA", "%LOCALAPPDATA%"),
            ("APPDATA", "%APPDATA%"),
        ] {
            if let Some(value) = std::env::var_os(variable) {
                let value = value.to_string_lossy();
                if !value.is_empty() {
                    out = out.replace(value.as_ref(), placeholder);
                }
            }
        }
        out
    }

    fn view_settings(&mut self, ui: &mut egui::Ui) {
        self.view_header(ui);

        theme::card_frame(None).show(ui, |ui| {
            ui.label(
                egui::RichText::new("PKCS#11 module")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.label(
                egui::RichText::new(
                    "Override auto-detection with a specific module. Leave empty for auto-detect.",
                )
                .size(12.0)
                .color(text_faint()),
            );
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Module path")
                    .color(text_faint())
                    .size(12.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.module_input)
                    .desired_width(f32::INFINITY)
                    .hint_text("C:\\Windows\\System32\\...\\module.dll"),
            );

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    let p = self.module_input.trim().to_string();
                    let to_save = if p.is_empty() { None } else { Some(p.as_str()) };
                    let _ = pkcs11::save_configured_module(to_save);
                    self.selected_module = p.clone();
                    self.connect();
                }
                if ui.button("Browse for driver…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose PKCS#11 driver")
                        .add_filter("PKCS#11 library", &["dll", "so", "dylib"])
                        .pick_file()
                {
                    let p = path.to_string_lossy().to_string();
                    self.module_input = p.clone();
                    let _ = pkcs11::save_configured_module(Some(&p));
                    self.selected_module = p;
                    self.connect();
                }
                if ui.button("Auto-detect").clicked() {
                    self.module_input.clear();
                    self.selected_module.clear();
                    let _ = pkcs11::save_configured_module(None);
                    self.connect();
                }
                if ui.button("Refresh list").clicked() {
                    pkcs11::clear_unresponsive();
                    self.available_modules = pkcs11::rescan_modules();
                    self.connect();
                }
            });

            if !self.available_modules.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Discovered modules")
                        .color(text_faint())
                        .size(12.0),
                );
                egui::ComboBox::from_id_salt("modcombo")
                    .selected_text(if self.module_input.is_empty() {
                        "(auto-detect)".to_string()
                    } else {
                        self.module_input.clone()
                    })
                    .show_ui(ui, |ui| {
                        for m in self.available_modules.clone() {
                            if ui.selectable_label(self.module_input == m, &m).clicked() {
                                self.module_input = m;
                            }
                        }
                    });
            }

            ui.add_space(10.0);
            if let Some((m, _)) = &self.manager {
                kv(ui, "Active module", m, true);
            } else if self.demo_key {
                kv(ui, "Active module", "Demo key (no driver)", false);
            } else {
                kv(ui, "Active module", "(none)", false);
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("PIN & Window preferences")
                    .font(Weight::SemiBold.font_id(14.0))
                    .color(text()),
            );
            ui.add_space(6.0);
            if ui
                .checkbox(&mut self.remember_pin, "Remember PIN in memory between signatures")
                .changed()
            {
                let _ = pkcs11::save_config_key(
                    "rememberPin",
                    serde_json::json!(self.remember_pin),
                );
                if !self.remember_pin {
                    self.pin = Zeroizing::new(String::new());
                    self.pin_set_at = None;
                }
            }
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Forget PIN after")
                        .size(12.5)
                        .color(text_dim()),
                );
                let resp =
                    ui.add(egui::Slider::new(&mut self.pin_timeout_min, 1..=60).suffix(" min"));
                if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                    self.pin_timeout = Duration::from_secs(self.pin_timeout_min as u64 * 60);
                    let _ = pkcs11::save_config_key(
                        "pinTimeoutMin",
                        serde_json::json!(self.pin_timeout_min),
                    );
                }
            });
            if ui
                .checkbox(
                    &mut self.close_to_tray,
                    "Keep running in the tray when the window is closed",
                )
                .changed()
            {
                let _ = pkcs11::save_config_key(
                    "closeToTray",
                    serde_json::json!(self.close_to_tray),
                );
            }
            if ui
                .checkbox(
                    &mut self.demo_key,
                    format!("Use a demo key instead of a real one (PIN {DEMO_PIN}, signatures are not valid)"),
                )
                .changed()
            {
                let _ = pkcs11::save_config_key("demoKey", serde_json::json!(self.demo_key));
                self.connect();
            }

            ui.add_space(14.0);
            if ui.button("Copy diagnostics").clicked() {
                let text = self.diagnostics_text();
                ui.ctx().copy_text(text);
                self.set_status(
                    Level::Ok,
                    "Diagnostics copied to clipboard".to_string(),
                );
            }
            ui.label(
                egui::RichText::new(
                    "Includes version, token metadata, and the last 50 log lines. Never includes your PIN or signatures.",
                )
                .size(11.0)
                .color(text_faint()),
            );

            ui.add_space(14.0);
            if ui.button("Quit VyomKunji").clicked() {
                self.quit_requested = true;
                if let Some(ctx) = &self.ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });
    }

    fn view_activity(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(self.nav.title())
                        .font(Weight::Bold.font_id(23.0))
                        .color(text()),
                );
                ui.label(
                    egui::RichText::new(self.nav.hint())
                        .size(12.0)
                        .color(text_faint()),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.button("Copy").clicked() {
                    let text: String = self
                        .log
                        .iter()
                        .rev()
                        .map(|e| format!("{}  {}", e.time, e.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(text);
                }
            });
        });
        ui.add_space(14.0);

        theme::card_frame(None).show(ui, |ui| {
            if self.log.is_empty() {
                ui.label("Nothing logged yet.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for e in &self.log {
                    ui.horizontal(|ui| {
                        ui.painter().circle_filled(
                            ui.min_rect().left_center() + egui::vec2(4.0, 0.0),
                            3.5,
                            e.level.color(),
                        );
                        ui.label(
                            egui::RichText::new(&e.time)
                                .family(egui::FontFamily::Monospace)
                                .size(11.0)
                                .color(text_faint()),
                        );
                        ui.label(egui::RichText::new(&e.text).size(12.5).color(text_dim()));
                    });
                    ui.add_space(3.0);
                }
            });
        });
    }
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
