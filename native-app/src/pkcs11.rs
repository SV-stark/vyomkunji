// PKCS#11 token access for DSC signing.
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use cryptoki::context::{CInitializeArgs, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, CertificateType, KeyType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use serde_json::{json, Value};
use std::path::Path;

pub struct TokenManager {
    pkcs11: Pkcs11,
}

/// Optional explicit override via environment variable. Accepts either a single
/// DLL path or a directory (scanned recursively).
fn env_module_override() -> Option<String> {
    std::env::var("VYOMKUNJI_PKCS11_MODULE")
        .or_else(|_| std::env::var("SIGNER_PKCS11_MODULE"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Path to the VyomKunji user config file.
pub fn config_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(
        std::path::PathBuf::from(appdata)
            .join("VyomKunji")
            .join("config.json"),
    )
}

/// Read a user-configured PKCS#11 module path from config.json, if present
/// and pointing at an existing file.
pub fn configured_module_path() -> Option<String> {
    let path = config_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    // Minimal JSON parse without pulling in a parser dependency: look for
    // "pkcs11Module": "...".
    let key = "\"pkcs11Module\"";
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    let val = rest[..end].to_string();
    if val.is_empty() {
        return None;
    }
    if std::path::Path::new(&val).is_file() {
        Some(val)
    } else {
        None
    }
}

/// Persist (or clear) the user-configured PKCS#11 module path.
#[allow(dead_code)]
pub fn save_configured_module(path: Option<&str>) -> std::io::Result<()> {
    let cfg = config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    if let Some(parent) = cfg.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = match path {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => String::new(),
    };
    // Preserve any other keys by rewriting a minimal object; config only holds
    // this key today.
    let json = if value.is_empty() {
        "{}".to_string()
    } else {
        format!(
            "{{\"pkcs11Module\":\"{}\"}}",
            value.replace("\\\\", "\\\\\\\\").replace('"', "\\\\\"")
        )
    };
    std::fs::write(cfg, json)
}

fn push_if_exists(out: &mut Vec<String>, p: String) {
    let path = std::path::Path::new(&p);
    if path.is_file() && !out.iter().any(|x| x.eq_ignore_ascii_case(&p)) {
        out.push(p);
    }
}

/// Curated set of well-known PKCS#11 module file names used by Indian (and
/// common global) DSC token drivers. Matching is case-insensitive.
fn known_dll_names() -> &'static [&'static str] {
    &[
        // Watchdata / ProxKey
        "WDPKCS.dll",
        "WDPKCS64.dll",
        // SafeNet / Gemalto eToken family (used by many Indian CAs)
        "etpkcs11.dll",
        "eToken.dll",
        // ePass 2003 / HYP 2003
        "eps2003csp11.dll",
        "eps2003csp11_64.dll",
        "hyp2003csp11.dll",
        "HYP2003csp11IND.dll",
        "HYP2003csp11IND_s.dll",
        // AET / Rainbow / StarSign middleware
        "aetpkss1.dll",
        "aetpksse.dll",
        "aetpkssw.dll",
        // Siemens Card OS
        "siecap11.dll",
        "etpkcs11.dll",
        // Datakey
        "dkck201.dll",
        "dkck232.dll",
        // Utimaco
        "pkcs201n.dll",
        // OpenSC
        "opensc-pkcs11.dll",
        // nShield
        "cknfast.dll",
        // Generic / TeleSec style
        "pkcs11.dll",
        "cryptoki.dll",
        "cryptoki22.dll",
        // Schlumberger / SafeNet
        "slbck.dll",
        "acpkcs.dll",
        "acpkcs211.dll",
        // SecuTech
        "utp11.dll",
        // InnaIT
        "innaitpkcs11.dll",
        // OpenSC / others
        "p11card.dll",
        "micardopkcs11.dll",
        "nxpkcs11.dll",
        "smartp11.dll",
        "spypk11.dll",
        "fort32.dll",
        "auCryptoki2-0.dll",
        "3gp11csp.dll",
        // CryptoID / Longmai mToken (blue & purple variants)
        "cryptoida_pkcs11.dll",
        "cryptoida_pkcs11_f2.dll",
        "cryptoida_pkcs11_f3.dll",
        "cryptoid_pkcs11.dll",
        // SafeSign / AET / G&D (StarSign, StarCos)
        "aetpkss1.dll",
        "aetpksse.dll",
        "aetpkssw.dll",
        "safesignpkcs11.dll",
        "safesign.dll",
        // Precision InnaIT
        "innaitpkcs11.dll",
        "precisionpkcs11.dll",
        // Indian CA branded guesses (vendors frequently rebrand SafeNet/OpenSC)
        "capricornpkcs11.dll",
        "pantasignpkcs11.dll",
        "safescryptpkcs11.dll",
        "emudhrapkcs11.dll",
        "ncodepkcs11.dll",
        "vsignpkcs11.dll",
        "idsignpkcs11.dll",
        "xtratrustpkcs11.dll",
        "prodigisignpkcs11.dll",
        "care4signpkcs11.dll",
        "signxpkcs11.dll",
    ]
}

fn known_vendor_directories() -> Vec<String> {
    let mut dirs = Vec::new();
    let push = |dirs: &mut Vec<String>, p: &str| dirs.push(p.to_string());

    // Watchdata / ProxKey
    push(
        &mut dirs,
        r"C:\Windows\System32\Watchdata\PROXKey CSP India V3.0",
    );
    push(
        &mut dirs,
        r"C:\Windows\SysWOW64\Watchdata\PROXKey CSP India V3.0",
    );
    push(&mut dirs, r"C:\Program Files\Watchdata");
    push(&mut dirs, r"C:\Program Files (x86)\Watchdata");

    // eMudhra
    push(&mut dirs, r"C:\Program Files\eMudhra");
    push(&mut dirs, r"C:\Program Files (x86)\eMudhra");
    push(&mut dirs, r"C:\Program Files\eMudhra USB token");
    push(&mut dirs, r"C:\Program Files (x86)\eMudhra USB token");

    // (n)Code Solutions
    push(&mut dirs, r"C:\Program Files\nCode Solutions");
    push(&mut dirs, r"C:\Program Files (x86)\nCode Solutions");
    push(&mut dirs, r"C:\Program Files\GNFC");
    push(&mut dirs, r"C:\Program Files (x86)\GNFC");

    // SafeNet / Gemalto (used by Safescrypt, Capricorn, Pantasign, Vsign, ...)
    push(&mut dirs, r"C:\Program Files\SafeNet");
    push(&mut dirs, r"C:\Program Files (x86)\SafeNet");
    push(&mut dirs, r"C:\Program Files\Gemalto");
    push(&mut dirs, r"C:\Program Files (x86)\Gemalto");
    push(&mut dirs, r"C:\Program Files\Safenet");
    push(&mut dirs, r"C:\Program Files (x86)\Safenet");

    // InnaIT
    push(&mut dirs, r"C:\Program Files\InnaIT");
    push(&mut dirs, r"C:\Program Files (x86)\InnaIT");
    push(&mut dirs, r"C:\Program Files\Precision");
    push(&mut dirs, r"C:\Program Files (x86)\Precision");

    // SafeSign / AET / G&D (StarSign, StarCos)
    push(&mut dirs, r"C:\Program Files\A.E.T");
    push(&mut dirs, r"C:\Program Files (x86)\A.E.T");
    push(&mut dirs, r"C:\Program Files\SafeSign");
    push(&mut dirs, r"C:\Program Files (x86)\SafeSign");
    push(&mut dirs, r"C:\Program Files\Giesecke & Devrient");
    push(&mut dirs, r"C:\Program Files (x86)\Giesecke & Devrient");
    push(&mut dirs, r"C:\Program Files\G&D");
    push(&mut dirs, r"C:\Program Files (x86)\G&D");

    // OpenSC
    push(&mut dirs, r"C:\Program Files\OpenSC Project");
    push(&mut dirs, r"C:\Program Files (x86)\OpenSC Project");

    // Aladdin (legacy)
    push(&mut dirs, r"C:\Program Files\Aladdin");
    push(&mut dirs, r"C:\Program Files (x86)\Aladdin");

    dirs
}

/// Recursively scan `root` (up to `depth` levels) collecting candidate PKCS#11
/// modules. Only files whose name matches a known module name or contains
/// "pkcs"/"cryptoki"/"token" are collected, to keep the list small.
fn scan_dir_for_pkcs11(root: &std::path::Path, depth: usize, out: &mut Vec<String>) {
    if depth == 0 || !root.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                let lower = name.to_lowercase();
                if !lower.ends_with(".dll") {
                    continue;
                }
                let known = known_dll_names()
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(name));
                let pattern = lower.contains("pkcs")
                    || lower.contains("cryptoki")
                    || lower.contains("token")
                    || lower.contains("csp11")
                    || lower.contains("safesign")
                    || lower.contains("aet")
                    || lower.contains("starcos")
                    || lower.contains("inna")
                    || lower.contains("precision");
                if known || pattern {
                    let ps = path.to_string_lossy().to_string();
                    if !out.iter().any(|x| x.eq_ignore_ascii_case(&ps)) {
                        out.push(ps);
                    }
                }
            }
        } else if path.is_dir() && depth > 1 {
            scan_dir_for_pkcs11(&path, depth - 1, out);
        }
    }
}

