#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  printf 'usage: %s --identity FILE --archive FILE --manifest FILE --signature FILE --allowed-signers FILE --extract-dir DIR\n' "$0" >&2
  exit 2
}

identity=
archive=
manifest=
signature=
allowed_signers=
extract_dir=
while (($#)); do
  case $1 in
    --identity) identity=$2; shift 2 ;;
    --archive) archive=$2; shift 2 ;;
    --manifest) manifest=$2; shift 2 ;;
    --signature) signature=$2; shift 2 ;;
    --allowed-signers) allowed_signers=$2; shift 2 ;;
    --extract-dir) extract_dir=$2; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n $identity && -n $archive && -n $manifest && -n $signature ]] || usage
[[ -n $allowed_signers && -n $extract_dir ]] || usage

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
identity_tool="$script_dir/validation-candidate-identity.py"
for tool in bash python3 tar zstd find grep awk; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'missing required candidate-verification tool: %s\n' "$tool" >&2
    exit 2
  }
done
[[ -f $identity_tool && ! -L $identity_tool ]] || {
  printf 'candidate identity validator is missing or symlinked\n' >&2
  exit 2
}
[[ ! -e $extract_dir && ! -L $extract_dir ]] || {
  printf 'refusing to overwrite extraction directory: %s\n' "$extract_dir" >&2
  exit 2
}

python3 "$identity_tool" check --identity "$identity" --require-enabled
python3 "$identity_tool" verify-files \
  --identity "$identity" \
  --archive "$archive" \
  --manifest "$manifest" \
  --signature "$signature" \
  --allowed-signers "$allowed_signers"

members_file=$(mktemp)
types_file=$(mktemp)
cleanup() {
  rm -f -- "$members_file" "$types_file"
}
trap cleanup EXIT

tar --zstd --list --quoting-style=escape --file "$archive" >"$members_file"
python3 "$identity_tool" check-members --identity "$identity" --members "$members_file"
tar --zstd --list --verbose --file "$archive" | awk '{print substr($1, 1, 1)}' >"$types_file"
if grep -Ev '^[-d]$' "$types_file" | grep -q .; then
  printf 'candidate archive contains a non-file, non-directory member\n' >&2
  exit 1
fi

mkdir -p -- "$extract_dir"
tar --zstd --extract --file "$archive" --directory "$extract_dir" \
  --no-same-owner --no-same-permissions
if find "$extract_dir" ! -type f ! -type d -print -quit | grep -q .; then
  printf 'candidate archive extracted a symlink or special filesystem object\n' >&2
  exit 1
fi

archive_root=$(python3 "$identity_tool" export-github --identity "$identity" \
  | awk -F= '$1 == "archive_root" {sub(/^[^=]*=/, ""); print; exit}')
payload="$extract_dir/$archive_root"
[[ -d $payload && ! -L $payload ]] || {
  printf 'candidate archive root is absent after extraction\n' >&2
  exit 1
}
python3 "$identity_tool" verify-extracted \
  --identity "$identity" \
  --root "$payload" \
  --manifest "$manifest" \
  --signature "$signature" \
  --allowed-signers "$allowed_signers"
printf '%s\n' "$payload"
