# Features — Signer India G2C

Comparison baseline: **xorkeesign India G2C** (Odyssey Technologies) —
https://www.odysseytec.com/india-g2c

Legend: ✅ supported · 🟡 partial / in progress · ❌ not yet · ➕ our extra

---

## 1. Core signing (parity with xorkee)

| # | Feature | xorkee | Signer | Notes |
|---|---|---|---|---|
| 1 | Single browser extension for all portals | ✅ | ✅ | Chrome MV3 + Firefox MV2 extensions cover IT, GST, MCA, EPFO, TDS |
| 2 | No Java required | ✅ | ✅ | Native host replaces Java applets |
| 3 | No token drivers required | ✅ | ✅ (target) | PKCS#11/PC-SC direct; driver install not needed |
| 4 | No PKI components / emSigner / emBridge / CrisSigner | ✅ | ✅ | We emulate their endpoints via shims |
| 5 | Unified signing UX across portals | ✅ | ✅ (planned) | Consistent button + popup |
| 6 | Driverless token usage | ✅ | ✅ (target) | Token accessed via PC-SC |
| 7 | Works on platforms portals don't support | ✅ | 🟡 | Extension+host on Win/macOS/Linux; mobile Phase 5 |
| 8 | Free usage on some portals | ✅ | ✅ (planned) | IT, GST, MCA, EPFO, TDS free tier |
| 9 | Subscription portals (IREPS, ICEGATE, DGFT, etc.) | ✅ | 🟡 | Phase 2 |
| 10 | Mobile signing (Android) with DSC | ✅ | 🟡 | Phase 5 (remote approval first) |

## 2. Portal coverage

| Portal | xorkee | Signer | Priority |
|---|---|---|---|
| Income Tax (incometax.gov.in) | ✅ (free) | 🟡 Chrome+Firefox scaffold | **P0** |
| GST | ✅ (free) | 🟡 planned | P1 |
| MCA | ✅ (free) | 🟡 planned | P1 |
| EPFO | ✅ (free) | 🟡 planned | P1 |
| TDS / TRACES | ✅ (free) | 🟡 planned | P1 |
| IREPS | ✅ (subs) | ❌ | P2 |
| IRCEP | ✅ (subs) | ❌ | P2 |
| CPPP / eProcurement | ✅ (subs) | ❌ | P2 |
| ICEGATE | ✅ (subs) | ❌ | P2 |
| DGFT / COO | ✅ (subs) | ❌ | P2 |
| WHBS | ✅ (subs) | ❌ | P2 |

## 3. Security & trust

| Feature | xorkee | Signer | Notes |
|---|---|---|---|
| Private key never leaves token | ✅ (claimed) | ✅ | On-token signing via PKCS#11 |
| PIN captured locally, not logged | ✅ | ✅ | Secure input in the native app |
| Origin-checked extension↔host channel | ✅ | ✅ | Native messaging (OS-level) |
| GUI approval + PIN entry | ✅ | ✅ | Every request approved in the app |
| Keeps running with the window closed | ✅ | ✅ | Tray + resident shell (fastframe) |
| Window returns when a signature is asked for | ✅ | ✅ | Re-opens by itself on a request |
| Native text rendering / system typography | ✅ | ✅ | fastframe-text + Inter |
| Light and dark palettes, user-editable | ✅ | ➕ | fastframe-theme, JSON palette file |
| Remote signing without sharing token | ✅ | ✅ (planned, Phase 5) | E2E encrypted relay, explicit approval |
| Audit log of signing events | ❓ | ➕ | Activity log in the app |
| Open protocol / inspectable | ❌ (proprietary) | ➕ | Documented JSON protocol |

## 4. Differentiators (Signer extras)

- **Two browser extensions** (Chrome MV3 + Firefox MV2) sharing one native app.
- **Background-resident app** — closing the window leaves it in the tray, and it opens
  itself the moment a portal asks for a signature.
- **GUI approval model** — every sign request is shown for explicit approval + PIN.
- **JSON palette file** — the colours are the user's to change, light and dark.
- **Built on fastframe** — the same foundation as ZapFast/Spotifast: native text
  rendering, Inter, tray, resident shell.
- **Open, documented protocol** (`shared/protocol.md`) — third parties can integrate.
- **emSigner-compatible shims** so portals work unmodified.
- **Mock/test mode** for development without a real token.
- **Audit log** of every signing event (who/when/portal/hash).
- **Planned**: offline verification of signed payloads, cert expiry warnings,
  PAN-based cert auto-selection for IT portal.

## 5. Gaps vs xorkee (to close)

- Real PKCS#11 token signing ✅ (enumeration done; signing pending live PIN test) — M3.
- Mobile Android app + USB-OTG token access — M5.
- Subscription/billing for paid portals — M6.
- Vendor-specific token matrix (eMudhra, (n)Code, SafeScrypt) & PKCS#11 lib paths.
- Live capture of IT portal signer protocol (exact payload).
- Per-portal adapters (the shim is generic until each portal is captured).

## 6. Non-goals (for now)
- We do not issue DSCs; we only use existing tokens.
- We do not store private keys or PINs.
- We do not modify portal server-side behavior.
