// popup.js
const $ = (id) => document.getElementById(id);

function sendNative(payload) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve);
  });
}

async function refresh() {
  const status = await sendNative({ id: crypto.randomUUID(), type: "status" });
  $("status").textContent = status?.ok ? "connected" : "disconnected";
  $("token").textContent = status?.ok && status.token_connected ? "present" : "none";
  const tokens = await sendNative({ id: crypto.randomUUID(), type: "listTokens" });
  const certs = [];
  if (tokens?.ok && tokens.tokens) {
    for (const t of tokens.tokens) for (const c of t.certs) certs.push(c);
  }
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
}

$("refresh").addEventListener("click", refresh);
$("certSelect").addEventListener("change", (e) => {
  chrome.storage.local.set({ defaultCertCN: e.target.value });
});
refresh();
