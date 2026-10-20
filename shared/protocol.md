# Extension ↔ Native App Protocol

Transport: Chrome Native Messaging (stdin/stdout) OR WebSocket `ws://127.0.0.1:PORT`.
Framing (native messaging): 4-byte little-endian length prefix + UTF-8 JSON.

## Common envelope
```json
{ "id": "uuid-v4", "type": "<type>", ...fields }
```
Every request has `id`; every response echoes `id`.

## Message types

### ping
Request:  `{ "id", "type": "ping" }`
Response: `{ "id", "ok": true, "version": "x.y.z" }`

### listTokens
Request:  `{ "id", "type": "listTokens" }`
Response: `{ "id", "ok": true, "tokens": [ { "id", "label", "serial", "certs": [certInfo] } ] }`

### selectCert
Request:  `{ "id", "type": "selectCert", "certId": "..." }`
Response: `{ "id", "ok": true }`

### sign
Request:
```json
{
  "id", "type": "sign",
  "portal": "incometax.gov.in",
  "payload": { "data": "<base64>", "hashAlg": "SHA256", "signAlg": "RSA|ECDSA", "alreadyHashed": false },
  "certId": "optional",
  "pin": "optional (prefer interactive prompt)"
}
```
Response success: `{ "id", "ok": true, "signature": "<base64>", "cert": "<base64 DER>", "signedAt": "ISO8601" }`
Response error:   `{ "id", "ok": false, "error": { "code", "message" } }`

Error codes: TOKEN_NOT_FOUND, PIN_REQUIRED, PIN_INVALID, CERT_EXPIRED, SIGN_FAILED, USER_CANCELLED.

### status
Request:  `{ "id", "type": "status" }`
Response: `{ "id", "ok": true, "tokenConnected": bool, "certSelected": bool }`

## Remote signing (relay) — Phase 5
Consultant → relay → client device. Payload encrypted end-to-end; client app shows
approval screen with document hash, portal, cert; on approval signs and returns.
