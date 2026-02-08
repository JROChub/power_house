#!/usr/bin/env bash
set -euo pipefail

ALERT_EMAIL=${PH_ALERT_EMAIL:-}
ALERT_FROM=${PH_ALERT_FROM:-}
ALERT_SUBJECT_PREFIX=${PH_ALERT_SUBJECT_PREFIX:-"[power_house]"}
HOSTNAME_SHORT=$(hostname -s 2>/dev/null || hostname)

MESSAGE=${1:-"Power-House alert"}
DETAILS=${2:-""}

SUBJECT="$ALERT_SUBJECT_PREFIX $HOSTNAME_SHORT"
BODY="${MESSAGE}\n\n${DETAILS}\n"

log_fallback() {
  logger -t powerhouse-alert "$MESSAGE" || true
  logger -t powerhouse-alert "$DETAILS" || true
}

if [[ -z "$ALERT_EMAIL" ]]; then
  log_fallback
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
