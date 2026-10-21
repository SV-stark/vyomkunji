// Dev self-test: reach the token, enumerate certs, and sign a test payload.
// Run: cargo run --bin test_sign   (then enter the token PIN when prompted)
#![allow(dead_code)]
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use std::io::Write;

#[path = "../cms_sign.rs"]
mod cms_sign;
#[path = "../pkcs11.rs"]
mod pkcs11;

fn main() {
    let (module, tm, tokens) = match pkcs11::TokenManager::auto_connected(pkcs11::PROBE_TIMEOUT) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FAILED: {}", e);
            std::process::exit(1);
        }
    };
    println!("module: {}", module);

    for t in tokens.as_array().unwrap() {
        println!("TOKEN {} ({})", t["label"], t["serial"]);
        for c in t["certs"].as_array().unwrap() {
            println!("  cert: {} | id: {}", c["cn"], c["id"]);
        }
    }

    print!("PIN: ");
    std::io::stdout().flush().unwrap();
    let mut pin = String::new();
    std::io::stdin().read_line(&mut pin).unwrap();
    let pin = pin.trim();

    let cn_filter = std::env::var("SIGNER_TEST_CN").ok();
    let data = b"SIGNER-SELFTEST-PAYLOAD";
    println!("signing {} bytes with SHA256_RSA_PKCS...", data.len());
    match tm.sign(pin, data, cn_filter.as_deref(), false) {
        Ok(r) => {
            let sig = r["signature"].as_str().unwrap();
            println!(
                "SIGN OK: signature {} bytes (base64 {} chars)",
                B64.decode(sig).unwrap().len(),
                sig.len()
            );
            println!("certId: {}", r["certId"]);
            if let Some(cert) = r.get("cert").and_then(|v| v.as_str()) {
                println!("cert DER: base64 {} chars", cert.len());
            }
        }
        Err(e) => println!("SIGN FAILED: {}", e),
    }
}
