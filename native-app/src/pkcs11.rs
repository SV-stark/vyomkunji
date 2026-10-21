// PKCS#11 token access for DSC signing.
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use cryptoki::context::{CInitializeArgs, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, CertificateType, KeyType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone)]
pub struct TokenManager {
    pkcs11: Pkcs11,
}

/// How long to wait for a single driver to initialise and list its token.
/// Cold starts and antivirus scans can make vendor DLLs slow, so this is
/// deliberately generous; a driver that exceeds it is treated as unresponsive
/// and skipped for the rest of the session.
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

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

/// Remember which module last produced a token, so the next launch can try it
/// first instead of initialising every installed driver. This is only a hint:
/// `auto()` still falls back to the full candidate list.
fn last_module_file() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(
        std::path::PathBuf::from(appdata)
            .join("VyomKunji")
            .join("last_module.txt"),
    )
}

fn last_module_hint() -> Option<String> {
    let text = std::fs::read_to_string(last_module_file()?).ok()?;
    let value = text.trim();
    if value.is_empty() || !Path::new(value).is_file() {
        return None;
    }
    Some(value.to_string())
}

/// Record the module that successfully exposed a token (used as a fast-path
/// hint on the next start).
#[allow(dead_code)]
pub fn save_last_module(path: &str) {
    let Some(file) = last_module_file() else {
        return;
    };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(file, path.trim());
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
        // `DirEntry::file_type` reuses the attributes Windows already returned
        // from the directory read; `Path::is_file` would stat every entry again
        // and was a measurable part of the startup scan.
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_file() {
            let name = match entry.file_name().to_str() {
                Some(s) => s.to_ascii_lowercase(),
                None => continue,
            };
            if !name.ends_with(".dll") {
                continue;
            }
            let known = known_dll_names()
                .iter()
                .any(|k| k.eq_ignore_ascii_case(&name));
            let pattern = name.contains("pkcs")
                || name.contains("cryptoki")
                || name.contains("token")
                || name.contains("csp11")
                || name.contains("safesign")
                || name.contains("aet")
                || name.contains("starcos")
                || name.contains("inna")
                || name.contains("precision");
            if known || pattern {
                let ps = entry.path().to_string_lossy().to_string();
                if !out.iter().any(|x| x.eq_ignore_ascii_case(&ps)) {
                    out.push(ps);
                }
            }
        } else if file_type.is_dir() && depth > 1 {
            scan_dir_for_pkcs11(&entry.path(), depth - 1, out);
        }
    }
}

