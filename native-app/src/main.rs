mod bridge;
mod cms_sign;
mod pkcs11;
mod timeutil;

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use pkcs11::TokenManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read, Write};
use std::time::Duration;
use timeutil::now_iso;

#[derive(Deserialize)]
struct Req {
    id: String,
    #[serde(rename = "type")]
    rtype: String,
    #[serde(default)]
    portal: Option<String>,
    /// Real tab URL supplied by the extension background script. Browser
    /// requests always carry this at the top level.
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    payload: Option<Value>,
}

#[derive(Serialize, Default)]
struct Resp {
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_connected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cert_selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tokens: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cert: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
    /// Extra fields (pkcs7, chain, mechanism, hash, keyType, ...) are forwarded
    /// without forcing every call site to know about them.
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

/// Copy optional signing metadata from a GUI/direct-token response into the
/// protocol response.
fn copy_signing_metadata(out: &mut Resp, source: &Value) {
    for key in [
        "pkcs7",
        "chain",
        "mechanism",
        "hash",
        "keyType",
        "certId",
        "certCN",
    ] {
        if let Some(value) = source.get(key) {
            out.extra.insert(key.to_string(), value.clone());
        }
    }
}

fn read_msg() -> Option<String> {
    let mut len_buf = [0u8; 4];
    let mut stdin = io::stdin();
    if stdin.read_exact(&mut len_buf).is_err() {
        return None;
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > 10_000_000 {
        return None;
    }
    let mut buf = vec![0u8; len];
    if stdin.read_exact(&mut buf).is_err() {
        return None;
    }
    String::from_utf8(buf).ok()
}

fn write_msg(s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;
    let mut stdout = io::stdout();
    let _ = stdout.write_all(&len.to_le_bytes());
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

fn main() {
    let mock = std::env::var("VYOMKUNJI_MOCK")
        .or_else(|_| std::env::var("SIGNER_MOCK"))
        .is_ok();

    while let Some(msg) = read_msg() {
        let req: Req = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                write_msg(
                    &serde_json::to_string(&Resp {
                        id: "".to_string(),
                        ok: false,
                        error: Some(
                            serde_json::json!({"code":"BAD_REQUEST","message":e.to_string()}),
                        ),
                        ..Default::default()
                    })
                    .unwrap(),
                );
                continue;
            }
        };

        let resp = handle(&req, mock);
        write_msg(&serde_json::to_string(&resp).unwrap());
    }
}

fn handle(req: &Req, mock: bool) -> Resp {
    let err = |code: &str, message: String| Resp {
        id: req.id.clone(),
        ok: false,
        error: Some(serde_json::json!({"code":code,"message":message})),
        ..Default::default()
    };

    if req.rtype == "ping" {
        return Resp {
            id: req.id.clone(),
            ok: true,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..Default::default()
        };
    }

    if mock {
        return handle_mock(req, err);
    }

    // Real PKCS#11 path. Initialise lazily per operation so a signature that
    // the GUI will handle never pays for (or blocks on) a token probe here.
    match req.rtype.as_str() {
        "status" => {
            if let Some(resp) = bridge::request_gui(
                &serde_json::json!({ "type": "status" }),
                Duration::from_millis(1500),
            ) && resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
            {
                return Resp {
                    id: req.id.clone(),
                    ok: true,
                    module: resp
                        .get("module")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    token_connected: resp.get("token_connected").and_then(|v| v.as_bool()),
                    cert_selected: resp.get("cert_selected").and_then(|v| v.as_bool()),
                    ..Default::default()
                };
            }
            let (module, _tm, tokens) = match TokenManager::auto_connected(pkcs11::PROBE_TIMEOUT) {
                Ok(v) => v,
                Err(e) => return err("NO_PKCS11_MODULE", e),
            };
            let cert_selected = tokens
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false);
            Resp {
                id: req.id.clone(),
                ok: true,
                module: Some(module),
                token_connected: Some(true),
                cert_selected: Some(cert_selected),
                ..Default::default()
            }
        }
        "listTokens" => {
            if let Some(resp) = bridge::request_gui(
                &serde_json::json!({ "type": "listTokens" }),
                Duration::from_millis(2000),
            ) && resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
            {
                return Resp {
                    id: req.id.clone(),
                    ok: true,
                    module: resp
                        .get("module")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    token_connected: resp.get("token_connected").and_then(|v| v.as_bool()),
                    tokens: resp.get("tokens").cloned(),
                    ..Default::default()
                };
            }
            let (module, _tm, tokens) = match TokenManager::auto_connected(pkcs11::PROBE_TIMEOUT) {
                Ok(v) => v,
                Err(e) => return err("NO_PKCS11_MODULE", e),
            };
            Resp {
                id: req.id.clone(),
                ok: true,
                module: Some(module),
                token_connected: Some(true),
                tokens: Some(tokens),
                ..Default::default()
            }
        }
        "selectCert" => {
            let payload = req.payload.clone().unwrap_or(Value::Null);
            let cert_cn = payload
                .get("certCN")
                .and_then(|v| v.as_str())
                .or_else(|| payload.get("certId").and_then(|v| v.as_str()));
            let _ = bridge::request_gui(
                &serde_json::json!({
                    "type": "selectCert",
                    "certCN": cert_cn
                }),
                Duration::from_millis(1000),
            );
            Resp {
                id: req.id.clone(),
                ok: true,
                ..Default::default()
            }
        }
        "sign" => {
            let payload = req.payload.clone().unwrap_or(Value::Null);
            let data_b64 = payload.get("data").and_then(|v| v.as_str()).unwrap_or("");
            let bytes = match B64.decode(data_b64) {
                Ok(b) => b,
                Err(e) => return err("BAD_BASE64", e.to_string()),
            };
            let pin = payload.get("pin").and_then(|v| v.as_str()).unwrap_or("");
            let cn = payload.get("certCN").and_then(|v| v.as_str());
            let portal = req
                .portal
                .as_deref()
                .or_else(|| payload.get("portal").and_then(|v| v.as_str()))
                .unwrap_or("portal");
            let already_hashed = payload
                .get("alreadyHashed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let hash = pkcs11::HashAlgorithm::parse(
                payload
                    .get("hashAlg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SHA256"),
            );
            let pkcs7 = payload
                .get("pkcs7")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let origin = req
                .origin
                .as_deref()
                .or_else(|| payload.get("origin").and_then(|v| v.as_str()))
                .unwrap_or("")
                .chars()
                .take(512)
                .collect::<String>();

            // Preferred: route to the GUI for user approval + PIN entry.
            let gui_req = serde_json::json!({
                "portal": portal,
                "origin": origin,
                "payload": {
                    "data": data_b64,
                    "certCN": cn,
                    "alreadyHashed": already_hashed,
                    "hashAlg": hash.name(),
                    "pkcs7": pkcs7
                }
            });
            if let Some(resp) = bridge::request_gui(&gui_req, Duration::from_secs(120)) {
                if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    // Responses without a signature are status/approval errors
                    // even if ok=true; keep the protocol honest.
                    if resp.get("signature").and_then(|v| v.as_str()).is_none()
                        && resp.get("pkcs7").and_then(|v| v.as_str()).is_none()
                    {
                        return err(
                            "SIGN_FAILED",
                            "GUI returned no signature material".to_string(),
                        );
                    }
                    let mut out = Resp {
                        id: req.id.clone(),
                        ok: true,
                        token_connected: Some(true),
                        cert_selected: Some(true),
                        signature: resp
                            .get("signature")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        cert: resp
                            .get("cert")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        signed_at: Some(
                            resp.get("signedAt")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        ),
                        ..Default::default()
                    };
                    copy_signing_metadata(&mut out, &resp);
                    return out;
                } else {
                    return err(
                        resp.get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("SIGN_FAILED"),
                        resp.get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    );
                }
            }

            // Fallback: sign directly only for local/headless callers that
            // explicitly opt in. A browser-originated request must always go
            // through the GUI approval path, even if it carries a PIN.
            if pin.is_empty() {
                return err(
                    "PIN_REQUIRED",
                    "GUI not running and no PIN provided".to_string(),
                );
            }
            let allow_headless = std::env::var("VYOMKUNJI_ALLOW_HEADLESS_PIN")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if !allow_headless || !origin.is_empty() {
                return err(
                    "GUI_UNAVAILABLE",
                    "GUI is not running; direct PIN signing is disabled".to_string(),
                );
            }
            let (module, tm, _tokens) = match TokenManager::auto_connected(pkcs11::PROBE_TIMEOUT) {
                Ok(v) => v,
                Err(e) => return err("NO_PKCS11_MODULE", e),
            };
            let opts = pkcs11::SignOptions {
                cert_cn: cn,
                already_hashed,
                hash,
                pkcs7,
            };
            match tm.sign_with(pin, &bytes, &opts) {
                Ok(r) => {
                    let mut out = Resp {
                        id: req.id.clone(),
                        ok: true,
                        module: Some(module),
                        token_connected: Some(true),
                        cert_selected: Some(true),
                        signature: r
                            .get("signature")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        cert: r
                            .get("cert")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        signed_at: Some(now_iso()),
                        ..Default::default()
                    };
                    copy_signing_metadata(&mut out, &r);
                    out
                }
                Err(e) => err("SIGN_FAILED", e),
            }
        }
        other => err("UNKNOWN_TYPE", format!("unknown type {}", other)),
    }
}

