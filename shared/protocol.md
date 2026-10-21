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
  "id": "uuid-v4",
  "type": "sign",
  "portal": "incometax.gov.in",
  "payload": {
    "data": "<base64>",
    "hashAlg": "SHA256",
    "signAlg": "RSA",
    "alreadyHashed": false,
    "certCN": "optional filter substring"
  },
  "certId": "optional",
  "pin": "optional (prefer interactive prompt in GUI)"
}
```

- **`alreadyHashed`**:
  - `false` (default): `data` contains raw unhashed bytes. The token hashes with SHA-256 and signs using `Mechanism::Sha256RsaPkcs`.
  - `true`: `data` contains a 32-byte pre-computed SHA-256 digest. Signer wraps it with the standard RFC 3447 ASN.1 `DigestInfo` prefix and signs using `Mechanism::RsaPkcs`.

Response success:
```json
{
  "id": "uuid-v4",
  "ok": true,
  "signature": "<base64 PKCS#1 v1.5 signature>",
  "cert": "<base64 DER X.509 certificate>",
  "certCN": "Common Name of certificate",
  "certId": "<base64 PKCS#11 object ID>",
  "signedAt": "2026-10-21T07:30:00Z"
}
```

Response error:
```json
{
  "id": "uuid-v4",
  "ok": false,
  "error": { "code": "ERROR_CODE", "message": "Human readable reason" }
}
```

Error codes: `TOKEN_NOT_FOUND`, `NO_TOKEN`, `PIN_REQUIRED`, `PIN_INVALID`, `CERT_EXPIRED`, `SIGN_FAILED`, `USER_CANCELLED`, `BAD_BASE64`, `HOST_UNAVAILABLE`.

### status
Request:  `{ "id", "type": "status" }`
Response: `{ "id", "ok": true, "tokenConnected": bool, "certSelected": bool }`

## Remote signing (relay) — Phase 5
Consultant → relay → client device. Payload encrypted end-to-end; client app shows
approval screen with document hash, portal, cert; on approval signs and returns.
