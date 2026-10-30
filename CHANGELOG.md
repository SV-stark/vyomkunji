# Changelog

All notable changes to **VyomKunji — India G2C Digital Signing** are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.5.0] - 2026-10-30

### Added
- **Maximum Indian portal coverage**: Central portals (GeM, ESIC, eOffice, ICEGATE,
  DGFT, CPPP, IREPS, GST, Income Tax, MCA21, EPFO, TRACES) and all State e-Procurement
  portals (`*.gov.in`, `*.nic.in`, `tenderwizard.com`, `nprocure.com`) supported with
  dynamic domain matching.
- **Full localhost signer emulation (HTTP + WebSocket)**: Main-world hook handles
  emSigner, emBridge, CrisSigner, and NICDSign ports (8080-8090, 1440-1450, 1580-1590,
  1625-1635, 26000-26005). Transparently answers health checks (`/isAlive`, `/check`,
  `/version`) without prompting the user, and serves certificate enumeration
  (`/getCertificates`, `/listTokens`) with full X.509 DER metadata.
- **WebSocket signer interception**: `window.WebSocket` constructor hook enables
  modern WebSocket-based localhost signer workflows on ports 8080 and 1585.
- **Interactive options page (`options.js`)**: Real-time diagnostic ping test,
  connected token and certificate inventory, default certificate selector,
  and comprehensive Indian portal directory.
- **Extended PKCS#11 vendor matrix**: Auto-detects mToken (CryptoID `mtoken_pkcs11.dll`,
  `mPki11.dll`), eMudhra (`SignatureP11.dll`, `TrustKeyP11.dll`), Watchdata, and
  ePass 2003 modules.

### Changed
- **Wired GUI bridge queries**: `status` and `listTokens` are routed directly to the
  desktop GUI's in-memory manager over the TCP bridge when running, eliminating
  PKCS#11 driver re-initialisation lag and hardware token lock contention.
- **Wired `selectCert` protocol**: End-to-end support for selecting active certificates
  from the extension popup, options UI, and host protocol.
- **Origin trust validation**: `origin_trusted` in the desktop GUI recognizes all
  authentic `*.gov.in`, `*.nic.in`, and verified e-tender domains.
- **Crate updates**: Updated dependencies via `cargo update` (resolving yanked
  `clipboard-win` to v5.4.3), verified against `cargo deny check`.

---

## [0.4.2] - 2026-10-22

Phase 2 subscription portals and Firefox/Chrome connectivity fixes.

### Added
- **Phase 2 portal support (generic shim)**: IREPS, CPPP/eProcurement, ICEGATE,
  DGFT (and `etenders.gov.in`) are now covered by the generic content script and
  main-world network interceptor, matching the GST/MCA/EPFO/TDS flow.
- **Token name in extension popup**: the popup now shows the attached token's label
  instead of a generic "present", with error handling to avoid a stuck "checking..."
  state.

### Fixed
- **Native host launch on Windows (Chrome + Firefox)**: `install-host.ps1`
  over-escaped backslashes in the host manifest `path`, so the browser could not
  launch `vyomkunji-host.exe`. The path is now escaped correctly for JSON.
- **Phase 2 documentation**: README, USAGE and features.md updated to reflect the
  newly supported subscription portals.

---

## [0.4.1] - 2026-10-21

Maintenance release: dependency updates and CI hardening only - no behaviour
changes for users.

### Changed
- **`rand` 0.8 → 0.10**: the bridge session token is still 32 random bytes; the
  call site was adapted to the new API (`rand::Rng`, `rand::rng()`).
- **`base64` 0.22 → 0.23**, **`x509-parser` 0.16 → 0.18** (certificate metadata
  parsing), **`tray-icon` 0.25 → 0.26** (tray item).
- **`sha1` 0.10 → 0.11** and **`sha2` 0.10 → 0.11** (digest implementation).
- **CMS stack kept on one `der` generation**: `der` 0.7, `x509-cert` 0.2,
  `const-oid` 0.9 and `cms` 0.2.3 stay together, because `cms` 0.2.x needs
  `der` 0.7 while `der` 0.8 / `x509-cert` 0.3 require `cms` 0.3 (still a release
  candidate). Mixing the two produced two incompatible `der` versions and a
  build failure.
- **GitHub Actions updated**: `actions/checkout` 7.0.1, `actions/setup-node`
  7.1.0, `softprops/action-gh-release` 3.0.3.

### Fixed
- **Dependabot guard rails**: the CMS stack is grouped and `der`/`x509-cert`/
  `const-oid`/`egui`/`eframe` are pinned to the versions that build, so
  Dependabot no longer proposes unbuildable upgrade sets.
