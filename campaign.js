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
    "acceptance-state",
    "acceptance-uptime",
    "acceptance-errors",
    "acceptance-latency",
    "acceptance-drills",
    "campaign-note",
    "campaign-note-title",
    "campaign-note-detail",
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
    "failure-summary",
    "failure-list",
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
    const probes = Number(drill.errors_observed) || 0;
    const label = drill.status === "passed" ? "CONTROLLED PROBE MISSES" : "EDGE ERRORS";
    return `RECOVERY ${Number(drill.recovery_seconds).toFixed(3)}S / ${probes} ${label}`;
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

function compactReason(failure) {
  const errors = Array.isArray(failure.errors) ? failure.errors : [];
  if (failure.kind === "telemetry_gap") {
    return `EVIDENCE GAP ${Number(failure.gap_seconds || 0).toFixed(3)}S / ${Number(failure.missed_samples) || 0} MISSED / ${errors.length} NETWORK ERRORS`;
  }
  if (failure.kind === "observer_intake_incident") {
    return `ADMISSION PLANE CAUTION / ${errors.join(" / ").replace(/\s+/g, " ").slice(0, 180)}`;
  }
  if (!errors.length) return "NO ERROR DETAIL PUBLISHED";
  return errors.join(" / ").replace(/\s+/g, " ").slice(0, 220);
}

function failureItems(failures = {}) {
  return Array.isArray(failures.recent) ? failures.recent : [];
}

function isControllerGap(failure) {
  const errors = Array.isArray(failure.errors) ? failure.errors : [];
  return failure.kind === "telemetry_gap" && errors.length === 0;
}

function isAdmissionIncident(failure) {
  return failure.kind === "observer_intake_incident";
}

function summarizeContinuity(failures = {}) {
  const items = failureItems(failures);
  const gaps = items.filter(isControllerGap);
  const admissionIncidents = items.filter(isAdmissionIncident);
  const networkFailures = items.filter(
    (failure) => !isControllerGap(failure) && !isAdmissionIncident(failure),
  );
  const missedSamples = gaps.reduce(
    (total, failure) => total + (Number(failure.missed_samples) || 0),
    0,
  );
  const observerIntakeIncidents = Math.max(
    Number(failures.observer_intake_total) || 0,
    admissionIncidents.length,
  );
  return {
    gaps,
    admissionIncidents,
    networkFailures,
    missedSamples,
    observerIntakeIncidents,
    onlyControllerGaps: gaps.length > 0 && networkFailures.length === 0,
    onlyCautions: (gaps.length > 0 || observerIntakeIncidents > 0) && networkFailures.length === 0,
  };
}

function networkLooksHealthy(campaign, network, rpc, maxLatency, requiredErrors, requiredDrillFailures) {
  const validatorTotal = Number(network.validators_total) || 0;
  const validatorHealthy = Number(network.validators_healthy) || 0;
  const status = String(network.status || "").toLowerCase();
  const p95 = rpc.p95_ms == null ? null : Number(rpc.p95_ms);
  return status === "operational"
    && validatorTotal > 0
    && validatorHealthy >= validatorTotal
    && (Number(rpc.errors) || 0) <= requiredErrors
    && (p95 == null || p95 <= maxLatency)
    && (Number(campaign.drills?.failed) || 0) <= requiredDrillFailures;
}

function countLabel(count, singular, plural = `${singular}S`) {
  return `${count} ${count === 1 ? singular : plural}`;
}

