#![allow(dead_code)] // functions are shared across the host and GUI binaries
// Local TCP bridge between the native messaging host and the GUI app.
// Messages are compact JSON, newline-delimited (serde_json escapes newlines).
//
// The bridge is authenticated with a per-session random token stored
// next to the port file. Both files live under the current user's
// %APPDATA% (which Windows protects with a per-user ACL), so another
// local user cannot read them. A same-user process still cannot issue
// a request without first reading the token file.
use rand::RngCore;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

const TOKEN_BYTES: usize = 32;

fn app_dir() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join("VyomKunji"))
}

fn port_file_path() -> Option<PathBuf> {
    let dir = app_dir()?;
    let current = dir.join("gui.port");
    if current.exists() {
        return Some(current);
    }
    let appdata = std::env::var("APPDATA").ok()?;
    let legacy = PathBuf::from(appdata).join("Signer").join("gui.port");
    if legacy.exists() {
        return Some(legacy);
    }
    Some(current)
}

fn token_file_path() -> Option<PathBuf> {
    let dir = app_dir()?;
    let current = dir.join("gui.token");
    if current.exists() {
        return Some(current);
    }
    let appdata = std::env::var("APPDATA").ok()?;
    let legacy = PathBuf::from(appdata).join("Signer").join("gui.token");
    if legacy.exists() {
        return Some(legacy);
    }
    Some(current)
}

fn random_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Constant-time-ish comparison so a token guess does not leak through timing.
fn tokens_match(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn write_port(port: u16) -> std::io::Result<()> {
    let dir = app_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("gui.port"), format!("{}", port))?;

    // also write to legacy path so older scripts continue to find it
    if let Ok(appdata) = std::env::var("APPDATA") {
        let legacy_dir = PathBuf::from(appdata).join("Signer");
        let _ = std::fs::create_dir_all(&legacy_dir);
        let _ = std::fs::write(legacy_dir.join("gui.port"), format!("{}", port));
    }
    Ok(())
}

fn write_token(token: &str) -> std::io::Result<()> {
    let dir = app_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("gui.token"), token)?;

    if let Ok(appdata) = std::env::var("APPDATA") {
        let legacy_dir = PathBuf::from(appdata).join("Signer");
        let _ = std::fs::create_dir_all(&legacy_dir);
        let _ = std::fs::write(legacy_dir.join("gui.token"), token);
    }
    Ok(())
}

pub fn read_port() -> Option<u16> {
    let path = port_file_path()?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_token() -> Option<String> {
    let path = token_file_path()?;
    let token = std::fs::read_to_string(path).ok()?.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

/// Remove the endpoint files when the GUI exits, so a stale port/token pair
/// cannot be reused by a later process that happens to get the same port.
pub fn clear_endpoint() {
    for name in ["gui.port", "gui.token"] {
        if let Some(path) = app_dir().map(|dir| dir.join(name)) {
            let _ = std::fs::remove_file(path);
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            let _ = std::fs::remove_file(PathBuf::from(appdata).join("Signer").join(name));
        }
    }
}

/// Clear the endpoint only if it still belongs to `port`. Guards against one
/// GUI instance deleting a newer instance's endpoint during shutdown.
pub fn clear_endpoint_if_port(port: u16) {
    if read_port() == Some(port) {
        clear_endpoint();
    }
}

/// Client (native host): send an authenticated request to the GUI and wait for
/// its response.
pub fn request_gui(req: &Value, timeout: Duration) -> Option<Value> {
    let port = read_port()?;
    let token = read_token()?;
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    let mut req = req.clone();
    if let Some(obj) = req.as_object_mut() {
        obj.insert("token".to_string(), Value::String(token));
    }
    let mut line = serde_json::to_string(&req).ok()?;
    line.push('\n');
    stream.write_all(line.as_bytes()).ok()?;
    stream.flush().ok()?;
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp).ok()?;
    serde_json::from_str(resp.trim()).ok()
}

fn unauthorized() -> Value {
    serde_json::json!({
        "ok": false,
        "error": {
            "code": "UNAUTHORIZED",
            "message": "Missing or invalid bridge token"
        }
    })
}

/// Pure authentication check used by the bridge server and unit tests.
fn is_authorized(req: &Value, expected: &str) -> bool {
    let supplied = req.get("token").and_then(|v| v.as_str()).unwrap_or("");
    tokens_match(supplied, expected)
}

/// Spawn a GUI bridge server. Each authenticated request is forwarded to
/// `to_gui` (the GUI main thread) and the GUI's response is sent back on
/// `resp_tx`. Returns the listening port and the session token.
pub fn serve_gui(
    to_gui: std::sync::mpsc::Sender<(Value, std::sync::mpsc::Sender<Value>)>,
) -> std::io::Result<(u16, String)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let token = random_token();
    // Write the token before publishing the port: a host that reads the new
    // port must never pair it with the previous session's token.
    write_token(&token)?;
    write_port(port)?;

    let expected = token.clone();
    std::thread::spawn(move || {
        loop {
            let (stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let to_gui = to_gui.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                });
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let req: Value = match serde_json::from_str(line.trim()) {
                    Ok(v) => v,
                    Err(_) => return,
                };

                if !is_authorized(&req, &expected) {
                    let mut out = stream;
                    let mut s = serde_json::to_string(&unauthorized()).unwrap_or_default();
                    s.push('\n');
                    let _ = out.write_all(s.as_bytes());
                    let _ = out.flush();
                    return;
                }

                let (resp_tx, resp_rx) = std::sync::mpsc::channel::<Value>();
                if to_gui.send((req.clone(), resp_tx)).is_err() {
                    return;
                }
                let response = resp_rx.recv_timeout(Duration::from_secs(120)).unwrap_or_else(|_| {
                serde_json::json!({"ok": false, "error": {"code": "USER_CANCELLED", "message": "Timed out"}})
            });
                let mut out = stream;
                let mut s = serde_json::to_string(&response).unwrap_or_default();
                s.push('\n');
                let _ = out.write_all(s.as_bytes());
                let _ = out.flush();
            });
        }
    });

    Ok((port, token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_is_exact() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc123", "abc124"));
        assert!(!tokens_match("abc123", "abc1234"));
        assert!(!tokens_match("", "abc"));
        assert!(tokens_match("", ""));
    }

    #[test]
    fn random_tokens_are_hex_and_unique() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), TOKEN_BYTES * 2);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn unauthenticated_request_is_rejected() {
        // Exercise the exact check used by `serve_gui` without binding a socket.
        assert!(!tokens_match("wrong-token", "right-token"));
        assert!(!is_authorized(&serde_json::json!({}), "right-token"));
        assert!(!is_authorized(
            &serde_json::json!({"token": "wrong-token"}),
            "right-token"
        ));
        assert!(is_authorized(
            &serde_json::json!({"token": "right-token"}),
            "right-token"
        ));
    }
}
