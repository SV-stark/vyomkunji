# Signer — India G2C Digital Signing

One browser extension (**Chrome + Firefox**) and one native app for DSC signing on
Indian government portals. Priority: https://www.incometax.gov.in

No Java. No token drivers. No emSigner, emBridge or CrisSigner.

## Architecture

```
Chrome / Firefox
      │  native messaging (Chrome Native Messaging protocol)
      ▼
signer-host.exe ──────────────► DSC token (PKCS#11: WDPKCS.dll)
      │  localhost TCP bridge (127.0.0.1, JSON lines)
      ▼
gui.exe  ── tray + resident shell: keeps serving with the window closed,
           re-opens by itself when a portal asks for a signature
```

- **`signer-host.exe`** — the browser bridge. Enumerates tokens, forwards sign requests.
- **`gui.exe`** — the app. Shows the approval panel, takes the PIN, signs on the token.
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
| `native-app/` | The Rust app: host, GUI, PKCS#11, bridge |
| `extension/` | Chrome MV3 extension |
| `extension-firefox/` | Firefox MV2 extension (shared sources, kept in step) |
| `shared/protocol.md` | The extension ↔ app message protocol |
| `docs/USAGE.md` | **How to sign on any portal** |
| `docs/THEME.md` | Writing a palette file |
| `docs/portal-integration/` | Per-portal notes |
| `plan/PLAN.md`, `features.md` | Plan and comparison with xorkeesign |
| `CHANGELOG.md` | What changed, and when |
| `tools/` | Sync and icon helpers |

## Quick start

1. **Build**
   ```
   cd native-app
   cargo build --release
   ```
   Produces `gui.exe` (8.8 MB), `signer-host.exe` (0.8 MB), `test_sign.exe` (0.7 MB).

2. **Load the extension**
   - Chrome: `chrome://extensions` → Developer mode → Load unpacked → `extension/`
   - Firefox: `about:debugging` → Load Temporary Add-on → `extension-firefox/manifest.json`

3. **Register the native host**
   ```
   native-app\install-host.ps1 -Browser Both -ChromeExtensionId <id> -FirefoxExtensionUuid <uuid>
   ```

4. **Run the app**
   ```
   H:\target\release\gui.exe
   ```
   It connects to the token at start-up. Keep it running — or leave it in the tray.

5. **Sign**
   On the portal, click **"Sign with DSC"** → the app asks for the PIN → **Approve & Sign**.

Full walkthrough: **[docs/USAGE.md](docs/USAGE.md)**.

## Token support

PKCS#11, so any token with a PKCS#11 module works. Auto-detected on this machine:

```
C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll
```

Override with the environment variable `SIGNER_PKCS11_MODULE`. Verified against a
ProxKey (WD PROXKey WD05383117): both DSC owners are read, with issuer, serial and
validity.

Self-test (lists certificates and signs a test payload, prompting for the PIN):
```
cargo run --bin test_sign
```

## Development

```
cd native-app
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo build --release
```

`tools\sync-extensions.ps1` copies the shared extension sources into the Firefox build.

## Status

- Token access (PKCS#11) — **done**, verified on ProxKey
- Chrome + Firefox extensions — **done**
- Tray + background-resident app — **done**
- Light/dark palettes + JSON theming — **done**
- On-token signing — implemented, awaiting a live PIN test
- Portal interception — works generically; each portal still needs a live capture
  (Income Tax first)

See [CHANGELOG.md](CHANGELOG.md) for the full history.
