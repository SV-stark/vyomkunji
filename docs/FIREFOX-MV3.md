# Firefox Manifest V3 (completed migration)

Status: **done**. The Firefox add-on in `extension-firefox/` is a Manifest V3
extension. It is validated by Mozilla's own `web-ext lint` (0 errors, 0 warnings)
and by the repository's manifest-shape tests.

Firefox does not use Chromium's service-worker background model. A Firefox MV3
extension runs a non-persistent **event page** declared with
`background.scripts`, so the shared `src/` tree is reused verbatim while the
two manifests stay platform-specific.

## What changed

| MV2 field (old Firefox build) | MV3 field (current Firefox build) |
| --- | --- |
| `"manifest_version": 2` | `"manifest_version": 3` |
| `"browser_action": { ... }` | `"action": { ... }` |
| host URLs inside `"permissions"` | `"host_permissions": [ ... ]`; `permissions` keeps `nativeMessaging`, `storage`, `activeTab` |
| `"background": { "scripts": [...] }` | `"background": { "scripts": [...] }` - Firefox MV3 event page, **no** `service_worker` key and no `persistent` key |
| `"web_accessible_resources": ["src/content/interceptor-main.js"]` | array of objects: `{ "resources": [...], "matches": [ ...portal origins... ] }` |
| - | `browser_specific_settings.gecko.data_collection_permissions.required = ["none"]` (AMO declaration, Firefox 140+) |
| - | `browser_specific_settings.gecko.strict_min_version` raised to `140.0` |

The Chrome build (`extension/manifest.json`) stays MV3 with
`background.service_worker`. The shared `src/` tree is byte-identical between
the two builds; `tools/sync-extensions.ps1` enforces that and
`tools/check-extensions.ps1` fails the build if the two drift apart.

## Background script (event page) behaviour

`src/background.js` is written to run unchanged as a Chrome service worker and
as a Firefox event page:

1. **No DOM APIs.** The script only uses `chrome.runtime.*`, `setTimeout`, and
   `crypto.randomUUID()`, all of which exist in both contexts.
2. **Host probing.** `com.vyomkunji.host` is probed first, `com.signer.host`
   second. The result is cached; a disconnect clears the cache so the next
   request re-probes.
3. **Request correlation.** Every request carries an id; responses are routed
   back by id, and unrelated messages are ignored.
4. **Disconnect handling.** When the native host goes away, every in-flight
   request is rejected with `HOST_DISCONNECTED` instead of hanging.
5. **Suspension.** Firefox may terminate an idle event page. The next message
   wakes it, the state is rebuilt, and the host is probed again. A request that
   was in flight during the teardown is rejected, never silently lost.
6. **Origin pinning.** The tab URL reported by the browser always wins over
   anything a page puts in the payload.

## Validation performed

* `npx web-ext lint --source-dir extension-firefox` - 0 errors, 0 warnings,
  0 notices (Mozilla's validator).
* `npx web-ext build --source-dir extension-firefox` - produces a loadable
  add-on package; the release workflow publishes it as
  `vyomkunji-firefox-<tag>.xpi`.
* `node --test extension/tests/manifest.test.mjs` - asserts the MV3 shape of
  both builds (background model, `action`, `host_permissions`, object-form
  `web_accessible_resources`, identical content-script registrations, add-on
  id, and no Chrome signing key in the Firefox build).
* `tools/check-extensions.ps1` - byte-level parity of the shared sources plus
  the same manifest assertions in PowerShell.
* `tools/sync-extensions.ps1` - keeps `extension/src` and
  `extension-firefox/src` identical.

## Manual test checklist (Firefox)

1. `about:debugging#/runtime/this-firefox` -> **Load Temporary Add-on** ->
   `extension-firefox/manifest.json`.
2. The add-on must load **without** a manifest warning; the background entry
   shows an event page (not a service worker).
3. Open `https://www.incometax.gov.in/`, trigger a DSC signature, and confirm
   the desktop app shows the portal name and the real tab URL.
4. Wait ~30 s so the event page is suspended, trigger another signature, and
   confirm the host is re-probed and the request still succeeds.
5. Stop `vyomkunji-host.exe` while a request is pending: the content script
   must receive `HOST_DISCONNECTED` and the approval card must close.
6. Open the popup and the options page: both must render, and the popup must
   show the token status.
7. Confirm the host permission prompt lists only the government portal origins.

## Rollback

The MV2 build is preserved in git history:

```powershell
git log --oneline -- extension-firefox/manifest.json
git show <commit>:extension-firefox/manifest.json
```

To go back to MV2, restore the previous `extension-firefox/manifest.json`
(`manifest_version: 2`, `browser_action`, host URLs in `permissions`, string
`web_accessible_resources`) and re-run `tools/sync-extensions.ps1`. The host
protocol and the shared `src/` tree are unaffected, so no native-app change is
required.

## Known limitations

* Firefox does not implement `background.service_worker`; the event page stays
  the supported model. The shared script therefore avoids service-worker-only
  and page-only APIs.
* MV3 in Firefox requires a fairly recent browser (`strict_min_version`
  `140.0`), which is also the first ESR that understands the AMO data
  collection declaration.
* Remote code is forbidden in MV3. Everything the add-on needs is packaged; no
  script is fetched from the network.
