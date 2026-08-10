#!/usr/bin/env bash

# This file is sourced by the fixed S09 ROCgdb runner.

S09_GUARDED_CHILD_PID=
S09_GUARDED_CONTROL_FD=
S09_GUARDED_GROUP_VERIFIED=
S09_GUARDED_STATUS_FD=
S09_GUARDED_DEFERRED_SIGNAL=
S09_GUARDED_DEFERRED_SIGNAL_EPOCH=0
S09_GUARDED_EXIT_POLL_LIMIT=30
S09_GUARDED_STARTUP_POLL_LIMIT=20
S09_GUARDED_TEARDOWN_ABANDONED=
S09_GUARDED_WAIT_STATUS=
S09_RAW_TRANSCRIPT_FD=
S09_RAW_TRANSCRIPT_PATH=

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

s09_close_guarded_control_fd() {
  if [[ -n "${S09_GUARDED_CONTROL_FD:-}" ]]; then
    exec {S09_GUARDED_CONTROL_FD}>&-
    S09_GUARDED_CONTROL_FD=
  fi
}

s09_wait_guarded_leader() {
  local child_pid="$1"
  local wait_epoch wait_status

  S09_GUARDED_WAIT_STATUS=
  while :; do
    wait_epoch="${S09_GUARDED_DEFERRED_SIGNAL_EPOCH}"
    if wait "${child_pid}"; then
      wait_status=0
    else
      wait_status=$?
    fi
    if ((wait_status > 128)) &&
      ((S09_GUARDED_DEFERRED_SIGNAL_EPOCH != wait_epoch)); then
      continue
    fi
    if ((wait_status == 127)); then
      return 1
    fi
    S09_GUARDED_WAIT_STATUS="${wait_status}"
    return 0
  done
}