// Legacy mock mode for development without a token.
fn handle_mock(req: &Req, err: impl Fn(&str, String) -> Resp) -> Resp {
    match req.rtype.as_str() {
        "status" => Resp {
            id: req.id.clone(),
            ok: true,
            module: Some("mock".to_string()),
            token_connected: Some(true),
            cert_selected: Some(true),
            ..Default::default()
        },
        "listTokens" => Resp {
            id: req.id.clone(),
            ok: true,
            module: Some("mock".to_string()),
            token_connected: Some(true),
            tokens: Some(serde_json::json!([{
                "id":"mock-token-1","label":"Mock DSC Token","serial":"MOCK123",
                "certs":[{"id":"mock-cert-1","subject":"CN=Mock Signer","cn":"Mock Signer"}]
            }])),
            ..Default::default()
        },
        "selectCert" => Resp {
            id: req.id.clone(),
            ok: true,
            ..Default::default()
        },
        _ => err("UNKNOWN_TYPE", format!("unknown type {}", req.rtype)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(rtype: &str, payload: Option<Value>) -> Req {
        Req {
            id: "test-id".to_string(),
            rtype: rtype.to_string(),
            portal: None,
            origin: None,
            payload,
        }
    }

    #[test]
    fn ping_reports_the_cargo_package_version() {
        let response = handle(&request("ping", None), true);
        assert!(response.ok);
        assert_eq!(response.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn select_cert_returns_ok() {
        let response = handle(
            &request("selectCert", Some(serde_json::json!({"certCN": "Test"}))),
            true,
        );
        assert!(response.ok);
        assert_eq!(response.id, "test-id");
    }

    #[test]
    fn unknown_request_types_are_rejected() {
        let response = handle(&request("definitely-not-real", None), true);
        assert!(!response.ok);
        let error = response.error.expect("error object");
        assert_eq!(error["code"], "UNKNOWN_TYPE");
    }

    #[test]
    fn mock_token_listing_is_stable() {
        let response = handle(&request("listTokens", None), true);
        assert!(response.ok);
        let tokens = response.tokens.expect("tokens");
        assert_eq!(tokens[0]["serial"], "MOCK123");
    }

    #[test]
    fn top_level_portal_and_origin_are_parsed() {
        let json = r#"{
            "id": "id",
            "type": "sign",
            "portal": "GST",
            "origin": "https://gst.gov.in/return",
            "payload": { "data": "YWJj" }
        }"#;
        let request: Req = serde_json::from_str(json).expect("valid request");
        assert_eq!(request.portal.as_deref(), Some("GST"));
        assert_eq!(request.origin.as_deref(), Some("https://gst.gov.in/return"));
    }

    #[test]
    fn signing_metadata_is_flattened_into_the_response() {
        let mut response = Resp {
            id: "id".to_string(),
            ok: true,
            ..Default::default()
        };
        let source = serde_json::json!({
            "pkcs7": "Y21z",
            "chain": ["Y2VydA=="],
            "mechanism": "RSA_PKCS_CMS",
            "hash": "SHA256",
            "keyType": "RSA",
            "certId": "aWQ=",
            "certCN": "Test Signer",
            "ignored": "value"
        });
        copy_signing_metadata(&mut response, &source);
        let value = serde_json::to_value(&response).expect("serialize");
        assert_eq!(value["pkcs7"], "Y21z");
        assert_eq!(value["chain"][0], "Y2VydA==");
        assert_eq!(value["hash"], "SHA256");
        assert!(value.get("ignored").is_none());
    }
}
