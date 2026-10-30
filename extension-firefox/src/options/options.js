// options.js — Settings & Diagnostics for VyomKunji
const $ = (id) => document.getElementById(id);

function sendNative(payload) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve);
  });
}

async function testConnection() {
  const resultEl = $("testResult");
  resultEl.textContent = "Testing connection…";
  resultEl.className = "status-testing";

  const t0 = performance.now();
  try {
    const ping = await sendNative({ id: crypto.randomUUID(), type: "ping" });
    const elapsed = Math.round(performance.now() - t0);
    if (ping?.ok) {
      resultEl.textContent = `Connected to native host (v${ping.version || "unknown"}, ${elapsed}ms)`;
      resultEl.className = "status-ok";
    } else {
      resultEl.textContent = `Connection failed: ${ping?.error?.message || "unknown error"}`;
      resultEl.className = "status-err";
    }
  } catch (e) {
    resultEl.textContent = `Connection error: ${String(e)}`;
    resultEl.className = "status-err";
  }
  await loadStatus();
}

async function loadStatus() {
  try {
    const status = await sendNative({ id: crypto.randomUUID(), type: "status" });
    if (status?.ok) {
      $("hostStatus").textContent = "Running & Authenticated";
      $("hostStatus").className = "status-ok";
      $("modulePath").textContent = status.module || "Auto-detected (System)";
    } else {
      $("hostStatus").textContent = status?.error?.code === "HOST_UNAVAILABLE" ? "Host Not Registered" : "Disconnected";
      $("hostStatus").className = "status-err";
      $("modulePath").textContent = "—";
    }

    const tokens = await sendNative({ id: crypto.randomUUID(), type: "listTokens" });
    const tokenList = (tokens?.ok && Array.isArray(tokens.tokens)) ? tokens.tokens : [];
    const certSelect = $("defaultCertSelect");
    certSelect.innerHTML = "";

    const stored = await chrome.storage.local.get("defaultCertCN");
    const currentSaved = stored.defaultCertCN || "";

    const optNone = document.createElement("option");
    optNone.value = "";
    optNone.textContent = "Auto-select / First available";
    certSelect.appendChild(optNone);

    let certCount = 0;
    if (tokenList.length) {
      $("tokenStatus").textContent = tokenList.map((t) => `${t.label || "Token"} (${t.serial || "S/N"})`).join(", ");
      for (const t of tokenList) {
        for (const c of (t.certs || [])) {
          certCount++;
          const opt = document.createElement("option");
          opt.value = c.cn || c.id;
          opt.textContent = `${c.cn || c.subject || c.id} (Expires: ${c.notAfter || "N/A"})`;
          if (currentSaved && (currentSaved === opt.value || currentSaved === c.cn)) {
            opt.selected = true;
          }
          certSelect.appendChild(opt);
        }
      }
      $("certCount").textContent = `${certCount} certificate(s) available`;
    } else {
      $("tokenStatus").textContent = status?.ok ? "No USB token inserted" : "—";
      $("certCount").textContent = "0 certificates found";
    }
  } catch (e) {
    console.error("[VyomKunji options]", e);
  }
}

$("btnTest").addEventListener("click", testConnection);

$("defaultCertSelect").addEventListener("change", (e) => {
  const val = e.target.value;
  chrome.storage.local.set({ defaultCertCN: val });
  const saveNotice = $("saveNotice");
  saveNotice.textContent = "Saved preference";
  setTimeout(() => {
    saveNotice.textContent = "";
  }, 2500);
  if (val) {
    sendNative({ type: "selectCert", payload: { certCN: val } }).catch(() => {});
  }
});

loadStatus();
