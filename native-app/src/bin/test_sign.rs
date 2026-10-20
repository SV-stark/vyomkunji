// Dev self-test: reach the token, enumerate certs, and sign a test payload.
// Run: cargo run --bin test_sign   (then enter the token PIN when prompted)
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::io::Write;

#[path = "../pkcs11.rs"]
mod pkcs11;

fn main() {
    let (module, tm) = match pkcs11::TokenManager::auto() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FAILED: {}", e);
            std::process::exit(1);
        }
    };
    println!("module: {}", module);

    let tokens = tm.list_tokens().expect("list");
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

    let data = b"SIGNER-SELFTEST-PAYLOAD";
    println!("signing {} bytes with SHA256_RSA_PKCS...", data.len());
    match tm.sign(pin, data, Some("SUYASH SINGH VERMA")) {
        Ok(r) => {
            let sig = r["signature"].as_str().unwrap();
            println!(
                "SIGN OK: signature {} bytes (base64 {} chars)",
                B64.decode(sig).unwrap().len(),
                sig.len()
            );
            println!("certId: {}", r["certId"]);
        }
        Err(e) => println!("SIGN FAILED: {}", e),
    }
}
