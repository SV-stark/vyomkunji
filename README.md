# VyomKunji — India G2C Digital Signing

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)

One browser extension (**Chrome + Firefox**) and one native app for DSC signing on
Indian government portals. Priority: https://www.incometax.gov.in

No Java. No token drivers. No emSigner, emBridge or CrisSigner.

## Architecture

```
Chrome / Firefox
      │  native messaging (Chrome Native Messaging protocol)
      ▼
vyomkunji-host.exe ───────────► DSC token (PKCS#11: WDPKCS.dll)
      │  localhost TCP bridge (127.0.0.1, JSON lines)
      ▼
vyomkunji.exe  ── tray + resident shell: keeps serving with the window closed,
                  re-opens by itself when a portal asks for a signature
```

- **`vyomkunji-host.exe`** — the browser bridge. Enumerates tokens, forwards sign requests.
- **`vyomkunji.exe`** — the desktop app. Shows the approval panel, takes the PIN, signs on the token.
- **Tray** — closing the window keeps the app alive; a new request brings it back.

## The stack

| Part | Choice |
|---|---|
| Extension | Chrome MV3 + Firefox MV2, plain JavaScript |
| Native host | Rust, `cryptoki` (PKCS#11), Chrome Native Messaging |
| App GUI | Rust, **egui/eframe 0.36.2** with **fastframe** (MIT, `v0.14.0`) |
| fastframe crates | `text`, `fonts`, `theme`, `tray`, `shell` |

fastframe is the shared foundation behind ZapFast/Spotifast/Solco: native text
rendering, Inter, JSON palettes, a tray item and the keep-running shell.

## Project layout

| Path | What it is |
|---|---|
| `native-app/` | The Rust app: host (`vyomkunji-host`), GUI (`vyomkunji`), PKCS#11, bridge |
| `extension/` | Chrome MV3 extension |
| `extension-firefox/` | Firefox MV2 extension (shared sources, kept in step) |
| `shared/protocol.md` | The extension ↔ app message protocol |
| `docs/USAGE.md` | **How to sign on any portal** |
| `docs/THEME.md` | Writing a palette file |
| `docs/portal-integration/` | Per-portal notes |
| `plan/PLAN.md` | Architecture and implementation plan |
| `CHANGELOG.md` | What changed, and when |
| `tools/` | Sync and icon helpers |
| `LICENSE` | GNU Affero General Public License v3.0 |

## Quick start

1. **Build**
   ```bash
   cd native-app
   cargo build --release
   ```
   Produces `vyomkunji.exe` (GUI app), `vyomkunji-host.exe` (browser host), `test_sign.exe` (self-test).

2. **Load the extension**
   - Chrome: `chrome://extensions` → Developer mode → Load unpacked → `extension/`
   - Firefox: `about:debugging` → Load Temporary Add-on → `extension-firefox/manifest.json`

3. **Register the native host**
   ```powershell
   native-app\install-host.ps1 -Browser Both -ChromeExtensionId <id> -FirefoxExtensionId <id>
   ```

4. **Run the app**
   ```powershell
   target\release\vyomkunji.exe
   ```
   It connects to the token at start-up. Keep it running — or leave it in the tray.

5. **Sign**
   On the portal, click **"Sign with DSC"** → VyomKunji pops up and asks for the PIN → **Approve & Sign**.

Full walkthrough: **[docs/USAGE.md](docs/USAGE.md)**.

## Token support

PKCS#11, so any token with a PKCS#11 module works. Auto-detected on this machine:

```
C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll
```

Override with the environment variable `VYOMKUNJI_PKCS11_MODULE` (or `SIGNER_PKCS11_MODULE`). Verified against a
ProxKey token: DSC certificates are read with issuer, serial and validity.

Self-test (lists certificates and signs a test payload, prompting for the PIN):
```bash
cargo run --bin test_sign
# Or filter by certificate Common Name:
$env:VYOMKUNJI_TEST_CN = "Your Name"; cargo run --bin test_sign
```

## Development

```bash
cd native-app
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

`tools\sync-extensions.ps1` copies the shared extension sources into the Firefox build.

## License

This project is licensed under the **GNU Affero General Public License v3.0** (AGPL-3.0). See [LICENSE](LICENSE) for details.
