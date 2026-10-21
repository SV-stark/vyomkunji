<div align="center">

# VyomKunji · व्योमकुंजी
### Driverless, Java-Free Digital Signature (DSC) Client for Indian Government Portals

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![CI Status](https://github.com/SV-stark/vyomkunji/actions/workflows/ci.yml/badge.svg)](https://github.com/SV-stark/vyomkunji/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/SV-stark/vyomkunji?color=green&include_prereleases)](https://github.com/SV-stark/vyomkunji/releases/latest)
[![Platform](https://img.shields.io/badge/Platform-Windows%20x64-lightgrey.svg)](https://github.com/SV-stark/vyomkunji/releases/latest)
[![Browsers](https://img.shields.io/badge/Browsers-Chrome%20(MV3)%20%7C%20Firefox%20(MV2)-orange.svg)](https://github.com/SV-stark/vyomkunji/releases/latest)

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
                            │ Localhost TCP Bridge (127.0.0.1, JSON-Lines)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    vyomkunji.exe                            │
│  Desktop UI (egui 0.36 + fastframe resident tray shell)     │
│   - Document Hash Preview (SHA-256)                         │
│   - Token & Certificate Selection                           │
│   - Secure PIN Entry Dialog                                 │
└───────────────────────────┬─────────────────────────────────┘
                            │ PKCS#11 API (cryptoki)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              Hardware USB Token (FIPS / CC)                 │
│         (Watchdata ProxKey, eMudhra, SafeScrypt)            │
│                                                             │
│   🔒 On-Chip C_Sign (RSA-2048 / PKCS#1 v1.5)               │
│   ⛔ Private Key NEVER leaves the token silicon              │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 Getting Started

### Option 1: Quick Install (Pre-Built Release)

1. **Download the latest release**:
   Download `vyomkunji-v0.1.0-windows-x64.zip` from [GitHub Releases](https://github.com/SV-stark/vyomkunji/releases/latest).
2. **Extract the archive** to any folder on your computer (e.g. `C:\Program Files\VyomKunji` or your User folder).
3. **Register the Native Host**:
   Open PowerShell in the extracted folder and run:
   ```powershell
   .\install-host.ps1 -Browser Both
   ```
4. **Install the Browser Extension**:
   * **Google Chrome / Microsoft Edge**:
     1. Open `chrome://extensions` and enable **Developer mode** (top right).
     2. Extract `vyomkunji-chrome-v0.1.0.zip` and click **Load unpacked**.
   * **Mozilla Firefox**:
     1. Open `about:debugging#/runtime/this-firefox`.
     2. Click **Load Temporary Add-on** and select `manifest.json` from the extracted Firefox extension zip.
5. **Plug in your DSC Token and Launch `vyomkunji.exe`**:
   The application will detect your hardware token and display your digital certificates with their validity dates.

---

### Option 2: Build From Source

#### Prerequisites
* [Rust toolchain](https://rustup.rs/) (stable channel, 1.75+)
* Windows 10/11 x64
* PowerShell 5.1+ / 7+

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

# Compile release binaries
cargo build --release
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
* **Tamper-Evident SHA-256 Digest Inspection**: Before approving a signature, the desktop GUI calculates and presents the explicit SHA-256 hash of the payload and the originating portal URL for user consent.
* **Strict Native Messaging Boundaries**: The extension and native host communicate solely via OS-level stdin/stdout pipes restricted by explicit origin whitelists (`allowed_origins` and `allowed_extensions`).
* **Zero Cloud Dependence**: Everything executes strictly on your local machine. No external servers or telemetry services are contacted.

---

## ⚙️ Configuration & Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `VYOMKUNJI_PKCS11_MODULE` | Explicit path to a vendor PKCS#11 DLL module | Auto-detected (`WDPKCS.dll` under System32/SysWOW64) |
| `VYOMKUNJI_TEST_CN` | Filter Common Name (CN) for the `test_sign` utility | None (signs with first available cert) |
| `VYOMKUNJI_GUI_TEST_PIN` | Token PIN used for unattended automated testing | None (interactive prompt) |
| `VYOMKUNJI_MOCK` | Simulates a virtual token for UI development | Unset |

---

## 📂 Repository Layout

```
vyomkunji/
├── .github/workflows/       # GitHub Actions CI and Automated Release pipelines
├── docs/                    # Architectural specs, USAGE guide, and theme documentation
│   ├── portal-integration/  # Per-portal reverse-engineering and payload notes
│   ├── THEME.md             # Customizing JSON color palettes
│   └── USAGE.md             # Comprehensive portal-by-portal user manual
├── extension/               # Chrome Manifest V3 browser extension source
├── extension-firefox/       # Firefox Manifest V2 companion extension source
├── native-app/              # Rust workspace
│   ├── src/
│   │   ├── bin/gui.rs       # egui/fastframe desktop GUI and tray shell
│   │   ├── bin/test_sign.rs # Command-line signing self-test utility
│   │   ├── main.rs          # Chrome native messaging host entrypoint
│   │   ├── pkcs11.rs        # Hardware token communication & RSA-PKCS#1 v1.5 crypto
│   │   ├── bridge.rs        # Localhost JSON-lines inter-process bridge
│   │   ├── theme.rs         # Dynamic dark/light color palette engine
│   │   └── timeutil.rs      # Dependency-free civil timestamp & date conversions
│   ├── Cargo.toml           # Rust package manifest
│   └── install-host.ps1     # Native messaging host registry installer
├── shared/                  # Common protocol documentation
│   └── protocol.md          # Extension ↔ Native App JSON wire format
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
