# Promotion Pack (Forums + Hackathons)

This pack is designed to grow to 1,000+ nodes without paying for more VPS instances.

## Forum Announcement (Short)
Title: MFENX Power-House: permissionless nodes open

Body:
Power-House is opening permissionless node joins. Run a node, keep it up, and help test quorum finality. Join guide and bootstraps below.

Join:
- `julian net start --bootstrap /ip4/137.184.33.2/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q --bootstrap /ip4/146.190.126.101/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

## Forum Announcement (Long)
Title: Join the MFENX Power-House Network (Permissionless)

Body:
We are opening permissionless node joins for Power-House. The goal is to stress-test quorum finality and DA commitments under real-world load. You can run a node with a standard Linux host and keep it online.

What you do:
- Run a node with the join command below.
- Keep it online and report uptime.
- Share metrics if you want to be listed as a public operator.

Join Command:
```
julian net start \
  --node-id node \
  --log-dir ./logs/node \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /ip4/137.184.33.2/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /ip4/146.190.126.101/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 1500 \
  --quorum 7 \
  --metrics :9100
```

Contact:
- Post your node ID, region, and metrics endpoint.

## Hackathon Prompt
Challenge: Build or operate a Power-House node and keep 90% uptime for two weeks.

Scoring:
- Uptime (primary)
- Latency to quorum
- Clean logs (low invalid envelopes)

Deliverables:
- Node ID + region
- Metrics endpoint
- Brief ops notes

## Operator Listing Template
- Name:
- Node ID:
- Region:
- Metrics URL:
- Uptime window:

## Links
- Permissionless join guide: `docs/permissionless_join.md`
- Community onboarding: `docs/community_onboarding.md`
