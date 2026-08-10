#!/usr/bin/env bash

# This file is sourced by the fixed S09 ROCgdb runner.

S09_GUARDED_CHILD_PID=

s09_delete_raw_transcript() {
  local raw_path="${S09_RAW_TRANSCRIPT_PATH:-}"
  if [[ -n "${raw_path}" ]]; then
    rm -f -- "${raw_path}"
    if [[ -e "${raw_path}" || -L "${raw_path}" ]]; then
      printf 's09-raw-transcript-guard: raw transcript could not be removed\n' >&2
      return 1
    fi
  fi
}

s09_guarded_group_exists() {
  local child_pid="${S09_GUARDED_CHILD_PID:-}"
  [[ -n "${child_pid}" ]] && kill -0 -- "-${child_pid}" 2>/dev/null
}

s09_stop_guarded_group() {
  local child_pid="${S09_GUARDED_CHILD_PID:-}"
  local attempt
  if [[ -z "${child_pid}" ]]; then
    return 0
  fi

  # setsid makes the child's PID its process-group ID. Target that exact group,
  # never the runner's inherited supervisor group. Also target the leader while
  # setsid is still starting so a signal cannot escape through the setup window.
  kill -TERM -- "${child_pid}" 2>/dev/null || true
  kill -TERM -- "-${child_pid}" 2>/dev/null || true
  for ((attempt = 0; attempt < 25; attempt++)); do
    if ! kill -0 -- "${child_pid}" 2>/dev/null && ! s09_guarded_group_exists; then
      break
    fi
    /usr/bin/sleep 0.02
  done
  if kill -0 -- "${child_pid}" 2>/dev/null || s09_guarded_group_exists; then
    kill -KILL -- "${child_pid}" 2>/dev/null || true
    kill -KILL -- "-${child_pid}" 2>/dev/null || true
  fi
  wait "${child_pid}" 2>/dev/null || true
  for ((attempt = 0; attempt < 25; attempt++)); do
    if ! kill -0 -- "-${child_pid}" 2>/dev/null; then
      break
    fi
    /usr/bin/sleep 0.02
  done
  S09_GUARDED_CHILD_PID=

  if kill -0 -- "-${child_pid}" 2>/dev/null; then
    printf 's09-raw-transcript-guard: guarded process group survived cleanup\n' >&2
    return 1
  fi
}

s09_guarded_exit() {
  local status=$?
  local cleanup_status=0
  trap - EXIT HUP INT TERM
  s09_stop_guarded_group || cleanup_status=1
  s09_delete_raw_transcript || cleanup_status=1
  if ((cleanup_status != 0)); then
    status=125
  fi
  exit "${status}"
}

s09_guarded_signal() {
  local status="$1"
  local cleanup_status=0
  trap - HUP INT TERM
  s09_stop_guarded_group || cleanup_status=1
  s09_delete_raw_transcript || cleanup_status=1
  if ((cleanup_status != 0)); then
    status=125
  fi
  exit "${status}"
}

s09_run_guarded_raw_command() {
  local attempt child_pid child_pgid status
  if (($# == 0)); then
    printf 's09-raw-transcript-guard: guarded command is missing\n' >&2
    return 2
  fi
  if [[ -n "${S09_GUARDED_CHILD_PID:-}" ]]; then
    printf 's09-raw-transcript-guard: guarded command is already active\n' >&2
    return 2
  fi

  # Non-interactive job control is explicitly disabled so setsid can exec in
  # place and the background PID is also the new session/process-group ID.
  set +m
  /usr/bin/setsid --wait -- "$@" >"${S09_RAW_TRANSCRIPT_PATH}" 2>&1 &
  child_pid=$!
  S09_GUARDED_CHILD_PID="${child_pid}"

  child_pgid=
  for ((attempt = 0; attempt < 25; attempt++)); do
    child_pgid="$(/usr/bin/ps -o pgid= -p "${child_pid}" 2>/dev/null | /usr/bin/tr -d '[:space:]')"
    if [[ -z "${child_pgid}" || "${child_pgid}" == "${child_pid}" ]]; then
      break
    fi
    /usr/bin/sleep 0.01
  done
  if [[ -n "${child_pgid}" && "${child_pgid}" != "${child_pid}" ]]; then
    s09_stop_guarded_group || true
    printf 's09-raw-transcript-guard: guarded command did not enter its own process group\n' >&2
    return 125
  fi

  if wait "${child_pid}"; then
    status=0
  else
    status=$?
  fi
  S09_GUARDED_CHILD_PID=
  if kill -0 -- "-${child_pid}" 2>/dev/null; then
    S09_GUARDED_CHILD_PID="${child_pid}"
    s09_stop_guarded_group || status=125
  fi
  return "${status}"
}

s09_install_raw_transcript_guard() {
  if (($# != 1)) || [[ "$1" != /* ]]; then
    printf 's09-raw-transcript-guard: raw path must be absolute\n' >&2
    return 2
  fi
  S09_RAW_TRANSCRIPT_PATH="$1"
  readonly S09_RAW_TRANSCRIPT_PATH
  trap 's09_guarded_exit' EXIT
  trap 's09_guarded_signal 129' HUP
  trap 's09_guarded_signal 130' INT
  trap 's09_guarded_signal 143' TERM
}
