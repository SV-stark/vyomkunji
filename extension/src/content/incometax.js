// content/incometax.js — Income Tax e-Filing Portal (incometax.gov.in) DSC signing interceptor.
(function () {
  "use strict";
  const PORTAL = "incometax.gov.in";

  const SIGN_TRIGGERS = [
    "sign with dsc",
    "dsc sign",
    "sign using dsc",
    "sign with digital signature",
    "verify with dsc",
    "verify using dsc",
    "proceed to verification",
    "sign & submit",
    "sign and submit",
    "digitally sign",
    "digital signature"
  ];

  function uuid() {
    try {
      return crypto.randomUUID();
    } catch (_) {
      return "vk-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2);
    }
  }

  function isBase64(str) {
    if (typeof str !== "string" || !str || str.length % 4 !== 0) return false;
    return /^[A-Za-z0-9+/]+={0,2}$/.test(str);
  }

  function sendNative(payload) {
    return new Promise((resolve) => {
      chrome.runtime.sendMessage({ type: "nativeRequest", payload }, (resp) => {
        resolve(resp);
      });
    });
  }

  function extractFromObject(obj) {
    let target = obj;
    if (obj.data && typeof obj.data === "object") target = obj.data;
    if (obj.payload && typeof obj.payload === "object") target = obj.payload;

    const rawData =
      target.data ??
      target.signData ??
      target.dataToSign ??
      target.content ??
      target.file ??
      target.hash ??
      target.digest ??
      target.xml ??
      "";

    const rawDataStr = typeof rawData === "string" ? rawData : JSON.stringify(rawData);
    const isDigest = !!(target.hash || target.digest || target.alreadyHashed);
    const hashAlg = (target.hashAlg || target.hashAlgorithm || target.digestAlgorithm || "SHA256").toUpperCase();
    const signAlg = (target.signAlg || target.keyType || "RSA").toUpperCase();
    const pkcs7 = !!(target.pkcs7 || target.includeChain || target.type === "pdf" || target.action === "signpdf");
    const certCN = target.certCN || target.commonName || target.alias || null;
    const data = isBase64(rawDataStr) ? rawDataStr : btoa(unescape(encodeURIComponent(rawDataStr)));

    return { data, hashAlg, signAlg, alreadyHashed: isDigest, pkcs7, certCN };
  }

  function parseSigningPayload(rawBody) {
    if (rawBody && typeof rawBody === "object") {
      return extractFromObject(rawBody);
    }
    const str = String(rawBody || "").trim();
    if (!str) {
      return { data: "", hashAlg: "SHA256", signAlg: "RSA", alreadyHashed: false, pkcs7: false, certCN: null };
    }

    if ((str.startsWith("{") && str.endsWith("}")) || (str.startsWith("[") && str.endsWith("]"))) {
      try {
        const obj = JSON.parse(str);
        return extractFromObject(obj);
      } catch (_) {}
    }

    if (str.includes("=") && (str.includes("&") || str.startsWith("data=") || str.startsWith("action="))) {
      try {
        const params = new URLSearchParams(str);
        const extracted = params.get("data") || params.get("signData") || params.get("dataToSign");
        if (extracted) {
          return {
            data: isBase64(extracted) ? extracted : btoa(unescape(encodeURIComponent(extracted))),
            hashAlg: (params.get("hashAlg") || "SHA256").toUpperCase(),
            signAlg: (params.get("signAlg") || "RSA").toUpperCase(),
            alreadyHashed: params.get("alreadyHashed") === "true",
            pkcs7: params.get("pkcs7") === "true" || params.get("type") === "pdf",
            certCN: params.get("certCN") || null
          };
        }
      } catch (_) {}
    }

    return {
      data: isBase64(str) ? str : btoa(unescape(encodeURIComponent(str))),
      hashAlg: "SHA256",
      signAlg: "RSA",
      alreadyHashed: false,
      pkcs7: false,
      certCN: null
    };
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

    const { id, action, body } = ev.data;

    // 1. Certificate enumeration request from the portal
    if (action === "listTokens") {
      const resp = await sendNative({ id: id || uuid(), type: "listTokens" });
      const tokenList = (resp && Array.isArray(resp.tokens)) ? resp.tokens : [];
      const certificates = [];
      for (const t of tokenList) {
        for (const c of (t.certs || [])) {
          certificates.push({
            alias: c.cn || c.subject || c.id || "DSC",
            serialNumber: c.serial || "",
            serial: c.serial || "",
            subject: c.subject || ("CN=" + (c.cn || "")),
            issuer: c.issuer || "",
            notBefore: c.notBefore || "",
            notAfter: c.notAfter || "",
            certificate: c.der || "",
            cert: c.der || ""
          });
        }
      }
      window.postMessage(
        {
          source: "signer-content-response",
          id: id,
          ok: true,
          certificates: certificates
        },
        "*"
      );
      return;
    }

    // 2. Signing request from the portal
    let defaultCert = null;
    try {
      const stored = await Promise.resolve(chrome.storage.local.get("defaultCertCN"));
      defaultCert = (stored && stored.defaultCertCN) || null;
    } catch (_) {}

    const parsed = parseSigningPayload(body);
    const req = {
      id: id || uuid(),
      type: "sign",
      portal: PORTAL,
      payload: {
        data: parsed.data,
        hashAlg: parsed.hashAlg || "SHA256",
        signAlg: parsed.signAlg || "RSA",
        alreadyHashed: !!parsed.alreadyHashed,
        pkcs7: !!parsed.pkcs7,
        certCN: parsed.certCN || defaultCert
      }
    };

    const resp = await sendNative(req);
    window.postMessage(
      {
        source: "signer-content-response",
        id: id,
        ok: !!(resp && resp.ok),
        signature: (resp && resp.signature) || "",
        pkcs7: (resp && resp.pkcs7) || "",
        chain: (resp && resp.chain) || [],
        cert: (resp && resp.cert) || "",
        error: resp && resp.error
      },
      "*"
    );
  });

  // Detect DSC sign triggers
  function findSignButtons() {
    const candidates = Array.from(
      document.querySelectorAll(
        "button, a, input[type=button], input[type=submit], [role=button], .btn-sign-dsc, [data-action='sign']"
      )
    );
    return candidates.filter((el) => {
      const t = ((el.innerText || el.value || el.title || el.id || el.className || "") + "").toLowerCase();
      return SIGN_TRIGGERS.some((tr) => t.includes(tr));
    });
  }

  async function handleSignClick(ev) {
    const dataEl = document.querySelector(
      "[data-sign-data], #signData, input[name='signData'], textarea[name='signData'], #signXML, input[name='signXML'], #digest, [data-digest], #signatureData"
    );
    const rawData = dataEl
      ? dataEl.value || dataEl.getAttribute("data-sign-data") || dataEl.getAttribute("data-digest") || ""
      : "";

    let certCN = null;
    try {
      const stored = await Promise.resolve(chrome.storage.local.get("defaultCertCN"));
      certCN = (stored && stored.defaultCertCN) || null;
    } catch (_) {}

    const parsed = parseSigningPayload(rawData);
    const req = {
      id: uuid(),
      type: "sign",
      portal: PORTAL,
      payload: {
        data: parsed.data,
        hashAlg: parsed.hashAlg || "SHA256",
        signAlg: parsed.signAlg || "RSA",
        alreadyHashed: parsed.alreadyHashed,
        pkcs7: parsed.pkcs7,
        certCN: parsed.certCN || certCN
      }
    };

    const resp = await sendNative(req);
    if (!resp || !resp.ok) {
      alert("VyomKunji: signing failed - " + (resp && resp.error ? resp.error.message : "unknown"));
      return;
    }

    const sigField = document.querySelector(
      "#signature, input[name='signature'], [data-signature], #signedData, input[name='signedData'], input[name='pkcs7'], #pkcs7Signature, #signDataOutput"
    );
    if (sigField) {
      const sigVal = resp.signature || resp.pkcs7 || "";
      if (["INPUT", "TEXTAREA"].includes(sigField.tagName)) {
        sigField.value = sigVal;
        sigField.dispatchEvent(new Event("input", { bubbles: true }));
        sigField.dispatchEvent(new Event("change", { bubbles: true }));
      } else {
        sigField.setAttribute("data-signature", sigVal);
      }
    }

    console.log("[VyomKunji] signed OK on " + PORTAL, resp.signedAt);
    window.dispatchEvent(new CustomEvent("signer:signed", { detail: resp }));
    window.dispatchEvent(new CustomEvent("vyomkunji:signed", { detail: resp }));
  }

  function attach() {
    findSignButtons().forEach((btn) => {
      if (btn.dataset.signerAttached) return;
      btn.dataset.signerAttached = "1";
      btn.addEventListener(
        "click",
        (ev) => {
          ev.preventDefault();
          ev.stopImmediatePropagation();
          handleSignClick(ev);
        },
        true
      );
    });
  }

  injectMainWorldInterceptor();

  const obs = new MutationObserver(() => attach());
  obs.observe(document.documentElement, { childList: true, subtree: true });
  attach();

  console.log("[VyomKunji] incometax content script loaded");
})();
