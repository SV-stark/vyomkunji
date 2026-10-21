# Extension ↔ Native App Protocol

Transport: Chrome / Firefox Native Messaging (stdin/stdout).
Framing (native messaging): 4-byte little-endian length prefix + UTF-8 JSON.

The native host then forwards the request to the desktop app over a **local TCP
bridge** on `127.0.0.1` using newline-delimited JSON. The listening port is
published in `%APPDATA%\VyomKunji\gui.port` and the per-session authentication
token in `%APPDATA%\VyomKunji\gui.token` (the legacy `%APPDATA%\Signer\...`
paths are still read for compatibility). Every bridge request must carry the
`token` field; requests without a valid token are answered with
`{"ok": false, "error": {"code": "UNAUTHORIZED"}}`.

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
  "origin": "https://www.incometax.gov.in/iec/foservices/",
  "payload": {
    "data": "<base64>",
    "hashAlg": "SHA256",
    "signAlg": "RSA",
    "alreadyHashed": false,
    "pkcs7": false,
    "certCN": "optional filter substring"
  },
  "certId": "optional",
  "pin": "optional (GUI approval is preferred)"
}
```

- **`origin`** (top level): the real tab URL, always supplied by the extension
  background script and never by the page. A non-empty origin means the request
  came from a browser and therefore requires GUI approval - the host refuses to
  sign it headlessly even when a PIN is present.
- **`portal`** (top level): the portal key detected by the content script
  (`incometax.gov.in`, `gst.gov.in`, ...). It is also accepted inside `payload`
  for older callers.
- **`hashAlg`**: `SHA1`, `SHA256` (default), `SHA384`, or `SHA512`.
- **`alreadyHashed`**:
  - `false` (default): `data` contains raw unhashed bytes. The token hashes with
    the selected algorithm and signs in a single operation
    (`Mechanism::Sha256RsaPkcs` and friends, or `Mechanism::EcdsaSha*` for EC
    keys).
  - `true`: `data` contains a digest of exactly the selected length (20 / 32 /
    48 / 64 bytes). For RSA the digest is wrapped in the standard RFC 3447 ASN.1
    `DigestInfo` structure and signed with `Mechanism::RsaPkcs`; an
    already-wrapped `DigestInfo` is accepted as-is. Any other length is
    rejected instead of being signed blindly.
- **`pkcs7`**: `true` returns a detached CMS/PKCS#7 `SignedData` in the `pkcs7`
  field instead of a raw signature. The structure contains the signing
  certificate and any intermediates found on the token.

Response success (raw signature):
```json
{
  "id": "uuid-v4",
  "ok": true,
  "signature": "<base64 PKCS#1 v1.5 or ECDSA signature>",
  "cert": "<base64 DER X.509 certificate>",
  "certCN": "Common Name of certificate",
  "certId": "<base64 PKCS#11 object ID>",
  "hash": "SHA256",
  "keyType": "RSA",
  "mechanism": "SHA256_RSA_PKCS",
  "signedAt": "2026-10-21T07:30:00Z"
}
```

Response success (detached CMS):
```json
{
  "id": "uuid-v4",
  "ok": true,
  "pkcs7": "<base64 DER CMS SignedData>",
  "chain": [ "<base64 DER intermediate certificate>" ],
  "cert": "<base64 DER X.509 certificate>",
  "certCN": "Common Name of certificate",
  "certId": "<base64 PKCS#11 object ID>",
  "hash": "SHA256",
  "keyType": "RSA",
  "mechanism": "RSA_PKCS_CMS",
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

Error codes: `TOKEN_NOT_FOUND`, `NO_TOKEN`, `NO_PKCS11_MODULE`, `PIN_REQUIRED`,
`PIN_INVALID`, `CERT_EXPIRED`, `SIGN_FAILED`, `USER_CANCELLED`, `BAD_BASE64`,
`HOST_UNAVAILABLE`, `GUI_UNAVAILABLE`, `UNAUTHORIZED`, `BAD_REQUEST`,
`UNKNOWN_TYPE`.

`GUI_UNAVAILABLE` is returned when the desktop app is not running and direct
host-side signing is not allowed. Direct signing requires **both**
`VYOMKUNJI_ALLOW_HEADLESS_PIN=1` and a request without a browser `origin`.

### status
Request:  `{ "id", "type": "status" }`
Response: `{ "id", "ok": true, "tokenConnected": bool, "certSelected": bool }`

## Remote signing (relay) — Phase 5
Consultant → relay → client device. Payload encrypted end-to-end; client app shows
approval screen with document hash, portal, cert; on approval signs and returns.