#[cfg(windows)]
fn has_pkcs11_entrypoint(path: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lpfilename: *const u16) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            hmodule: *mut std::ffi::c_void,
            lpprocname: *const i8,
        ) -> *mut std::ffi::c_void;
        fn FreeLibrary(hlibmodule: *mut std::ffi::c_void) -> i32;
    }
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let h = LoadLibraryW(wide.as_ptr());
        if h.is_null() {
            return false;
        }
        let proc = GetProcAddress(h, c"C_GetFunctionList".as_ptr());
        let _ = FreeLibrary(h);
        !proc.is_null()
    }
}

fn discover_modules_impl() -> Vec<String> {
    let mut v = Vec::new();

    // Vendor-specific directories (deeper scan).
    for d in known_vendor_directories() {
        let dir = std::path::Path::new(&d);
        if dir.is_dir() {
            scan_dir_for_pkcs11(dir, 4, &mut v);
        }
    }

    // System directories: shallow scan catches modules dropped directly in
    // System32/SysWOW64 (common for many token drivers).
    for sys in [r"C:\Windows\System32", r"C:\Windows\SysWOW64"] {
        let root = std::path::Path::new(sys);
        if root.is_dir() {
            scan_dir_for_pkcs11(root, 2, &mut v);
        }
    }

    // Program Files roots are intentionally NOT scanned generically: a broad
    // scan there is slow and pulls in unrelated DLLs. Vendor-specific
    // directories above already cover the common install locations. Users with
    // a non-standard install path can set VYOMKUNJI_PKCS11_MODULE.

    // Legacy explicit paths (kept for compatibility).
    push_if_exists(
        &mut v,
        r"C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll".to_string(),
    );
    push_if_exists(
        &mut v,
        r"C:\Windows\SysWOW64\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll".to_string(),
    );
    push_if_exists(&mut v, r"C:\Windows\System32\WDPKCS.dll".to_string());

    // Case-insensitive dedup.
    let mut deduped = Vec::new();
    for p in v {
        if !deduped.iter().any(|x: &String| x.eq_ignore_ascii_case(&p)) {
            deduped.push(p);
        }
    }

    // Keep only genuine PKCS#11 modules (must export C_GetFunctionList).
    // This filters out random DLLs picked up by the directory scan.
    #[cfg(windows)]
    {
        deduped.retain(|p| has_pkcs11_entrypoint(p));
    }
    deduped
}

