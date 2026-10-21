# How to Digitally Sign on Any Portal with VyomKunji

This guide walks through signing a document or return on a supported government portal
using the VyomKunji extension, the VyomKunji app and your DSC token. **No Java, no token
drivers, no emSigner/emBridge/CrisSigner.**

## One-time setup

1. **Build the native app**
   ```bash
   cd native-app
   cargo build --release
   ```
   Binaries: `vyomkunji.exe` (the app), `vyomkunji-host.exe` (the browser bridge),
   `test_sign.exe` (a self-test).

2. **Register the native messaging host** (lets the browser reach the app)
   - Chrome: `native-app\install-host.ps1 -Browser Chrome -ChromeExtensionId <extension-id>`
   - Firefox: `native-app\install-host.ps1 -Browser Firefox -FirefoxExtensionId <extension-id>`
   - `<extension-id>`: find it at `chrome://extensions` / `about:debugging` (or use default `vyomkunji-g2c@vyomkunji.local`).

3. **Load the extension**
   - Chrome: `chrome://extensions` → Developer mode → Load unpacked → select `extension/`.
   - Firefox: `about:debugging` → This Firefox → Load Temporary Add-on → select
     `extension-firefox/manifest.json`.

4. **Start the app and connect your token**
   ```powershell
   target\release\vyomkunji.exe
   ```
   Click **Connect to token** (it also connects at start-up). The app detects the
   PKCS#11 module — for ProxKey that is
   `C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll` — and lists your
   DSCs with their expiry.

5. **Choose the default certificate** from the extension popup (toolbar icon). This
   certificate is used for portal signings unless you override it.

## Signing on a portal

1. Open the portal in the browser (e.g. https://www.incometax.gov.in) and fill the form.
2. Click the portal's **"Sign with DSC"** (or "Sign using DSC" / "Sign with Digital
   Signature") button.
3. The extension intercepts it and sends a signing request to the app.
4. The **VyomKunji app** shows an approval panel with the portal, the payload size and its
   SHA-256 digest. Enter your token **PIN** and click **Approve & Sign**.
5. The app signs on-token (`SHA256_RSA_PKCS` or `RSA_PKCS` for pre-hashed digests) and returns the signature; the extension
   injects it into the portal and dispatches `signer:signed` / `vyomkunji:signed`. Finish verification as usual.

> The app must be running. If it is not, the request returns `PIN_REQUIRED`.

## The app runs in the background

- **Closing the window does not quit the app.** It moves to the notification area
  (tray) and keeps serving signing requests.
- When a portal asks for a signature while the window is closed, the app **opens its
  window by itself** and shows the approval panel.
- The tray menu has **Show VyomKunji**, **Pending requests** and **Quit VyomKunji**.
- Only **Quit VyomKunji** (tray) ends the process.

## Appearance

- **Light mode / Dark mode** button in the top bar.
- The colours can be replaced with a JSON palette file — see [THEME.md](THEME.md).
- The interface is drawn in Inter, with the desktop's own hinting and antialiasing.

## Supported portals

| Portal | URL | Status |
|---|---|---|
| Income Tax | incometax.gov.in | Priority — shim + interceptor |
| GST | gst.gov.in | Generic interceptor |
| MCA | mca.gov.in | Generic interceptor |
| EPFO | epfindia.gov.in | Generic interceptor |
| TDS / TRACES | tdscpc.gov.in, traces.gov.in | Generic interceptor |
| Subscription portals | IREPS, ICEGATE, DGFT, … | Phase 2 |

## Adding a new portal

1. Add an entry in `extension/src/content/generic.js` (`PORTALS` map) with the host key,
   a display name and the button texts to watch (lowercase substrings).
2. Add the host to `host_permissions` (Chrome) and `permissions` (Firefox).
3. If the portal uses a local signer helper (emSigner or the DSC signer on localhost),
   the main-world fetch/XHR hook (`interceptor-main.js`) already forwards those calls to the app — confirm the response
   shape matches what the portal expects.
4. Run `tools\sync-extensions.ps1` to copy the shared sources into the Firefox build.

## Headless / API use

The native host speaks the JSON protocol in `shared/protocol.md` over Chrome Native
Messaging. Message types: `ping`, `status`, `listTokens`, `sign`.

For a quick token check and signing test:
```bash
cargo run --bin test_sign
```
To filter by a specific certificate Common Name during test:
```bash
$env:VYOMKUNJI_TEST_CN = "Your Name"
cargo run --bin test_sign
```

For automated tests, set `VYOMKUNJI_GUI_TEST_PIN=<pin>`: the app then connects by itself
and approves requests without a click.

## Troubleshooting
- **No PKCS#11 module found** → set `VYOMKUNJI_PKCS11_MODULE` to your token's `.dll`/`.so`.
- **App not running / PIN_REQUIRED** → start `vyomkunji.exe`; the tray keeps it alive.
- **Nothing happens when you click "Sign with DSC"** → check that `interceptor-main.js` is permitted to inject, check the browser console for `[VyomKunji]` logs, and verify the Activity tab in the VyomKunji app.
- **Wrong certificate** → change the default certificate in the extension popup.
- **Sign fails** → check the app's status bar; the usual causes are an invalid PIN, a
  locked token, or a mechanism mismatch.
