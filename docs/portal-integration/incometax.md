# Income Tax Portal Integration Notes (incometax.gov.in)

## Signing surfaces
- ITR filing verification: "Proceed to Verification" → "Sign with DSC".
- Other forms/notices requiring DSC.

## Observed helper endpoints (to verify via live capture)
- Legacy DSC signer: `http://localhost:1585/...` (older)
- emSigner: `http://127.0.0.1:8080/emsigner/...` or `https://127.0.0.1:...`
- Portal JS may call a local service and expect JSON `{ status, signature, certificate }`.

## Our interception architecture
1. **Main-World Hook (`interceptor-main.js`)**: Content scripts inject a script into the page DOM (`document.head || document.documentElement`). This overrides `window.fetch` and `window.XMLHttpRequest` directly inside the page's execution context for all `localhost` / `127.0.0.1` signer URLs (ports 8080, 1585, etc.).
2. **Two-Way Message Bridge**: When the portal's JavaScript makes a signing call, `interceptor-main.js` packages the request and posts a message (`signer-page-request`) to the content script, which routes it through the Chrome Native Messaging host to the Signer desktop app.
3. **Response Emulation**: On approval and signing, the response is posted back (`signer-content-response`) and formatted into the exact emSigner JSON structure (`status: "success"`, `signature`, `cert`) so the portal proceeds without error.
4. **DOM Click Interceptor**: As a complementary channel, the content script scans for "Sign with DSC" buttons, intercepts user clicks, reads `#signData` or `[data-sign-data]`, and injects the resulting signature into `#signature` or `[data-signature]`.

## Payload handling
- If portal sends raw ITR XML / hash, pass through as `data` (base64) with `alreadyHashed=false`
  so native app hashes with SHA256 (portal-specific hash alg TBD).
- If portal sends precomputed hash, set `alreadyHashed=true`.

## Cert policy
- Portal typically requires Class 2/3 DSC with PAN in CN/subject. Native app should filter
  certs by portal policy (configurable per portal).

## TODO (requires live capture)
- Capture exact request/response of emSigner on IT portal; document field names.
- Confirm hash algorithm and whether data is hashed client-side.
- Handle multiple DSCs and cert selection UX.
