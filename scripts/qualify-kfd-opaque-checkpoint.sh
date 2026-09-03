#!/usr/bin/env bash
set -euo pipefail

umask 077

usage() {
  printf 'usage: %s [--bless]\n' "$0" >&2
  exit 2
}

bless=false
if (( $# == 1 )); then
  [[ $1 == --bless ]] || usage
  bless=true
elif (( $# != 0 )); then
  usage
fi

root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
cd "$root"
[[ -z $(git status --porcelain=v1 --untracked-files=all) ]] || {
  printf 'qualification requires an exact clean source tree\n' >&2
  exit 1
}

source_commit=$(git rev-parse HEAD^{commit})
source_tree=$(git rev-parse HEAD^{tree})
temporary=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-kfd-checkpoint-qualification.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
receipt="$temporary/fe2o3-direct-kfd-opaque-checkpoint-qualification-v1.json"

FE2O3_KFD_OPAQUE_CHECKPOINT_QUALIFICATION_RECEIPT_V1="$receipt" \
  cargo test --locked -p fe2o3-runtime --features hardware-qualification \
  --test kfd_opaque_checkpoint_live \
  mi300x_captures_nonempty_same_queue_opaque_checkpoint -- \
  --exact --ignored --nocapture --test-threads=1

[[ -f $receipt && ! -L $receipt ]] || {
  printf 'live qualification emitted no regular receipt\n' >&2
  exit 1
}
[[ $(stat -c '%a:%h' "$receipt") == 600:1 ]] || {
  printf 'live qualification receipt lost private single-link custody\n' >&2
  exit 1
}

receipt_sha256=$(sha256sum "$receipt" | awk '{print $1}')
receipt_bytes=$(wc -c < "$receipt" | tr -d ' ')
printf 'producer_commit=%s\nproducer_tree=%s\nreceipt_sha256=%s\nreceipt_bytes=%s\n' \
  "$source_commit" "$source_tree" "$receipt_sha256" "$receipt_bytes"

$bless || exit 0

evidence=docs/evidence/mi300x-direct-kfd-opaque-checkpoint-qualification-v1.json
narrative=docs/evidence/mi300x-direct-kfd-opaque-checkpoint-qualification-2026-09-03.md
install -m 0644 "$receipt" "$evidence.new"
mv -f "$evidence.new" "$evidence"
cat > "$narrative.new" <<EOF
# MI300X direct-KFD opaque-checkpoint qualification, 2026-09-03

This is a bounded, caller-bound qualification of one exact active capture. The
producer source was commit \`$source_commit\`, tree \`$source_tree\`. The host
kernel was \`$(uname -r)\`.

The canonical redacted receipt is
\`mi300x-direct-kfd-opaque-checkpoint-qualification-v1.json\`. Its raw SHA-256
is \`$receipt_sha256\` over $receipt_bytes bytes. Its producer-manifest SHA-256
is \`18fdfd09a075ea73d0e7f731954d0a0681172cff163082c369a3a3f509492258\`.

The ignored MI300X gate ran the repository-owned finite Wave64 liveness fixture,
joined its target-declared publication to the exact KFD queue, suspended that
queue, captured every nonempty control-stack and wave-state range announced by
the eight public KFD headers, dropped the private zeroizing checkpoint, resumed
the queue, validated target output, observed runtime disable and terminal
telemetry, finished the debugger session, and reaped the successful child before
publishing this file.

The receipt contains relative range metadata and scoped native correlation
commitments. It contains no checkpoint bytes, stopped-state scope, raw address,
native process/GPU/queue/event ID, descriptor, handle, or live selector. Its
self-identity and Git-pinned raw digest detect substitution but are not a
signature and do not authenticate KFD, firmware, hardware, or physical artifact
execution. Capture was sequential and non-atomic; runtime and physical
suspension were not reobserved. Wave, lane, register, PC, source, and target
memory decoding remain unavailable.
EOF
chmod 0644 "$narrative.new"
mv -f "$narrative.new" "$narrative"
printf 'blessed=%s\n' "$evidence"
