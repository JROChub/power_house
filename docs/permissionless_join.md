# Permissionless Join Guide

This guide enables open community joining without additional infrastructure spend.

## When To Use
Use permissionless mode for community growth, hackathons, and public testnets.
Use stake-gated mode for incentive mainnet and stricter quorum control.

## Open Mode (Permissionless)
On boot nodes, remove policy gating so any peer can connect and gossip.

Set in `/etc/powerhouse/powerhouse-common.env`:
```
# comment out or remove
# PH_POLICY=/etc/powerhouse/governance.json
```
Restart boot services after change.

## Join Command (Community)
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

## Auto-Discovery Guidance
Use DNS seed records so community nodes can join without hard-coded IPs.
Example seed format:
```
/dns/boot.mfenx.com/tcp/7001/p2p/<peer-id>
/dns/boot.mfenx.com/tcp/7002/p2p/<peer-id>
```

## Stake-Gated Mode (Incentive Mainnet)
Keep policy enabled to require stake-backed membership:
```
PH_POLICY=/etc/powerhouse/governance.json
```
This prevents unknown keys from counting toward quorum.

## Notes
- Permissionless join does not require you to add new VPS instances.
- For high traffic, add more boot nodes, not more full nodes.
