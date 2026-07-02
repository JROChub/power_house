#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: scripts/deploy_monitoring_stack.sh <validator-1-ssh> <validator-2-ssh> <validator-3-ssh>" >&2
  exit 1
fi

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
HOSTS=("$@")
VALIDATOR_REGISTRY="${VALIDATOR_REGISTRY:-$ROOT/deployment/generated/mfenx-production/validator-registry.json}"
OBSERVER_REGISTRY="${OBSERVER_REGISTRY:-$ROOT/deployment/generated/mfenx-production/observer-registry.json}"
SSH_ARGS=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a SSH_ARGS <<<"$SSH_OPTS"
fi

host_address() {
  printf '%s\n' "${1#*@}"
}

NODE1=$(host_address "${HOSTS[0]}")
NODE_ADDRESSES=()
for configured_host in "${HOSTS[@]}"; do
  NODE_ADDRESSES+=("$(host_address "$configured_host")")
done
TRUSTED_PROXIES=$(IFS=,; echo "${NODE_ADDRESSES[*]}")
[[ -f "$VALIDATOR_REGISTRY" ]] || {
  echo "signed validator registry not found: $VALIDATOR_REGISTRY" >&2
  exit 1
}

for index in 0 1 2; do
  host="${HOSTS[$index]}"
  prometheus_url="http://$NODE1:9090"
  [[ "$index" -eq 0 ]] && prometheus_url="http://127.0.0.1:9090"
  echo "-> installing system telemetry on $host"
  ssh "${SSH_ARGS[@]}" "$host" \
    "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y prometheus-node-exporter"
  ssh "${SSH_ARGS[@]}" "$host" \
    "install -d -m 0750 /etc/powerhouse /var/lib/powerhouse/monitoring && install -d -m 0755 /usr/local/lib/powerhouse /etc/prometheus/file_sd /var/lib/powerhouse/reliability"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/status_api.py" \
    "$host:/usr/local/lib/powerhouse/status_api.py"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/validator_registry.py" \
    "$host:/usr/local/lib/powerhouse/validator_registry.py"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/observer_registry.py" \
    "$host:/usr/local/lib/powerhouse/observer_registry.py"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/observer_intake.py" \
    "$host:/usr/local/lib/powerhouse/observer_intake.py"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-status-api.service" \
    "$host:/etc/systemd/system/powerhouse-status-api.service"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-validator-registry.service" \
    "$host:/etc/systemd/system/powerhouse-validator-registry.service"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-validator-registry.timer" \
    "$host:/etc/systemd/system/powerhouse-validator-registry.timer"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-observer-registry.service" \
    "$host:/etc/systemd/system/powerhouse-observer-registry.service"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-observer-registry.timer" \
    "$host:/etc/systemd/system/powerhouse-observer-registry.timer"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-observer-intake.service" \
    "$host:/etc/systemd/system/powerhouse-observer-intake.service"
  scp "${SSH_ARGS[@]}" "$VALIDATOR_REGISTRY" \
    "$host:/etc/powerhouse/validator-registry.json"
  if [[ -f "$OBSERVER_REGISTRY" ]]; then
    scp "${SSH_ARGS[@]}" "$OBSERVER_REGISTRY" \
      "$host:/etc/powerhouse/observer-registry.json"
  else
    ssh "${SSH_ARGS[@]}" "$host" "rm -f /etc/powerhouse/observer-registry.json"
  fi
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/nginx-mfenx-rpc.conf" \
    "$host:/etc/nginx/sites-available/mfenx-rpc"
  intake_upstream="$NODE1:9195"
  [[ "$index" -eq 0 ]] && intake_upstream="127.0.0.1:9195"
  ssh "${SSH_ARGS[@]}" "$host" env \
    "PROMETHEUS_URL=$prometheus_url" \
    "OBSERVER_INTAKE_UPSTREAM=$intake_upstream" \
    "OBSERVER_REGISTRY_URL=$([[ "$index" -eq 0 ]] && printf '' || printf 'http://%s:9195/observer-registry.json' "$NODE1")" \
    "OBSERVER_INTAKE_PRIMARY=$([[ "$index" -eq 0 ]] && printf 1 || printf 0)" \
    "OBSERVER_INTAKE_TRUSTED_PROXIES=127.0.0.1,::1,$TRUSTED_PROXIES" \
    "POWER_HOUSE_RELEASE=${POWER_HOUSE_RELEASE:?set POWER_HOUSE_RELEASE}" bash -s <<'REMOTE'