s09_observe_guarded_exit() {
  local attempt read_status
  if [[ -z "${S09_GUARDED_STATUS_FD:-}" ]]; then
    return 1
  fi

  for ((attempt = 0; attempt < S09_GUARDED_EXIT_POLL_LIMIT; attempt++)); do
    if IFS= read -r -t 0.1 _ <&"${S09_GUARDED_STATUS_FD}"; then
      continue
    else
      read_status=$?
    fi
    if ((read_status == 1)); then
      return 0
    fi
    if ((read_status == 142)) ||
      { ((read_status > 128)) && [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; }; then
      continue
    fi
    return 1
  done
  return 1
}

s09_guarded_launcher() {
  # The launcher, not the outer shell, owns all numeric PGID operations. It
  # stays the session/group leader until DRAIN atomically targets its own PGID.
  # shellcheck disable=SC2016
  unset PYTHONBREAKPOINT PYTHONHOME PYTHONINSPECT PYTHONPATH PYTHONPLATLIBDIR
  unset PYTHONSAFEPATH PYTHONSTARTUP PYTHONUSERBASE PYTHONWARNINGS
  exec /usr/bin/python3 -I -S -c '
import os
import selectors
import signal
import subprocess
import sys
import time

raw_fd = int(sys.argv[1])
command = sys.argv[2:]
protocol = os.fdopen(os.dup(1), "w", buffering=1, encoding="ascii")
cancelled = 0


def emit(message):
    protocol.write(message + "\n")


def latch(signum, _frame):
    global cancelled
    cancelled = signum


def drain():
    try:
        os.close(raw_fd)
    except OSError:
        pass
    term_sent = False
    try:
        os.killpg(os.getpgrp(), signal.SIGTERM)
        term_sent = True
    except OSError:
        pass
    if term_sent:
        time.sleep(0.5)
    try:
        os.killpg(os.getpgrp(), signal.SIGKILL)
    except OSError:
        os._exit(125)
    while True:
        signal.pause()


try:
    os.setsid()
except OSError as error:
    emit(f"ANCHOR-ERROR {error.errno}")
    os._exit(125)

for caught in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
    signal.signal(caught, latch)

leader = os.getpid()
group = os.getpgrp()
if leader != group:
    emit(f"ANCHOR-ERROR {leader} {group}")
    os._exit(125)

emit(f"ANCHOR {leader} {group}")
emit(f"READY {leader} {group}")

selector = selectors.DefaultSelector()
selector.register(sys.stdin.buffer, selectors.EVENT_READ)
process = None
status_sent = False

while True:
    if cancelled:
        drain()
    for key, _events in selector.select(0.02):
        line = key.fileobj.readline()
        if not line or line == b"DRAIN\n":
            drain()
        if line != b"START\n" or process is not None:
            emit("PROTOCOL-ERROR")
            drain()
        try:
            process = subprocess.Popen(
                command,
                stdin=subprocess.DEVNULL,
                stdout=raw_fd,
                stderr=subprocess.STDOUT,
                close_fds=True,
                pass_fds=(raw_fd,),
            )
        except (OSError, ValueError):
            emit("STATUS 125")
            status_sent = True
    if process is not None and not status_sent:
        returncode = process.poll()
        if returncode is not None:
            status = returncode if returncode >= 0 else 128 - returncode
            emit(f"STATUS {min(status, 255)}")
            status_sent = True
' "$@"
}

s09_stop_guarded_group() {
  local child_pid="${S09_GUARDED_CHILD_PID:-}"
  local preserve_raw="${1:-0}"
  local cleanup_status=0

  # Cancellation and failure invalidate the evidence before teardown. A
  # completed command may retain the pathname, but no writer descriptor.
  if [[ "${preserve_raw}" != 1 ]]; then
    s09_delete_raw_transcript || cleanup_status=1
  fi
  s09_close_raw_transcript_fd || cleanup_status=1
  if ((cleanup_status != 0)); then
    s09_delete_raw_transcript || cleanup_status=1
  fi
  if [[ -z "${child_pid}" ]]; then
    s09_close_guarded_control_fd
    return "${cleanup_status}"
  fi
  if [[ "${S09_GUARDED_TEARDOWN_ABANDONED:-}" == 1 ]]; then
    s09_close_guarded_control_fd
    s09_close_guarded_status_fd
    s09_delete_raw_transcript || cleanup_status=1
    return 1
  fi

  if [[ "${S09_GUARDED_GROUP_VERIFIED:-}" != 1 ]]; then
    # EOF asks the trusted launcher to self-drain if it reached its event loop.
    # No numeric operation is permitted for an unanchored shell child.
    cleanup_status=1
  else
    if ! printf 'DRAIN\n' >&"${S09_GUARDED_CONTROL_FD}"; then
      cleanup_status=1
      s09_delete_raw_transcript || cleanup_status=1
    fi
  fi
  s09_close_guarded_control_fd

  if ! s09_observe_guarded_exit; then
    # A same-UID SIGSTOP can force this bounded fail-closed path. Without a
    # protected supervisor the local guard cannot resume or reap that process.
    cleanup_status=1
    S09_GUARDED_TEARDOWN_ABANDONED=1
    s09_close_guarded_status_fd
    disown "${child_pid}" 2>/dev/null || true
  elif ! s09_wait_guarded_leader "${child_pid}"; then
    cleanup_status=1
  elif [[ "${S09_GUARDED_GROUP_VERIFIED:-}" == 1 &&
    "${S09_GUARDED_WAIT_STATUS}" != 137 ]]; then
    cleanup_status=1
  fi
  if ((cleanup_status != 0)); then
    s09_delete_raw_transcript || cleanup_status=1
  fi
  if [[ "${S09_GUARDED_TEARDOWN_ABANDONED:-}" != 1 ]]; then
    S09_GUARDED_CHILD_PID=
    S09_GUARDED_GROUP_VERIFIED=
    s09_close_guarded_status_fd
  fi
  return "${cleanup_status}"
}

s09_guarded_exit() {
  local status=$?
  local cleanup_status=0
  trap - EXIT
  trap '' HUP INT TERM
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
  trap '' HUP INT TERM
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
  S09_GUARDED_DEFERRED_SIGNAL_EPOCH=$((S09_GUARDED_DEFERRED_SIGNAL_EPOCH + 1))
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
  local deferred_status
  s09_install_active_signal_traps
  deferred_status="${S09_GUARDED_DEFERRED_SIGNAL:-}"
  S09_GUARDED_DEFERRED_SIGNAL=
  if [[ -n "${deferred_status}" ]]; then
    s09_guarded_signal "${deferred_status}"
  fi
}

s09_run_guarded_raw_command() {
  local attempt child_pid raw_fd read_status status supervisor_message
  if (($# == 0)); then
    printf 's09-raw-transcript-guard: guarded command is missing\n' >&2
    return 2
  fi
  if [[ -n "${S09_GUARDED_CHILD_PID:-}" ]]; then
    printf 's09-raw-transcript-guard: guarded command is already active\n' >&2
    return 2
  fi

  S09_GUARDED_DEFERRED_SIGNAL=
  S09_GUARDED_DEFERRED_SIGNAL_EPOCH=0
  S09_GUARDED_GROUP_VERIFIED=
  S09_GUARDED_TEARDOWN_ABANDONED=
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
  coproc S09_GUARDED_COPROC {
    s09_guarded_launcher "${raw_fd}" "$@" 2>&1
  }
  child_pid="${S09_GUARDED_COPROC_PID}"
  S09_GUARDED_CHILD_PID="${child_pid}"
  S09_GUARDED_STATUS_FD="${S09_GUARDED_COPROC[0]}"
  S09_GUARDED_CONTROL_FD="${S09_GUARDED_COPROC[1]}"

  supervisor_message=
  for ((attempt = 0; attempt < S09_GUARDED_STARTUP_POLL_LIMIT; attempt++)); do
    if IFS= read -r -t 0.1 supervisor_message <&"${S09_GUARDED_STATUS_FD}"; then
      break
    else
      read_status=$?
    fi
    if ((read_status == 142)) ||
      { ((read_status > 128)) && [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; }; then
      continue
    fi
    supervisor_message=
    break
  done
  if [[ ! "${supervisor_message}" =~ ^ANCHOR\ ([0-9]+)\ ([0-9]+)$ ]] ||
    [[ "${BASH_REMATCH[1]}" != "${child_pid}" ||
      "${BASH_REMATCH[2]}" != "${child_pid}" ]]; then
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded launcher did not anchor\n' >&2
    return 125
  fi
  S09_GUARDED_GROUP_VERIFIED=1
  if [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
  fi

  supervisor_message=
  for ((attempt = 0; attempt < S09_GUARDED_STARTUP_POLL_LIMIT; attempt++)); do
    if [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
      break
    fi
    if IFS= read -r -t 0.1 supervisor_message <&"${S09_GUARDED_STATUS_FD}"; then
      break
    else
      read_status=$?
    fi
    if ((read_status == 142)) ||
      { ((read_status > 128)) && [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; }; then
      continue
    fi
    supervisor_message=
    break
  done
  if [[ -n "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; then
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
  fi
  if [[ ! "${supervisor_message}" =~ ^READY\ ([0-9]+)\ ([0-9]+)$ ]] ||
    [[ "${BASH_REMATCH[1]}" != "${child_pid}" ||
      "${BASH_REMATCH[2]}" != "${child_pid}" ]]; then
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded launcher did not become ready\n' >&2
    return 125
  fi
  s09_finish_deferred_signal_transition

  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_deferred_signal_traps
  if ! printf 'START\n' >&"${S09_GUARDED_CONTROL_FD}"; then
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded launcher could not start command\n' >&2
    return 125
  fi
  s09_finish_deferred_signal_transition

  if ! IFS= read -r supervisor_message <&"${S09_GUARDED_STATUS_FD}"; then
    S09_GUARDED_DEFERRED_SIGNAL=
    s09_install_deferred_signal_traps
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded supervisor returned no status\n' >&2
    return 125
  fi
  if [[ ! "${supervisor_message}" =~ ^STATUS\ ([0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])$ ]]; then
    S09_GUARDED_DEFERRED_SIGNAL=
    s09_install_deferred_signal_traps
    s09_stop_guarded_group || true
    s09_finish_deferred_signal_transition
    printf 's09-raw-transcript-guard: guarded supervisor returned invalid status\n' >&2
    return 125
  fi
  status="${BASH_REMATCH[1]}"

  # STATUS does not release the leader. Drain every process still in the pinned
  # PGID while that verified PID==PGID identity remains live, then reap once.
  S09_GUARDED_DEFERRED_SIGNAL=
  s09_install_deferred_signal_traps
  if ! s09_stop_guarded_group 1; then
    status=125
    s09_delete_raw_transcript || true
  fi
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
  trap 's09_guarded_exit' EXIT
  s09_install_active_signal_traps
}
