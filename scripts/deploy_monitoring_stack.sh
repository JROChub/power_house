#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: scripts/deploy_monitoring_stack.sh <validator-1-ssh> <validator-2-ssh> <validator-3-ssh>" >&2
  exit 1
fi

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
HOSTS=("$@")
SSH_ARGS=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a SSH_ARGS <<<"$SSH_OPTS"
fi

host_address() {
  printf '%s\n' "${1#*@}"
}

NODE1=$(host_address "${HOSTS[0]}")
NODE2=$(host_address "${HOSTS[1]}")
NODE3=$(host_address "${HOSTS[2]}")
TEMP_CONFIG=$(mktemp)
trap 'rm -f "$TEMP_CONFIG"' EXIT
sed \
  -e "s/__NODE1__/$NODE1/g" \
  -e "s/__NODE2__/$NODE2/g" \
  -e "s/__NODE3__/$NODE3/g" \
  "$ROOT/infra/monitoring/prometheus.yml" >"$TEMP_CONFIG"

for index in 0 1 2; do
  host="${HOSTS[$index]}"
  prometheus_url="http://$NODE1:9090"
  [[ "$index" -eq 0 ]] && prometheus_url="http://127.0.0.1:9090"
  echo "-> installing system telemetry on $host"
  ssh "${SSH_ARGS[@]}" "$host" \
    "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y prometheus-node-exporter"
  ssh "${SSH_ARGS[@]}" "$host" \
    "install -d -m 0750 /etc/powerhouse /usr/local/lib/powerhouse"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/status_api.py" \
    "$host:/usr/local/lib/powerhouse/status_api.py"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/powerhouse-status-api.service" \
    "$host:/etc/systemd/system/powerhouse-status-api.service"
  scp "${SSH_ARGS[@]}" "$ROOT/infra/monitoring/nginx-mfenx-rpc.conf" \
    "$host:/etc/nginx/sites-available/mfenx-rpc"
  ssh "${SSH_ARGS[@]}" "$host" env \
    "PROMETHEUS_URL=$prometheus_url" \
    "POWER_HOUSE_RELEASE=${POWER_HOUSE_RELEASE:?set POWER_HOUSE_RELEASE}" bash -s <<'REMOTE'
set -euo pipefail
chmod 0755 /usr/local/lib/powerhouse/status_api.py
cat >/etc/default/prometheus-node-exporter <<'EOF'
ARGS="--web.listen-address=0.0.0.0:9101"
EOF
cat >/etc/powerhouse/status-api.env <<EOF
PROMETHEUS_URL=$PROMETHEUS_URL
RPC_URL=https://rpc.mfenx.com
POWER_HOUSE_RELEASE=$POWER_HOUSE_RELEASE
EOF
chmod 0640 /etc/powerhouse/status-api.env
ln -sfn /etc/nginx/sites-available/mfenx-rpc /etc/nginx/sites-enabled/mfenx-rpc
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl daemon-reload
systemctl enable --now prometheus-node-exporter powerhouse-status-api
systemctl restart prometheus-node-exporter powerhouse-status-api
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

scp "${SSH_ARGS[@]}" "$TEMP_CONFIG" "$monitor:/etc/prometheus/prometheus.yml"
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
