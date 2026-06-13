resource "digitalocean_project" "powerhouse" {
  name        = var.project_name
  description = "MFENX Power House production validator and RPC infrastructure"
  purpose     = "Blockchain"
  environment = "Production"
}

resource "digitalocean_tag" "validator" {
  name = "mfenx-rpc-validator"
}

resource "digitalocean_droplet" "validator" {
  for_each = var.nodes

  name       = "mfenx-${each.key}"
  image      = "ubuntu-24-04-x64"
  region     = each.value.region
  size       = var.droplet_size
  monitoring = true
  backups    = true
  ssh_keys   = var.ssh_key_fingerprints
  tags       = [digitalocean_tag.validator.id]
  user_data  = file("${path.module}/../../digitalocean/cloud-init.yaml")

  lifecycle {
    prevent_destroy = true
  }
}

resource "digitalocean_project_resources" "validators" {
  project = digitalocean_project.powerhouse.id
  resources = [
    for validator in digitalocean_droplet.validator :
    validator.urn
  ]
}

resource "digitalocean_loadbalancer" "rpc" {
  name          = "lax-mfenx-rpc"
  type          = "GLOBAL"
  network       = "EXTERNAL"
  network_stack = "DUALSTACK"
  region        = var.nodes["validator-1"].region
  project_id    = digitalocean_project.powerhouse.id
  droplet_ids   = [for validator in digitalocean_droplet.validator : validator.id]

  domains {
    name       = var.rpc_domain
    is_managed = true
  }

  glb_settings {
    target_protocol = "http"
    target_port     = 80

    cdn {
      is_enabled = false
    }
  }

  healthcheck {
    protocol                 = "http"
    port                     = 80
    path                     = "/healthz"
    check_interval_seconds   = 10
    response_timeout_seconds = 5
    healthy_threshold        = 2
    unhealthy_threshold      = 3
  }

  redirect_http_to_https    = true
  http_idle_timeout_seconds = 60
  tls_cipher_policy         = "STRONG"

  lifecycle {
    prevent_destroy = true
  }
}

resource "digitalocean_firewall" "validator" {
  name = "mfenx-rpc-firewall"
  tags = [digitalocean_tag.validator.name]

  inbound_rule {
    protocol         = "tcp"
    port_range       = "22"
    source_addresses = [var.operator_ssh_cidr]
  }

  inbound_rule {
    protocol                  = "tcp"
    port_range                = "80"
    source_load_balancer_uids = [digitalocean_loadbalancer.rpc.id]
  }

  inbound_rule {
    protocol    = "tcp"
    port_range  = "7001"
    source_tags = [digitalocean_tag.validator.name]
  }

  inbound_rule {
    protocol    = "tcp"
    port_range  = "9090"
    source_tags = [digitalocean_tag.validator.name]
  }

  inbound_rule {
    protocol    = "tcp"
    port_range  = "9100-9101"
    source_tags = [digitalocean_tag.validator.name]
  }

  outbound_rule {
    protocol              = "tcp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "udp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }
}
