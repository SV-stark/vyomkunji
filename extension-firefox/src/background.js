// background.js — service worker
const NATIVE_HOST = "com.signer.host";

let port = null;
function connectNative() {
  if (port) return port;
  try {
    port = chrome.runtime.connectNative(NATIVE_HOST);
    port.onDisconnect.addListener(() => {
      console.warn("native host disconnected", chrome.runtime.lastError);
      port = null;
    });
    port.onMessage.addListener((msg) => {
      // fan out to content scripts / popup via runtime messages
      chrome.runtime.sendMessage({ type: "nativeResponse", payload: msg }).catch(() => {});
    });
  } catch (e) {
    console.error("native connect failed", e);
    port = null;
  }
  return port;
}

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (msg.type === "nativeRequest") {
    const p = connectNative();
    if (!p) {
      sendResponse({ ok: false, error: { code: "HOST_UNAVAILABLE", message: "Native host not connected" } });
      return true;
    }
    // correlate by id
    const handler = (resp) => {
      if (resp && resp.id === msg.payload.id) {
        p.onMessage.removeListener(handler);
        sendResponse(resp);
      }
    };
    p.onMessage.addListener(handler);
    p.postMessage(msg.payload);
    return true; // async
  }
  if (msg.type === "ping") {
    sendResponse({ ok: true });
    return false;
  }
  return false;
});

// keep-alive-ish: ensure connection on startup
connectNative();
console.log("Signer background ready");
