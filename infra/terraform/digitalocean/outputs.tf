output "validator_addresses" {
  value = {
    for name, validator in digitalocean_droplet.validator :
    name => validator.ipv4_address
  }
}

output "rpc_load_balancer_id" {
  value = digitalocean_loadbalancer.rpc.id
}

output "rpc_hostname" {
  value = var.rpc_domain
}
