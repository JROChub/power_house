# DigitalOcean Terraform

This module declares the three-region MFENX validator topology, global RPC
load balancer, health-aware routing, and deny-by-default firewall.

It is suitable for a new environment. The production resources already exist,
so import them before the first plan. Never run `terraform apply` against
production until `terraform plan` reports no replacement or destruction.

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
