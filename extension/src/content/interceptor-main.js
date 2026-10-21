// interceptor-main.js — Injected into the page's MAIN execution world to intercept
// localhost/emSigner requests from portal scripts (incometax, GST, MCA, etc.).
(function () {
  "use strict";

  if (window.__signer_injected) return;
  window.__signer_injected = true;

  function isSignerUrl(url) {
    if (!url) return false;
    const str = String(url).toLowerCase();
    const isLocal = str.includes("127.0.0.1") || str.includes("localhost");
    const isSigner = str.includes("emsigner") || str.includes("dsigner") || str.includes("sign") ||
                     str.includes(":8080") || str.includes(":1585");
    return isLocal && isSigner;
  }

  function requestSigningFromContent(url, body) {
    const id = "req-" + Math.random().toString(36).substring(2) + Date.now().toString(36);
    return new Promise((resolve) => {
      function onMessage(ev) {
        if (!ev.data || ev.data.source !== "signer-content-response" || ev.data.id !== id) return;
        window.removeEventListener("message", onMessage);
        resolve(ev.data);
      }
      window.addEventListener("message", onMessage);
      window.postMessage({
        source: "signer-page-request",
        id: id,
        portal: window.location.host,
        url: String(url),
        body: body ? String(body) : ""
      }, "*");
    });
  }

  // 1. Monkeypatch window.fetch
  const origFetch = window.fetch;
  window.fetch = async function (input, init) {
    const url = typeof input === "string" ? input : (input && input.url ? input.url : "");
    if (isSignerUrl(url)) {
      console.log("[VyomKunji Main-World] Intercepted fetch to", url);
      const body = init && init.body ? init.body : "";
      const resp = await requestSigningFromContent(url, body);
      if (resp && resp.ok) {
        const payload = {
          status: "success",
          signature: resp.signature,
          cert: resp.cert || "",
          certificate: resp.cert || ""
        };
        return new Response(JSON.stringify(payload), {
          status: 200,
          headers: { "Content-Type": "application/json" }
        });
      } else {
        const errPayload = {
          status: "failed",
          errorMessage: (resp && resp.error && resp.error.message) || "Signing rejected or failed"
        };
        return new Response(JSON.stringify(errPayload), {
          status: 200,
          headers: { "Content-Type": "application/json" }
        });
      }
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
        console.log("[VyomKunji Main-World] Intercepted XHR to", targetUrl);
        requestSigningFromContent(targetUrl, body).then((resp) => {
          const respData = (resp && resp.ok) ? {
            status: "success",
            signature: resp.signature,
            cert: resp.cert || "",
            certificate: resp.cert || ""
          } : {
            status: "failed",
            errorMessage: (resp && resp.error && resp.error.message) || "Signing rejected or failed"
          };
          const text = JSON.stringify(respData);

          Object.defineProperty(xhr, "readyState", { value: 4, writable: true });
          Object.defineProperty(xhr, "status", { value: 200, writable: true });
          Object.defineProperty(xhr, "statusText", { value: "OK", writable: true });
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

  console.log("[VyomKunji] Main-world network hooks installed.");
})();
