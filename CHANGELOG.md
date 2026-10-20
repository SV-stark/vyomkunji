# Changelog

All notable changes to **Signer — India G2C Digital Signing** are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
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

### Changed
- **egui / eframe 0.31 → 0.36.2** (the new `App::ui` API, `Panel::top/left/bottom`).
- The GUI was redesigned: sidebar navigation, card-based screens, certificate grid with
  expiry colouring, request approval card, activity log, status bar.
- Release binary grew from 4.99 MB to 8.77 MB (Inter, tray and shell crates).

### Fixed
- The `✓` glyph is drawn by the painter instead of the font, so it no longer depends on
  the system font covering it.
- Certificate metadata now carries issuer, serial and validity, so expiry can be
  colour-coded.

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
