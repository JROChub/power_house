# DigitalOcean Terraform

This module retains the retired chain `177155` three-region validator design,
global RPC load balancer, health-aware routing, and deny-by-default firewall.

MFENX Native 2026 uses chain `2026092601` with three validator processes on one
host and a separately maintained runtime. This module does not describe or
manage that deployment. See [current network deployment scope](../../../docs/production_rpc_deployment.md).

For a new isolated environment, review its own resource names, chain identity,
DNS, and access rules first. The import IDs below are historical examples from
the retired deployment; they are not an inventory of existing production
resources. Use only independently verified IDs for an environment you manage,
and review `terraform plan` for replacement or destruction before applying it.

```bash
cp terraform.tfvars.example terraform.tfvars
export DIGITALOCEAN_TOKEN=...
terraform init

terraform import digitalocean_project.powerhouse 59143ead-d9cc-42d2-8d17-31234722bc91
terraform import digitalocean_tag.validator mfenx-rpc-validator
terraform import 'digitalocean_droplet.validator["validator-1"]' 577186607
terraform import 'digitalocean_droplet.validator["validator-2"]' 577186698
terraform import 'digitalocean_droplet.validator["validator-3"]' 577186818
terraform import digitalocean_loadbalancer.rpc 45e7c84d-b722-4afb-a06b-7168f712a377
terraform import digitalocean_firewall.validator 903d9998-4e6b-4107-b589-ac82983701cb

terraform plan
```

The module uses `prevent_destroy` on validators and the RPC edge. The sealed
consensus bundle remains outside Terraform state and is deployed with
`scripts/deploy_rpc_cluster.sh`.
