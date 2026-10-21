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

fn default_module_paths() -> Vec<String> {
    if let Ok(p) =
        std::env::var("VYOMKUNJI_PKCS11_MODULE").or_else(|_| std::env::var("SIGNER_PKCS11_MODULE"))
    {
        return vec![p];
    }
    let mut v = vec![
        r"C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll".to_string(),
        r"C:\Windows\SysWOW64\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll".to_string(),
    ];
    // generic candidates
    v.push(r"C:\Windows\System32\WDPKCS.dll".to_string());
    v
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
    pub fn auto() -> Result<(String, Self), String> {
        for p in default_module_paths() {
            if !Path::new(&p).exists() {
                continue;
            }
            match Self::new(&p) {
                Ok(m) => return Ok((p, m)),
                Err(e) => return Err(format!("{}: {}", p, e)),
            }
        }
        Err("No PKCS#11 module found (set SIGNER_PKCS11_MODULE)".to_string())
    }

    pub fn new(module_path: &str) -> Result<Self, String> {
        let pk = Pkcs11::new(module_path).map_err(|e| e.to_string())?;
        pk.initialize(CInitializeArgs::OsThreads)
            .map_err(|e| e.to_string())?;
        Ok(Self { pkcs11: pk })
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
