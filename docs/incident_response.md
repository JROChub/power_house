# MFENX Incident Response

Release scope: Power House v0.3.4.

## Severity

| Level | Definition | Acknowledge | Restore or update |
| --- | --- | --- | --- |
| SEV-1 | Public RPC unavailable, conflicting finalized state, or key compromise | 5 minutes | 30 minutes |
| SEV-2 | One validator unavailable, elevated errors, or monitoring blind spot | 15 minutes | 2 hours |
| SEV-3 | Degraded non-critical service or documentation defect | 1 business day | Planned release |

## First Response

1. Record UTC start time and affected surfaces.
2. Run the external RPC publication probe.
3. Compare `/healthz`, block height, finalized hash, and state root on all
   validators.
4. Remove a divergent or unhealthy backend from public routing.
5. Preserve service, Nginx, and monitoring logs.
6. Do not delete chain state or rotate consensus keys during diagnosis.

## Recovery

- Process failure: allow systemd and the healthcheck cooldown to restart it.
- Disk pressure: stop writes, archive logs, and expand storage before restart.
- Replica lag: restore the last agreed finalized state, then allow live catch-up.
- Version mismatch: use the rolling deployment script and verify
  `web3_clientVersion` on every backend.
- Key compromise: remove the key from membership, rotate policy under change
  control, and publish an incident notice.

## Communication

Update the public status page for SEV-1 and SEV-2 events. State what is known,
which functions are affected, and the next update time. Do not speculate about
root cause before evidence exists.

## Postmortem

Within three business days, record timeline, impact, trigger, contributing
conditions, detection gap, recovery actions, and assigned corrective work.
Postmortems must identify a test or control that prevents recurrence.
