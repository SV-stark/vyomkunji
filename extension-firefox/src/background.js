// background.js — native messaging bridge for Chrome MV3 (service worker)
// and Firefox MV3 (event page).
//
// Responsibilities:
//  * pick a working native host (new name first, legacy name as fallback),
//  * keep one persistent native port,
//  * correlate responses by request id,
//  * fail every in-flight request on disconnect instead of hanging forever,
//  * attach the real tab origin so the GUI can show it on the approval card.
//
// The script is deliberately free of DOM APIs so the same file runs as a
// Chrome service worker and as a Firefox event page. Both browsers may
// suspend the background context; the next request re-probes the host, and
// a disconnect rejects every in-flight request instead of leaving it hanging.
const NATIVE_HOST = "com.vyomkunji.host";
const LEGACY_HOST = "com.signer.host";
const REQUEST_TIMEOUT_MS = 120000;
const PROBE_TIMEOUT_MS = 2500;

function makeError(code, message) {
  return { ok: false, error: { code, message } };
}

let activeHost = null;
let active = null; // { host, port, pending: Map<id, {resolve, timer}> }
let resolving = null;

function probeHost(host) {
  return new Promise((resolve) => {
    let port;
    try {
      port = chrome.runtime.connectNative(host);
    } catch (_) {
      resolve(false);
      return;
    }
    let done = false;
    const finish = (ok) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      try { port.disconnect(); } catch (_) {}
      resolve(ok);
    };
    const timer = setTimeout(() => finish(false), PROBE_TIMEOUT_MS);
    port.onMessage.addListener((msg) => {
      if (msg && msg.id === "__vyomkunji_probe__") finish(!!msg.ok);
    });
    port.onDisconnect.addListener(() => {
      // Read lastError so the browser does not log an unchecked error.
      try { void chrome.runtime.lastError; } catch (_) {}
      finish(false);
    });
    try {
      port.postMessage({ id: "__vyomkunji_probe__", type: "ping" });
    } catch (_) {
      finish(false);
    }
  });
}

async function resolveHost() {
  if (activeHost) return activeHost;
  if (!resolving) {
    resolving = (async () => {
      for (const host of [NATIVE_HOST, LEGACY_HOST]) {
        if (await probeHost(host)) {
          activeHost = host;
          break;
        }
      }
      return activeHost;
    })().finally(() => {
      resolving = null;
    });
  }
  return resolving;
}

function openPort(host) {
  const port = chrome.runtime.connectNative(host);
  const state = { host, port, pending: new Map() };
  port.onMessage.addListener((msg) => {
    const id = msg && msg.id;
    const entry = state.pending.get(id);
    if (!entry) return;
    state.pending.delete(id);
    clearTimeout(entry.timer);
    entry.resolve(msg);
  });
  port.onDisconnect.addListener(() => {
    let message = "native host disconnected";
    try {
      const err = chrome.runtime.lastError;
      if (err && err.message) message = err.message;
    } catch (_) {}
    for (const entry of state.pending.values()) {
      clearTimeout(entry.timer);
      entry.resolve(makeError("HOST_DISCONNECTED", message));
    }
    state.pending.clear();
    if (active === state) active = null;
    // Force a fresh probe on the next request: the host may have been upgraded
    // or the legacy fallback may now be the right target.
    activeHost = null;
  });
  return state;
}

function newId() {
  try {
    return crypto.randomUUID();
  } catch (_) {
    return "vk-" + Date.now() + "-" + Math.random().toString(16).slice(2);
  }
}

async function sendNative(payload) {
  const host = await resolveHost();
  if (!host) {
    return makeError(
      "HOST_UNAVAILABLE",
      "VyomKunji native host is not registered for this browser"
    );
  }

  let state;
  try {
    state = active && active.host === host ? active : openPort(host);
  } catch (e) {
    // The host is not installed or the browser refused the connection.
    activeHost = null;
    return makeError(
      "HOST_UNAVAILABLE",
      "VyomKunji native host is not registered for this browser (" + String(e) + ")"
    );
  }
  active = state;

  return new Promise((resolve) => {
    const id = payload.id || newId();
    const wire = Object.assign({}, payload, { id });
    const timer = setTimeout(() => {
      state.pending.delete(id);
      resolve(makeError("TIMEOUT", "The signing request timed out"));
    }, REQUEST_TIMEOUT_MS);
    state.pending.set(id, { resolve, timer });
    try {
      state.port.postMessage(wire);
    } catch (e) {
      clearTimeout(timer);
      state.pending.delete(id);
      if (active === state) active = null;
      activeHost = null;
      resolve(makeError("HOST_DISCONNECTED", String(e)));
    }
  });
}

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  // Only accept messages from this extension's own content scripts/pages.
  if (!sender || sender.id !== chrome.runtime.id) return false;

  if (msg && msg.type === "nativeRequest") {
    const payload = Object.assign({}, msg.payload || {});
    // Always trust the browser-reported tab URL over anything supplied in the
    // page payload: the approval card must show where the request came from.
    payload.origin =
      (sender.tab && sender.tab.url) || sender.origin || sender.url || "";
    sendNative(payload)
      .then(sendResponse)
      .catch((e) => sendResponse(makeError("INTERNAL", String(e))));
    return true; // async response
  }

  if (msg && msg.type === "ping") {
    sendResponse({ ok: true });
    return false;
  }
  return false;
});

// Warm the host cache when the worker starts. This is best-effort: the
// promise is re-created on demand if the worker is suspended first.
resolveHost().catch(() => {});
