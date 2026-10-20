# How to Digitally Sign on Any Portal with Signer

This guide walks through signing a document / return on a supported government portal
using the Signer extension + GUI + your DSC token. No Java, no token drivers, no
emSigner/emBridge/CrisSigner.

## One-time setup

1. **Build / install the native app**
   ```
   cd native-app
   cargo build --release
   ```
   Binaries: `gui.exe`, `signer-host.exe`, `test_sign.exe`.

2. **Register the native messaging host** (lets the browser reach the app)
   - Chrome: `native-app\install-host.ps1 -Browser Chrome -ChromeExtensionId <extension-id>`
   - Firefox: `native-app\install-host.ps1 -Browser Firefox -FirefoxExtensionUuid <uuid>`
   - `<extension-id>` / `<uuid>`: find it at `chrome://extensions` / `about:debugging`.

3. **Load the extension**
   - Chrome: `chrome://extensions` → Developer mode → Load unpacked → select `extension/`.
   - Firefox: `about:debugging` → This Firefox → Load Temporary Add-on → select `extension-firefox/manifest.json`.

4. **Start the GUI and connect your token**
   ```
   H:\target\release\gui.exe
   ```
   Click **Connect to token** — the app detects the PKCS#11 module (ProxKey:
   `C:\Windows\System32\Watchdata\PROXKey CSP India V3.0\WDPKCS.dll`) and lists your DSCs.

5. **Choose the default certificate** from the extension popup (top-right icon). This
   cert is used for portal signings unless you override it.

## Signing on a portal

1. Open the portal in the browser (e.g. https://www.incometax.gov.in) and fill the form.
2. Click the portal's **"Sign with DSC"** (or "Sign using DSC" / "Sign with Digital Signature") button.
3. The extension intercepts it and sends a signing request to the app.
4. The **Signer GUI** shows an approval panel with the portal and payload size. Enter your
   token **PIN** and click **Approve & Sign**.
5. The app signs on-token via `SHA256_RSA_PKCS` and returns the signature; the extension
   injects it into the portal and dispatches `signer:signed`. Complete verification as usual.

> The GUI must be running. If it isn't, the request returns `PIN_REQUIRED` and you'll need
> to start the GUI (or pass a PIN directly via the native API for headless use).

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

1. Add an entry in `extension/src/content/generic.js` (`PORTALS` map) with:
   - host key
   - display name
   - button text triggers (lowercase substrings)
2. Add the host to `host_permissions` (Chrome) and `permissions` (Firefox).
3. If the portal uses a local signer helper (emSigner/DSC signer on localhost), the
   fetch/XHR shim already forwards those calls to the app — just confirm the response shape.

## Headless / API use

The native host speaks the JSON protocol in `shared/protocol.md` over Chrome Native
Messaging. Message types: `ping`, `status`, `listTokens`, `sign`. For a quick token check:
```
cargo run --bin test_sign
```

## Troubleshooting
- **No PKCS#11 module found** → set `SIGNER_PKCS11_MODULE` to your token's `.dll`/`.so`.
- **GUI not running / PIN_REQUIRED** → start `gui.exe` before signing.
- **Wrong certificate** → change the default cert in the extension popup.
- **Sign fails** → check the GUI status text; most failures are PIN, locked token, or an
  unsupported mechanism on the token.
