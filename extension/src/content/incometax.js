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
      // fallback: ask user to confirm generic sign
      console.log("[Signer] no explicit sign data found; requesting empty sign");
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
      alert("Signer: signing failed - " + (resp?.error?.message || "unknown"));
      return;
    }
    // Inject signature into portal's expected field if present
    const sigField = document.querySelector("#signature, input[name='signature'], [data-signature-target]");
    if (sigField) {
      if (sigField.tagName === "INPUT" || sigField.tagName === "TEXTAREA") sigField.value = resp.signature;
      else sigField.setAttribute("data-signature", resp.signature);
    }
    console.log("[Signer] signed OK", resp.signedAt);
    // dispatch event portal may listen for
    window.dispatchEvent(new CustomEvent("signer:signed", { detail: resp }));
  }

  function attach() {
    findSignButtons().forEach((btn) => {
      if (btn.dataset.signerAttached) return;
      btn.dataset.signerAttached = "1";
      btn.addEventListener("click", (ev) => {
        // intercept: prevent portal's default signer invocation
        ev.preventDefault();
        ev.stopImmediatePropagation();
        handleSignClick(ev);
      }, true);
    });
  }

  // emSigner HTTP shim: intercept fetch/XHR to localhost signer endpoints
  const origFetch = window.fetch;
  window.fetch = async function (input, init) {
    const url = typeof input === "string" ? input : input?.url;
    if (url && /127\.0\.0\.1|localhost/.test(url) && /emsigner|dsigner|sign/i.test(url)) {
      console.log("[Signer] intercepted fetch to", url);
      // forward to native app as a "sign" with raw body
      const body = init?.body ? String(init.body) : "";
      const req = { id: uuid(), type: "sign", portal: PORTAL, payload: { data: btoa(body), hashAlg: "SHA256", signAlg: "RSA", alreadyHashed: false } };
      const resp = await sendNative(req);
      if (resp && resp.ok) {
        return new Response(JSON.stringify({ status: "success", signature: resp.signature, cert: resp.cert }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
    }
    return origFetch.apply(this, arguments);
  };

  // Re-scan on DOM changes
  const obs = new MutationObserver(() => attach());
  obs.observe(document.documentElement, { childList: true, subtree: true });
  attach();

  console.log("[Signer] incometax content script loaded");
})();
