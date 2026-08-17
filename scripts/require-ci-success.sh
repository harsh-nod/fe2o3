#!/usr/bin/env bash

set -Eeuo pipefail

if (($# == 0)); then
  printf '%s\n' 'at least one CI dependency result is required' >&2
  exit 2
fi

index=0
for result in "$@"; do
  index=$((index + 1))
  if [[ "${result}" != success ]]; then
    printf 'CI dependency %d did not succeed: %s\n' \
      "${index}" "${result:-absent}" >&2
    exit 1
  fi
done

printf 'all %d CI dependencies succeeded\n' "$#"
