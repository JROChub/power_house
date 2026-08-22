#!/usr/bin/env bash
set -Eeuo pipefail

rustc --version --verbose
cargo --version --verbose
rustc +nightly-2026-07-10 --version --verbose
cargo audit --version
cargo deny --version
cargo fuzz --version
ssh -V
uname -a
