<div align="center">

# VyomKunji · व्योमकुंजी
### Driverless, Java-Free Digital Signature (DSC) Client for Indian Government Portals

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![CI Status](https://github.com/SV-stark/vyomkunji/actions/workflows/ci.yml/badge.svg)](https://github.com/SV-stark/vyomkunji/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/SV-stark/vyomkunji?color=green&include_prereleases)](https://github.com/SV-stark/vyomkunji/releases/latest)
[![Platform](https://img.shields.io/badge/Platform-Windows%20x64-lightgrey.svg)](https://github.com/SV-stark/vyomkunji/releases/latest)
[![Browsers](https://img.shields.io/badge/Browsers-Chrome%20(MV3)%20%7C%20Firefox%20(MV3)-orange.svg)](https://github.com/SV-stark/vyomkunji/releases/latest)

<p align="center">
  <b>No Java Runtime. No Vendor Token Drivers. No emSigner, emBridge, or CrisSigner.</b><br>
  A unified, modern Rust desktop client and companion browser extension for effortless hardware DSC signing.
</p>

</div>

---

## 📖 Overview

Digitally signing returns, forms, and compliance filings across Indian Government-to-Citizen (G2C) portals has historically been a painful ordeal. Citizens, Chartered Accountants, Tax Practitioners, and Company Secretaries often face:

* Fragile and conflicting **Java Runtime Environment (JRE)** versions.
* Mutually incompatible proprietary background utilities (**emSigner, emBridge, CrisSigner**).
* Heavy, proprietary token driver suites that fail across modern operating systems.
* Inconsistent browser extension and localhost network bridge failures.

**VyomKunji** (*व्योमकुंजी* — Sanskrit for *Celestial Key*) replaces this entire patchwork with a single, high-performance, open-source stack:
1. **Lightweight Native Rust Host & Desktop UI**: Communicates directly with physical cryptographic tokens over standard **PKCS#11**, keeping private keys strictly hardware-bound.
2. **Universal Browser Extension (Chrome & Firefox)**: Transparently intercepts legacy localhost signing endpoints (`http://127.0.0.1:8080`, `http://localhost:1585`) and DOM triggers, translating them into secure signing requests.
3. **Tray-Resident Desktop Daemon**: Runs silently in the Windows system tray with zero CPU overhead, automatically awakening to present an approval and PIN prompt only when a signature is requested.

---

## 🏛️ Supported Portals

| Portal | Scope & Services | Status |
|---|---|---|
| **Income Tax e-Filing** (`incometax.gov.in`) | ITR filing, statutory forms, notice replies, e-Verification | 🟢 **Primary Supported (P0)** |
| **GST Portal** (`gst.gov.in`) | Monthly/Annual GST returns (GSTR-1, 3B, 9), refund applications | 🟢 **Supported (Generic Shim)** |
| **MCA21 v3** (`mca.gov.in`) | Company incorporations, annual ROC filings, director KYC | 🟢 **Supported (Generic Shim)** |
| **EPFO Unified Portal** (`epfindia.gov.in`) | Employer PF returns, DSC registration, claim approvals | 🟢 **Supported (Generic Shim)** |
| **TRACES / TDS** (`traces.gov.in`, `tdscpc.gov.in`) | 24Q, 26Q TDS returns, certificate generation | 🟢 **Supported (Generic Shim)** |
| **e-Procurement / CPPP / ICEGATE** | Government tenders, customs, foreign trade | 🟡 **Phase 2 (In Development)** |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                       Web Browser                           │
│  (incometax.gov.in / gst.gov.in / mca.gov.in / epfindia)    │
│                                                             │
│   Page Scripts (emSigner fetch / XHR / DSC sign buttons)    │
│                           │                                 │
│                           ▼                                 │
│   [VyomKunji] Main-World Network Hook (interceptor-main.js) │
│                           │                                 │
│                           ▼ (window.postMessage)            │
│   [VyomKunji] Content Script & Background Service Worker    │
└───────────────────────────┬─────────────────────────────────┘
                            │ Native Messaging (stdin/stdout, 4-byte LE length)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                 vyomkunji-host.exe                          │
│         Browser Native Messaging Bridge                     │
└───────────────────────────┬─────────────────────────────────┘
                  │ Localhost TCP Bridge (127.0.0.1, JSON-Lines, token-auth)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    vyomkunji.exe                            │
│  Desktop UI (egui 0.36 + fastframe resident tray shell)     │
│   - Payload Digest Preview (SHA-1 / 256 / 384 / 512)        │
│   - Portal & Browser Origin Confirmation                    │
│   - Token & Certificate Selection                           │
│   - Secure PIN Entry Dialog                                 │
│   - Raw Signature or Detached CMS/PKCS#7 Output             │
└───────────────────────────┬─────────────────────────────────┘
                            │ PKCS#11 API (cryptoki)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              Hardware USB Token (FIPS / CC)                 │
│         (Watchdata ProxKey, eMudhra, SafeScrypt)            │
│                                                             │
│   🔒 On-Chip C_Sign (RSA PKCS#1 v1.5 / ECDSA)              │
│   ⛔ Private Key NEVER leaves the token silicon              │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 Getting Started

### Option 1: Quick Install (Pre-Built Release)

1. **Download the latest release**:
   Download `vyomkunji-v0.3.0-windows-x64.zip` from [GitHub Releases](https://github.com/SV-stark/vyomkunji/releases/latest).
2. **Extract the archive** to any folder on your computer (e.g. `C:\Program Files\VyomKunji` or your User folder).
   > A one-click installer (`vyomkunji-<version>-setup.exe`) is published as well. It installs per-user or machine-wide, registers the native host for Chrome/Edge and Firefox, closes a running app before upgrading, and can add an optional "start at sign-in" entry.
3. **Register the Native Host**:
   Open PowerShell in the extracted folder and run:
   ```powershell
   .\install-host.ps1 -Browser Both
   ```
4. **Install the Browser Extension**:
   * **Google Chrome / Microsoft Edge**:
     1. Open `chrome://extensions` and enable **Developer mode** (top right).
     2. Extract `vyomkunji-chrome-v0.3.0.zip` and click **Load unpacked**.
   * **Mozilla Firefox**:
     1. Open `about:debugging#/runtime/this-firefox`.
     2. Click **Load Temporary Add-on** and select `manifest.json` from the extracted Firefox extension zip.
     > The Firefox build is a Manifest V3 add-on (event-page background, Firefox 140+). Details and the rollback path are in [`docs/FIREFOX-MV3.md`](docs/FIREFOX-MV3.md).
5. **Plug in your DSC Token and Launch `vyomkunji.exe`**:
   The application will detect your hardware token and display your digital certificates with their validity dates.

---

### Option 2: Build From Source

#### Prerequisites
* [Rust toolchain](https://rustup.rs/) (stable channel, **1.98+** - the project uses the Rust 2024 edition; the minimum version is pinned as `rust-version` in `native-app/Cargo.toml`)
* Windows 10/11 x64
* PowerShell 5.1+ / 7+
* Optional: [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for the supply-chain checks

#### Build Instructions
```bash
# Clone the repository
git clone https://github.com/SV-stark/vyomkunji.git
cd vyomkunji/native-app

# Run formatting and linter checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# Execute test suite
cargo test

# Supply-chain check (advisories, licenses, banned crates, sources)
cargo deny check

# Compile release binaries
cargo build --release
```

From the repository root, the extension parity and versioning helpers can be run as well:
```powershell
powershell -File tools\sync-extensions.ps1     # Chrome -> Firefox shared sources
powershell -File tools\check-extensions.ps1   # byte-level parity + manifest/JS checks
powershell -File tools\check-version.ps1      # single-source version guard
node --test extension/tests/background.test.mjs
```

Compiled binaries will be generated in `target/release/`:
* `vyomkunji.exe` (Desktop GUI & Tray resident daemon)
* `vyomkunji-host.exe` (Native messaging host)
* `test_sign.exe` (CLI self-test utility)

---

## 🔒 Security & Privacy Architecture

VyomKunji was designed from day one around **zero-trust and non-repudiation**:

* **Hardware-Bound Private Keys**: All cryptographic signing operations are delegated directly to the physical token's secure element via PKCS#11 `C_Sign`. Private keys cannot be exported, extracted, or cloned.
* **Ephemeral PIN Handling**: The token PIN is held exclusively in volatile memory for the duration of the signing session. It is never logged to disk, persisted, or transmitted over any network socket.
* **Authenticated Local Bridge**: The host talks to the desktop app over `127.0.0.1` using newline-delimited JSON, and every request must carry the per-session token from `%APPDATA%\VyomKunji\gui.token`. Unauthenticated requests are rejected.
* **Tamper-Evident Digest Inspection**: Before approving a signature, the desktop GUI shows the payload digest (SHA-1/256/384/512), the portal name, and the browser tab URL that actually triggered the request, so the user can confirm the origin at a glance.
* **Origin Pinning**: The extension always reports the browser-reported tab URL; a page cannot spoof the origin shown in the approval card. Cross-origin frames are ignored by the content scripts.
* **Browser Requests Always Need Approval**: Direct host-side signing with a PIN is disabled by default and can only be enabled with `VYOMKUNJI_ALLOW_HEADLESS_PIN=1` for requests that carry no browser origin.
* **Strict Native Messaging Boundaries**: The extension and native host communicate solely via OS-level stdin/stdout pipes restricted by explicit origin whitelists (`allowed_origins` and `allowed_extensions`).
* **Privacy-Conscious Diagnostics**: The copyable diagnostics bundle contains versions, module and token metadata, and recent log lines - never PINs, signatures, certificate bodies, or your Windows username.
* **Zero Cloud Dependence**: Everything executes strictly on your local machine. No external servers or telemetry services are contacted.

---

## ⚙️ Configuration & Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `VYOMKUNJI_PKCS11_MODULE` | Explicit path to a vendor PKCS#11 DLL module | Auto-detected (`WDPKCS.dll` under System32/SysWOW64) |
| `VYOMKUNJI_TEST_CN` | Filter Common Name (CN) for the `test_sign` utility | None (signs with first available cert) |
| `VYOMKUNJI_GUI_TEST_PIN` | Token PIN used for unattended automated testing | None (interactive prompt) |
| `VYOMKUNJI_PIN_TIMEOUT_MIN` | Idle minutes before the in-memory PIN is cleared | `5` |
| `VYOMKUNJI_MOCK` | Simulates a virtual token for UI development | Unset |
| `VYOMKUNJI_STARTUP_LOG` | Appends startup timings to `%TEMP%\vyomkunji-startup.log` (rotated at 1 MiB) | Unset |
| `VYOMKUNJI_CRASH_LOG` | Appends local panic details to `%LOCALAPPDATA%\VyomKunji\crash.log` | Unset |
| `VYOMKUNJI_ALLOW_HEADLESS_PIN` | Allows the host to sign with a PIN **without** GUI approval - only for requests with no browser origin | Unset (GUI approval required) |

---

## 🧾 Signature Formats

| Mode | Request field | Response |
|---|---|---|
| Raw PKCS#1 v1.5 signature (default) | `pkcs7: false` | `signature` (base64) plus `cert`, `certId`, `certCN` |
| Detached CMS / PKCS#7 | `pkcs7: true` | `pkcs7` (base64 DER `SignedData`) plus `chain`, `cert`, `mechanism`, `hash`, `keyType` |

The digest algorithm is chosen per request with `hashAlg` (`SHA1`, `SHA256`, `SHA384`, `SHA512`; default `SHA256`). With `alreadyHashed: true` the payload must be a digest of exactly that length (or, for RSA, an already-wrapped `DigestInfo` structure). RSA and EC tokens are both supported.

---

## 📂 Repository Layout

```
vyomkunji/
├── .github/
│   ├── dependabot.yml       # Automated dependency update requests
│   └── workflows/           # GitHub Actions CI and Automated Release pipelines
├── docs/                    # Architectural specs, USAGE guide, and theme documentation
│   ├── portal-integration/  # Per-portal reverse-engineering and payload notes
│   ├── FIREFOX-MV3.md       # Firefox Manifest V3 migration (event page, validation, rollback)
│   ├── THEME.md             # Customizing JSON color palettes
│   └── USAGE.md             # Comprehensive portal-by-portal user manual
├── extension/               # Chrome Manifest V3 browser extension source
│   └── tests/               # Node tests for the background bridge and manifests
├── extension-firefox/       # Firefox Manifest V3 companion extension source
├── native-app/              # Rust workspace (Rust 2024 edition)
│   ├── src/
│   │   ├── bin/gui.rs       # egui/fastframe desktop GUI and tray shell
│   │   ├── bin/test_sign.rs # Command-line signing self-test utility
│   │   ├── main.rs          # Chrome native messaging host entrypoint
│   │   ├── pkcs11.rs        # Hardware token communication, hashing, RSA/ECDSA signing
│   │   ├── cms_sign.rs      # Detached CMS/PKCS#7 SignedData assembly
│   │   ├── bridge.rs        # Token-authenticated localhost JSON-lines bridge
│   │   ├── theme.rs         # Dynamic dark/light color palette engine
│   │   └── timeutil.rs      # Dependency-free civil timestamp & date conversions
│   ├── Cargo.toml           # Rust package manifest
│   └── install-host.ps1     # Native messaging host registry installer
├── shared/                  # Common protocol documentation
│   └── protocol.md          # Extension ↔ Native App JSON wire format
├── tools/                   # Version, parity, signing, and sync helpers
├── deny.toml                # cargo-deny policy (advisories, licenses, bans, sources)
├── LICENSE                  # GNU Affero General Public License v3.0 (AGPL-3.0)
└── README.md                # Project documentation
```

---

## 🤝 Contributing

Contributions are welcome! Please follow these guidelines:
1. Ensure all code adheres to Rust guidelines: run `cargo fmt` and `cargo clippy --all-targets -- -D warnings`.
2. Keep extensions in sync: after modifying files in `extension/src/`, execute `powershell tools/sync-extensions.ps1` to sync with `extension-firefox/src/`.
3. Open an issue first to discuss any major architectural changes or additional portal shims.

---

## 📄 License

VyomKunji is free and open-source software licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. See the [LICENSE](LICENSE) file for the full text.

Commercial entities and ERP vendors wishing to license VyomKunji components under alternative terms may reach out via GitHub Issues.
