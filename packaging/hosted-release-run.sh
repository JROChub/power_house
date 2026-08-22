#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  printf 'usage: %s --identity FILE --archive FILE --manifest FILE --signature FILE --allowed-signers FILE --output DIR\n' "$0" >&2
  exit 2
}

identity=
archive=
manifest=
signature=
allowed_signers=
output=
while (($#)); do
  case $1 in
    --identity) identity=$2; shift 2 ;;
    --archive) archive=$2; shift 2 ;;
    --manifest) manifest=$2; shift 2 ;;
    --signature) signature=$2; shift 2 ;;
    --allowed-signers) allowed_signers=$2; shift 2 ;;
    --output) output=$2; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n $identity && -n $archive && -n $manifest && -n $signature ]] || usage
[[ -n $allowed_signers && -n $output ]] || usage

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
identity_tool="$script_dir/validation-candidate-identity.py"
verify_tool="$script_dir/validation-verify-candidate.sh"

for tool in bash sha256sum ssh-keygen jq tar zstd find sort awk sed cp mkdir mv mktemp grep stat date python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'missing required tool: %s\n' "$tool" >&2
    exit 2
  }
done
for input in "$identity" "$archive" "$manifest" "$signature" "$allowed_signers" "$identity_tool" "$verify_tool"; do
  [[ -f $input && ! -L $input ]] || {
    printf 'input is missing, non-regular, or symlinked: %s\n' "$input" >&2
    exit 2
  }
done
python3 "$identity_tool" check --identity "$identity" --require-enabled
release_id=$(jq -er '.release_id' "$identity")
identity_sha256=$(sha256sum "$identity" | awk '{print $1}')
expected_archive_sha256=$(jq -er '.archive.sha256' "$identity")
expected_manifest_sha256=$(jq -er '.manifest.sha256' "$identity")
expected_signature_sha256=$(jq -er '.signature.sha256' "$identity")
expected_allowed_signers_sha256=$(jq -er '.allowed_signers.sha256' "$identity")
expected_key_fingerprint=$(jq -er '.allowed_signers.key_fingerprint' "$identity")
expected_executor_sha256=$(jq -er '.workload.executor_sha256' "$identity")
expected_verifier_sha256=$(jq -er '.workload.verifier_sha256' "$identity")
expected_output_root=$(jq -er '.workload.canonical_output_root' "$identity")
archive_name=$(jq -er '.archive.name' "$identity")
archive_root=$(jq -er '.archive.root' "$identity")
executor_path=$(jq -er '.archive.executor_path' "$identity")
verifier_path=$(jq -er '.archive.verifier_path' "$identity")
manifest_name=$(jq -er '.manifest.name' "$identity")
signature_name=$(jq -er '.signature.name' "$identity")
allowed_signers_name=$(jq -er '.allowed_signers.name' "$identity")
[[ ! -e $output && ! -L $output ]] || {
  printf 'refusing to overwrite output: %s\n' "$output" >&2
  exit 2
}

mkdir -p "$output/inputs" "$output/logs" "$output/run"
stage=initialization
started_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
executor_sha256=
verifier_sha256=
observed_output_root=
reference_accepted=false
executor_replay_accepted=false
signature_verified=false
archive_inventory_verified=false

finish() {
  local status=$?
  trap - EXIT
  local result=FAIL
  local seal_status=0
  local checksum_tmp=
  if ((status == 0)); then
    result=PASS
  fi
  local finished_at
  finished_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  jq -n -S \
    --arg schema 'mfenx.step3-hosted-release-run.v1' \
    --arg status "$result" \
    --arg failure_stage "$stage" \
    --arg started_at "$started_at" \
    --arg finished_at "$finished_at" \
    --arg release_id "$release_id" \
    --arg identity_sha256 "$identity_sha256" \
    --arg archive_sha256 "$expected_archive_sha256" \
    --arg manifest_sha256 "$expected_manifest_sha256" \
    --arg signature_sha256 "$expected_signature_sha256" \
    --arg allowed_signers_sha256 "$expected_allowed_signers_sha256" \
    --arg key_fingerprint "$expected_key_fingerprint" \
    --arg executor_sha256 "$executor_sha256" \
    --arg verifier_sha256 "$verifier_sha256" \
    --arg output_root "$observed_output_root" \
    --arg expected_output_root "$expected_output_root" \
    --argjson exit_status "$status" \
    --argjson reference_accepted "$reference_accepted" \
    --argjson executor_replay_accepted "$executor_replay_accepted" \
    --argjson signature_verified "$signature_verified" \
    --argjson archive_inventory_verified "$archive_inventory_verified" '
      {
        schema: $schema,
        status: $status,
        exit_status: $exit_status,
        failure_stage: (if $status == "PASS" then null else $failure_stage end),
        started_at: $started_at,
        finished_at: $finished_at,
        environment_scope: "ephemeral_github_hosted_ubuntu_vm",
        signed_input: {
          release_id: $release_id,
          identity_sha256: $identity_sha256,
          archive_sha256: $archive_sha256,
          candidate_manifest_sha256: $manifest_sha256,
          candidate_signature_sha256: $signature_sha256,
          allowed_signers_sha256: $allowed_signers_sha256,
          release_key_fingerprint: $key_fingerprint,
          candidate_signature_verified: $signature_verified,
          archive_inventory_verified: $archive_inventory_verified
        },
        execution: {
          executor_sha256: $executor_sha256,
          verifier_sha256: $verifier_sha256,
          workload: "canonical_4x4_sequence_by_identity_wrapping_i32_gemm",
          output_root: $output_root,
          expected_output_root: $expected_output_root,
          executor_exact_replay_accepted: $executor_replay_accepted,
          standalone_reference_verifier_accepted: $reference_accepted
        },
        claim_scope: {
          release_workload_reproduction: ($status == "PASS"),
          reproducible_binary_build: false,
          physical_machine_attestation: false,
          product_network_service_used: false,
          host_class: "GitHub-hosted virtual machine"
        }
      }
    ' >"$output/record.json"
  checksum_tmp=$(mktemp) || seal_status=1
  if ((seal_status == 0)); then
    (
      cd "$output"
      if find . ! -type f ! -type d -print -quit | grep -q .; then
        printf 'non-regular object found in hosted-run evidence\n' >&2
        exit 1
      fi
      find . -type f ! -path './SHA256SUMS' -printf '%P\n' | LC_ALL=C sort \
        | while IFS= read -r relative; do sha256sum "$relative"; done
    ) >"$checksum_tmp" || seal_status=1
  fi
  if ((seal_status == 0)); then
    mv -- "$checksum_tmp" "$output/SHA256SUMS" || seal_status=1
  fi
  if ((seal_status == 0)); then
    (cd "$output" && sha256sum -c SHA256SUMS >/dev/null) || seal_status=1
  fi
  if ((seal_status != 0)); then
    status=1
  fi
  exit "$status"
}
trap finish EXIT

