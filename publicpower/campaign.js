const fields = Object.fromEntries(
  [
    "utc-clock",
    "campaign-state",
    "campaign-phase",
    "elapsed",
    "remaining",
    "ends-at",
    "progress-value",
    "progress-bar",
    "uptime",
    "samples",
    "sample-detail",
    "rpc-p95",
    "rpc-detail",
    "validators",
    "observers",
    "observer-detail",
    "evidence-events",
    "evidence-head",
    "report-hash",
    "drill-summary",
    "drill-list",
    "updated-at",
  ].map((id) => [id, document.querySelector(`#${id}`)]),
);

function duration(value) {
  const seconds = Math.max(0, Math.floor(Number(value) || 0));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return [hours, minutes, remainder].map((part) => String(part).padStart(2, "0")).join(":");
}

function timestamp(value) {
  if (!value) return "--";
  return new Date(value).toISOString().replace(".000", "");
}

function drillLabel(kind) {
  return {
    validator_failover: "VALIDATOR FAILOVER",
    intake_recovery: "INTAKE RECOVERY",
    replica_recovery: "REGISTRY REPLICA RECOVERY",
  }[kind] || String(kind || "UNKNOWN DRILL").replaceAll("_", " ").toUpperCase();
}

function drillDetail(drill) {
  if (drill.status === "scheduled") return `SCHEDULED AT +${duration(drill.offset_seconds)}`;
  if (drill.recovery_seconds != null) {
    return `RECOVERY ${Number(drill.recovery_seconds).toFixed(3)}S / ${Number(drill.errors_observed) || 0} EDGE ERRORS`;
  }
  return drill.detail || drill.status.toUpperCase();
}

function renderDrills(drills = {}) {
  const items = Array.isArray(drills.items) ? drills.items : [];
  fields["drill-summary"].textContent = `${Number(drills.completed) || 0} / ${Number(drills.scheduled) || 0}`;
  if (!items.length) {
    fields["drill-list"].innerHTML = '<li class="awaiting"><i></i><span><b>AWAITING CAMPAIGN STATE</b><small>DRILL SCHEDULE NOT YET PUBLISHED</small></span><strong>--</strong></li>';
    return;
  }
  fields["drill-list"].replaceChildren(
    ...items.map((drill) => {
      const item = document.createElement("li");
      item.className = drill.status || "scheduled";
      const marker = document.createElement("i");
      const copy = document.createElement("span");
      const title = document.createElement("b");
      const detail = document.createElement("small");
      const state = document.createElement("strong");
      title.textContent = drillLabel(drill.kind);
      detail.textContent = drillDetail(drill);
      state.textContent = String(drill.status || "scheduled").toUpperCase();
      copy.append(title, detail);
      item.append(marker, copy, state);
      return item;
    }),
  );
}

function render(data) {
  const campaign = data.reliability_campaign || {};
  const state = campaign.status || "not_started";
  const network = campaign.network || {};
  const rpc = campaign.rpc || {};
  const evidence = campaign.evidence || {};
  document.body.dataset.state = state;
  fields["campaign-state"].textContent = state.replaceAll("_", " ").toUpperCase();
  fields["campaign-phase"].textContent = String(campaign.phase || "awaiting").replaceAll("_", " ").toUpperCase();
  fields.elapsed.textContent = duration(campaign.elapsed_seconds);
  fields.remaining.textContent = duration(campaign.remaining_seconds);
  fields["ends-at"].textContent = timestamp(campaign.ends_at);
  const progress = Math.max(0, Math.min(100, Number(campaign.progress_percent) || 0));
  fields["progress-value"].textContent = `${progress.toFixed(3)}%`;
  fields["progress-bar"].style.width = `${progress}%`;
  fields.uptime.textContent = campaign.uptime_percent == null
    ? "COLLECTING"
    : `${Number(campaign.uptime_percent).toFixed(5)}%`;
  fields.samples.textContent = Number(campaign.sample_count || 0).toLocaleString("en-US");
  fields["sample-detail"].textContent = `${Number(campaign.failed_samples) || 0} FAILED / MAX STREAK ${Number(campaign.max_consecutive_failures) || 0}`;
  fields["rpc-p95"].textContent = rpc.p95_ms == null ? "-- MS" : `${Number(rpc.p95_ms).toFixed(3)} MS`;
  fields["rpc-detail"].textContent = `${Number(rpc.requests) || 0} REQUESTS / ${Number(rpc.errors) || 0} ERRORS`;
  fields.validators.textContent = `${Number(network.validators_healthy) || 0} / ${Number(network.validators_total) || 0}`;
  fields.observers.textContent = `${Number(network.observers_healthy) || 0} / ${Number(network.observers_total) || 0}`;
  fields["observer-detail"].textContent = `${Number(network.observer_connections) || 0} CONNECTIONS`;
  fields["evidence-events"].textContent = Number(evidence.events || 0).toLocaleString("en-US");
  fields["evidence-head"].textContent = evidence.head_sha256 || "AWAITING FIRST EVENT";
  fields["report-hash"].textContent = evidence.final_report_sha256 || "PENDING CAMPAIGN COMPLETION";
  fields["updated-at"].textContent = timestamp(campaign.updated_at || data.generated_at);
  renderDrills(campaign.drills);
}

async function refresh() {
  try {
    const response = await fetch("https://rpc.mfenx.com/network-status.json", { cache: "no-store" });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
    render(data);
  } catch (error) {
    document.body.dataset.state = "stalled";
    fields["campaign-state"].textContent = "STATUS UNAVAILABLE";
    fields["campaign-phase"].textContent = error.message.toUpperCase();
  }
}

function tickClock() {
  fields["utc-clock"].textContent = new Date().toISOString().slice(11, 19);
}

tickClock();
refresh();
setInterval(tickClock, 1000);
setInterval(refresh, 10_000);
