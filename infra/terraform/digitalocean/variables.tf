variable "project_name" {
  type    = string
  default = "MFENX Power-House"
}

variable "ssh_key_fingerprints" {
  type        = list(string)
  description = "DigitalOcean SSH key fingerprints installed on every validator."
}

variable "operator_ssh_cidr" {
  type        = string
  description = "Public operator CIDR allowed to reach TCP 22."
}

variable "rpc_domain" {
  type    = string
  default = "rpc.mfenx.com"
}

variable "droplet_size" {
  type    = string
  default = "s-2vcpu-2gb"
}

variable "nodes" {
  type = map(object({
    region = string
  }))
  default = {
    validator-1 = { region = "nyc3" }
    validator-2 = { region = "sfo3" }
    validator-3 = { region = "ams3" }
  }
}
