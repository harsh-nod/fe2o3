#!/usr/bin/env bash
set -uo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
run() {
    local name=$1
    shift
    printf '%s\n' "$*" > "$packet/raw/$name.command"
    "$@" > "$packet/raw/$name.log" 2>&1
    local status=$?
    printf '%s\n' "$status" > "$packet/raw/$name.status"
    printf '%s: %s\n' "$name" "$status"
}
run ssh ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x hostname
run issue gh issue view 182 --repo harsh-nod/fe2o3 --json number,state,title,updatedAt,url
