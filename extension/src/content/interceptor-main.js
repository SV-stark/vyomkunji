// interceptor-main.js — Injected into the page's MAIN execution world to intercept
// localhost/emSigner/emBridge/CrisSigner requests from portal scripts.
(function () {
  "use strict";

  if (window.__signer_injected) return;
  window.__signer_injected = true;

  function isSignerUrl(url) {
    if (!url) return false;
    const str = String(url).toLowerCase();
    const isLocal = str.includes("127.0.0.1") || str.includes("localhost");
    if (!isLocal) return false;

    // Common ports for Indian government DSC signers:
    // 8080-8090 (emSigner), 1440-1450 (emBridge), 1580-1590 (legacy DSC signer),
    // 1625-1635 (NICDSign), 26000-26005 (CrisSigner), 9090-9100
    const portMatch = str.match(/:(\d+)/);
    if (portMatch) {
      const port = parseInt(portMatch[1], 10);
      if (
        (port >= 8080 && port <= 8090) ||
        (port >= 1440 && port <= 1450) ||
        (port >= 1580 && port <= 1590) ||
        (port >= 1625 && port <= 1635) ||
        (port >= 26000 && port <= 26005) ||
        (port >= 9090 && port <= 9100) ||
        port === 4567 || port === 5678 || port === 12000 || port === 12001
      ) {
        return true;
      }
    }

    return (
      str.includes("emsigner") ||
      str.includes("embridge") ||
      str.includes("crissigner") ||
      str.includes("dsigner") ||
      str.includes("dsc") ||
      str.includes("sign") ||
      str.includes("token") ||
      str.includes("cert") ||
      str.includes("pki")
    );
  }

  function isHealthUrl(url, body) {
    const str = String(url || "").toLowerCase();
    const bodyStr = String(body || "").toLowerCase();
    const path = str.split("?")[0].replace(/^(https?|ws|wss):\/\/[^/]+/i, "").toLowerCase();
    if (path === "" || path === "/" || path === "/isalive" || path === "/check" || path === "/ping" || path === "/version" || path === "/status" || path === "/checkservice") {
      return true;
    }
    if (path.includes("version") && !path.includes("sign")) {
      return true;
    }
    if (bodyStr.includes('"action":"check"') || bodyStr.includes('"action":"isalive"') || bodyStr.includes('"action":"version"')) {
      return true;
    }
    return false;
  }

  function isCertListUrl(url, body) {
    const str = String(url || "").toLowerCase();
    const bodyStr = String(body || "").toLowerCase();
    return (
      str.includes("getcertificates") ||
      str.includes("getcertificate") ||
      str.includes("dsc/getcert") ||
      str.includes("listtokens") ||
      bodyStr.includes('"action":"getcertificates"') ||
      bodyStr.includes('"action":"listtokens"') ||
      bodyStr.includes('"action":"getcert"')
    );
  }

  function requestFromContent(action, url, body) {
    const id = "req-" + Math.random().toString(36).substring(2) + Date.now().toString(36);
    return new Promise((resolve) => {
      function onMessage(ev) {
        if (!ev.data || ev.data.source !== "signer-content-response" || ev.data.id !== id) return;
        window.removeEventListener("message", onMessage);
        resolve(ev.data);
      }
      window.addEventListener("message", onMessage);
      window.postMessage(
        {
          source: "signer-page-request",
          id: id,
          action: action || "sign",
          portal: window.location.host,
          url: String(url),
          body: body ? String(body) : ""
        },
        "*"
      );
    });
  }

  async function handleSignerRequest(url, body) {
    if (isHealthUrl(url, body)) {
      console.log("[VyomKunji Main-World] Responded to health check on", url);
      return {
        status: "success",
        status_code: 200,
        version: "2.6",
        service: "emSigner",
        message: "Service Running"
      };
    }

    if (isCertListUrl(url, body)) {
      console.log("[VyomKunji Main-World] Intercepted certificate list query on", url);
      const resp = await requestFromContent("listTokens", url, body);
      const certList = (resp && Array.isArray(resp.certificates)) ? resp.certificates : [];
      const primary = certList[0] || {};
      return {
        status: "success",
        status_code: 200,
        certificates: certList,
        certificate: primary.certificate || primary.cert || "",
        cert: primary.certificate || primary.cert || ""
      };
    }

    console.log("[VyomKunji Main-World] Intercepted signing request on", url);
    const resp = await requestFromContent("sign", url, body);
    if (resp && resp.ok) {
      const sig = resp.signature || resp.pkcs7 || "";
      const cert = resp.cert || "";
      return {
        status: "success",
        status_code: 200,
        signature: sig,
        signData: sig,
        sign: sig,
        signedData: sig,
        pkcs7: resp.pkcs7 || sig,
        cert: cert,
        certificate: cert,
        chain: resp.chain || [],
        data: {
          signature: sig,
          signData: sig,
          certificate: cert,
          cert: cert
        }
      };
    }

    return {
      status: "failed",
      status_code: 500,
      errorMessage: (resp && resp.error && resp.error.message) || "Signing rejected or failed",
      error: (resp && resp.error && resp.error.message) || "Signing rejected or failed"
    };
  }

  // 1. Monkeypatch window.fetch
  const origFetch = window.fetch;
  window.fetch = async function (input, init) {
    const url = typeof input === "string" ? input : (input && input.url ? input.url : "");
    if (isSignerUrl(url)) {
      const body = init && init.body ? init.body : "";
      const payload = await handleSignerRequest(url, body);
      const statusCode = payload.status === "success" ? 200 : 500;
      return new Response(JSON.stringify(payload), {
        status: statusCode,
        headers: { "Content-Type": "application/json" }
      });
    }
    return origFetch.apply(this, arguments);
  };

  // 2. Monkeypatch window.XMLHttpRequest
  const OrigXHR = window.XMLHttpRequest;
  function SignerXHR() {
    const xhr = new OrigXHR();
    let targetUrl = "";
    let isIntercepted = false;

    const origOpen = xhr.open;
    xhr.open = function (method, url) {
      targetUrl = url;
      if (isSignerUrl(url)) {
        isIntercepted = true;
      }
      return origOpen.apply(xhr, arguments);
    };

    const origSend = xhr.send;
    xhr.send = function (body) {
      if (isIntercepted) {
        handleSignerRequest(targetUrl, body).then((respData) => {
          const text = JSON.stringify(respData);
          const statusCode = respData.status === "success" ? 200 : 500;

          Object.defineProperty(xhr, "readyState", { value: 4, writable: true });
          Object.defineProperty(xhr, "status", { value: statusCode, writable: true });
          Object.defineProperty(xhr, "statusText", { value: statusCode === 200 ? "OK" : "Error", writable: true });
          Object.defineProperty(xhr, "responseText", { value: text, writable: true });
          Object.defineProperty(xhr, "response", { value: text, writable: true });

          if (typeof xhr.onreadystatechange === "function") xhr.onreadystatechange();
          if (typeof xhr.onload === "function") xhr.onload();
          xhr.dispatchEvent(new Event("readystatechange"));
          xhr.dispatchEvent(new Event("load"));
        });
        return;
      }
      return origSend.apply(xhr, arguments);
    };

    return xhr;
  }
  SignerXHR.prototype = OrigXHR.prototype;
  window.XMLHttpRequest = SignerXHR;

  // 3. Monkeypatch window.WebSocket
  const OrigWebSocket = window.WebSocket;
  function SignerWebSocket(url, protocols) {
    if (isSignerUrl(url)) {
      console.log("[VyomKunji Main-World] Intercepted WebSocket to", url);
      const ws = new EventTarget();
      ws.url = url;
      ws.readyState = 0; // CONNECTING
      ws.protocol = Array.isArray(protocols) ? protocols[0] : (protocols || "");
      ws.binaryType = "blob";
      ws.send = function (data) {
        handleSignerRequest(url, data).then((respData) => {
          const evt = new MessageEvent("message", { data: JSON.stringify(respData) });
          if (typeof ws.onmessage === "function") ws.onmessage(evt);
          ws.dispatchEvent(evt);
        });
      };
      ws.close = function () {
        ws.readyState = 3; // CLOSED
        const evt = new CloseEvent("close");
        if (typeof ws.onclose === "function") ws.onclose(evt);
        ws.dispatchEvent(evt);
      };
      setTimeout(() => {
        ws.readyState = 1; // OPEN
        const evt = new Event("open");
        if (typeof ws.onopen === "function") ws.onopen(evt);
        ws.dispatchEvent(evt);
      }, 10);
      return ws;
    }
    return new OrigWebSocket(url, protocols);
  }
  SignerWebSocket.prototype = OrigWebSocket.prototype;
  SignerWebSocket.CONNECTING = 0;
  SignerWebSocket.OPEN = 1;
  SignerWebSocket.CLOSING = 2;
  SignerWebSocket.CLOSED = 3;
  window.WebSocket = SignerWebSocket;

  console.log("[VyomKunji] Main-world network hooks installed (HTTP + WebSocket).");
})();