set -euo pipefail
chmod 0755 /usr/local/lib/powerhouse/status_api.py /usr/local/lib/powerhouse/validator_registry.py /usr/local/lib/powerhouse/observer_registry.py /usr/local/lib/powerhouse/observer_intake.py
if [[ -d /opt/powerhouse ]]; then
  chmod 0755 /opt/powerhouse
fi
if [[ -d /opt/powerhouse/releases ]]; then
  chmod 0755 /opt/powerhouse/releases
  find /opt/powerhouse/releases -maxdepth 1 -mindepth 1 -type d -exec chmod 0755 {} +
fi
chmod 0640 /etc/powerhouse/validator-registry.json
if [[ -f /etc/powerhouse/observer-registry.json ]]; then
  chmod 0640 /etc/powerhouse/observer-registry.json
fi
cat >/etc/default/prometheus-node-exporter <<'EOF'
ARGS="--web.listen-address=0.0.0.0:9101"
EOF
cat >/etc/powerhouse/status-api.env <<EOF
PROMETHEUS_URL=$PROMETHEUS_URL
RPC_URL=https://rpc.mfenx.com
POWER_HOUSE_RELEASE=$POWER_HOUSE_RELEASE
VALIDATOR_REGISTRY_STATE=/var/lib/powerhouse/monitoring/validator-registry-state.json
VALIDATOR_REGISTRY_MAX_AGE=45
OBSERVER_REGISTRY_STATE=/var/lib/powerhouse/monitoring/observer-registry-state.json
OBSERVER_REGISTRY_MAX_AGE=45
RELIABILITY_CAMPAIGN_STATE=/var/lib/powerhouse/reliability/campaign-status.json
RELIABILITY_CAMPAIGN_MAX_AGE=180
EOF
chmod 0640 /etc/powerhouse/status-api.env
cat >/etc/powerhouse/observer-registry.env <<EOF
OBSERVER_REGISTRY_URL=$OBSERVER_REGISTRY_URL
OBSERVER_REGISTRY_PATH=$([[ "$OBSERVER_INTAKE_PRIMARY" == 1 ]] && printf /var/lib/powerhouse/observer-intake/observer-registry.json || printf /var/lib/powerhouse/monitoring/observer-registry.json)
EOF
chmod 0640 /etc/powerhouse/observer-registry.env
cat >/etc/powerhouse/observer-intake.env <<EOF
OBSERVER_INTAKE_BINARY=/usr/local/bin/julian
OBSERVER_INTAKE_REGISTRY=/var/lib/powerhouse/observer-intake/observer-registry.json
OBSERVER_INTAKE_STATE_DIR=/var/lib/powerhouse/observer-intake
OBSERVER_INTAKE_CHAIN_ID=177155
OBSERVER_INTAKE_HOST=0.0.0.0
OBSERVER_INTAKE_PORT=9195
OBSERVER_INTAKE_AUTO_PROMOTE=1
OBSERVER_INTAKE_QUEUE_LIMIT=1000
OBSERVER_INTAKE_MAX_SUBMISSIONS=10000
OBSERVER_INTAKE_RETENTION_SECONDS=2592000
OBSERVER_INTAKE_ALLOWED_ORIGINS=https://mfenx.com
OBSERVER_INTAKE_TRUSTED_PROXIES=$OBSERVER_INTAKE_TRUSTED_PROXIES
EOF
chmod 0640 /etc/powerhouse/observer-intake.env
sed -i "s/__OBSERVER_INTAKE_UPSTREAM__/$OBSERVER_INTAKE_UPSTREAM/g" /etc/nginx/sites-available/mfenx-rpc
ln -sfn /etc/nginx/sites-available/mfenx-rpc /etc/nginx/sites-enabled/mfenx-rpc
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl daemon-reload
systemctl enable --now \
  prometheus-node-exporter \
  powerhouse-validator-registry.timer \
  powerhouse-observer-registry.timer \
  powerhouse-status-api
if [[ "$OBSERVER_INTAKE_PRIMARY" == 1 ]]; then
  id -u powerhouse-intake >/dev/null 2>&1 || useradd --system --home-dir /nonexistent --shell /usr/sbin/nologin powerhouse-intake
  install -d -o powerhouse-intake -g powerhouse-intake -m 0750 /var/lib/powerhouse/observer-intake
  if [[ -f /etc/powerhouse/observer-registry.json && ! -f /var/lib/powerhouse/observer-intake/observer-registry.json ]]; then
    install -o powerhouse-intake -g powerhouse-intake -m 0640 \
      /etc/powerhouse/observer-registry.json \
      /var/lib/powerhouse/observer-intake/observer-registry.json
  fi
  chown -R powerhouse-intake:powerhouse-intake /var/lib/powerhouse/observer-intake
  systemctl enable --now powerhouse-observer-intake
  systemctl restart powerhouse-observer-intake