function renderFailures(failures = {}) {
  const items = Array.isArray(failures.recent) ? failures.recent.slice().reverse() : [];
  const continuity = summarizeContinuity(failures);
  fields["failure-summary"].textContent = continuity.onlyCautions
    ? countLabel(
      continuity.missedSamples + continuity.observerIntakeIncidents,
      "CAUTION",
    )
    : `${Number(failures.total) || 0} TOTAL`;
  if (!items.length) {
    fields["failure-list"].innerHTML = '<li class="clear"><i></i><span><b>NO FAILED SAMPLES RECORDED</b><small>THE CURRENT CAMPAIGN IS CLEAN</small></span><strong>CLEAR</strong></li>';
    return;
  }
  fields["failure-list"].replaceChildren(
    ...items.map((failure) => {
      const item = document.createElement("li");
      item.className = failure.kind === "telemetry_gap"
        ? "gap"
        : failure.kind === "observer_intake_incident"
          ? "intake"
          : "failed";
      const marker = document.createElement("i");
      const copy = document.createElement("span");
      const title = document.createElement("b");
      const detail = document.createElement("small");
      const state = document.createElement("strong");
      title.textContent = timestamp(failure.recorded_at);
      detail.textContent = compactReason(failure);
      state.textContent = String(failure.kind || "failure").replaceAll("_", " ").toUpperCase();
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
  const acceptance = campaign.acceptance || {};
  const continuity = summarizeContinuity(campaign.failures);
  document.body.dataset.state = state;
  fields["campaign-state"].textContent = state.replaceAll("_", " ").toUpperCase();
  fields["campaign-phase"].textContent = String(campaign.phase || "awaiting").replaceAll("_", " ").toUpperCase();
  fields.elapsed.textContent = duration(campaign.elapsed_seconds);
  fields.remaining.textContent = duration(campaign.remaining_seconds);
  fields["ends-at"].textContent = timestamp(campaign.ends_at);
  const progress = Math.max(0, Math.min(100, Number(campaign.progress_percent) || 0));
  fields["progress-value"].textContent = `${progress.toFixed(3)}%`;
  fields["progress-bar"].style.width = `${progress}%`;
  const uptimeRequired = Number(acceptance.required_uptime_percent ?? 100);
  const maxLatency = Number(acceptance.max_rpc_p95_ms ?? 1000);
  const requiredErrors = Number(acceptance.required_rpc_errors ?? 0);
  const requiredDrillFailures = Number(acceptance.required_drill_failures ?? 0);
  const currentUptime = campaign.uptime_percent;
  const currentErrors = Number(rpc.errors) || 0;
  const currentDrillFailures = Number(campaign.drills?.failed) || 0;
  const hasControllerGapCaution = continuity.onlyControllerGaps && continuity.missedSamples > 0;
  const hasAdmissionCaution = continuity.onlyCautions && continuity.observerIntakeIncidents > 0;
  const strictGatesOnTrack = currentUptime != null
    && Number(currentUptime) >= uptimeRequired
    && currentErrors <= requiredErrors
    && (rpc.p95_ms == null || Number(rpc.p95_ms) <= maxLatency)
    && currentDrillFailures <= requiredDrillFailures;
  const healthyWithContinuityCaution = (hasControllerGapCaution || hasAdmissionCaution)
    && networkLooksHealthy(campaign, network, rpc, maxLatency, requiredErrors, requiredDrillFailures);
  const gateState = state === "passed" ? "passed" : state === "failed" ? "failed" : (strictGatesOnTrack || healthyWithContinuityCaution) ? "on-track" : "off-track";
  const acceptanceGate = document.querySelector(".acceptance-gates");
  acceptanceGate.dataset.gate = gateState;
  acceptanceGate.dataset.evidence = healthyWithContinuityCaution ? "caution" : "nominal";
  fields["acceptance-state"].textContent = gateState.replaceAll("-", " ").toUpperCase();
  fields["campaign-note"].dataset.tone = healthyWithContinuityCaution ? "caution" : "nominal";
  if (healthyWithContinuityCaution) {
    const cautions = [];
    if (continuity.missedSamples > 0) {
      cautions.push(countLabel(continuity.missedSamples, "controller sample", "controller samples"));
    }
    if (continuity.observerIntakeIncidents > 0) {
      cautions.push(countLabel(continuity.observerIntakeIncidents, "observer-intake incident"));
    }
    fields["campaign-note-title"].textContent = state === "passed"
      ? "NETWORK PASSED / ADMISSION AND EVIDENCE CAUTION"
      : "NETWORK ON TRACK / ADMISSION AND EVIDENCE CAUTION";
    fields["campaign-note-detail"].textContent = `${cautions.join(" / ")} retained in the evidence journal. Admission-plane incidents do not rewrite RPC or validator reliability, and the dedicated intake recovery drill remains a required gate.`;
  } else {
    fields["campaign-note-title"].textContent = "NETWORK ACCEPTANCE AND EVIDENCE CONTINUITY ARE EVALUATED SEPARATELY";
    fields["campaign-note-detail"].textContent = "Controller telemetry gaps and observer-admission incidents are retained in the hash-chained evidence journal. RPC errors, validator health, latency, observer registry health, and drill failures remain the network acceptance gates.";
  }
  fields["acceptance-uptime"].textContent = `${uptimeRequired.toFixed(5)}%`;
  fields["acceptance-errors"].textContent = `${requiredErrors} REQUIRED`;
  fields["acceptance-latency"].textContent = `\u2264 ${maxLatency.toLocaleString("en-US")} MS`;
  fields["acceptance-drills"].textContent = `${requiredDrillFailures} REQUIRED`;
  fields.uptime.textContent = campaign.uptime_percent == null
    ? "COLLECTING"
    : `${Number(campaign.uptime_percent).toFixed(5)}%`;
  fields.samples.textContent = Number(campaign.sample_count || 0).toLocaleString("en-US");
  fields["sample-detail"].textContent = healthyWithContinuityCaution
    ? `${countLabel(currentErrors, "RPC ERROR")} / ${countLabel(continuity.missedSamples, "CONTROLLER GAP")} / ${countLabel(continuity.observerIntakeIncidents, "INTAKE CAUTION")}`
    : `${Number(campaign.failed_samples) || 0} FAILED / MAX STREAK ${Number(campaign.max_consecutive_failures) || 0}`;
  fields["rpc-p95"].textContent = rpc.p95_ms == null ? "-- MS" : `${Number(rpc.p95_ms).toFixed(3)} MS`;
  fields["rpc-detail"].textContent = `${Number(rpc.requests) || 0} REQUESTS / ${Number(rpc.errors) || 0} ERRORS`;
  fields.validators.textContent = `${Number(network.validators_healthy) || 0} / ${Number(network.validators_total) || 0}`;
  fields.observers.textContent = `${Number(network.observers_healthy) || 0} / ${Number(network.observers_total) || 0}`;
  fields["observer-detail"].textContent = `${Number(network.observer_connections) || 0} CONNECTIONS`;
  fields["evidence-events"].textContent = Number(evidence.events || 0).toLocaleString("en-US");
  fields["evidence-head"].textContent = evidence.head_sha256 || "AWAITING FIRST EVENT";
  fields["report-hash"].textContent = evidence.final_report_sha256 || "PENDING CAMPAIGN COMPLETION";
  fields["updated-at"].textContent = timestamp(campaign.updated_at || data.generated_at);
  renderFailures(campaign.failures);
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