- **CI and release reliability**: the `web-ext` steps install the package once
  with retries instead of fetching it per step, so a transient npm `ECONNRESET`
  no longer fails a build or a release.
- **Manual release dispatch**: the release workflow can now be re-run from an
  existing tag (`gh workflow run release.yml --ref v0.4.1`) if the automatic
  run is lost.

### Known limitations
- The `egui` 0.37 upgrade is still blocked upstream: `fastframe` v0.14.0 pins
  `egui`/`eframe` 0.36.2, so the two versions cannot coexist in the graph.

---

## [0.4.0] - 2026-10-21

### Added
- **Authenticated GUI bridge**: every host-to-GUI request now carries a per-session
  32-byte random token stored in `%APPDATA%\VyomKunji\gui.token` (legacy
  `%APPDATA%\Signer\gui.token` remains readable). Requests without a valid token are
  rejected with `UNAUTHORIZED`.
- **Origin-aware approvals**: the extension always reports the browser's real tab URL,
  the host forwards it to the GUI, and the approval card shows it. Non-`https://*.gov.in`
  origins are marked **unverified**; local requests are labelled **Local request**.
- **Multiple digest algorithms**: `hashAlg` accepts `SHA1`, `SHA256`, `SHA384`, and
  `SHA512` for raw and pre-hashed signing.
- **EC token support**: certificates and private keys are classified as RSA or EC, and
  ECDSA signing uses the matching PKCS#11 mechanism.
- **Detached CMS / PKCS#7 output**: `pkcs7: true` returns a detached CMS `SignedData`
  containing the signing certificate and any intermediates found on the token
  (`chain`), alongside the raw signature metadata.
- **Headless PIN signing is opt-in**: direct host-side signing with a PIN requires
  `VYOMKUNJI_ALLOW_HEADLESS_PIN=1` **and** a request with no browser origin. Browser
  requests always require GUI approval.
- **Installer lifecycle**: per-user or machine-wide install, optional sign-in autostart,
  close-running-app handling, uninstall cleanup, and `install-host.ps1 -Unregister`.
- **Diagnostics**: Settings now has **Copy diagnostics** (version, module, token label,
  bridge state, recent log; no PINs, signatures, certificate bodies, or usernames), and
  `VYOMKUNJI_CRASH_LOG=1` optionally appends local panic details to
  `%LOCALAPPDATA%\VyomKunji\crash.log`.
- **Firefox MV3 migration completed** (`docs/FIREFOX-MV3.md`): the Firefox add-on is now
  Manifest V3 - host URLs moved to `host_permissions`, `browser_action` became
  `action`, `web_accessible_resources` uses the object form, and the background
  script runs as a Firefox **event page** (Firefox has no service-worker
  background). The add-on id, the shared `src/` tree, and the host protocol are
  unchanged; the MV2 manifest stays available in git history for rollback.
  `web-ext lint` reports 0 errors/warnings, and the release workflow now
  publishes a ready-to-sign `vyomkunji-firefox-<tag>.xpi`.
- **Supply-chain checks**: `deny.toml`, Dependabot updates, SHA-pinned GitHub Actions,
  and a CI `cargo-deny` job scoped to the Windows target.
- **Extension lifecycle tests**: Node tests for host probing, request correlation,
  disconnect handling, sender validation, and origin pinning; `tools/check-extensions.ps1`
  verifies byte-level Chrome/Firefox parity and manifest/script validity.
- **Version tooling**: `tools/check-version.ps1` (single-source version guard) and
  `tools/bump-version.ps1`.
- **Optional Windows code signing**: `tools/sign-windows.ps1` and release-workflow
  support for `WINDOWS_CERT_PFX_BASE64` / `WINDOWS_CERT_PASSWORD`; unsigned releases
  still publish `SHA256SUMS.txt`.

### Changed
- **Rust 2024 edition**: `native-app` now compiles with `edition = "2024"` and
  `rust-version = "1.98"` (the version required by the egui/eframe and fastframe
  dependency graph). The code now uses edition-2024 features such as `let` chains.
- **`cryptoki` 0.12.1**: fixes RUSTSEC-2026-0286 (out-of-bounds read when decoding
  `CKA_ALLOWED_MECHANISMS`).
- **Smaller dependency graph**: CMS is assembled directly through `cms`/`der`/`x509-cert`
  without the `cms` builder feature, removing the `rsa` crate and its Marvin advisory.
- **Startup log rotation**: `%TEMP%\vyomkunji-startup.log` is capped at 1 MiB with one
  `.1` generation.
