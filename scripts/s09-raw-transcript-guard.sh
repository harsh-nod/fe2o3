#!/usr/bin/env bash

# This file is sourced by the fixed S09 ROCgdb runner.

s09_delete_raw_transcript() {
  local raw_path="${S09_RAW_TRANSCRIPT_PATH:-}"
  if [[ -n "${raw_path}" ]]; then
    rm -f -- "${raw_path}"
  fi
}

s09_install_raw_transcript_guard() {
  if (($# != 1)) || [[ "$1" != /* ]]; then
    printf 's09-raw-transcript-guard: raw path must be absolute\n' >&2
    return 2
  fi
  S09_RAW_TRANSCRIPT_PATH="$1"
  readonly S09_RAW_TRANSCRIPT_PATH
  trap 's09_delete_raw_transcript' EXIT
  trap 's09_delete_raw_transcript; exit 129' HUP
  trap 's09_delete_raw_transcript; exit 130' INT
  trap 's09_delete_raw_transcript; exit 143' TERM
}
