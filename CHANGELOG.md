# Changelog

All notable changes to **VyomKunji — India G2C Digital Signing** are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.0] — 2026-10-21

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