- **Extension background probing**: background code probes `com.vyomkunji.host` first, then
  `com.signer.host`, keeps one native port, and rejects in-flight requests cleanly when
  the host disconnects.
- **Extension tests and checks**: `extension/tests/manifest.test.mjs` asserts the
  Manifest V3 shape of both builds, `tools/check-extensions.ps1` re-checks the same
  rules in PowerShell, and CI lints the Firefox add-on with `web-ext`.

### Fixed
- Portal names are read from the top-level `portal` field sent by the content scripts.
- Page-supplied `origin` values can no longer override the extension's real tab URL.
- A wrong PIN is no longer retried against every token slot, avoiding unnecessary retry
  counter increments.
- Legacy `com.signer.host` registrations and endpoint files are cleaned up on uninstall.
- Native-host manifests are written as UTF-8 without a BOM.
- Stale port/token files are removed on GUI shutdown, and the token is published before
  the port during startup.
- The installer's completion message expands `{app}` correctly.

---

## [0.3.0] - 2026-10-21

### Added
- **AGPL-3.0 License**: Added GNU Affero General Public License version 3.0 (`LICENSE`).
- **Project Rebranding**: Renamed project to **VyomKunji** across native binary (`vyomkunji`), host (`vyomkunji-host`), Chrome MV3 and Firefox MV2 extensions (`VyomKunji India G2C`), storage path (`%APPDATA%\VyomKunji`), and documentation. Backwards compatibility preserved for legacy `signer` binaries, hosts, and paths.
- `fastframe-shell` resident shell: the app keeps running with its window closed and
  re-opens by itself when a portal asks for a signature.
- `fastframe-tray` notification-area item (Show Signer / Pending requests / Quit).
- `fastframe-text` + `fastframe-fonts`: Inter as the interface face, with the desktop's
  own hinting and antialiasing.
- `fastframe-theme`: the colour palette is now a `fastframe_theme::Palette` implementation,
  so it can be overridden by a JSON file (`%APPDATA%\Signer\theme.json`).
- Light and dark palettes with a **Light mode / Dark mode** toggle in the top bar.
- Runtime-generated application and tray icon (keyhole mark, no image decoder needed).
- `docs/USAGE.md` — full end-to-end guide to signing on a portal.
- `docs/THEME.md` — how to write a palette file.
- Git repository with `master` (egui 0.31 baseline) and `feat/fastframe-shell` branches.
- **Startup diagnostics**: setting `VYOMKUNJI_STARTUP_LOG=1` appends timestamped startup milestones to `%TEMP%\vyomkunji-startup.log`.

### Changed
- **egui / eframe 0.31 → 0.36.2** (the new `App::ui` API, `Panel::top/left/bottom`).
- **PKCS#11 auto-detection broadened**: added SafeNet/Gemalto, G&D StarSign (SafeSign), Precision InnaIT, and mToken (CryptoID) module names/directories, plus `safesign`/`aet`/`starcos`/`inna`/`precision`/`csp11` scan patterns.
- The native host initialises PKCS#11 lazily and only after the GUI bridge declines a signature request.
- The GUI was redesigned: sidebar navigation, card-based screens, certificate grid with
  expiry colouring, request approval card, activity log, status bar.
- Release binary grew from 4.99 MB to 8.53 MB (Inter, tray and shell crates; stripped release profile).
- **Version bumped to `0.3.0`** across Cargo, extension manifests, installer, and documentation.

### Performance
- **Cold start is now ~10× faster on the UI path.** Module discovery parses each DLL's PE export table for `C_GetFunctionList` instead of calling `LoadLibraryW`, and the directory walk uses the file type Windows already returned (`DirEntry::file_type`) rather than re-statting every entry. On the test machine discovery fell from **3.06 s to ~0.13 s**.
- PKCS#11 detection now runs on a background thread. The window paints immediately (**~210–260 ms to first frame** in release builds) and shows “Detecting token…” while the driver is probed.
- Discovery results are cached for the process; the module that last exposed a token is remembered in `%APPDATA%\VyomKunji\last_module.txt` and tried first on the next launch.
- Driver probes have a 6-second timeout. A hung vendor driver is skipped for the session instead of blocking detection forever; Settings → Refresh list re-enables it.
- Hot-plug polling uses a cheap slot label/serial query on a worker thread instead of re-reading every certificate on the UI thread (which blocked for ~2.5 s per token on the test machine).
- Release builds use thin LTO, one codegen unit, and stripped symbols.

