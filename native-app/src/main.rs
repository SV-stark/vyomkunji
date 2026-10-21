mod bridge;
mod pkcs11;
mod timeutil;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
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
    payload: Option<Value>,
}

#[derive(Serialize)]
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
                        version: None,
                        module: None,
                        token_connected: None,
                        cert_selected: None,
                        tokens: None,
                        signature: None,
                        cert: None,
                        signed_at: None,
                        error: Some(
                            serde_json::json!({"code":"BAD_REQUEST","message":e.to_string()}),
                        ),
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
        version: None,
        module: None,
        token_connected: None,
        cert_selected: None,
        tokens: None,
        signature: None,
        cert: None,
        signed_at: None,
        error: Some(serde_json::json!({"code":code,"message":message})),
    };

    if req.rtype == "ping" {
        return Resp {
            id: req.id.clone(),
            ok: true,
            version: Some("0.2.0".to_string()),
            module: None,
            token_connected: None,
            cert_selected: None,
            tokens: None,
            signature: None,
            cert: None,
            signed_at: None,
            error: None,
        };
    }

    if mock {
        return handle_mock(req, err);
    }

    // Real PKCS#11 path
    let (module, tm) = match TokenManager::auto() {
        Ok(v) => v,
        Err(e) => return err("NO_PKCS11_MODULE", e),
    };

    match req.rtype.as_str() {
        "status" => match tm.list_tokens() {
            Ok(t) => Resp {
                id: req.id.clone(),
                ok: true,
                version: None,
                module: Some(module),
                token_connected: Some(true),
                cert_selected: Some(t.as_array().map(|a| !a.is_empty()).unwrap_or(false)),
                tokens: None,
                signature: None,
                cert: None,
                signed_at: None,
                error: None,
            },
            Err(e) => err("TOKEN_ERROR", e),
        },
        "listTokens" => match tm.list_tokens() {
            Ok(t) => Resp {
                id: req.id.clone(),
                ok: true,
                version: None,
                module: Some(module),
                token_connected: Some(true),
                cert_selected: None,
                tokens: Some(t),
                signature: None,
                cert: None,
                signed_at: None,
                error: None,
            },
            Err(e) => err("TOKEN_ERROR", e),
        },
        "sign" => {
            let payload = req.payload.clone().unwrap_or(Value::Null);
            let data_b64 = payload.get("data").and_then(|v| v.as_str()).unwrap_or("");
            let bytes = match B64.decode(data_b64) {
                Ok(b) => b,
                Err(e) => return err("BAD_BASE64", e.to_string()),
            };
            let pin = payload.get("pin").and_then(|v| v.as_str()).unwrap_or("");
            let cn = payload.get("certCN").and_then(|v| v.as_str());
            let portal = payload
                .get("portal")
                .and_then(|v| v.as_str())
                .unwrap_or("portal");
            let already_hashed = payload
                .get("alreadyHashed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Preferred: route to the GUI for user approval + PIN entry.
            let gui_req = serde_json::json!({
                "portal": portal,
                "payload": {
                    "data": data_b64,
                    "certCN": cn,
                    "alreadyHashed": already_hashed
                }
            });
            if let Some(resp) = bridge::request_gui(&gui_req, Duration::from_secs(120)) {
                if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Resp {
                        id: req.id.clone(),
                        ok: true,
                        version: None,
                        module: Some(module),
                        token_connected: Some(true),
                        cert_selected: Some(true),
                        tokens: None,
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
                        error: None,
                    };
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

            // Fallback: sign directly (requires PIN in the request).
            if pin.is_empty() {
                return err(
                    "PIN_REQUIRED",
                    "GUI not running and no PIN provided".to_string(),
                );
            }
            match tm.sign(pin, &bytes, cn, already_hashed) {
                Ok(r) => Resp {
                    id: req.id.clone(),
                    ok: true,
                    version: None,
                    module: Some(module),
                    token_connected: Some(true),
                    cert_selected: Some(true),
                    tokens: None,
                    signature: r
                        .get("signature")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    cert: r
                        .get("cert")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    signed_at: Some(now_iso()),
                    error: None,
                },
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
            version: None,
            module: Some("mock".to_string()),
            token_connected: Some(true),
            cert_selected: Some(true),
            tokens: None,
            signature: None,
            cert: None,
            signed_at: None,
            error: None,
        },
        "listTokens" => Resp {
            id: req.id.clone(),
            ok: true,
            version: None,
            module: Some("mock".to_string()),
            token_connected: Some(true),
            cert_selected: None,
            tokens: Some(serde_json::json!([{
                "id":"mock-token-1","label":"Mock DSC Token","serial":"MOCK123",
                "certs":[{"id":"mock-cert-1","subject":"CN=Mock Signer","cn":"Mock Signer"}]
            }])),
            signature: None,
            cert: None,
            signed_at: None,
            error: None,
        },
        _ => err("UNKNOWN_TYPE", format!("unknown type {}", req.rtype)),
    }
}
