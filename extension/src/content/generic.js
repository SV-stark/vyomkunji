// content/generic.js — portal-generic DSC signing interceptor.
(function () {
  "use strict";

  const PORTALS = {
    "incometax.gov.in": { name: "Income Tax", triggers: ["sign with dsc", "dsc sign", "sign using dsc", "sign with digital signature"] },
    "gst.gov.in": { name: "GST", triggers: ["sign with dsc", "dsc sign", "sign using dsc"] },
    "mca.gov.in": { name: "MCA", triggers: ["sign with dsc", "dsc sign", "sign using dsc"] },
    "epfindia.gov.in": { name: "EPFO", triggers: ["sign with dsc", "dsc sign", "sign using dsc"] },
    "tdscpc.gov.in": { name: "TDS", triggers: ["sign with dsc", "dsc sign", "sign using dsc"] },
    "traces.gov.in": { name: "TRACES", triggers: ["sign with dsc", "dsc sign", "sign using dsc"] }
  };

  function detectPortal(host) {
    host = host.toLowerCase();
    for (const key of Object.keys(PORTALS)) {
      if (host === key || host.endsWith("." + key)) return key;
    }
    return null;
  }

  const PORTAL = detectPortal(location.host);
  if (!PORTAL) return;

  function uuid() { return crypto.randomUUID(); }

  function sendNative(payload) {
    return new Promise((resolve) => chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve));
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
    // Only accept messages from this page's own main world. Without this an
    // embedded cross-origin frame could try to trigger a signing request.
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

  function findSignButtons(triggers) {
    const els = Array.from(document.querySelectorAll("button, a, input[type=button], input[type=submit], [role=button]"));
    return els.filter((el) => {
      const t = ((el.innerText || el.value || el.title || "") + "").toLowerCase();
      return triggers.some((tr) => t.includes(tr));
    });
  }

  async function handleSignClick() {
    const dataEl = document.querySelector("[data-sign-data], #signData, input[name='signData'], textarea[name='signData']");
    const data = dataEl ? (dataEl.value || dataEl.getAttribute("data-sign-data") || "") : "";
    let certCN = null;
    try {
      const stored = await Promise.resolve(chrome.storage.local.get("defaultCertCN"));
      certCN = (stored && stored.defaultCertCN) || null;
    } catch (e) {}
    const req = {
      id: uuid(),
      type: "sign",
      portal: PORTAL,
      payload: { data: btoa(unescape(encodeURIComponent(data))), hashAlg: "SHA256", signAlg: "RSA", alreadyHashed: false, certCN }
    };
    const resp = await sendNative(req);
    if (!resp || !resp.ok) {
      alert("VyomKunji: signing failed - " + (resp && resp.error ? resp.error.message : "unknown"));
      return;
    }
    const sigField = document.querySelector("#signature, input[name='signature'], [data-signature-target]");
    if (sigField) {
      if (["INPUT", "TEXTAREA"].includes(sigField.tagName)) sigField.value = resp.signature;
      else sigField.setAttribute("data-signature", resp.signature);
    }
    window.dispatchEvent(new CustomEvent("signer:signed", { detail: resp }));
    window.dispatchEvent(new CustomEvent("vyomkunji:signed", { detail: resp }));
    console.log("[VyomKunji] signed via " + PORTAL, resp.signedAt);
  }

  function attach() {
    findSignButtons(PORTALS[PORTAL].triggers).forEach((btn) => {
      if (btn.dataset.signerAttached) return;
      btn.dataset.signerAttached = "1";
      btn.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        handleSignClick();
      }, true);
    });
  }

  injectMainWorldInterceptor();

  new MutationObserver(attach).observe(document.documentElement, { childList: true, subtree: true });
  attach();
  console.log("[VyomKunji] generic content script active on " + PORTAL);
})();
