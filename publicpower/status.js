const fields = {
  state: document.querySelector("#state-label"),
  detail: document.querySelector("#state-detail"),
  rpc: document.querySelector("#rpc-state"),
  validators: document.querySelector("#validators"),
  block: document.querySelector("#block-height"),
  peers: document.querySelector("#peer-count"),
  uptime: document.querySelector("#uptime"),
  client: document.querySelector("#client-version"),
  updated: document.querySelector("#updated-at"),
};

function setText(field, value) {
  fields[field].textContent = value;
}

async function refreshStatus() {
  try {
    const response = await fetch("https://rpc.mfenx.com/network-status.json", {
      cache: "no-store",
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
    const state = data.status || "degraded";
    document.body.className = state;
    setText("state", state.toUpperCase());
    setText(
      "detail",
      state === "operational"
        ? "All regional validators, telemetry exporters, and the public RPC probe are healthy."
        : "The network is reachable with one or more degraded operational signals."
    );
    setText("rpc", data.rpc?.reachable ? "ONLINE" : "UNAVAILABLE");
    setText("validators", `${data.validators_healthy} / ${data.validators_total}`);
    document.querySelector("#validator-identity").textContent =
      data.validator_registry?.verified && data.validator_registry?.fresh
        ? `${data.validator_registry.identity_verified} IDENTITIES VERIFIED`
        : "SIGNED REGISTRY UNAVAILABLE";
    setText("block", Number(data.block_height).toLocaleString("en-US"));
    setText("peers", Number(data.peer_connections).toLocaleString("en-US"));
    setText(
      "uptime",
      data.uptime_24h == null ? "COLLECTING" : `${data.uptime_24h.toFixed(3)}%`
    );
    setText("client", data.client || data.release || "UNKNOWN");
    setText("updated", new Date(data.generated_at).toISOString().replace(".000", ""));
  } catch (error) {
    document.body.className = "outage";
    setText("state", "UNAVAILABLE");
    setText("detail", `The public status feed could not be verified: ${error.message}`);
    setText("rpc", "UNAVAILABLE");
    setText("updated", new Date().toISOString().replace(".000", ""));
  }
}

refreshStatus();
setInterval(refreshStatus, 15_000);
