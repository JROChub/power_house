#!/usr/bin/env bash
set -euo pipefail

ALERT_EMAIL=${PH_ALERT_EMAIL:-}
ALERT_FROM=${PH_ALERT_FROM:-}
ALERT_SUBJECT_PREFIX=${PH_ALERT_SUBJECT_PREFIX:-"[power_house]"}
SLACK_WEBHOOK_URL=${PH_SLACK_WEBHOOK_URL:-}
PAGERDUTY_ROUTING_KEY=${PH_PAGERDUTY_ROUTING_KEY:-}
HOSTNAME_SHORT=$(hostname -s 2>/dev/null || hostname)

MESSAGE=${1:-"Power-House alert"}
DETAILS=${2:-""}

SUBJECT="$ALERT_SUBJECT_PREFIX $HOSTNAME_SHORT"
BODY="${MESSAGE}\n\n${DETAILS}\n"

log_fallback() {
  logger -t powerhouse-alert "$MESSAGE" || true
  logger -t powerhouse-alert "$DETAILS" || true
}

json_payload() {
  python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"
}

delivered=false

if [[ -n "$SLACK_WEBHOOK_URL" ]]; then
  payload=$(json_payload "$SUBJECT"$'\n'"$BODY")
  if curl --fail --silent --show-error \
    -H "Content-Type: application/json" \
    --data "{\"text\":$payload}" \
    "$SLACK_WEBHOOK_URL" >/dev/null; then
    delivered=true
  fi
fi

if [[ -n "$PAGERDUTY_ROUTING_KEY" ]]; then
  summary=$(json_payload "$SUBJECT")
  details=$(json_payload "$BODY")
  if curl --fail --silent --show-error \
    -H "Content-Type: application/json" \
    --data "{\"routing_key\":\"$PAGERDUTY_ROUTING_KEY\",\"event_action\":\"trigger\",\"payload\":{\"summary\":$summary,\"severity\":\"error\",\"source\":\"$HOSTNAME_SHORT\",\"custom_details\":{\"message\":$details}}}" \
    https://events.pagerduty.com/v2/enqueue >/dev/null; then
    delivered=true
  fi
fi

if [[ -z "$ALERT_EMAIL" ]]; then
  log_fallback
  [[ "$delivered" == true ]] && exit 0
  exit 0
fi

if command -v sendmail >/dev/null 2>&1; then
  {
    if [[ -n "$ALERT_FROM" ]]; then
      echo "From: $ALERT_FROM"
    fi
    echo "To: $ALERT_EMAIL"
    echo "Subject: $SUBJECT"
    echo
    echo -e "$BODY"
  } | sendmail -t
  exit 0
fi

if command -v mail >/dev/null 2>&1; then
  echo -e "$BODY" | mail -s "$SUBJECT" "$ALERT_EMAIL"
  exit 0
fi

if command -v msmtp >/dev/null 2>&1; then
  {
    if [[ -n "$ALERT_FROM" ]]; then
      echo "From: $ALERT_FROM"
    fi
    echo "To: $ALERT_EMAIL"
    echo "Subject: $SUBJECT"
    echo
    echo -e "$BODY"
  } | msmtp -t
  exit 0
fi

log_fallback