### Fixed
- The `✓` glyph is drawn by the painter instead of the font, so it no longer depends on
  the system font covering it.
- Certificate metadata now carries issuer, serial and validity, so expiry can be
  colour-coded.
- **Firefox Native Messaging manifest**: Changed `allowed_origins` with UUID to `allowed_extensions` matching `signer-g2c@signer.local`.
- **Main-world network hook**: Added `interceptor-main.js` injected directly into page DOM to intercept `fetch` and `XMLHttpRequest` to localhost emSigner/dsigner endpoints across isolated worlds.
- **Certificate DER delivery**: Signature responses now return the base64-encoded X.509 certificate DER (`cert`) required by Indian portals to verify signatures.
- **Pre-hashed payload support**: Added `alreadyHashed` handling in PKCS#11; properly wraps 32-byte SHA-256 digests in ASN.1 DigestInfo and uses `Mechanism::RsaPkcs`.
- **Unit test suite**: Added unit tests for civil date conversions, ISO timestamps, and date diffing.

---

## [0.2.3] — 2026-10-21

### Fixed
- **Console window on launch**: added `#![windows_subsystem = "windows"]` to the GUI binary so `vyomkunji.exe` no longer opens a persistent CMD window (closing it no longer kills the app).

## [0.2.2] — 2026-10-21

### Added
- **GUI PKCS#11 module picker**: Settings screen with a module path field, a dropdown of discovered modules, Apply/Auto-detect/Refresh actions, persisted to `%APPDATA%\\VyomKunji\\config.json`.

## [0.2.1] — 2026-10-21

### Fixed
- **HYP2003 (HyperPKI) token detection**: added `HYP2003csp11IND.dll` (and the `_s` variant) to the PKCS#11 module name list and the directory-scan pattern (`csp11`), so HyperPKI HYP2003 tokens are auto-detected on Windows.

## [0.2.0] — 2026-10-21

### Added
- **Fixed Chrome extension ID**: shipped a `key` in `extension/manifest.json`, making the extension ID deterministic (`cnmiodibehhadaglhadkalhgjjeiblfp`) so the native host whitelist works out of the box.
- **One-click local installer**: `install.ps1` plus an Inno Setup script (`installer/vyomkunji.iss`) bundling binaries, manifests, and both extensions, registering the native host post-install.
- **PIN zeroize + idle auto-lock**: PIN held in a `Zeroizing<String>` and cleared after a configurable idle timeout (`VYOMKUNJI_PIN_TIMEOUT_MIN`, default 5 min).
- **USB hot-plug awareness**: GUI re-enumerates the token every 3 s and surfaces connect/disconnect/update status.
- **Expanded PKCS#11 auto-detection**: scans System32/SysWOW64 and vendor install directories, filters by `C_GetFunctionList`, tries every candidate, prefers a module exposing an inserted token; env override accepts a directory.

### Changed
- Version bumped to `0.2.0` across Cargo, manifests, installer, and docs.

---

## [0.1.0] — 2026-10-21

### Added
- **Native app** (`signer-host`): Chrome/Firefox native-messaging host.
  - Real PKCS#11 access through `cryptoki`; ProxKey's `WDPKCS.dll` auto-detected.
  - Token and certificate enumeration (verified against a ProxKey: two DSC owners read).
  - `SHA256_RSA_PKCS` on-token signing with certificate selection.
  - Message types `ping`, `status`, `listTokens`, `sign` (documented in `shared/protocol.md`).
- **Local bridge** between the host and the GUI (`bridge.rs`): newline-delimited JSON over
  `127.0.0.1`, port published to `%APPDATA%\Signer\gui.port`.
- **GUI** (`gui`): token browser, certificate list, approval panel with PIN entry,
  manual signing, activity log.
- **Chrome extension** (MV3): Income Tax content script plus a generic script for
  GST, MCA, EPFO, TDS and TRACES; emSigner/DSC-signer localhost shim; popup with
  default-certificate selection; options page.
- **Firefox extension** (MV2) sharing the same sources, with its own native-messaging
  host manifest and an installer that registers both browsers.
- `install-host.ps1` registers the native host for Chrome and/or Firefox.
- `tools/sync-extensions.ps1` keeps the two extensions' shared sources in step.
- `features.md` — comparison against xorkeesign India G2C.
- `plan/PLAN.md`, `shared/protocol.md`, `docs/portal-integration/incometax.md`.

### Known issues
- The emSigner shim returns a generic response envelope; each portal's exact payload and
  response shape still needs a live capture (Income Tax first).
- Signing is implemented but not yet confirmed end-to-end against a live token PIN.
