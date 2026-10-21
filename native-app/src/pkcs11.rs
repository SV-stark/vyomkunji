// PKCS#11 token access for DSC signing.
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, CertificateType, KeyType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::path::Path;

/// Digest algorithms understood by the host protocol. SHA-256 is the default;
/// the others exist for portals that still require SHA-1 and for CAs that issue
/// SHA-384/512 certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HashAlgorithm {
    Sha1,
    #[default]
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    pub fn parse(value: &str) -> Self {
        match value
            .trim()
            .to_ascii_uppercase()
            .replace(['-', '_'], "")
            .as_str()
        {
            "SHA1" => Self::Sha1,
            "SHA384" => Self::Sha384,
            "SHA512" => Self::Sha512,
            _ => Self::Sha256,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha384 => "SHA384",
            Self::Sha512 => "SHA512",
        }
    }

    pub fn digest_len(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => Sha1::digest(data).to_vec(),
            Self::Sha256 => Sha256::digest(data).to_vec(),
            Self::Sha384 => Sha384::digest(data).to_vec(),
            Self::Sha512 => Sha512::digest(data).to_vec(),
        }
    }

    /// PKCS#1 v1.5 DigestInfo prefix for `Mechanism::RsaPkcs`.
    fn digest_info_prefix(self) -> &'static [u8] {
        match self {
            Self::Sha1 => &[
                0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04,
                0x14,
            ],
            Self::Sha256 => &[
                0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x01, 0x05, 0x00, 0x04, 0x20,
            ],
            Self::Sha384 => &[
                0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x02, 0x05, 0x00, 0x04, 0x30,
            ],
            Self::Sha512 => &[
                0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x03, 0x05, 0x00, 0x04, 0x40,
            ],
        }
    }

    fn digest_info(self, digest: &[u8]) -> Vec<u8> {
        let prefix = self.digest_info_prefix();
        let mut out = Vec::with_capacity(prefix.len() + digest.len());
        out.extend_from_slice(prefix);
        out.extend_from_slice(digest);
        out
    }

    fn rsa_mechanism(self) -> Mechanism<'static> {
        match self {
            Self::Sha1 => Mechanism::Sha1RsaPkcs,
            Self::Sha256 => Mechanism::Sha256RsaPkcs,
            Self::Sha384 => Mechanism::Sha384RsaPkcs,
            Self::Sha512 => Mechanism::Sha512RsaPkcs,
        }
    }

    fn ecdsa_mechanism(self) -> Mechanism<'static> {
        match self {
            Self::Sha1 => Mechanism::EcdsaSha1,
            Self::Sha256 => Mechanism::EcdsaSha256,
            Self::Sha384 => Mechanism::EcdsaSha384,
            Self::Sha512 => Mechanism::EcdsaSha512,
        }
    }

    fn rsa_name(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1_RSA_PKCS",
            Self::Sha256 => "SHA256_RSA_PKCS",
            Self::Sha384 => "SHA384_RSA_PKCS",
            Self::Sha512 => "SHA512_RSA_PKCS",
        }
    }

    fn ecdsa_name(self) -> &'static str {
        match self {
            Self::Sha1 => "ECDSA_SHA1",
            Self::Sha256 => "ECDSA_SHA256",
            Self::Sha384 => "ECDSA_SHA384",
            Self::Sha512 => "ECDSA_SHA512",
        }
    }

    pub(crate) fn oid(self) -> const_oid::ObjectIdentifier {
        const_oid::ObjectIdentifier::new_unwrap(match self {
            Self::Sha1 => "1.3.14.3.2.26",
            Self::Sha256 => "2.16.840.1.101.3.4.2.1",
            Self::Sha384 => "2.16.840.1.101.3.4.2.2",
            Self::Sha512 => "2.16.840.1.101.3.4.2.3",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyKind {
    Rsa,
    Ec,
}

impl KeyKind {
    fn label(self) -> &'static str {
        match self {
            Self::Rsa => "RSA",
            Self::Ec => "EC",
        }
    }
}

/// Options for a signing request. `sign()` keeps the historical SHA-256 /
/// non-CMS behaviour; `sign_with()` exposes the negotiated algorithm and the
/// detached CMS output.
#[derive(Clone, Debug)]
pub struct SignOptions<'a> {
    pub cert_cn: Option<&'a str>,
    pub already_hashed: bool,
    pub hash: HashAlgorithm,
    pub pkcs7: bool,
}

impl<'a> Default for SignOptions<'a> {
    fn default() -> Self {
        Self {
            cert_cn: None,
            already_hashed: false,
            hash: HashAlgorithm::Sha256,
            pkcs7: false,
        }
    }
}

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
#[derive(Clone)]
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
            pk.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
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
    /// If `already_hashed` is true, expects a digest of the selected hash
    /// algorithm (or a 51-byte SHA-256 DigestInfo structure) and signs with
    /// `Mechanism::RsaPkcs`.
    /// If `already_hashed` is false, hashes and signs in one token operation.
    #[allow(dead_code)]
    pub fn sign(
        &self,
        pin: &str,
        data: &[u8],
        cert_cn_filter: Option<&str>,
        already_hashed: bool,
    ) -> Result<Value, String> {
        self.sign_with(
            pin,
            data,
            &SignOptions {
                cert_cn: cert_cn_filter,
                already_hashed,
                ..SignOptions::default()
            },
        )
    }

    /// Signing entry point with explicit hash / CMS options.
    pub fn sign_with(&self, pin: &str, data: &[u8], opts: &SignOptions) -> Result<Value, String> {
        let slots = self
            .pkcs11
            .get_slots_with_token()
            .map_err(|e| e.to_string())?;
        if slots.is_empty() {
            return Err("no token found".to_string());
        }
        let mut last_err = "no token found".to_string();
        for slot in slots {
            match self.sign_in_slot(slot, pin, data, opts) {
                Ok(r) => return Ok(r),
                Err(e) => {
                    // A wrong PIN must not be retried on every slot: each retry
                    // can advance the token's PIN retry counter.
                    let lower = e.to_ascii_lowercase();
                    if lower.contains("pin") || lower.contains("login failed") {
                        return Err(e);
                    }
                    last_err = e;
                }
            }
        }
        Err(last_err)
    }

    fn sign_in_slot(
        &self,
        slot: cryptoki::slot::Slot,
        pin: &str,
        data: &[u8],
        opts: &SignOptions,
    ) -> Result<Value, String> {
        let session = self
            .pkcs11
            .open_ro_session(slot)
            .map_err(|e| e.to_string())?;
        session
            .login(UserType::User, Some(&AuthPin::new(pin.into())))
            .map_err(|e| format!("login failed: {}", e))?;

        // Read every certificate first: the leaf is selected below and the
        // remaining certificates are used to build the chain.
        let cert_template = vec![
            Attribute::Class(ObjectClass::CERTIFICATE),
            Attribute::CertificateType(CertificateType::X_509),
        ];
        let cert_objects = session
            .find_objects(&cert_template)
            .map_err(|e| e.to_string())?;

        let mut all: Vec<(Vec<u8>, Vec<u8>, CertInfo)> = Vec::new();
        for c in &cert_objects {
            let attrs = match session.get_attributes(*c, &[AttributeType::Value, AttributeType::Id])
            {
                Ok(v) => v,
                Err(_) => continue,
            };
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
            all.push((id, der, info));
        }

        let chosen_index = all
            .iter()
            .position(|(_, _, info)| {
                opts.cert_cn
                    .map(|filter| info.cn.to_lowercase().contains(&filter.to_lowercase()))
                    .unwrap_or(true)
            })
            .ok_or_else(|| match opts.cert_cn {
                Some(filter) => format!("no certificate matching '{}'", filter),
                None => "no certificate found".to_string(),
            })?;

        let (chosen_id, chosen_der, chosen_info) = {
            let (id, der, info) = &all[chosen_index];
            (id.clone(), der.to_vec(), info.clone())
        };
        let kind = Self::key_kind(&chosen_der);
        let chain = Self::certificate_chain(&chosen_der, &all);

        // Find a private signing key matching the chosen certificate id. Some
        // tokens do not expose CKA_ID on keys, so fall back to any signing key.
        let key_template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::KeyType(match kind {
                KeyKind::Rsa => KeyType::RSA,
                KeyKind::Ec => KeyType::EC,
            }),
            Attribute::Sign(true),
        ];
        let mut keys = Vec::new();
        if !chosen_id.is_empty() {
            let mut with_id = key_template.clone();
            with_id.push(Attribute::Id(chosen_id.clone()));
            keys = session.find_objects(&with_id).map_err(|e| e.to_string())?;
        }
        if keys.is_empty() {
            keys = session
                .find_objects(&key_template)
                .map_err(|e| e.to_string())?;
        }
        let key = keys
            .into_iter()
            .next()
            .ok_or_else(|| "no private signing key found".to_string())?;

        let mut response = json!({
            "cert": B64.encode(&chosen_der),
            "certId": B64.encode(&chosen_id),
            "certCN": chosen_info.cn,
            "hash": opts.hash.name(),
            "keyType": kind.label(),
        });

        if opts.already_hashed {
            let expected = opts.hash.digest_len();
            let wrapped_len = opts.hash.digest_info_prefix().len() + expected;
            let is_digest = data.len() == expected;
            let is_digest_info = kind == KeyKind::Rsa
                && data.len() == wrapped_len
                && data.starts_with(opts.hash.digest_info_prefix());
            if !is_digest && !is_digest_info {
                let _ = session.logout();
                return Err(format!(
                    "alreadyHashed expects a {expected}-byte {} digest{}",
                    opts.hash.name(),
                    if kind == KeyKind::Rsa {
                        format!(" or a {wrapped_len}-byte DigestInfo")
                    } else {
                        String::new()
                    }
                ));
            }
        }

        if opts.pkcs7 {
            if opts.already_hashed && data.len() != opts.hash.digest_len() {
                let _ = session.logout();
                return Err(format!(
                    "alreadyHashed with pkcs7 requires a {}-byte {} digest",
                    opts.hash.digest_len(),
                    opts.hash.name()
                ));
            }
            let content_digest = if opts.already_hashed {
                data.to_vec()
            } else {
                opts.hash.digest(data)
            };

            let kind_for_signer = kind;
            let hash = opts.hash;
            let cms = crate::cms_sign::build_detached_cms(
                &chosen_der,
                &chain,
                &content_digest,
                hash,
                kind,
                |attrs_digest| match kind_for_signer {
                    KeyKind::Rsa => {
                        let payload = hash.digest_info(attrs_digest);
                        session
                            .sign(&Mechanism::RsaPkcs, key, &payload)
                            .map_err(|e| format!("sign failed: {}", e))
                    }
                    KeyKind::Ec => session
                        .sign(&Mechanism::Ecdsa, key, attrs_digest)
                        .map_err(|e| format!("sign failed: {}", e)),
                },
            )
            .inspect_err(|_| {
                let _ = session.logout();
            })?;

            let _ = session.logout();
            response["pkcs7"] = json!(B64.encode(&cms));
            response["mechanism"] = json!(match kind {
                KeyKind::Rsa => "RSA_PKCS_CMS",
                KeyKind::Ec => "ECDSA_CMS",
            });
            response["chain"] = json!(chain.iter().map(|c| B64.encode(c)).collect::<Vec<_>>());
            return Ok(response);
        }

        let (sig, mechanism_name) = match (kind, opts.already_hashed) {
            (KeyKind::Rsa, true) => {
                let payload = if data.len() == opts.hash.digest_len() {
                    opts.hash.digest_info(data)
                } else {
                    data.to_vec()
                };
                let s = session
                    .sign(&Mechanism::RsaPkcs, key, &payload)
                    .map_err(|e| format!("sign failed: {}", e))?;
                (s, "RSA_PKCS".to_string())
            }
            (KeyKind::Rsa, false) => {
                let s = session
                    .sign(&opts.hash.rsa_mechanism(), key, data)
                    .map_err(|e| format!("sign failed: {}", e))?;
                (s, opts.hash.rsa_name().to_string())
            }
            (KeyKind::Ec, true) => {
                let s = session
                    .sign(&Mechanism::Ecdsa, key, data)
                    .map_err(|e| format!("sign failed: {}", e))?;
                (s, "ECDSA".to_string())
            }
            (KeyKind::Ec, false) => {
                let s = session
                    .sign(&opts.hash.ecdsa_mechanism(), key, data)
                    .map_err(|e| format!("sign failed: {}", e))?;
                (s, opts.hash.ecdsa_name().to_string())
            }
        };

        let _ = session.logout();
        response["signature"] = json!(B64.encode(&sig));
        response["mechanism"] = json!(mechanism_name);
        Ok(response)
    }

    /// Determine whether the leaf certificate carries an RSA or EC public key.
    fn key_kind(der: &[u8]) -> KeyKind {
        use x509_parser::prelude::*;
        match X509Certificate::from_der(der) {
            Ok((_, cert)) => {
                if cert.public_key().algorithm.algorithm.to_id_string() == "1.2.840.10045.2.1" {
                    KeyKind::Ec
                } else {
                    KeyKind::Rsa
                }
            }
            Err(_) => KeyKind::Rsa,
        }
    }

    /// Build the issuer chain from certificates already present on the token.
    /// The leaf is not included; the loop stops at a self-signed certificate or
    /// when no issuer is available. Cycles are prevented by the `chain`
    /// membership check.
    fn certificate_chain(leaf_der: &[u8], all: &[(Vec<u8>, Vec<u8>, CertInfo)]) -> Vec<Vec<u8>> {
        use x509_parser::prelude::*;
        let mut chain: Vec<Vec<u8>> = Vec::new();
        let mut current = leaf_der.to_vec();
        while let Ok((_, cert)) = X509Certificate::from_der(&current) {
            let issuer = cert.issuer().as_raw().to_vec();
            if issuer == cert.subject().as_raw() {
                break;
            }
            let next = all
                .iter()
                .map(|(_, der, _)| der)
                .find(|der| {
                    X509Certificate::from_der(der)
                        .map(|(_, c)| c.subject().as_raw() == issuer.as_slice())
                        .unwrap_or(false)
                })
                .cloned();
            let Some(next) = next else { break };
            if chain.iter().any(|c| c == &next) {
                break;
            }
            chain.push(next.clone());
            current = next;
        }
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hash_algorithm_names() {
        assert_eq!(HashAlgorithm::parse("sha-256"), HashAlgorithm::Sha256);
        assert_eq!(HashAlgorithm::parse("SHA1"), HashAlgorithm::Sha1);
        assert_eq!(HashAlgorithm::parse("SHA_384"), HashAlgorithm::Sha384);
        assert_eq!(HashAlgorithm::parse("sha512"), HashAlgorithm::Sha512);
        assert_eq!(HashAlgorithm::parse("unknown"), HashAlgorithm::Sha256);
    }

    #[test]
    fn digest_lengths_match_the_algorithm() {
        assert_eq!(HashAlgorithm::Sha1.digest(b"abc").len(), 20);
        assert_eq!(HashAlgorithm::Sha256.digest(b"abc").len(), 32);
        assert_eq!(HashAlgorithm::Sha384.digest(b"abc").len(), 48);
        assert_eq!(HashAlgorithm::Sha512.digest(b"abc").len(), 64);
    }

    #[test]
    fn sha256_matches_the_known_test_vector() {
        let digest = HashAlgorithm::Sha256.digest(b"abc");
        assert_eq!(
            digest,
            vec![
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
    }

    #[test]
    fn digest_info_wraps_the_digest_with_the_right_prefix() {
        for hash in [
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha384,
            HashAlgorithm::Sha512,
        ] {
            let digest = vec![0xAA; hash.digest_len()];
            let wrapped = hash.digest_info(&digest);
            assert_eq!(
                wrapped.len(),
                hash.digest_info_prefix().len() + digest.len()
            );
            assert!(wrapped.starts_with(hash.digest_info_prefix()));
            assert!(wrapped.ends_with(&digest));
            assert_eq!(wrapped[0], 0x30, "DigestInfo must be a SEQUENCE");
        }
    }
}
