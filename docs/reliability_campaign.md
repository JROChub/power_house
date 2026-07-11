# 72-Hour Reliability Campaign

Release scope: Power House v0.3.24.

The production reliability campaign is controlled from a dedicated external
controller in Toronto, not from the validator cluster or an operator
workstation. It measures the public edge and all three validators, records a
SHA-256 hash-chained evidence journal, publishes a sanitized live state to
every region, and ends automatically after 72 hours.

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
- RPC traffic is bounded to three logical calls per sample and 30 logical,
  read-only requests per hourly burst. A logical probe has at most three
  bounded transport attempts.
- A passing report requires zero confirmed RPC errors and campaign-wide RPC
  p95 latency no greater than 1,000 milliseconds. A confirmed error means all
  bounded attempts for one logical request failed.
- A drill is not complete when its service merely returns to `active`; the
  controller waits for a healthy cross-region sample and journals every
  intermediate recovery probe.

## Measurements

The controller samples once per minute. Public HTTP probes and validator SSH
audits run concurrently so one slow region cannot block the full sampling
cycle:

- public RPC chain ID, block height, client version, and latency;
- public status and observer intake health;
- validator process state and local health responses;
- binary and signed validator-registry hashes;
- normalized observer-registry hashes;
- finalized block and finalized hash agreement;
- external observer health and connection count.

The public campaign state includes elapsed and remaining time, progress,
successful and failed network samples, controller telemetry gaps, reconciled
controller activity windows, one-block finality convergence windows,
observer-admission incidents, uptime,
RPC p50/p95/p99, confirmed request errors, recovered transport attempts, drill
outcomes, evidence event count, evidence head hash, and final report hash.
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

Production campaign evidence is stored outside the repository on the dedicated
controller under:

```text
/var/lib/powerhouse-reliability-controller/reliability/
  campaign-state.json
  campaign-status.json
  events.jsonl
  final-report.json
  SHA256SUMS
```

Each JSONL event binds the previous event hash. Recovered transport attempts
remain in the sample evidence and public counters; retries never erase the
original attempt error. A sample is accepted only when a bounded retry reaches
the same expected chain, release, registry, finality, and health state. At
completion, the controller
writes a final report and SHA-256 manifest. RPC errors, hash divergence,
network failed samples, blocked drills, or incomplete drills prevent a passing
campaign result. Controller telemetry gaps are retained in the evidence journal
and reported as evidence-continuity cautions, but they do not count as network
failed samples unless the collected network sample itself failed. When the
hash-chained journal proves the controller was actively running an RPC burst,
drill, or publish operation inside a previously reported gap window, the live
controller may reconcile that entry as a `controller_busy_window`. Reconciliation
does not delete evidence; it appends a reconciliation event, preserves the
original timestamps, and removes the entry from controller telemetry-gap totals.
Observer intake endpoint failures are reported as admission-plane incidents
when every RPC, validator, registry, finality, and observer-health gate remains
healthy. They stay in the hash-chained evidence journal and on the public
campaign page, but they do not reduce measured RPC/validator network uptime.
The scheduled intake-recovery drill is still a required gate and still fails
the campaign if the intake service cannot recover inside the configured window.
Finalized-state divergence remains a network failure unless the exact evidence
matches the guarded consensus-plane rule: the failed sample contains only
`finalized state differs across validators`, all three validator services are
active, the nodes differ by at most one finalized block, a 2-of-3 finalized
signature quorum is visible, and adjacent hash-chained samples on both sides
show exact finalized block/hash agreement. Only then may the live controller
reconcile the entry as a `finality_convergence_window`. The evidence remains
visible, and the controller appends a reconciliation event instead of deleting
or rewriting the journal.

## Operations

The production service uses
`infra/systemd/powerhouse-reliability-controller.service`. Before a new clock
starts, ten complete cross-region samples must pass with no failures and no
recovered retries:

```bash
python3 /usr/local/lib/powerhouse/reliability_campaign.py preflight \
  --config /etc/powerhouse/reliability-campaign.json \
  --samples 10 \
  --interval-seconds 2

systemctl status powerhouse-reliability-controller
journalctl -u powerhouse-reliability-controller -f

python3 /usr/local/lib/powerhouse/reliability_campaign.py status \
  --config /etc/powerhouse/reliability-campaign.json

python3 /usr/local/lib/powerhouse/reliability_campaign.py reclassify-controller-gaps \
  --config /etc/powerhouse/reliability-campaign.json

python3 /usr/local/lib/powerhouse/reliability_campaign.py reconcile-controller-gaps \
  --config /etc/powerhouse/reliability-campaign.json

python3 /usr/local/lib/powerhouse/reliability_campaign.py reconcile-finality-windows \
  --config /etc/powerhouse/reliability-campaign.json
```

The controller resumes the same campaign after process restart only when the
configuration fingerprint is unchanged. A second controller cannot acquire the
campaign lock.
The reclassification command is guarded: it only changes a completed failed
campaign to passed when all network acceptance gates pass and the only failure
source was legacy controller telemetry gaps.
The reconciliation command is narrower and can be used while a campaign is
running: it only reclassifies a silent-gap entry when an existing hash-chained
controller activity event falls inside that same window.
The finality-window reconciliation command is similarly narrow: it only
reclassifies a one-block validator audit skew when exact-finality evidence
appears immediately before and after that sample.
