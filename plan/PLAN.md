# VyomKunji — India G2C Digital Signing — Product Plan

## 1. Product Vision

Build a **single browser extension + native app** (mirroring the xorkeesign India G2C
approach) that lets citizens/professionals digitally sign on Indian government portals
using a DSC token **without Java, token drivers, PKI components, emSigner/emBridge/CrisSigner**.

- **Extension** (Chrome/Edge/Firefox, MV3) detects signing flows on supported portals.
- **Native app** (desktop + Android) owns the DSC token (PKCS#11 / PC-SC), performs
  hashing + signing, and returns the signature.
- **Remote signing**: a consultant can trigger a signing request that is approved & signed
  on the client's own device/phone — the DSC token never leaves the owner.

**Priority target portal:** https://www.incometax.gov.in (Income Tax e-Filing).

## 2. Supported Portals (phased)

### Phase 1 — Free portals
| Portal | URL | Signing use |
|---|---|---|
| Income Tax (priority) | incometax.gov.in | ITR filing, notices, forms |
| GST | gst.gov.in | Returns, refunds |
| MCA | mca.gov.in | Company filings |
| EPFO | unifiedportal-emp.epfindia.gov.in | PF returns |
| TDS | traces.gov.in / tdscpc.gov.in | TDS statements |

### Phase 2 — Subscription portals
IREPS, IRCEP, CPPP/eProcurement, ICEGATE, DGFT, COO, WHBS (and more).

## 3. Architecture

```
┌─────────────────────────────┐        ┌──────────────────────────────┐
│  Browser Extension (MV3)    │        │  Native App (Rust + Tauri)   │
│  - content scripts per portal│◄──────►│  - PKCS#11 / PC-SC token I/O │
│  - background service worker │ native │  - crypto (RSA/ECDSA + hash)  │
│  - popup (status/settings)  │ msg /  │  - cert picker / PIN prompt  │
│  - detects sign buttons     │  WS    │  - remote-sign relay client  │
└─────────────────────────────┘        └──────────────┬───────────────┘
                                                     │
                                          ┌──────────┴──────────┐
                                          │ Remote signer (phone)│
                                          │ approve + sign       │
                                          └─────────────────────┘
```

Transport options:
1. **Native Messaging** (primary, desktop): extension ↔ native host binary via stdin/stdout
   4-byte LE length + JSON. Lowest latency, no network.
2. **Local HTTP/WebSocket** (fallback + mobile bridge): native app exposes
   `ws://127.0.0.1:<port>`; extension connects; phone connects over LAN/TLS relay for
   remote signing.
3. **Remote signing relay**: cloud WebSocket broker pairs consultant ↔ client device by
   session code; end-to-end encrypted signing requests.

## 4. Signing Protocol (JSON)

Request (extension → app):
```json
{
  "id": "uuid",
  "type": "sign",
  "portal": "incometax.gov.in",
  "context": { "form": "ITR", "assessmentYear": "2026-27" },
  "payload": { "data": "<base64>", "hashAlg": "SHA256", "signAlg": "RSA" },
  "certFilter": { "issuer": "...", "expiryAfter": "..." }
}
```

Response (app → extension):
```json
{
  "id": "uuid",
  "ok": true,
  "signature": "<base64>",
  "cert": "<base64 DER>",
  "signedAt": "ISO8601"
}
```

Other message types: `listTokens`, `selectCert`, `pin`, `status`, `ping`.

## 5. Income Tax Portal Integration (priority)

### 5.1 Observed signing flow on incometax.gov.in
- User fills ITR / form → clicks "Proceed to Verification" → "Sign with DSC".
- Portal (modern) calls a local helper. Historically: emSigner on `http://127.0.0.1:8080`
  (emudhra/emsigner) or `http://localhost:1585` (older DSC signer) or Java applet.
- Request contains the data to sign (often a hash or the ITR XML digest) + expected
  cert policy.
- Portal expects back: base64 signature + certificate chain.

### 5.2 Our strategy
1. **Detect**: content script watches for DSC sign triggers (button text "Sign with DSC",
   network calls to localhost signer endpoints, or DOM injection of emSigner iframe).
2. **Intercept**: block/replace the portal's call to local signer; instead route payload
   to our extension background → native app.
3. **Sign**: native app hashes (if portal sends raw data) and signs with selected DSC.
4. **Inject**: return signature in the exact shape the portal expects (emSigner-compatible
   response envelope) so the portal accepts it without modification.
5. **Fallback**: if portal uses a signed JS bundle we cannot patch, provide a manual
   "Sign with Signer App" button injected into the page.

### 5.3 Compatibility shims to implement
- emSigner HTTP API shim: emulate `GET /emsigner/...` endpoints the portal calls, so
  the portal thinks emSigner is running, but requests are forwarded to our app.
- DSC signer (older) localhost endpoint shim.
- Certificate selection matching portal's cert policy (PAN-based CN, Class 2/3, etc.).

### 5.4 Risks
- Portal changes its signer protocol → need regression tests + versioned shims.
- Browser security (mixed content, localhost) → extension host permissions + native
  messaging avoids CORS.
- Legal/compliance: signing must preserve non-repudiation; use hardware token private key
  (never extractable). We only invoke token signing.

## 6. Security
- Private keys never leave the token; signing happens on-token via PKCS#11 `C_Sign`.
- PIN captured in native app (secure input), never logged, cleared after use.
- Extension ↔ app: native messaging (OS-level, origin-checked) or localhost with
  origin allow-list + random token.
- Remote signing: E2E encrypted (X25519 + AES-GCM), request shows document hash + portal
  + cert to signer for explicit approval; replay protection via nonce + expiry.
- Audit log of all signing events (local, tamper-evident).

## 7. Tech Stack
- Extension: JavaScript, Chrome MV3 + Firefox MV3 (event page), shared sources.
- Native app: Rust.
  - PKCS#11 via the `cryptoki` crate; PC/SC via the `pcsc` crate.
  - Crypto: on-token signing only; the private key never leaves the token.
- GUI: **egui/eframe 0.36.2** with **fastframe** (`text`, `fonts`, `theme`, `tray`, `shell`),
  MIT, pinned to the `v0.14.0` git tag. fastframe gives native text rendering, Inter,
  JSON palettes, a tray item and the keep-running shell the signing bridge needs.
- Relay server (remote signing, Phase 5): Rust (axum) or Node, WebSocket, deployed.
- Storage: JSON palette + log files under `%APPDATA%\Signer`.

## 8. Phases & Milestones
- M0 (week 1-2): Plan + protocol + repo scaffold. ✅
- M1 (week 3-4): Native host (native messaging) + token enumeration. ✅
- M2 (week 5-6): Extension MV3 (Chrome service worker + Firefox event page) + incometax.gov.in content script + shim. ✅
- M3 (week 7-8): Real PKCS#11 signing + cert picker + PIN. ✅ (PIN test pending)
- M4 (week 9-10): End-to-end on IT portal (ITR signing) + tests. 🟡
- M5 (week 11-12): Remote signing (phone approve) + audit log. 🟡
- M6 (week 13+): Other free portals, then subscription portals.

**Shipped alongside M0–M3:** the tray + resident shell (the app keeps serving with the
window closed and opens itself for a request), light/dark palettes and the JSON palette
file, and the redesigned GUI.

## 9. Deliverables in this repo
- `plan/PLAN.md` — this file.
- `features.md` — feature list and comparison with xorkeesign India G2C.
- `CHANGELOG.md` — release history.
- `shared/protocol.md` — message schema.
- `README.md`, `docs/USAGE.md`, `docs/THEME.md` — how to run, sign and theme it.
- `extension/` (Chrome MV3), `extension-firefox/` (Firefox MV3).
- `native-app/` — Rust host, GUI, PKCS#11 layer and the localhost bridge.
- `docs/portal-integration/incometax.md` — portal integration notes.

## 10. Open questions
- Confirm exact IT portal signer endpoint & payload shape (requires live capture).
- DSC token vendor matrix (eMudhra, (n)Code, SafeScrypt, etc.) and PKCS#11 lib paths.
- Licensing/subscription billing integration.
- Mobile (Android) token access: USB OTG + PC/SC over Android? or NFC? Scope.
