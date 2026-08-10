#!/usr/bin/env bash

# This file is sourced by the fixed S09 ROCgdb runner.

S09_GUARDED_CHILD_PID=
S09_GUARDED_STATUS_FD=
S09_GUARDED_DEFERRED_SIGNAL=
S09_RAW_TRANSCRIPT_FD=
S09_RAW_TRANSCRIPT_PATH=
S09_RAW_GUARD_SOURCE="${BASH_SOURCE[0]}"

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

s09_close_raw_transcript_fd() {
  if [[ -n "${S09_RAW_TRANSCRIPT_FD:-}" ]]; then
    exec {S09_RAW_TRANSCRIPT_FD}>&-
    S09_RAW_TRANSCRIPT_FD=
  fi
}

s09_close_guarded_status_fd() {
  if [[ -n "${S09_GUARDED_STATUS_FD:-}" ]]; then
    exec {S09_GUARDED_STATUS_FD}<&-
    S09_GUARDED_STATUS_FD=
  fi
}

s09_hold_guarded_group() {
  while :; do
    /usr/bin/sleep 1
  done
}

s09_guarded_group_supervisor() {
  local raw_fd="$1"
  local command_pid status supervisor_pgid
  local cancelled=0
  local proceed=0
  shift

  trap 'cancelled=1' HUP INT TERM
  trap 'proceed=1' USR1
  supervisor_pgid="$(/usr/bin/ps -o pgid= -p "${BASHPID}" | /usr/bin/tr -d '[:space:]')"
  if [[ -z "${supervisor_pgid}" || "${supervisor_pgid}" != "${BASHPID}" ]]; then
    printf 'INVALID-GROUP %s %s\n' "${BASHPID}" "${supervisor_pgid:-missing}"
    return 125
  fi
  printf 'READY %s %s\n' "${BASHPID}" "${supervisor_pgid}"
  while ((proceed == 0 && cancelled == 0)); do
    /usr/bin/sleep 1
  done
  if ((cancelled != 0)); then
    s09_hold_guarded_group
  fi

  proceed=0
  "$@" 1>&"${raw_fd}" 2>&1 &
  command_pid=$!
  if wait "${command_pid}"; then
    status=0
  else
    status=$?
  fi
  if ((cancelled != 0)); then
    s09_hold_guarded_group
  fi

  printf 'STATUS %d\n' "${status}"
  while ((proceed == 0 && cancelled == 0)); do
    /usr/bin/sleep 1
  done
  if ((cancelled != 0)); then
    s09_hold_guarded_group
  fi
}

s09_stop_guarded_group() {
  local child_pid="${S09_GUARDED_CHILD_PID:-}"
  local cleanup_status=0
  local attempt
  if [[ -z "${child_pid}" ]]; then
    return 0
  fi

  # The supervisor traps TERM and deliberately remains the live group leader.
  # Every numeric PGID operation therefore precedes the one and only reap.
  kill -TERM -- "-${child_pid}" 2>/dev/null || cleanup_status=1
  for ((attempt = 0; attempt < 25; attempt++)); do
    /usr/bin/sleep 0.02
  done
  if ! kill -KILL -- "-${child_pid}" 2>/dev/null; then
    kill -KILL -- "${child_pid}" 2>/dev/null || cleanup_status=1
  fi
  wait "${child_pid}" 2>/dev/null || true
  S09_GUARDED_CHILD_PID=
  s09_close_guarded_status_fd
  return "${cleanup_status}"
}

s09_guarded_exit() {
  local status=$?
  local cleanup_status=0
  trap - EXIT HUP INT TERM
  s09_delete_raw_transcript || cleanup_status=1
  s09_close_raw_transcript_fd || cleanup_status=1
  s09_stop_guarded_group || cleanup_status=1
  if ((cleanup_status != 0)); then
    status=125
  fi
  exit "${status}"
}

s09_guarded_signal() {
  local status="$1"
  local cleanup_status=0
  trap - HUP INT TERM
  # Remove the pathname before any potentially slow process teardown. The
  # command only inherited an already-open descriptor and cannot reopen it.
  s09_delete_raw_transcript || cleanup_status=1
  s09_close_raw_transcript_fd || cleanup_status=1
  s09_stop_guarded_group || cleanup_status=1
  if ((cleanup_status != 0)); then
    status=125
  fi
  exit "${status}"
}

