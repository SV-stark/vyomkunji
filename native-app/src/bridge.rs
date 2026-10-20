#![allow(dead_code)] // functions are shared across the host and GUI binaries
                     // Local TCP bridge between the native messaging host and the GUI app.
                     // Messages are compact JSON, newline-delimited (serde_json escapes newlines).
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

pub fn port_file_path() -> Option<std::path::PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .map(|a| std::path::PathBuf::from(a).join("Signer").join("gui.port"))
}

pub fn write_port(port: u16) -> std::io::Result<()> {
    let path = port_file_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{}", port))
}

pub fn read_port() -> Option<u16> {
    let path = port_file_path()?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Client: send a request to the GUI and wait for its response.
pub fn request_gui(req: &Value, timeout: Duration) -> Option<Value> {
    let port = read_port()?;
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let mut line = serde_json::to_string(req).ok()?;
    line.push('\n');
    stream.write_all(line.as_bytes()).ok()?;
    stream.flush().ok()?;
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp).ok()?;
    serde_json::from_str(resp.trim()).ok()
}

/// Spawn a GUI bridge server. Each incoming request is forwarded to `to_gui`
/// (the GUI main thread) and the GUI's response is sent back on `resp_tx`.
pub fn serve_gui(
    to_gui: std::sync::mpsc::Sender<(Value, std::sync::mpsc::Sender<Value>)>,
) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    write_port(port)?;

    std::thread::spawn(move || loop {
        let (stream, _) = match listener.accept() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let to_gui = to_gui.clone();
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
    });

    Ok(port)
}
