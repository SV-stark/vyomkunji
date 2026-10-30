# Income Tax Portal Integration Notes (incometax.gov.in)

## Signing surfaces
- ITR filing verification: "Proceed to Verification" → "Sign with DSC".
- Statutory forms (10E, 15CA/CB, 29B, etc.) and notice replies requiring DSC verification.
- User profile DSC registration / update.

## Observed helper endpoints & emulation
- Legacy DSC signer: `http://localhost:1585/...`
- emSigner: `http://127.0.0.1:8080/emsigner/...`, `https://127.0.0.1:8080/...` (and fallback ports 8081–8090)
- emBridge: `http://127.0.0.1:1447/...` (and ports 1440–1450)
- WebSocket signers: `ws://127.0.0.1:8080/...`, `wss://127.0.0.1:8080/...`

## Interception architecture
1. **Main-World Hook (`interceptor-main.js`)**: Content scripts inject into the page DOM (`document.head || document.documentElement`). This overrides `window.fetch`, `window.XMLHttpRequest`, and `window.WebSocket` directly inside the page's execution context for all localhost/127.0.0.1 signer endpoints.
2. **Two-Way Message Bridge**: When the portal's JavaScript makes a call:
   - **Health Checks** (`/`, `/isAlive`, `/check`, `/version`): Instantly answered with `{"status": "success", "status_code": 200, "version": "2.6", "message": "Service Running"}` without prompting the user.
   - **Certificate Enumeration** (`/getCertificates`, `/dsc/getCert`, `/listTokens`): Routed through `listTokens` to return the certificate list with DER data and validity dates.
   - **Signing Calls** (`/sign`, `/signData`, `/signPdf`): `interceptor-main.js` packages the request and posts a message (`signer-page-request`, `action: "sign"`) to the content script, which routes it through the Native Messaging host to the desktop app.
3. **Response Emulation**: On approval and signing, the response is posted back (`signer-content-response`) and formatted into the universal response envelope:
   ```json
   {
     "status": "success",
     "status_code": 200,
     "signature": "<base64>",
     "signData": "<base64>",
     "sign": "<base64>",
     "signedData": "<base64>",
     "pkcs7": "<base64>",
     "cert": "<base64 DER>",
     "certificate": "<base64 DER>",
     "chain": ["<base64 DER>"],
     "data": {
       "signature": "<base64>",
       "signData": "<base64>",
       "certificate": "<base64 DER>",
       "cert": "<base64 DER>"
     }
   }
   ```
4. **DOM Click Interceptor**: As a complementary channel, the content script scans for "Sign with DSC" / "Proceed to Verification" buttons, intercepts user clicks, reads `#signData` or `[data-sign-data]`, and injects the resulting signature into `#signature` or `[data-signature]`.

## Payload handling
- If portal sends raw ITR XML / string, payload is passed as `data` (base64) with `alreadyHashed=false`.
- If portal sends precomputed SHA-256 digest (`hash` or `digest` field or 32-byte hash), `alreadyHashed=true` is set automatically.
- If portal requests PDF / CMS signing, `pkcs7=true` returns a detached CMS `SignedData`.

## Certificate policy & selection
- Class 2/3 DSC with PAN in Subject / Common Name.
- Multiple DSCs supported via:
  1. Desktop GUI Certificate selection and card preview.
  2. Browser Extension Popup dropdown (`defaultCertCN`).
  3. Browser Extension Options page (`options.html` / `options.js`).
  4. Protocol message `selectCert`.