s09_guarded_defer_signal() {
  if [[ -z "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
    S09_GUARDED_DEFERRED_SIGNAL="$1"
  fi
  # Defer only process-group teardown. The pathname and parent-owned FD must
  # disappear at cancellation entry, including while supervisor startup or
  # the final PID/PGID-to-reap transition is in progress.
  s09_delete_raw_transcript || true
  s09_close_raw_transcript_fd || true
}

s09_install_active_signal_traps() {
  trap 's09_guarded_signal 129' HUP
  trap 's09_guarded_signal 130' INT
  trap 's09_guarded_signal 143' TERM
}

s09_install_deferred_signal_traps() {
  trap 's09_guarded_defer_signal 129' HUP
  trap 's09_guarded_defer_signal 130' INT
  trap 's09_guarded_defer_signal 143' TERM
}

s09_finish_deferred_signal_transition() {
  local deferred_status="${S09_GUARDED_DEFERRED_SIGNAL:-}"
  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_active_signal_traps
  if [[ -n "${deferred_status}" ]]; then
    s09_guarded_signal "${deferred_status}"
  fi
}

s09_run_guarded_raw_command() {
  local child_pid coproc_input_fd raw_fd read_status status supervisor_message
  if (($# == 0)); then
    printf 's09-raw-transcript-guard: guarded command is missing\n' >&2
    return 2
  fi
  if [[ -n "${S09_GUARDED_CHILD_PID:-}" ]]; then
    printf 's09-raw-transcript-guard: guarded command is already active\n' >&2
    return 2
  fi

  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_deferred_signal_traps

  # Install deferred traps before creating the pathname. If a signal arrives
  # before or during the open, the post-open checkpoint removes the newly
  # opened file before supervisor startup can continue.
  exec {raw_fd}>"${S09_RAW_TRANSCRIPT_PATH}"
  S09_RAW_TRANSCRIPT_FD="${raw_fd}"
  if [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
    s09_delete_raw_transcript || true
    s09_close_raw_transcript_fd || true
    s09_finish_deferred_signal_transition
  fi

  set +m
  # The nested Bash, rather than this parent shell, expands its positional args.
  # shellcheck disable=SC2016
  coproc S09_GUARDED_COPROC {
    exec /usr/bin/setsid --wait -- /bin/bash -c '
      source "$1"
      shift
      s09_guarded_group_supervisor "$@"
    ' s09-group-supervisor "${S09_RAW_GUARD_SOURCE}" "${raw_fd}" "$@"
  }
  child_pid="${S09_GUARDED_COPROC_PID}"
  S09_GUARDED_CHILD_PID="${child_pid}"
  S09_GUARDED_STATUS_FD="${S09_GUARDED_COPROC[0]}"
  coproc_input_fd="${S09_GUARDED_COPROC[1]}"
  exec {coproc_input_fd}>&-

  while :; do
    if IFS= read -r supervisor_message <&"${S09_GUARDED_STATUS_FD}"; then
      break
    else
      read_status=$?
    fi
    if ((read_status > 128)) && [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
      continue
    fi
    supervisor_message=
    break
  done
  if [[ ! "${supervisor_message}" =~ ^READY\ ([0-9]+)\ ([0-9]+)$ ]] ||
    [[ "${BASH_REMATCH[1]}" != "${child_pid}" ||
      "${BASH_REMATCH[2]}" != "${child_pid}" ]]; then
    wait "${child_pid}" 2>/dev/null || true
    S09_GUARDED_CHILD_PID=
    s09_close_guarded_status_fd
    s09_close_raw_transcript_fd
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded supervisor did not become ready\n' >&2
    return 125
  fi
  s09_finish_deferred_signal_transition

  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_deferred_signal_traps
  if ! kill -USR1 -- "${child_pid}" 2>/dev/null; then
    wait "${child_pid}" 2>/dev/null || true
    S09_GUARDED_CHILD_PID=
    s09_close_guarded_status_fd
    s09_close_raw_transcript_fd
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded supervisor could not start command\n' >&2
    return 125
  fi
  s09_finish_deferred_signal_transition

  if ! IFS= read -r supervisor_message <&"${S09_GUARDED_STATUS_FD}"; then
    S09_GUARDED_DEFERRED_SIGNAL=
    s09_install_deferred_signal_traps
    wait "${child_pid}" 2>/dev/null || true
    S09_GUARDED_CHILD_PID=
    s09_close_guarded_status_fd
    s09_close_raw_transcript_fd
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded supervisor returned no status\n' >&2
    return 125
  fi
  if [[ ! "${supervisor_message}" =~ ^STATUS\ ([0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])$ ]]; then
    s09_stop_guarded_group || true
    s09_close_raw_transcript_fd
    printf 's09-raw-transcript-guard: guarded supervisor returned invalid status\n' >&2
    return 125
  fi
  status="${BASH_REMATCH[1]}"

  # The supervisor holds its PID/PGID until USR1. Defer cancellation across the
  # release and reap transition so no handler can target a reused numeric ID.
  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_deferred_signal_traps
  if ! kill -USR1 -- "${child_pid}" 2>/dev/null; then
    status=125
  fi
  wait "${child_pid}" 2>/dev/null || status=125
  S09_GUARDED_CHILD_PID=
  s09_close_guarded_status_fd
  s09_close_raw_transcript_fd
  s09_finish_deferred_signal_transition
  return "${status}"
}

s09_install_raw_transcript_guard() {
  if (($# != 1)) || [[ "$1" != /* ]]; then
    printf 's09-raw-transcript-guard: raw path must be absolute\n' >&2
    return 2
  fi
  S09_RAW_TRANSCRIPT_PATH="$1"
  readonly S09_RAW_TRANSCRIPT_PATH
  readonly S09_RAW_GUARD_SOURCE
  trap 's09_guarded_exit' EXIT
  s09_install_active_signal_traps
}
