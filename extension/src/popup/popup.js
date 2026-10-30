// popup.js
const $ = (id) => document.getElementById(id);

function sendNative(payload) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage({ type: "nativeRequest", payload }, resolve);
  });
}

function truncateMiddle(str, maxLength) {
  if (!str || str.length <= maxLength) return str;
  const half = Math.floor((maxLength - 3) / 2);
  return str.slice(0, half) + "…" + str.slice(str.length - half);
}

async function refresh() {
  try {
    const status = await sendNative({ id: crypto.randomUUID(), type: "status" });
    if (status?.ok) {
      $("status").textContent = "connected";
      $("status").className = "badge-ok";
      if (status.module) {
        $("moduleRow").hidden = false;
        $("module").textContent = truncateMiddle(status.module, 28);
        $("module").title = status.module;
      }
    } else {
      const errMsg = status?.error?.code === "HOST_UNAVAILABLE" ? "host not registered" : "disconnected";
      $("status").textContent = errMsg;
      $("status").className = "badge-err";
      $("moduleRow").hidden = true;
    }

    const tokens = await sendNative({ id: crypto.randomUUID(), type: "listTokens" });
    const tokenList = (tokens?.ok && Array.isArray(tokens.tokens)) ? tokens.tokens : [];
    if (tokenList.length) {
      $("token").textContent = tokenList.map((t) => t.label || t.id || "DSC Token").join(", ");
    } else {
      $("token").textContent = status?.ok ? "none inserted" : "—";
    }

    const certs = [];
    for (const t of tokenList) {
      for (const c of (t.certs || [])) {
        certs.push(c);
      }
    }

    if (certs.length) {
      $("certRow").hidden = false;
      const sel = $("certSelect");
      sel.innerHTML = "";
      const stored = await chrome.storage.local.get("defaultCertCN");
      for (const c of certs) {
        const opt = document.createElement("option");
        opt.value = c.cn || c.id;
        opt.textContent = c.cn || c.subject || c.id;
        if (stored.defaultCertCN && stored.defaultCertCN === opt.value) {
          opt.selected = true;
        }
        sel.appendChild(opt);
      }
    } else {
      $("certRow").hidden = true;
    }
  } catch (e) {
    $("status").textContent = "disconnected";
    $("status").className = "badge-err";
    $("token").textContent = "—";
    $("moduleRow").hidden = true;
    console.error("[VyomKunji popup]", e);
  }
}

$("refresh").addEventListener("click", refresh);

$("certSelect").addEventListener("change", (e) => {
  const val = e.target.value;
  chrome.storage.local.set({ defaultCertCN: val });
  sendNative({ type: "selectCert", payload: { certCN: val } }).catch(() => {});
});

const openOptionsBtn = $("openOptions");
if (openOptionsBtn) {
  openOptionsBtn.addEventListener("click", () => {
    if (chrome.runtime.openOptionsPage) {
      chrome.runtime.openOptionsPage();
    } else {
      window.open(chrome.runtime.getURL("src/options/options.html"));
    }
  });
}

refresh();
