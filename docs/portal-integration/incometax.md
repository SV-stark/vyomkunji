# Income Tax Portal Integration Notes (incometax.gov.in)

## Signing surfaces
- ITR filing verification: "Proceed to Verification" → "Sign with DSC".
- Other forms/notices requiring DSC.

## Observed helper endpoints (to verify via live capture)
- Legacy DSC signer: `http://localhost:1585/...` (older)
- emSigner: `http://127.0.0.1:8080/emsigner/...` or `https://127.0.0.1:...`
- Portal JS may call a local service and expect JSON `{ status, signature, certificate }`.

## Our interception plan
1. Content script scans for "Sign with DSC" buttons → attaches click interceptor.
2. Overrides `window.fetch` / `XMLHttpRequest` for localhost/127.0.0.1 signer URLs → forwards
   payload to native host, returns emSigner-shaped response.
3. If no interceptable call, injects a "Sign with Signer" button fallback.

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