/// Read a DLL's PE export table and report whether it exports
/// `C_GetFunctionList`. This is deliberately done by parsing the file instead
/// of `LoadLibraryW` + `GetProcAddress`: loading every candidate runs vendor
/// `DllMain` code and was measured at ~3 s on a machine with three token
/// drivers, which dominated cold start.
#[cfg(windows)]
fn has_pkcs11_entrypoint(path: &str) -> bool {
    use std::io::{Read, Seek, SeekFrom};

    fn u16_at(b: &[u8], off: usize) -> Option<u16> {
        b.get(off..off + 2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32_at(b: &[u8], off: usize) -> Option<u32> {
        b.get(off..off + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    // (virtual address, virtual size, raw size, raw pointer)
    type Section = (u32, u32, u32, u32);

    fn rva_to_offset(sections: &[Section], rva: u32) -> Option<u64> {
        for &(va, vsize, raw_size, raw_ptr) in sections {
            if rva < va {
                continue;
            }
            let delta = rva - va;
            if delta >= vsize.max(raw_size) {
                continue;
            }
            if delta >= raw_size {
                return None;
            }
            return Some(raw_ptr as u64 + delta as u64);
        }
        None
    }

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut dos = [0u8; 64];
    if file.read_exact(&mut dos).is_err() || &dos[0..2] != b"MZ" {
        return false;
    }
    let pe_offset = u32_at(&dos, 0x3c).unwrap_or(0) as u64;

    // PE signature + COFF header + optional header + section table.
    let mut header = vec![0u8; 0x4000];
    if file.seek(SeekFrom::Start(pe_offset)).is_err() {
        return false;
    }
    let read = file.read(&mut header).unwrap_or(0);
    header.truncate(read);
    if header.get(0..4) != Some(b"PE\0\0") {
        return false;
    }

    let coff = 4;
    let machine = match u16_at(&header, coff) {
        Some(m) => m,
        None => return false,
    };

    // Only accept modules this process can actually load. This also keeps the
    // 32-bit SysWOW64 copies out of the candidate list on a 64-bit build.
    let host_machine = if cfg!(target_arch = "x86_64") {
        0x8664
    } else if cfg!(target_arch = "x86") {
        0x014c
    } else if cfg!(target_arch = "aarch64") {
        0xaa64
    } else {
        0
    };
    if host_machine != 0 && machine != host_machine {
        return false;
    }

    let section_count = match u16_at(&header, coff + 2) {
        Some(n) => n as usize,
        None => return false,
    };
    let optional_size = match u16_at(&header, coff + 16) {
        Some(n) => n as usize,
        None => return false,
    };

    let optional = coff + 20;
    let magic = match u16_at(&header, optional) {
        Some(m) => m,
        None => return false,
    };
    let data_directory = match magic {
        0x10b => optional + 96,  // PE32
        0x20b => optional + 112, // PE32+
        _ => return false,
    };

    let export_rva = match u32_at(&header, data_directory) {
        Some(v) => v,
        None => return false,
    };
    let export_size = match u32_at(&header, data_directory + 4) {
        Some(v) => v,
        None => return false,
    };
    if export_rva == 0 || export_size == 0 {
        return false;
    }

    let section_table = optional + optional_size;
    let mut sections = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let entry = section_table + index * 40;
        let vsize = match u32_at(&header, entry + 8) {
            Some(v) => v,
            None => return false,
        };
        let va = match u32_at(&header, entry + 12) {
            Some(v) => v,
            None => return false,
        };
        let raw_size = match u32_at(&header, entry + 16) {
            Some(v) => v,
            None => return false,
        };
        let raw_ptr = match u32_at(&header, entry + 20) {
            Some(v) => v,
            None => return false,
        };
        sections.push((va, vsize, raw_size, raw_ptr));
    }

    // IMAGE_EXPORT_DIRECTORY: NumberOfNames at +24, AddressOfNames at +32.
    let export_offset = match rva_to_offset(&sections, export_rva) {
        Some(off) => off,
        None => return false,
    };
    if file.seek(SeekFrom::Start(export_offset)).is_err() {
        return false;
    }
    let mut export_dir = [0u8; 40];
    if file.read_exact(&mut export_dir).is_err() {
        return false;
    }
    let name_count = match u32_at(&export_dir, 24) {
        Some(n) if n > 0 && n <= 65_536 => n as usize,
        _ => return false,
    };
    let names_rva = match u32_at(&export_dir, 32) {
        Some(v) => v,
        None => return false,
    };

    let names_offset = match rva_to_offset(&sections, names_rva) {
        Some(off) => off,
        None => return false,
    };
    if file.seek(SeekFrom::Start(names_offset)).is_err() {
        return false;
    }
    let mut name_rvas = vec![0u8; name_count * 4];
    if file.read_exact(&mut name_rvas).is_err() {
        return false;
    }

    for index in 0..name_count {
        let rva = u32::from_le_bytes([
            name_rvas[index * 4],
            name_rvas[index * 4 + 1],
            name_rvas[index * 4 + 2],
            name_rvas[index * 4 + 3],
        ]);
        let offset = match rva_to_offset(&sections, rva) {
            Some(off) => off,
            None => continue,
        };
        if file.seek(SeekFrom::Start(offset)).is_err() {
            continue;
        }
        let mut name = [0u8; 128];
        let read = file.read(&mut name).unwrap_or(0);
        let end = name[..read].iter().position(|&b| b == 0).unwrap_or(read);
        if &name[..end] == b"C_GetFunctionList" {
            return true;
        }
    }
    false
}

/// In-process cache: discovery is expensive (directory walk + PE reads) and
/// `auto()` used to repeat it on every call, including every hot-plug poll.
static MODULE_CACHE: std::sync::Mutex<Option<Vec<String>>> = std::sync::Mutex::new(None);

fn discover_modules_impl() -> Vec<String> {
    let mut cache = MODULE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(modules) = cache.as_ref() {
        return modules.clone();
    }
    let modules = scan_modules_uncached();
    *cache = Some(modules.clone());
    modules
}

/// Force a fresh scan (used by the GUI's "Refresh list" action).
#[allow(dead_code)]
pub fn rescan_modules() -> Vec<String> {
    let modules = scan_modules_uncached();
    *MODULE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(modules.clone());
    modules
}

fn scan_modules_uncached() -> Vec<String> {
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

/// Modules that timed out while being probed. Some drivers hang forever when a
/// reader is present but no card is inserted; skipping them for the session
/// keeps hot-plug polling responsive. `clear_unresponsive` (Settings → Refresh
/// list) re-enables them.
static UNRESPONSIVE: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);

fn is_unresponsive(path: &str) -> bool {
    let guard = UNRESPONSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .as_ref()
        .map(|set| set.contains(&path.to_ascii_lowercase()))
        .unwrap_or(false)
}

fn mark_unresponsive(path: &str) {
    let mut guard = UNRESPONSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get_or_insert_with(std::collections::HashSet::new)
        .insert(path.to_ascii_lowercase());
}

/// Allow previously unresponsive drivers to be probed again.
#[allow(dead_code)]
pub fn clear_unresponsive() {
    let mut guard = UNRESPONSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get_or_insert_with(std::collections::HashSet::new)
        .clear();
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
    let mut modules = discover_modules_impl();

    // Fast path: try the module that worked last time before probing all of
    // the others. If it no longer has the token, `auto()` continues down the
    // list as usual.
    if let Some(last) = last_module_hint() {
        if let Some(index) = modules
            .iter()
            .position(|module| module.eq_ignore_ascii_case(&last))
        {
            let module = modules.remove(index);
            modules.insert(0, module);
        } else {
            modules.insert(0, last);
        }
    }
    modules
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
    /// Detect a module and return it together with its token list.
    ///
    /// Every candidate is probed on its own thread with a timeout. Some vendor
    /// drivers block indefinitely when a reader is present but no card is
    /// inserted; without the timeout detection would never finish. A driver
    /// that times out is skipped for the rest of the session, and the user can
    /// explicitly retry from Settings.
    pub fn auto_connected(
        timeout: std::time::Duration,
    ) -> Result<(String, Self, serde_json::Value), String> {
        let candidates = default_module_paths();
        if candidates.is_empty() {
            return Err("No PKCS#11 module found (set VYOMKUNJI_PKCS11_MODULE)".to_string());
        }

        let mut last_err = String::new();
        let mut fallback: Option<(String, Self, serde_json::Value)> = None;

        for path in candidates {
            if !Path::new(&path).exists() || is_unresponsive(&path) {
                continue;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            let worker_path = path.clone();
            std::thread::spawn(move || {
                let result = (|| {
                    let manager = Self::new(&worker_path)?;
                    let tokens = manager.list_tokens()?;
                    Ok::<_, String>((manager, tokens))
                })();
                let _ = tx.send(result);
            });

            match rx.recv_timeout(timeout) {
                Ok(Ok((manager, tokens))) => {
                    let has_certs = tokens
                        .as_array()
                        .map(|items| !items.is_empty())
                        .unwrap_or(false);
                    if has_certs {
                        return Ok((path, manager, tokens));
                    }
                    // Keep the first module that loaded but exposed no
                    // certificates, so a scanned-but-empty token still reports
                    // a useful status instead of "no module".
                    if fallback.is_none() {
                        fallback = Some((path, manager, tokens));
                    }
                }
                Ok(Err(e)) => {
                    if last_err.is_empty() {
                        last_err = format!("{}: {}", path, e);
                    }
                }
                Err(_) => {
                    mark_unresponsive(&path);
                    if last_err.is_empty() {
                        last_err = format!("{}: driver did not respond", path);
                    }
                }
            }
        }

        if let Some(fallback) = fallback {
            return Ok(fallback);
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

    /// Cheap token-presence fingerprint: label + serial for every slot with a
    /// token. Unlike [`Self::list_tokens`] this does not read certificates, so
    /// it is safe to call from the UI thread on a timer.
    #[allow(dead_code)]
    pub fn token_signature(&self) -> Result<String, String> {
        let slots = self
            .pkcs11
            .get_slots_with_token()
            .map_err(|e| e.to_string())?;
        let mut parts = Vec::with_capacity(slots.len());
        for slot in slots {
            if let Ok(info) = self.pkcs11.get_token_info(slot) {
                parts.push(format!(
                    "{}:{}",
                    info.label().trim(),
                    info.serial_number().trim()
                ));
            }
        }
        Ok(parts.join("|"))
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