fn default_module_paths() -> Vec<String> {
    // Environment variable wins (automation).
    if let Some(p) = env_module_override() {
        let path = std::path::Path::new(&p);
        if path.is_dir() {
            let mut out = Vec::new();
            scan_dir_for_pkcs11(path, 5, &mut out);
            if !out.is_empty() {
                return out;
            }
        }
        return vec![p];
    }
    // User-configured module from the GUI (config.json).
    if let Some(p) = configured_module_path() {
        let path = std::path::Path::new(&p);
        if path.is_dir() {
            let mut out = Vec::new();
            scan_dir_for_pkcs11(path, 5, &mut out);
            if !out.is_empty() {
                return out;
            }
        }
        return vec![p];
    }
    discover_modules_impl()
}

/// Public alias used by the GUI module picker.
#[allow(dead_code)]
pub fn discover_modules() -> Vec<String> {
    discover_modules_impl()
}

/// Human-readable metadata extracted from a DER certificate.
pub struct CertInfo {
    pub subject: String,
    pub cn: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub serial: String,
}

impl TokenManager {
    /// Auto-detect and load a PKCS#11 module, returning (module_path, manager).
    ///
    /// Unlike the previous implementation, this tries *every* candidate module
    /// instead of giving up on the first one that fails to initialise, and it
    /// prefers a module that actually exposes an inserted token. This makes
    /// detection robust across the many Indian CA token drivers (ProxKey,
    /// eMudhra, (n)Code, SafeScrypt, Capricorn, Pantasign, Vsign, ...).
    pub fn auto() -> Result<(String, Self), String> {
        let candidates = default_module_paths();
        if candidates.is_empty() {
            return Err("No PKCS#11 module found (set VYOMKUNJI_PKCS11_MODULE)".to_string());
        }

        let mut last_err = String::new();
        let mut fallback: Option<(String, Self)> = None;

        for p in candidates {
            if !Path::new(&p).exists() {
                continue;
            }
            match Self::new(&p) {
                Ok(m) => {
                    // Prefer a module that actually exposes an inserted token.
                    let has_token = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        m.pkcs11
                            .get_slots_with_token()
                            .map(|s| !s.is_empty())
                            .unwrap_or(false)
                    }))
                    .unwrap_or(false);
                    if has_token {
                        return Ok((p, m));
                    }
                    if fallback.is_none() {
                        fallback = Some((p, m));
                    }
                }
                Err(e) => {
                    if last_err.is_empty() {
                        last_err = format!("{}: {}", p, e);
                    }
                }
            }
        }

        if let Some(fb) = fallback {
            return Ok(fb);
        }
        Err(if last_err.is_empty() {
            "No PKCS#11 module found (set VYOMKUNJI_PKCS11_MODULE)".to_string()
        } else {
            last_err
        })
    }

    pub fn new(module_path: &str) -> Result<Self, String> {
        // Some candidate DLLs are not real PKCS#11 modules and loading them can
        // panic inside cryptoki-sys (GetProcAddress failures). Guard against that.
        let load = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let pk = Pkcs11::new(module_path).map_err(|e| e.to_string())?;
            pk.initialize(CInitializeArgs::OsThreads)
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(pk)
        }));
        match load {
            Ok(Ok(pk)) => Ok(Self { pkcs11: pk }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(format!("panic while loading {}", module_path)),
        }
    }

    /// Human-readable metadata extracted from a DER certificate.
    fn parse_cert(der: &[u8]) -> CertInfo {
        use x509_parser::prelude::*;
        match X509Certificate::from_der(der) {
            Ok((_, cert)) => {
                let subject = cert.subject().to_string();
                let cn = cert
                    .subject()
                    .iter_common_name()
                    .next()
                    .map(|s| s.as_str().unwrap_or("").to_string())
                    .unwrap_or_default();
                let issuer = cert.issuer().to_string();
                let fmt_date = |t: &x509_parser::time::ASN1Time| {
                    let d = t.to_datetime();
                    format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
                };
                let not_before = fmt_date(&cert.validity().not_before);
                let not_after = fmt_date(&cert.validity().not_after);
                let serial = cert
                    .raw_serial()
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join("");
                CertInfo {
                    subject,
                    cn,
                    issuer,
                    not_before,
                    not_after,
                    serial,
                }
            }
            Err(_) => CertInfo {
                subject: String::new(),
                cn: String::new(),
                issuer: String::new(),
                not_before: String::new(),
                not_after: String::new(),
                serial: String::new(),
            },
        }
    }

    /// Enumerate tokens and X.509 certificates.
    pub fn list_tokens(&self) -> Result<Value, String> {
        let slots = self
            .pkcs11
            .get_slots_with_token()
            .map_err(|e| e.to_string())?;
        let mut tokens = Vec::new();
        for slot in slots {
            let tinfo = self
                .pkcs11
                .get_token_info(slot)
                .map_err(|e| e.to_string())?;
            let token = json!({
                "id": format!("slot-{}", slot.id()),
                "label": tinfo.label(),
                "serial": tinfo.serial_number(),
            });
            let mut certs = Vec::new();

            // public certs readable without login
            let session = self
                .pkcs11
                .open_ro_session(slot)
                .map_err(|e| e.to_string())?;
            let template = vec![
                Attribute::Class(ObjectClass::CERTIFICATE),
                Attribute::CertificateType(CertificateType::X_509),
            ];
            let objects = session.find_objects(&template).map_err(|e| e.to_string())?;
            for obj in objects {
                let attrs = session
                    .get_attributes(
                        obj,
                        &[
                            AttributeType::Value,
                            AttributeType::Label,
                            AttributeType::Id,
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                let mut der = Vec::new();
                let mut label = String::new();
                let mut id = Vec::new();
                for a in attrs {
                    match a {
                        Attribute::Value(v) => der = v,
                        Attribute::Label(v) => {
                            label = String::from_utf8_lossy(&v)
                                .trim_end_matches('\0')
                                .to_string()
                        }
                        Attribute::Id(v) => id = v,
                        _ => {}
                    }
                }
                if der.is_empty() {
                    continue;
                }
                let info = Self::parse_cert(&der);
                certs.push(json!({
                    "id": B64.encode(&id),
                    "label": if label.is_empty() { info.cn.clone() } else { label },
                    "subject": info.subject,
                    "cn": info.cn,
                    "issuer": info.issuer,
                    "notBefore": info.not_before,
                    "notAfter": info.not_after,
                    "serial": info.serial,
                    "der": B64.encode(&der),
                }));
            }

            let mut token_obj = token;
            token_obj["certs"] = json!(certs);
            tokens.push(token_obj);
        }
        Ok(json!(tokens))
    }

    /// Sign `data` with the token's signing key. Optionally filter cert CN.
    /// If `already_hashed` is true, expects a 32-byte SHA-256 digest (or 51-byte DigestInfo structure)
    /// and signs with `Mechanism::RsaPkcs`.
    /// If `already_hashed` is false, hashes with SHA-256 and signs with `Mechanism::Sha256RsaPkcs`.
    pub fn sign(
        &self,
        pin: &str,
        data: &[u8],
        cert_cn_filter: Option<&str>,
        already_hashed: bool,
    ) -> Result<Value, String> {
        let slots = self
            .pkcs11
            .get_slots_with_token()
            .map_err(|e| e.to_string())?;
        let mut last_err = "no token found".to_string();
        for slot in slots {
            if let Ok(r) = self.sign_in_slot(slot, pin, data, cert_cn_filter, already_hashed) {
                return Ok(r);
            }
            last_err = "signing failed in all tokens".to_string();
        }
        Err(last_err)
    }

    fn sign_in_slot(
        &self,
        slot: cryptoki::slot::Slot,
        pin: &str,
        data: &[u8],
        cert_cn_filter: Option<&str>,
        already_hashed: bool,
    ) -> Result<Value, String> {
        let session = self
            .pkcs11
            .open_ro_session(slot)
            .map_err(|e| e.to_string())?;
        session
            .login(UserType::User, Some(&AuthPin::new(pin.to_string())))
            .map_err(|e| format!("login failed: {}", e))?;

        // Find X.509 certs (to filter)
        let cert_template = vec![
            Attribute::Class(ObjectClass::CERTIFICATE),
            Attribute::CertificateType(CertificateType::X_509),
        ];
        let certs = session
            .find_objects(&cert_template)
            .map_err(|e| e.to_string())?;

        let mut chosen_id: Option<Vec<u8>> = None;
        let mut chosen_cn = String::new();
        let mut chosen_der = Vec::new();
        for c in &certs {
            let attrs = session
                .get_attributes(*c, &[AttributeType::Value, AttributeType::Id])
                .map_err(|e| e.to_string())?;
            let mut der = Vec::new();
            let mut id = Vec::new();
            for a in attrs {
                match a {
                    Attribute::Value(v) => der = v,
                    Attribute::Id(v) => id = v,
                    _ => {}
                }
            }
            if der.is_empty() {
                continue;
            }
            let info = Self::parse_cert(&der);
            if let Some(filter) = cert_cn_filter {
                if !info.cn.to_lowercase().contains(&filter.to_lowercase()) {
                    continue;
                }
            }
            chosen_id = Some(id);
            chosen_cn = info.cn;
            chosen_der = der;
            break;
        }

        // Find private RSA key matching chosen cert id (or any signing key)
        let mut key_template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::KeyType(KeyType::RSA),
            Attribute::Sign(true),
        ];
        if let Some(ref id) = chosen_id {
            key_template.push(Attribute::Id(id.clone()));
        }
        let keys = session
            .find_objects(&key_template)
            .map_err(|e| e.to_string())?;
        let key = keys
            .into_iter()
            .next()
            .ok_or_else(|| "no private signing key found".to_string())?;

        let (sig, mechanism_name) = if already_hashed {
            // SHA-256 DigestInfo prefix (RFC 3447 / PKCS #1 v2.2):
            // SEQUENCE { SEQUENCE { AlgorithmIdentifier sha-256, NULL }, OCTET STRING hash }
            const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
                0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x01, 0x05, 0x00, 0x04, 0x20,
            ];
            let payload_to_sign = if data.len() == 32 {
                let mut v = Vec::with_capacity(51);
                v.extend_from_slice(&SHA256_DIGEST_INFO_PREFIX);
                v.extend_from_slice(data);
                v
            } else {
                data.to_vec()
            };
            let s = session
                .sign(&Mechanism::RsaPkcs, key, &payload_to_sign)
                .map_err(|e| format!("sign failed: {}", e))?;
            (s, "RSA_PKCS")
        } else {
            let s = session
                .sign(&Mechanism::Sha256RsaPkcs, key, data)
                .map_err(|e| format!("sign failed: {}", e))?;
            (s, "SHA256_RSA_PKCS")
        };

        let _ = session.logout();

        Ok(json!({
            "signature": B64.encode(&sig),
            "cert": B64.encode(&chosen_der),
            "certId": B64.encode(chosen_id.unwrap_or_default()),
            "certCN": chosen_cn,
            "mechanism": mechanism_name,
        }))
    }
}
