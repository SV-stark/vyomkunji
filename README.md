# Signer — India G2C Digital Signing

Single browser extension (Chrome + Firefox) + native app (Rust/egui) for DSC signing
on Indian government portals. Priority: https://www.incometax.gov.in

## Architecture
```
Browser (Chrome/Firefox)  --native messaging-->  signer-host.exe  --localhost TCP-->  gui.exe (approval + PIN)
                                                             \--PKCS#11--> ProxKey token
```
- `signer-host.exe` — native messaging host; enumerates tokens and forwards sign requests to the GUI.
- `gui.exe` — egui app; shows the approval panel, PIN entry, and signs via PKCS#11.
- The GUI must be running for sign requests to be approved (headless signing still works via `test_sign`).

## Structure
- `plan/PLAN.md`, `features.md`, `shared/protocol.md`
- `docs/USAGE.md` — end-to-end guide to digitally sign on any portal
- `extension/` — Chrome MV3 extension
- `extension-firefox/` — Firefox MV2 extension
- `native-app/` — Rust host + GUI (`Cargo.toml`, `src/`)
- `docs/portal-integration/` — portal notes
- `tools/` — sync/build helpers

## How to digitally sign (any portal)
See **[docs/USAGE.md](docs/USAGE.md)** for the full step-by-step flow. In short:

1. Build + register the native host, load the extension.
2. Start `gui.exe` → **Connect to token** → pick the default certificate in the popup.
3. On the portal, click **"Sign with DSC"**.
4. The GUI shows an approval panel → enter **PIN** → **Approve & Sign**.
5. The signature is injected into the portal automatically.

Supported: Income Tax (priority), GST, MCA, EPFO, TDS/TRACES. Subscription portals
(IREPS, ICEGATE, DGFT, …) are Phase 2.

## Quick start (dev)
1. Build: `cd native-app && cargo build --release`
2. Load Chrome extension: chrome://extensions → Developer mode → Load unpacked → `extension/`
3. Register host (Chrome): `native-app/install-host.ps1 -Browser Chrome -ChromeExtensionId <id>`
4. Run GUI: `cargo run --bin gui` (or `H:\target\release\gui.exe`)
5. Test a portal: open the popup → select default certificate → visit a supported portal and click "Sign with DSC".

## Real token access (ProxKey / DSC)
Auto-detected PKCS#11 module:
`C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll`
Override with env `SIGNER_PKCS11_MODULE`.

Self-test (lists certs + signs a test payload; prompts for PIN on console):
```
cargo run --bin test_sign
```

## Firefox
1. Load `extension-firefox/` in `about:debugging` → "Load Temporary Add-on".
2. Find the extension's UUID at `about:debugging#/runtime/this-firefox`.
3. Register host: `native-app/install-host.ps1 -Browser Firefox -FirefoxExtensionUuid <uuid>`
4. Keep the GUI running.

## Status
- Real PKCS#11 enumeration ✅ (verified: token + 2 DSC owners read)
- Chrome + Firefox extensions scaffolded ✅
- GUI approval bridge ✅
- Portal sign interception 🟡 (needs live capture per portal)
- `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean ✅
- `SIGNER_MOCK=1` runs the development mock instead of a real token.
