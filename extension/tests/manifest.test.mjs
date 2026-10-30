// Manifest-shape tests for the Chrome (MV3 service worker) and Firefox
// (MV3 event page) extension builds.
// Run with: node --test extension/tests/manifest.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import test from "node:test";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const chrome = JSON.parse(
  readFileSync(join(root, "extension", "manifest.json"), "utf8")
);
const firefox = JSON.parse(
  readFileSync(join(root, "extension-firefox", "manifest.json"), "utf8")
);

const HOST_PATTERN = /^https:\/\/(\*\.)?[a-z0-9.-]+\.(gov|nic)\.in\/\*$/;

function hostEntries(manifest) {
  return manifest.host_permissions || [];
}

test("both builds target Manifest V3", () => {
  assert.equal(chrome.manifest_version, 3);
  assert.equal(firefox.manifest_version, 3);
});

test("the Chrome build uses a service worker, the Firefox build an event page", () => {
  assert.equal(chrome.background.service_worker, "src/background.js");
  assert.equal(chrome.background.scripts, undefined);
  assert.deepEqual(firefox.background.scripts, ["src/background.js"]);
  // Firefox does not implement the Chrome service-worker background model.
  assert.equal(firefox.background.service_worker, undefined);
});

test("neither build uses the MV2 browser_action key", () => {
  assert.equal(chrome.browser_action, undefined);
  assert.equal(firefox.browser_action, undefined);
  assert.ok(chrome.action.default_popup);
  assert.ok(firefox.action.default_popup);
});

test("host permissions live in host_permissions, not in permissions", () => {
  for (const manifest of [chrome, firefox]) {
    const hosts = hostEntries(manifest);
    assert.ok(hosts.length > 0, "host_permissions must not be empty");
    for (const entry of hosts) {
      assert.match(entry, HOST_PATTERN);
    }
    for (const permission of manifest.permissions || []) {
      assert.ok(
        !permission.includes("://"),
        `host pattern ${permission} must not live in permissions`
      );
    }
  }
});

test("the Chrome and Firefox host permissions stay identical", () => {
  assert.deepEqual(hostEntries(chrome), hostEntries(firefox));
});

test("web_accessible_resources use the MV3 object form with matches", () => {
  for (const manifest of [chrome, firefox]) {
    assert.ok(Array.isArray(manifest.web_accessible_resources));
    for (const entry of manifest.web_accessible_resources) {
      assert.ok(Array.isArray(entry.resources));
      assert.ok(Array.isArray(entry.matches));
      assert.ok(entry.matches.length > 0);
    }
  }
  const chromeMatches =
    chrome.web_accessible_resources[0].matches.join(",");
  const firefoxMatches =
    firefox.web_accessible_resources[0].matches.join(",");
  assert.equal(chromeMatches, firefoxMatches);
});

test("content script registrations stay identical", () => {
  assert.equal(chrome.content_scripts.length, firefox.content_scripts.length);
  for (let i = 0; i < chrome.content_scripts.length; i += 1) {
    assert.deepEqual(
      chrome.content_scripts[i].matches,
      firefox.content_scripts[i].matches
    );
    assert.deepEqual(
      chrome.content_scripts[i].js,
      firefox.content_scripts[i].js
    );
  }
});

test("the Firefox build keeps its add-on id and declares no data collection", () => {
  const gecko = firefox.browser_specific_settings.gecko;
  assert.match(gecko.id, /@/);
  assert.ok(gecko.strict_min_version);
  assert.deepEqual(gecko.data_collection_permissions.required, ["none"]);
  // The Chrome signing key must not leak into the Firefox build.
  assert.equal(firefox.key, undefined);
});

test("the two builds ship the same version", () => {
  assert.equal(chrome.version, firefox.version);
});