else
  systemctl disable --now powerhouse-observer-intake 2>/dev/null || true
fi
systemctl restart prometheus-node-exporter
systemctl start powerhouse-validator-registry.service
systemctl start powerhouse-observer-registry.service
systemctl restart powerhouse-status-api
systemctl reload nginx
REMOTE
done

monitor="${HOSTS[0]}"
echo "-> installing Prometheus, Alertmanager, blackbox exporter, and Grafana on $monitor"
ssh "${SSH_ARGS[@]}" "$monitor" \
  "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl gnupg prometheus prometheus-alertmanager prometheus-blackbox-exporter"
ssh "${SSH_ARGS[@]}" "$monitor" bash -s <<'REMOTE'
set -euo pipefail
if ! command -v grafana-server >/dev/null 2>&1; then
  install -d -m 0755 /etc/apt/keyrings
  curl -fsSL https://apt.grafana.com/gpg.key \
    | gpg --dearmor --yes -o /etc/apt/keyrings/grafana.gpg
  echo "deb [signed-by=/etc/apt/keyrings/grafana.gpg] https://apt.grafana.com stable main" \
    >/etc/apt/sources.list.d/grafana.list
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y grafana
fi
install -d -m 0755 \
  /etc/grafana/provisioning/datasources \
  /etc/grafana/provisioning/dashboards \
  /var/lib/grafana/dashboards \
  /usr/local/lib/powerhouse
REMOTE

scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/prometheus.yml" \
  "$monitor:/etc/prometheus/prometheus.yml"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-alerts.yml" \
  "$monitor:/etc/prometheus/powerhouse-alerts.yml"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/blackbox.yml" \
  "$monitor:/etc/prometheus/blackbox.yml"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/alertmanager.yml" \
  "$monitor:/etc/prometheus/alertmanager.yml"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/alert_receiver.py" \
  "$monitor:/usr/local/lib/powerhouse/alert_receiver.py"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-alert-receiver.service" \
  "$monitor:/etc/systemd/system/powerhouse-alert-receiver.service"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/grafana-datasource.yml" \
  "$monitor:/etc/grafana/provisioning/datasources/powerhouse.yml"
scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/grafana-dashboard.yml" \
  "$monitor:/etc/grafana/provisioning/dashboards/powerhouse.yml"
scp "${SSH_ARGS[@]}" "$ROOT/contrib/grafana/mfenx_powerhouse_dashboard.json" \
  "$monitor:/var/lib/grafana/dashboards/powerhouse.json"

ssh "${SSH_ARGS[@]}" "$monitor" bash -s <<'REMOTE'
set -euo pipefail
chmod 0755 /usr/local/lib/powerhouse/alert_receiver.py
chown -R grafana:grafana /var/lib/grafana/dashboards
sed -i 's/^ARGS=.*/ARGS="--web.listen-address=0.0.0.0:9090"/' /etc/default/prometheus
sed -i 's|^ARGS=.*|ARGS="--config.file=/etc/prometheus/blackbox.yml --web.listen-address=127.0.0.1:9115"|' /etc/default/prometheus-blackbox-exporter
sed -i 's/^;http_addr =$/http_addr = 127.0.0.1/' /etc/grafana/grafana.ini
systemctl daemon-reload
systemctl enable --now \
  prometheus \
  prometheus-alertmanager \
  prometheus-blackbox-exporter \
  grafana-server \
  powerhouse-alert-receiver
systemctl restart \
  prometheus \
  prometheus-alertmanager \
  prometheus-blackbox-exporter \
  grafana-server \
  powerhouse-alert-receiver
promtool check config /etc/prometheus/prometheus.yml
promtool check rules /etc/prometheus/powerhouse-alerts.yml
amtool check-config /etc/prometheus/alertmanager.yml
REMOTE

for host in "${HOSTS[@]}"; do
  ssh "${SSH_ARGS[@]}" "$host" \
    "systemctl is-active --quiet prometheus-node-exporter powerhouse-status-api nginx && curl -fsS http://127.0.0.1/network-status.json"
  echo
done

echo "Monitoring stack deployed. Use an SSH tunnel to $NODE1:3000 for Grafana."
