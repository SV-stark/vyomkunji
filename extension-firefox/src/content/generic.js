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

  function uuid() { return crypto.randomUUID(); }

  function sendNative(payload) {
    return new Promise((resolve) => chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve));
  }

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
      alert("Signer: signing failed - " + (resp && resp.error ? resp.error.message : "unknown"));
      return;
    }
    const sigField = document.querySelector("#signature, input[name='signature'], [data-signature-target]");
    if (sigField) {
      if (["INPUT", "TEXTAREA"].includes(sigField.tagName)) sigField.value = resp.signature;
      else sigField.setAttribute("data-signature", resp.signature);
    }
    window.dispatchEvent(new CustomEvent("signer:signed", { detail: resp }));
    console.log("[Signer] signed via " + PORTAL, resp.signedAt);
  }

  const PORTAL = detectPortal(location.host);
  if (!PORTAL) return;

  // emSigner / DSC-signer localhost shim
  const origFetch = window.fetch;
  window.fetch = async function (input, init) {
    const url = typeof input === "string" ? input : input && input.url;
    if (url && /127\.0\.0\.1|localhost/.test(url) && /emsigner|dsigner|sign/i.test(url)) {
      const body = init && init.body ? String(init.body) : "";
      let certCN = null;
      try { const s = await Promise.resolve(chrome.storage.local.get("defaultCertCN")); certCN = (s && s.defaultCertCN) || null; } catch (e) {}
      const req = { id: uuid(), type: "sign", portal: PORTAL, payload: { data: btoa(unescape(encodeURIComponent(body))), hashAlg: "SHA256", signAlg: "RSA", certCN } };
      const resp = await sendNative(req);
      if (resp && resp.ok) {
        return new Response(JSON.stringify({ status: "success", signature: resp.signature, cert: resp.cert || "" }), {
          status: 200, headers: { "Content-Type": "application/json" }
        });
      }
    }
    return origFetch.apply(this, arguments);
  };

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

  new MutationObserver(attach).observe(document.documentElement, { childList: true, subtree: true });
  attach();
  console.log("[Signer] generic content script active on " + PORTAL);
})();
