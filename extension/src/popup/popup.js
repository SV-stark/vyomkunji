// popup.js
const $ = (id) => document.getElementById(id);

function sendNative(payload) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve);
  });
}

async function refresh() {
  try {
    const status = await sendNative({ id: crypto.randomUUID(), type: "status" });
    $("status").textContent = status?.ok ? "connected" : "disconnected";
    const tokens = await sendNative({ id: crypto.randomUUID(), type: "listTokens" });
    const tokenList = (tokens?.ok && Array.isArray(tokens.tokens)) ? tokens.tokens : [];
    if (tokenList.length) {
      $("token").textContent = tokenList.map((t) => t.label || t.id || "token").join(", ");
    } else {
      $("token").textContent = status?.ok ? "none" : "—";
    }
    const certs = [];
    for (const t of tokenList) for (const c of (t.certs || [])) certs.push(c);
    if (certs.length) {
      $("certRow").hidden = false;
      const sel = $("certSelect");
      sel.innerHTML = "";
      const stored = await chrome.storage.local.get("defaultCertCN");
      for (const c of certs) {
        const opt = document.createElement("option");
        opt.value = c.cn;
        opt.textContent = c.cn || c.subject || c.id;
        if (stored.defaultCertCN && stored.defaultCertCN === c.cn) opt.selected = true;
        sel.appendChild(opt);
      }
    } else {
      $("certRow").hidden = true;
    }
  } catch (e) {
    $("status").textContent = "disconnected";
    $("token").textContent = "—";
    console.error("[VyomKunji popup]", e);
  }
}

$("refresh").addEventListener("click", refresh);
$("certSelect").addEventListener("change", (e) => {
  chrome.storage.local.set({ defaultCertCN: e.target.value });
});
refresh();
