# 72-Hour Reliability Campaign

Release scope: Power House v0.3.13.

The production reliability campaign is controlled from an external machine,
not from the validator cluster. It measures the public edge and all three
validators, records a SHA-256 hash-chained evidence journal, publishes a
sanitized live state to every region, and ends automatically after 72 hours.

Live state is rendered at [mfenx.com/campaign.html](https://mfenx.com/campaign.html).

## Safety Invariants

- Validator membership, keys, genesis, quorum, and finalized state are never
  modified by the campaign.
- A failure drill starts only after the public network is operational, all
  three validators are healthy, releases and binary hashes match, signed
  validator and observer registries match, and finalized block/hash values
  agree across all regions.
- Only validator 1 is used for controlled consensus-node failure. Validators 2
  and 3 preserve the configured 2-of-3 quorum throughout the drill.
- Every destructive replica operation creates a rollback copy first and is
  limited to the non-consensus observer registry replica.
- RPC traffic is bounded to three calls per sample and 30 read-only requests
  per hourly burst.
- A passing report requires zero RPC errors and campaign-wide RPC p95 latency
  no greater than 1,000 milliseconds.
- A drill is not complete when its service merely returns to `active`; the
  controller waits for a healthy cross-region sample and journals every
  intermediate recovery probe.

## Measurements

The controller samples once per minute:

- public RPC chain ID, block height, client version, and latency;
- public status and observer intake health;
- validator process state and local health responses;
- binary and signed validator-registry hashes;
- normalized observer-registry hashes;
- finalized block and finalized hash agreement;
- external observer health and connection count.

The public campaign state includes elapsed and remaining time, progress,
successful and failed network samples, controller telemetry gaps, uptime, RPC
p50/p95/p99, drill outcomes, evidence event count, evidence head hash, and
final report hash.
The dedicated page also renders the current pass criteria and whether the
running evidence remains on track. Campaign UI is not added to the primary
observatory.

## Failure Schedule

| Offset | Drill | Acceptance gate |
| --- | --- | --- |
| 6 hours | Validator 1 `SIGKILL` and automatic recovery | Public RPC returns no errors; service and 3-of-3 health recover |
| 24 hours | Observer intake `SIGKILL` and automatic recovery | Public intake returns healthy within 90 seconds |
| 48 hours | Delete and reconstruct one observer-registry replica | Native verifier accepts restored replica and hashes converge |
| 66 hours | Repeat validator failover | Same release, registry, finality, and public RPC gates pass |

An initial manually invoked validator drill is recorded separately before the
long soak proceeds.

## Evidence

Local campaign evidence is stored outside the repository under:

```text
~/.local/state/powerhouse/reliability/
  campaign-state.json
  campaign-status.json
  events.jsonl
  final-report.json
  SHA256SUMS
```

Each JSONL event binds the previous event hash. At completion, the controller
writes a final report and SHA-256 manifest. RPC errors, hash divergence,
network failed samples, blocked drills, or incomplete drills prevent a passing
campaign result. Controller telemetry gaps are retained in the evidence journal
and reported as evidence-continuity cautions, but they do not count as network
failed samples unless the collected network sample itself failed.

## Operations

```bash
systemctl --user status powerhouse-reliability-campaign
journalctl --user -u powerhouse-reliability-campaign -f

python3 ~/.local/lib/powerhouse/reliability_campaign.py status \
  --config ~/.config/powerhouse/reliability-campaign.json

python3 ~/.local/lib/powerhouse/reliability_campaign.py reclassify-controller-gaps \
  --config ~/.config/powerhouse/reliability-campaign.json
```

The controller resumes the same campaign after process restart only when the
configuration fingerprint is unchanged. A second controller cannot acquire the
campaign lock.
The reclassification command is guarded: it only changes a completed failed
campaign to passed when all network acceptance gates pass and the only failure
source was legacy controller telemetry gaps.
