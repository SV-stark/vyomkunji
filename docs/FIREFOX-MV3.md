# Firefox MV3 migration plan

Status: **planned, not started**. The shipped Firefox build remains Manifest V2
(`extension-firefox/manifest.json`) and must keep working while the MV3 port is
developed and tested.

Firefox supports Manifest V3, but it does not use Chromium's service-worker
background model. Firefox MV3 runs a non-persistent **event page** declared with
`background.scripts`. That difference is the main reason this port is a separate
plan rather than a copy of the Chrome manifest.

## Why migrate

- Mozilla has committed to MV3 as the long-term extension platform.
- MV2 remains acceptable on addons.mozilla.org for now, but new APIs and store
  tooling increasingly assume MV3.
- Chrome already ships MV3, so a single conceptual model (non-persistent
  background + explicit host permissions) makes the two extensions easier to
  reason about.

## Non-goals

- Do not switch Firefox to `background.service_worker`. Firefox's event page is
  the supported mechanism; a service worker would simply fail to start.
- Do not drop MV2 support in the same release. Keep a tagged MV2 build available
  for Firefox ESR and for rollback.
- Do not broaden host permissions. The migration must keep the same government
  portal allow-list.

## Manifest changes

| MV2 field | MV3 replacement |
| --- | --- |
| `"manifest_version": 2` | `"manifest_version": 3` |
| `"browser_action"` | `"action"` |
| `"permissions": [ ...https URLs... ]` | `"host_permissions": [ ...https URLs... ]`; keep `nativeMessaging`, `storage`, `activeTab` in `permissions` |
| `"background": { "scripts": [...] }` | `"background": { "scripts": [...], "persistent": false }` (event page; no service worker) |
| `"web_accessible_resources": ["file.js"]` | array of objects: `{ "resources": ["file.js"], "matches": ["https://..."] }` |
| `"content_security_policy": "..."` (string) | object with `extension_pages` (and `sandbox` only if used) |
| remote script/CDN references | forbidden; everything must be packaged |

`browser_specific_settings.gecko.id` stays
(`vyomkunji-g2c@vyomkunji.local`). `strict_min_version` should be raised to the
first Firefox release used for MV3 validation (currently 115 ESR is the floor;
test on current ESR and current release before choosing the final value).

The Chrome manifest is intentionally **not** a template for Firefox:
`extension/manifest.json` uses `"service_worker"` and a Chrome `"key"`, neither
of which belongs in the Firefox manifest.

## Background script changes

`extension-firefox/src/background.js` is currently shared verbatim with Chrome
through `tools/sync-extensions.ps1`. The code already avoids DOM-only APIs and
uses `chrome.runtime.connectNative`, `onMessage`, `onDisconnect`, and
`setTimeout`, so it should run as an event page. Verify these points:

1. **Lifetime.** The event page can be suspended between requests. Do not cache
   mutable state across events without persisting it. On wake-up, re-probe the
   native host (`com.vyomkunji.host`, then `com.signer.host`) before sending a
   request.
2. **Pending requests.** An in-flight `nativeRequest` keeps the event page alive
   in practice, but the 120 s timeout must still reject cleanly if Firefox
   suspends or the port disconnects. The existing `HOST_DISCONNECTED` path
   covers this; add an event-page test that forces a port disconnect.
3. **Timers.** `setTimeout` is available in event pages. If a future refactor
   moves to a service worker, swap the timeout mechanism.
4. **Top-level assumptions.** `resolveHost()` currently starts a probe at load.
   That is acceptable for an event page, but it should be lazy (probe on the
   first request) so a wake-up caused by a popup message does not race two
   probes.
5. **Message channel.** Return `true` from `onMessage` when responding
   asynchronously. Firefox MV3 keeps the channel open under the same rules.

No code change is required for the `browser`/`chrome` namespace: Firefox exposes
both, and the existing `chrome.*` calls work.

## Host permissions and content scripts

- Content script `matches` stay unchanged.
- `web_accessible_resources` must list the exact portal origins that may load
  `src/content/interceptor-main.js`. Use the same allow-list as `matches`; do
  not use `"<all_urls>"`.
- If runtime injection is ever added, request the specific origin with
  `browser.permissions.request({ origins: [...] })` and handle denial. The
  current build injects only through declarative content scripts, so no
  `scripting` permission is needed.

## Packaging and store metadata

- Firefox uses the same `src/` tree. Keep `tools/sync-extensions.ps1` as the
  single source of truth and never edit `extension-firefox/src` by hand.
- Add a Firefox-specific manifest check to `tools/check-extensions.ps1`:
  reject `service_worker`, require `background.scripts`, require
  `host_permissions` (not host URLs in `permissions`), and require
  `action`/`web_accessible_resources` in the MV3 shape.
- Build the XPI with `web-ext build --source-dir extension-firefox` and lint
  with `web-ext lint`. Keep the generated XPI out of git.
- AMO requires source submission when the package is built from minified or
  generated code. This project ships readable source, so the normal source
  archive is sufficient; document the exact build command in the release notes.

## Testing plan

1. **Static checks**
   - `node --check` every script (already in `tools/check-extensions.ps1`).
   - `web-ext lint --source-dir extension-firefox`.
   - Manifest assertions described above.
2. **Load unpacked / temporary add-on**
   - `about:debugging#/runtime/this-firefox` → Load Temporary Add-on →
     `extension-firefox/manifest.json`.
   - Confirm the event page appears under "Inspect" and that no service-worker
     error is logged.
3. **Lifecycle**
   - Trigger a signature, wait for the event page to suspend (about 30 s idle),
     trigger another signature, and confirm the host re-probes and signs.
   - Kill `vyomkunji-host.exe` while a request is pending; confirm the content
     script receives `HOST_DISCONNECTED` and the approval card closes.
   - Close and reopen the popup; confirm token/certificate state reloads.
4. **Origin enforcement**
   - On an allowed portal, confirm the GUI shows the real tab URL.
   - On a non-allowed `https://*.gov.in` page, confirm the GUI warns.
   - From a cross-origin iframe, confirm the content script ignores the message.
5. **Native messaging**
   - Test a fresh profile with only `com.vyomkunji.host` registered, then only
     `com.signer.host`, then both.
   - Test after an uninstall/reinstall of the host.
6. **Regression**
   - Run the Chrome build and the MV2 Firefox build against the same host.
   - Verify CMS (`pkcs7`) and chain fields survive the event-page round trip.

## Rollback plan

- Keep the last MV2 XPI attached to the release and keep the MV2 tag.
- If MV3 fails validation, revert `extension-firefox/manifest.json` to MV2 and
  republish the previous XPI; the host protocol is unchanged, so no native-app
  rollback is needed.
- Store the MV3 work on a branch until the checklist above passes. Do not ship
  MV3 and MV2 as the same add-on version: publish a new version only after the
  MV3 XPI passes AMO review.

## Suggested sequence

1. Add the Firefox manifest assertions and a `web-ext lint` CI job (MV2 first).
2. Convert a branch manifest to MV3 and run it as a temporary add-on.
3. Make host probing lazy and add an event-page disconnect test.
4. Run the lifecycle and portal tests on Firefox ESR and current release.
5. Publish the MV3 XPI, keep the MV2 XPI available, and update this document
   with the tested Firefox versions.
