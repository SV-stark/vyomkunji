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
   - Remove the registration again with `native-app\install-host.ps1 -Unregister`.

   > The one-click installer (`installer\vyomkunji.iss`, published as
   > `vyomkunji-<version>-setup.exe`) performs the same registration, can install
   > per-user or machine-wide, closes a running app before upgrading, and offers an
   > optional "start at sign-in" task.

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
4. The **VyomKunji app** shows an approval panel with:
   * the **portal** that asked for the signature,
   * the **origin** - the real browser tab URL (marked *unverified* when it is not an
     `https://*.gov.in` portal, or *Local request* when the request did not come
     from a browser),
   * the **payload digest** and its algorithm, and
   * the **output format** (raw signature or detached CMS/PKCS#7).
   Enter your token **PIN** and click **Approve & Sign**.
5. The app signs on-token (`SHA256_RSA_PKCS` by default, or the mechanism of the
   selected `hashAlg`; `RSA_PKCS` for pre-hashed digests) and returns the signature; the extension
   injects it into the portal and dispatches `signer:signed` / `vyomkunji:signed`. Finish verification as usual.

> The app must be running. If it is not, the request returns `PIN_REQUIRED` (or
> `GUI_UNAVAILABLE` when direct signing is disabled).

## Signature formats and hash algorithms

| Mode | Request field | What you get back |
|---|---|---|
| Raw signature (default) | `pkcs7: false` | `signature` (base64) plus `cert`, `certId`, `certCN` |
| Detached CMS / PKCS#7 | `pkcs7: true` | `pkcs7` (base64 DER `SignedData`) plus `chain` (intermediates), `cert`, `mechanism`, `hash`, `keyType` |

* `hashAlg` selects the digest: `SHA1`, `SHA256` (default), `SHA384`, `SHA512`.
* `alreadyHashed: true` means the payload is already a digest. It must have exactly
  the length of the selected algorithm (20 / 32 / 48 / 64 bytes); for RSA an
  already-wrapped `DigestInfo` structure is accepted too. Other lengths are
  rejected instead of being signed.
* RSA and EC tokens are both supported - the key type is derived from the signing
  certificate.

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

### Signing without the desktop app

Direct host-side signing (the request carries the PIN) is **disabled by default**.
It can only be enabled for local, non-browser requests:

```powershell
$env:VYOMKUNJI_ALLOW_HEADLESS_PIN = "1"
```

Both conditions must hold: the variable must be set **and** the request must not
carry a browser `origin`. Any request coming from a browser extension always
requires GUI approval, so a page can never make the host sign on its own.

Example request (native messaging payload):

```json
{
  "id": "0f8c2b1e-6b4f-4f0a-9a2f-2c9b6f1d4a55",
  "type": "sign",
  "portal": "gst.gov.in",
  "payload": {
    "data": "<base64 payload>",
    "hashAlg": "SHA256",
    "alreadyHashed": false,
    "pkcs7": true,
    "certCN": null
  },
  "pin": "1234"
}
```

## Diagnostics

* **Settings → Copy diagnostics** copies a support bundle to the clipboard: app
  version, active PKCS#11 module, token label, bridge state, current status, and the
  last 50 log lines. It never contains the PIN, signatures, certificate bodies, or
  your Windows user name.
* `VYOMKUNJI_STARTUP_LOG=1` appends startup milestones to
  `%TEMP%\vyomkunji-startup.log` (rotated at 1 MiB, one generation kept).
* `VYOMKUNJI_CRASH_LOG=1` appends panic details to
  `%LOCALAPPDATA%\VyomKunji\crash.log`. Set `RUST_BACKTRACE=1` as well to include a
  backtrace. Both files are local only and are never uploaded.

## Troubleshooting
- **No PKCS#11 module found** → set `VYOMKUNJI_PKCS11_MODULE` to your token's `.dll`/`.so`.
- **App not running / PIN_REQUIRED** → start `vyomkunji.exe`; the tray keeps it alive.
- **GUI_UNAVAILABLE** → the app is not running and direct signing is disabled. Start the
  app, or set `VYOMKUNJI_ALLOW_HEADLESS_PIN=1` for local, non-browser requests only.
- **The approval card shows an unexpected origin** → the request did not come from a
  supported portal. The extension always reports the real tab URL, so a spoofed origin
  means the request was not initiated by the supported content scripts. Decline it.
- **Nothing happens when you click "Sign with DSC"** → check that `interceptor-main.js` is permitted to inject, check the browser console for `[VyomKunji]` logs, and verify the Activity tab in the VyomKunji app.
- **Wrong certificate** → change the default certificate in the extension popup.
- **Sign fails** → check the app's status bar; the usual causes are an invalid PIN, a
  locked token, or a mechanism mismatch.
- **The request times out** → the native host was not found or disconnected. Re-run
  `install-host.ps1 -Browser Both`, and check that the app's bridge is running.
- **Need proof for a bug report** → use **Settings → Copy diagnostics** and include the
  text (it contains no secrets).
