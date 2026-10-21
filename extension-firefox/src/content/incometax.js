// content/incometax.js
(function () {
  "use strict";
  const PORTAL = "incometax.gov.in";

  function uuid() {
    return crypto.randomUUID();
  }

  function sendNative(payload) {
    return new Promise((resolve) => {
      chrome.runtime.sendMessage({ type: "nativeRequest", payload }, (resp) => {
        resolve(resp);
      });
    });
  }

  // Inject main-world network interceptor into the page DOM
  function injectMainWorldInterceptor() {
    try {
      const script = document.createElement("script");
      script.src = chrome.runtime.getURL("src/content/interceptor-main.js");
      script.onload = function () {
        this.remove();
      };
      (document.head || document.documentElement).appendChild(script);
    } catch (e) {
      console.warn("[VyomKunji] could not inject main-world interceptor", e);
    }
  }

  // Bridge messages from the page's main-world interceptor to the background native host
  window.addEventListener("message", async (ev) => {
    if (!ev.data || ev.data.source !== "signer-page-request") return;
    if (ev.source !== window || ev.origin !== location.origin) return;
    const { id, body } = ev.data;

    let certCN = null;
    try {
      const stored = await Promise.resolve(chrome.storage.local.get("defaultCertCN"));
      certCN = (stored && stored.defaultCertCN) || null;
    } catch (e) {}

    const req = {
      id: id || uuid(),
      type: "sign",
      portal: PORTAL,
      payload: {
        data: body ? btoa(unescape(encodeURIComponent(body))) : "",
        hashAlg: (body && body.hashAlg) || "SHA256",
        signAlg: (body && body.signAlg) || "RSA",
        alreadyHashed: !!(body && body.alreadyHashed),
        pkcs7: !!(body && (body.pkcs7 || body.includeChain)),
        certCN
      }
    };

    const resp = await sendNative(req);
    window.postMessage({
      source: "signer-content-response",
      id: id,
      ok: !!(resp && resp.ok),
      signature: (resp && resp.signature) || "",
      pkcs7: (resp && resp.pkcs7) || "",
      chain: (resp && resp.chain) || [],
      cert: (resp && resp.cert) || "",
      error: resp && resp.error
    }, "*");
  });

  // Detect DSC sign triggers
  function findSignButtons() {
    const candidates = Array.from(document.querySelectorAll("button, a, input[type=button], input[type=submit]"));
    return candidates.filter((el) => {
      const t = (el.innerText || el.value || "").toLowerCase();
      return t.includes("sign with dsc") || t.includes("dsc sign") || t.includes("sign using dsc");
    });
  }

  async function handleSignClick(ev) {
    // Collect payload from portal's hidden fields if present
    const dataEl = document.querySelector("[data-sign-data], #signData, input[name='signData']");
    const data = dataEl ? (dataEl.value || dataEl.getAttribute("data-sign-data")) : null;
    if (!data) {
      console.log("[VyomKunji] no explicit sign data found; requesting sign");
    }
    const req = {
      id: uuid(),
      type: "sign",
      portal: PORTAL,
      payload: { data: data ? btoa(unescape(encodeURIComponent(data))) : "", hashAlg: "SHA256", signAlg: "RSA", alreadyHashed: false, certCN: null }
    };
    try {
      const stored = await Promise.resolve(chrome.storage.local.get("defaultCertCN"));
      req.payload.certCN = (stored && stored.defaultCertCN) || null;
    } catch (e) {}
    const resp = await sendNative(req);
    if (!resp || !resp.ok) {
      alert("VyomKunji: signing failed - " + (resp?.error?.message || "unknown"));
      return;
    }
    // Inject signature into portal's expected field if present
    const sigField = document.querySelector("#signature, input[name='signature'], [data-signature-target]");
    if (sigField) {
      if (sigField.tagName === "INPUT" || sigField.tagName === "TEXTAREA") sigField.value = resp.signature;
      else sigField.setAttribute("data-signature", resp.signature);
    }
    console.log("[VyomKunji] signed OK", resp.signedAt);
    window.dispatchEvent(new CustomEvent("signer:signed", { detail: resp }));
    window.dispatchEvent(new CustomEvent("vyomkunji:signed", { detail: resp }));
  }

  function attach() {
    findSignButtons().forEach((btn) => {
      if (btn.dataset.signerAttached) return;
      btn.dataset.signerAttached = "1";
      btn.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        handleSignClick(ev);
      }, true);
    });
  }

  injectMainWorldInterceptor();

  // Re-scan on DOM changes
  const obs = new MutationObserver(() => attach());
  obs.observe(document.documentElement, { childList: true, subtree: true });
  attach();

  console.log("[VyomKunji] incometax content script loaded");
})();
