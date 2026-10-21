// Lifecycle tests for the extension's native-messaging bridge.
// Run with: node --test extension/tests
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import test from "node:test";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(join(here, "..", "src", "background.js"), "utf8");

function harness(options = {}) {
  const { hostOk = true, onRequest = null } = options;
  const ports = [];
  const state = { lastSent: null };
  const listeners = [];

  function makePort(host) {
    const port = {
      host,
      posted: [],
      disconnected: false,
      onMessage: { listeners: [], addListener(fn) { this.listeners.push(fn); } },
      onDisconnect: { listeners: [], addListener(fn) { this.listeners.push(fn); } },
      emit(message) {
        for (const fn of this.onMessage.listeners) fn(message);
      },
      disconnect() {
        if (this.disconnected) return;
        this.disconnected = true;
        for (const fn of this.onDisconnect.listeners) fn();
      },
      postMessage(message) {
        this.posted.push(message);
        state.lastSent = { host, message, port: this };
        queueMicrotask(() => {
          if (!hostOk) {
            this.disconnect();
            return;
          }
          if (message.type === "ping") {
            this.emit({ id: message.id, ok: true, version: "test" });
            return;
          }
          if (onRequest) onRequest(this, message);
        });
      },
    };
    ports.push(port);
    return port;
  }

  const chrome = {
    runtime: {
      id: "test-extension",
      lastError: null,
      onMessage: {
        addListener(fn) {
          listeners.push(fn);
        },
      },
      connectNative(host) {
        return makePort(host);
      },
    },
  };

  const context = vm.createContext({
    chrome,
    crypto: { randomUUID: () => "uuid-" + Math.random().toString(16).slice(2) },
    setTimeout,
    clearTimeout,
    queueMicrotask,
    console,
    Promise,
    Map,
    Object,
  });
  vm.runInContext(source, context, { filename: "background.js" });

  async function send(message, sender = { id: "test-extension" }) {
    assert.equal(listeners.length, 1, "background should register one listener");
    return new Promise((resolve) => {
      const async = listeners[0](message, sender, resolve);
      assert.equal(async, true, "listener must keep the response channel open");
    });
  }

  return { chrome, ports, state, send, listener: listeners[0] };
}

test("attaches the tab URL as origin and preserves the request id", async () => {
  const h = harness({
    onRequest: (port, message) =>
      port.emit({ id: message.id, ok: true, signature: "sig" }),
  });
  const url = "https://www.incometax.gov.in/iec/foportal/";
  const response = await h.send(
    {
      type: "nativeRequest",
      payload: {
        id: "req-1",
        type: "sign",
        data: "abc",
        origin: "https://evil.example/",
      },
    },
    { id: "test-extension", tab: { url } }
  );

  assert.equal(response.ok, true);
  assert.equal(response.signature, "sig");
  const wire = h.state.lastSent;
  assert.equal(wire.host, "com.vyomkunji.host");
  assert.equal(wire.message.id, "req-1");
  assert.equal(wire.message.origin, url);
});

test("correlates responses by id and ignores unrelated messages", async () => {
  const h = harness({
    onRequest: (port, message) => {
      port.emit({ id: "some-other-request", ok: true, signature: "wrong" });
      port.emit({ id: message.id, ok: true, signature: "right" });
    },
  });
  const response = await h.send({
    type: "nativeRequest",
    payload: { id: "req-2", type: "sign", data: "abc" },
  });
  assert.equal(response.signature, "right");
});

test("rejects in-flight requests when the native host disconnects", async () => {
  const h = harness({ onRequest: (port) => port.disconnect() });
  const response = await h.send({
    type: "nativeRequest",
    payload: { id: "req-3", type: "sign", data: "abc" },
  });
  assert.equal(response.ok, false);
  assert.equal(response.error.code, "HOST_DISCONNECTED");
});

test("falls back to the legacy host name when the new host is missing", async () => {
  const ports = [];
  const chrome = {
    runtime: {
      id: "test-extension",
      lastError: null,
      onMessage: { addListener() {} },
      connectNative(host) {
        const port = {
          host,
          onMessage: { listeners: [], addListener(fn) { this.listeners.push(fn); } },
          onDisconnect: { listeners: [], addListener(fn) { this.listeners.push(fn); } },
          emit(message) { for (const fn of this.onMessage.listeners) fn(message); },
          disconnect() {},
          postMessage(message) {
            if (host === "com.vyomkunji.host") {
              this.onDisconnect.listeners.forEach((fn) => fn());
            } else {
              this.emit({ id: message.id, ok: true, version: "test" });
            }
          },
        };
        ports.push(port);
        return port;
      },
    },
  };
  const context = vm.createContext({
    chrome,
    crypto: { randomUUID: () => "uuid" },
    setTimeout,
    clearTimeout,
    queueMicrotask,
    console,
    Promise,
    Map,
    Object,
  });
  vm.runInContext(source, context, { filename: "background.js" });
  // The startup probe runs asynchronously; give it a tick.
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.ok(ports.some((p) => p.host === "com.signer.host"));
});

test("ignores messages from another extension", () => {
  const h = harness();
  const before = h.ports.length;
  const result = h.listener(
    { type: "nativeRequest", payload: { type: "sign", data: "abc" } },
    { id: "other-extension" },
    () => {
      throw new Error("must not respond to a foreign extension");
    }
  );
  assert.equal(result, false);
  assert.equal(h.ports.length, before);
});