stage=copy_signed_inputs
cp -- "$identity" "$output/inputs/validation-candidate-identity.json"
cp -- "$archive" "$output/inputs/$archive_name"
cp -- "$manifest" "$output/inputs/$manifest_name"
cp -- "$signature" "$output/inputs/$signature_name"
cp -- "$allowed_signers" "$output/inputs/$allowed_signers_name"

stage=record_pinned_hashes
{
  printf '%s  %s\n' "$identity_sha256" "$output/inputs/validation-candidate-identity.json"
  printf '%s  %s\n' "$expected_archive_sha256" "$output/inputs/$archive_name"
  printf '%s  %s\n' "$expected_manifest_sha256" "$output/inputs/$manifest_name"
  printf '%s  %s\n' "$expected_signature_sha256" "$output/inputs/$signature_name"
  printf '%s  %s\n' "$expected_allowed_signers_sha256" "$output/inputs/$allowed_signers_name"
} >"$output/logs/pinned-inputs.sha256"
sha256sum -c "$output/logs/pinned-inputs.sha256" >"$output/logs/pinned-input-validation.log"

stage=verify_and_extract_candidate
bash "$verify_tool" \
  --identity "$output/inputs/validation-candidate-identity.json" \
  --archive "$output/inputs/$archive_name" \
  --manifest "$output/inputs/$manifest_name" \
  --signature "$output/inputs/$signature_name" \
  --allowed-signers "$output/inputs/$allowed_signers_name" \
  --extract-dir "$output/extracted" \
  >"$output/logs/candidate-verification.stdout.log" \
  2>"$output/logs/candidate-verification.stderr.log"
signature_verified=true
archive_inventory_verified=true
payload="$output/extracted/$archive_root"
[[ -d $payload && ! -L $payload ]]

stage=resolve_signed_candidate_binaries
executor="$payload/$executor_path"
verifier="$payload/$verifier_path"
[[ -x $executor && -x $verifier && ! -L $executor && ! -L $verifier ]]
executor_sha256=$(sha256sum "$executor" | awk '{print $1}')
verifier_sha256=$(sha256sum "$verifier" | awk '{print $1}')
[[ $executor_sha256 == "$expected_executor_sha256" ]]
[[ $verifier_sha256 == "$expected_verifier_sha256" ]]

store="$output/run/store"
left="$output/run/left.tensor.json"
right="$output/run/right.tensor.json"
image="$output/run/product.image.json"
result="$output/run/product.result.json"

stage=create_left_matrix
"$executor" matrix-create --store "$store" --rows 4 --columns 4 \
  --fill sequence --output "$left" --io-mib 1 \
  >"$output/logs/create-left.stdout.log" 2>"$output/logs/create-left.stderr.log"
stage=create_right_matrix
"$executor" matrix-create --store "$store" --rows 4 --columns 4 \
  --fill identity --output "$right" --io-mib 1 \
  >"$output/logs/create-right.stdout.log" 2>"$output/logs/create-right.stderr.log"
stage=compile_workload
"$executor" compile --store "$store" --left "$left" --right "$right" \
  --output "$image" --max-managed-mib 64 --lanes 2 --io-mib 1 \
  >"$output/logs/compile.stdout.log" 2>"$output/logs/compile.stderr.log"
stage=execute_workload
"$executor" run --store "$store" --image "$image" \
  --checkpoint-dir "$output/run/checkpoint" --output "$result" --io-mib 1 \
  >"$output/logs/run.stdout.log" 2>"$output/logs/run.stderr.log"
stage=executor_replay
"$executor" verify --store "$store" --image "$image" --result "$result" --io-mib 1 \
  >"$output/run/executor.verify.json" 2>"$output/logs/executor-verify.stderr.log"
executor_replay_accepted=true
stage=reference_replay
"$verifier" --store "$store" --image "$image" --result "$result" \
  --report "$output/run/reference.verify.json" \
  >"$output/logs/reference-verify.stdout.log" 2>"$output/logs/reference-verify.stderr.log"
jq -e '
  .accepted == true
  and .arithmetic.operation == "streamed_i32_gemm"
  and .arithmetic.values_checked == 16
  and .checks.exact_output_replay == true
' "$output/run/reference.verify.json" >"$output/logs/reference-report-gate.log"
reference_accepted=true
observed_output_root=$(jq -er '.outputs[0].tensor.manifest_digest' "$result")
[[ $observed_output_root == "$expected_output_root" ]]
stage=complete
