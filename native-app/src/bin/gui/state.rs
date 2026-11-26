// State, bridge, token and signing logic.
use super::*;

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
    pub(super) fn new(waker: &fastframe_shell::Waker) -> Self {
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
    pub(super) fn attach(&mut self, ctx: &egui::Context) {
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
    pub(super) fn apply_theme(&mut self, ctx: &egui::Context) {
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

    pub(super) fn set_status(&mut self, level: Level, text: String) {
        self.status_level = level;
        self.status = text.clone();
        self.push_log(level, text);
    }

    /// Ask for a token connection without blocking the UI. The PKCS#11
    /// subsystem can take seconds to enumerate drivers and probe tokens, so it
    /// must never run on the path to the first frame.
    pub(super) fn connect(&mut self) {
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

    pub(super) fn current_cert(&self) -> Option<&CertRow> {
        self.certs.get(self.selected)
    }

    pub(super) fn sign_manual(&mut self) {
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

    pub(super) fn approve_pending(&mut self) {
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

    pub(super) fn cancel(&mut self) {
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
    pub(super) fn pump_requests(&mut self) {
        while let Ok((req, resp)) = self.gui_rx.try_recv() {
            self.accept_request(req, resp);
        }
        self.poll_sign();
        self.poll_connect();
        self.poll_presence();
        self.refresh_token();
    }

    /// Handle tray clicks.
    pub(super) fn drain_tray(&mut self) {
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
