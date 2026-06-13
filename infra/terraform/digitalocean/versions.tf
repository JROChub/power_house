terraform {
  required_version = ">= 1.6.0"

  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = ">= 2.68.0, < 3.0.0"
    }
  }
}

provider "digitalocean" {}
