#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 3 ]] || exit 64
source_dir=$1
tool=$2
build_dir=$3
work_parent=$(/usr/bin/mktemp -d --tmpdir="$build_dir" secure-protocol.XXXXXXXX)
trap '/usr/bin/rm -rf -- "$work_parent"' EXIT

"$source_dir/tests/secure_protocol.sh" "$source_dir" "$tool" \
  "$work_parent/run"
