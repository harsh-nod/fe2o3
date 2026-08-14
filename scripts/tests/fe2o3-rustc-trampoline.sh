#!/bin/bash

set -Eeuo pipefail
umask 077

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly REPO_ROOT
readonly SOURCE="${REPO_ROOT}/scripts/fe2o3-rustc-trampoline.c"
readonly BUILD="${REPO_ROOT}/scripts/fe2o3-rustc-trampoline-build.sh"
TEST_ROOT="$(mktemp -d "${HOME}/.fe2o3-rustc-trampoline-test.XXXXXX")"
readonly TEST_ROOT
readonly PRODUCTION_ONE="${TEST_ROOT}/production-one"
readonly PRODUCTION_TWO="${TEST_ROOT}/production-two"
readonly REPLACED_SYMLINK="${TEST_ROOT}/replaced-symlink"
readonly SYMLINK_VICTIM="${TEST_ROOT}/symlink-victim"
readonly VERIFY_SYMLINK="${TEST_ROOT}/verify-symlink"
readonly PUBLIC_OUTPUT_DIRECTORY="${TEST_ROOT}/public-output"
readonly TEST_TRAMPOLINE="${TEST_ROOT}/test-trampoline"
readonly WRAPPER="${TEST_ROOT}/cargo-fe2o3-wrapper"
readonly ALTERNATE_WRAPPER="${TEST_ROOT}/alternate-wrapper"
readonly PRELOAD="${TEST_ROOT}/hostile-preload.so"
readonly PRELOAD_MARKER="${TEST_ROOT}/hostile-preload-ran"
readonly HARNESS="${TEST_ROOT}/broker-harness.py"
readonly WATCHDOG_SECONDS=30
readonly WATCHDOG_KILL_SECONDS=5

cleanup() {
  chmod -R u+w -- "${TEST_ROOT}" 2>/dev/null || true
  rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

run_watchdog() {
  /usr/bin/timeout --signal=TERM --kill-after="${WATCHDOG_KILL_SECONDS}s" \
    "${WATCHDOG_SECONDS}s" "$@"
}

expect_build_failure() {
  local expected="$1"
  shift
  local output="${TEST_ROOT}/expected-build-failure.out"
  if run_watchdog "$@" >"${output}" 2>&1; then
    fail "build command unexpectedly succeeded: $*"
  fi
  /usr/bin/grep -F -- "${expected}" "${output}" >/dev/null || {
    /usr/bin/sed -n '1,120p' "${output}" >&2
    fail "build failure did not contain: ${expected}"
  }
}

assert_test_elf() {
  local executable="$1"
  local header program_headers dynamic_tags
  header="$(/usr/bin/readelf --file-header --wide -- "${executable}")"
  program_headers="$(/usr/bin/readelf --program-headers --wide -- "${executable}")"
  dynamic_tags="$(/usr/bin/readelf --dynamic --wide -- "${executable}")"
  [[ "${header}" == *'Class:                             ELF64'* ]] ||
    fail 'test trampoline is not ELF64'
  [[ "${header}" == *'Type:                              DYN (Position-Independent Executable file)'* ]] ||
    fail 'test trampoline is not PIE'
  [[ "${header}" == *'Machine:                           Advanced Micro Devices X86-64'* ]] ||
    fail 'test trampoline is not x86-64'
  if /usr/bin/grep -E '^[[:space:]]*INTERP[[:space:]]' <<<"${program_headers}" >/dev/null; then
    fail 'test trampoline has PT_INTERP'
  fi
  if /usr/bin/grep -E '\((NEEDED|RPATH|RUNPATH)\)' <<<"${dynamic_tags}" >/dev/null; then
    fail 'test trampoline has a dynamic dependency or search path'
  fi
  /usr/bin/grep -E 'GNU_STACK.* RW ' <<<"${program_headers}" >/dev/null ||
    fail 'test trampoline stack is not non-executable'
  /usr/bin/grep -E 'GNU_RELRO.* R ' <<<"${program_headers}" >/dev/null ||
    fail 'test trampoline lacks RELRO'
  /usr/bin/strings --all -- "${executable}" |
    /usr/bin/grep -Fx 'FE2O3_RUSTC_TRAMPOLINE_TEST_ONLY_BUILD' >/dev/null ||
    fail 'test trampoline lacks its compile-time ELF marker'
  /usr/bin/strings --all -- "${executable}" |
    /usr/bin/grep -Fx \
      'FE2O3_RUSTC_TRAMPOLINE_FOUNDATION_NON_AUTHORITATIVE' >/dev/null ||
    fail 'test trampoline lacks the foundation marker'
}

compile_test_trampoline() {
  run_watchdog /usr/bin/cc \
    -std=c11 -O2 -fPIE -static-pie -march=x86-64 -mtune=generic \
    -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
    -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
    -DFE2O3_RUSTC_TRAMPOLINE_TEST_ONLY=1 \
    -Wl,-z,relro,-z,now,-z,noexecstack,--fatal-warnings,--build-id=none \
    "${SOURCE}" -o "${TEST_TRAMPOLINE}"
  chmod 0555 "${TEST_TRAMPOLINE}"
  assert_test_elf "${TEST_TRAMPOLINE}"
}

cat >"${TEST_ROOT}/wrapper.c" <<'EOF'
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <sys/prctl.h>
#include <sys/resource.h>

extern char **environ;

int main(int argc, char **argv) {
  printf("ARGC=%d\n", argc);
  for (int index = 0; index < argc; ++index) {
    printf("ARG[%d]=%s\n", index, argv[index]);
  }
  for (int index = 0; environ[index] != NULL; ++index) {
    printf("ENV[%d]=%s\n", index, environ[index]);
  }
  for (int descriptor = 3; descriptor < 256; ++descriptor) {
    errno = 0;
    if (fcntl(descriptor, F_GETFD) >= 0 || errno != EBADF) {
      printf("FD[%d]\n", descriptor);
    }
  }
  struct rlimit core;
  struct sigaction pipe_action;
  sigset_t mask;
  if (getrlimit(RLIMIT_CORE, &core) != 0) {
    return 91;
  }
  if (sigaction(SIGPIPE, NULL, &pipe_action) != 0 ||
      sigprocmask(SIG_SETMASK, NULL, &mask) != 0) {
    return 92;
  }
  int blocked = 0;
  for (int signal_number = 1; signal_number < NSIG; ++signal_number) {
    if (sigismember(&mask, signal_number) == 1) {
      ++blocked;
    }
  }
  printf("SIGNAL_STATE=%d:%d\n", pipe_action.sa_handler == SIG_DFL, blocked);
  printf("NO_NEW_PRIVS=%d\n", prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0));
  printf("CORE=%llu:%llu\n", (unsigned long long)core.rlim_cur,
         (unsigned long long)core.rlim_max);
  return fflush(stdout) == 0 ? 0 : 92;
}
EOF

cat >"${TEST_ROOT}/alternate.c" <<'EOF'
#include <stdio.h>
int main(void) {
  puts("ALTERNATE_WRAPPER_EXECUTED");
  return 93;
}
EOF

cat >"${TEST_ROOT}/preload.c" <<EOF
#define _GNU_SOURCE
#include <fcntl.h>
#include <unistd.h>
__attribute__((constructor)) static void hostile_preload(void) {
  int descriptor = open("${PRELOAD_MARKER}", O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (descriptor >= 0) {
    (void)close(descriptor);
  }
}
EOF

run_watchdog /usr/bin/cc -std=c11 -O2 -Wall -Wextra -Werror -Wformat=2 \
  -Wl,-z,relro,-z,now,-z,noexecstack "${TEST_ROOT}/wrapper.c" -o "${WRAPPER}"
run_watchdog /usr/bin/cc -std=c11 -O2 -Wall -Wextra -Werror -Wformat=2 \
  -Wl,-z,relro,-z,now,-z,noexecstack "${TEST_ROOT}/alternate.c" \
  -o "${ALTERNATE_WRAPPER}"
run_watchdog /usr/bin/cc -std=c11 -O2 -fPIC -shared -Wall -Wextra -Werror \
  -Wformat=2 -Wl,-z,relro,-z,now,-z,noexecstack "${TEST_ROOT}/preload.c" \
  -o "${PRELOAD}"
chmod 0555 "${WRAPPER}" "${ALTERNATE_WRAPPER}" "${PRELOAD}"

cat >"${HARNESS}" <<'PY'
#!/usr/bin/env python3
import array
import fcntl
import hashlib
import os
import select
import signal
import socket
import struct
import sys
import time

MAGIC = b"F2AUBR3\0"
DOMAIN = b"FE2O3/PROTECTED-AUTHORITY-BROKER-V3-BINDING\0"
HEADER = 24
BINDING_LEN = 336
HELLO_LEN = 408
BOOTSTRAP_LEN = 120
BINDING_FD = 189
BROKER_FD = 190
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK |
    fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE
)
GOLDEN_BINDING_IDENTITY = bytes.fromhex(
    "2a9cae7959e9efd207de5f859e100688"
    "479f359469847efed8533356d714a591"
)


def digest(label):
    return hashlib.sha256(label.encode("ascii")).digest()


def sha256(data):
    return hashlib.sha256(data).digest()


def make_binding(trampoline_bytes, wrapper_bytes):
    value = bytearray(BINDING_LEN)
    value[0:32] = digest("policy")
    value[32:64] = digest("protected-admission")
    value[64:96] = digest("build-session")
    struct.pack_into("<H", value, 96, 1)
    struct.pack_into("<H", value, 98, 2)
    struct.pack_into("<I", value, 100, 0)
    value[104:136] = digest("cargo-environment")
    value[136:168] = sha256(trampoline_bytes)
    value[168:200] = sha256(wrapper_bytes)
    value[200:232] = digest("compiler-closure")
    value[232:264] = digest("runtime-object")
    value[264:296] = digest("codegen-backend")
    value[296] = 0
    return bytes(value)


def verify_rust_codec_golden_identity():
    value = bytearray(BINDING_LEN)
    value[0:32] = bytes([1]) * 32
    value[32:64] = bytes([2]) * 32
    value[64:96] = bytes([3]) * 32
    struct.pack_into("<H", value, 96, 1)
    struct.pack_into("<H", value, 98, 2)
    struct.pack_into("<I", value, 100, 0)
    for index, seed in enumerate(range(4, 10)):
        offset = 104 + index * 32
        value[offset:offset + 32] = bytes([seed]) * 32
    value[296] = 1
    value[304:336] = bytes([10]) * 32
    observed = sha256(DOMAIN + struct.pack("<Q", BINDING_LEN) + value)
    assert observed == GOLDEN_BINDING_IDENTITY


def sealed_memfd(data, mode, *, sealed=True, read_only=True):
    descriptor = os.memfd_create(
        "fe2o3-test-object", os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING
    )
    offset = 0
    while offset < len(data):
        offset += os.write(descriptor, data[offset:])
    os.fchmod(descriptor, mode)
    if sealed:
        fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS, REQUIRED_SEALS)
    if not read_only:
        return descriptor
    read_descriptor = os.open(
        f"/proc/self/fd/{descriptor}", os.O_RDONLY | os.O_CLOEXEC
    )
    os.close(descriptor)
    return read_descriptor


def frame_header(kind, payload_length, sequence, flags=0):
    return MAGIC + struct.pack("<HHIII", 3, kind, payload_length, sequence, flags)


def child_start_ticks(pid):
    raw = open(f"/proc/{pid}/stat", "rb").read()
    return int(raw.rsplit(b") ", 1)[1].split()[19])


def wait_child(pid, timeout=3.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        observed, status = os.waitpid(pid, os.WNOHANG)
        if observed == pid:
            return os.waitstatus_to_exitcode(status)
        time.sleep(0.01)
    try:
        os.killpg(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    time.sleep(0.05)
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    observed, status = os.waitpid(pid, 0)
    assert observed == pid
    raise AssertionError("trampoline child exceeded broker harness timeout")


def expected_success_output(binding, bootstrap_identity):
    binding_identity = sha256(DOMAIN + struct.pack("<Q", BINDING_LEN) + binding)
    entries = [
        "ARGC=5",
        "ARG[0]=cargo-fe2o3",
        "ARG[1]=/untrusted/rustc",
        "ARG[2]=--crate-name",
        "ARG[3]=demo",
        "ARG[4]=--cfg=feature=verified",
        "ENV[0]=FE2O3_BROKER_V3_FD=190",
        "ENV[1]=FE2O3_BROKER_V3_BINDING_SHA256=" + binding_identity.hex(),
        "ENV[2]=FE2O3_BROKER_V3_BOOTSTRAP_SHA256=" + bootstrap_identity.hex(),
        "ENV[3]=FE2O3_AUTHORITY_BUILD_SESSION_SHA256=" + binding[64:96].hex(),
        "ENV[4]=FE2O3_AUTHORITY_CARGO_ENVIRONMENT_SHA256=" + binding[104:136].hex(),
        "ENV[5]=FE2O3_AUTHORITY_TARGET=gfx942:xnack-",
        "ENV[6]=FE2O3_AUTHORITY_PIPELINE=collected-tiled-gemm-v1",
        "ENV[7]=HOME=/nonexistent",
        "ENV[8]=LANG=C",
        "ENV[9]=LC_ALL=C",
        "ENV[10]=PATH=/usr/bin:/bin",
        "ENV[11]=SOURCE_DATE_EPOCH=0",
        "ENV[12]=TZ=UTC",
        "FD[190]",
        "SIGNAL_STATE=1:0",
        "NO_NEW_PRIVS=1",
        "CORE=0:0",
    ]
    return "\n".join(entries) + "\n"


def run_scenario(scenario, trampoline_path, wrapper_path, alternate_path,
                 preload_path, preload_marker):
    trampoline_bytes = open(trampoline_path, "rb").read()
    wrapper_bytes = open(wrapper_path, "rb").read()
    alternate_bytes = open(alternate_path, "rb").read()
    binding = make_binding(trampoline_bytes, wrapper_bytes)
    binding_identity = sha256(DOMAIN + struct.pack("<Q", BINDING_LEN) + binding)
    bootstrap_identity = digest("bootstrap-transfer")

    binding_bytes = binding
    if scenario == "noncanonical-binding":
        mutated_binding = bytearray(binding)
        struct.pack_into("<I", mutated_binding, 100, 1)
        binding_bytes = bytes(mutated_binding)
    elif scenario == "trampoline-binding-substitution":
        mutated_binding = bytearray(binding)
        mutated_binding[136:168] = digest("substituted-trampoline")
        binding_bytes = bytes(mutated_binding)
    binding_descriptor = sealed_memfd(
        binding_bytes,
        0o444,
        sealed=scenario != "unsealed-binding",
        read_only=scenario != "writable-binding",
    )
    socket_type = socket.SOCK_STREAM if scenario == "wrong-socket-type" else socket.SOCK_SEQPACKET
    broker_socket, child_socket = socket.socketpair(socket.AF_UNIX, socket_type)
    broker_socket.settimeout(1.5)
    output_read, output_write = os.pipe2(os.O_CLOEXEC)
    error_read, error_write = os.pipe2(os.O_CLOEXEC)
    leaked_descriptor = os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC)

    arguments = [
        "attacker-controlled-trampoline-path",
        "/untrusted/rustc",
        "--crate-name",
        "demo",
        "--cfg=feature=verified",
    ]
    if scenario == "response-file":
        arguments = ["attacker-controlled-trampoline-path", "@response-file"]
    elif scenario == "empty-argument":
        arguments = ["attacker-controlled-trampoline-path", ""]
    hostile_environment = {
        "LD_PRELOAD": preload_path,
        "LD_LIBRARY_PATH": "/attacker/library/path",
        "RUSTC": "/attacker/rustc",
        "RUSTC_WRAPPER": "/attacker/wrapper",
        "FE2O3_BROKER_V3_BINDING_SHA256": "f" * 64,
        "UNRELATED_HOSTILE_VALUE": "must-not-survive",
    }

    original_path_bytes = None
    if scenario == "pathname-substitution":
        original_path_bytes = wrapper_bytes
        os.chmod(wrapper_path, 0o755)
        with open(wrapper_path, "wb") as stream:
            stream.write(alternate_bytes)
        os.chmod(wrapper_path, 0o555)

    pid = os.fork()
    if pid == 0:
        try:
            os.setpgid(0, 0)
            signal.signal(signal.SIGPIPE, signal.SIG_IGN)
            signal.pthread_sigmask(
                signal.SIG_BLOCK, {signal.SIGUSR1, signal.SIGTERM}
            )
            os.dup2(output_write, 1, inheritable=True)
            os.dup2(error_write, 2, inheritable=True)
            os.dup2(binding_descriptor, BINDING_FD, inheritable=True)
            os.dup2(child_socket.fileno(), BROKER_FD, inheritable=True)
            os.dup2(leaked_descriptor, 77, inheritable=True)
            os.execve(trampoline_path, arguments, hostile_environment)
        except BaseException as error:
            os.write(2, ("harness child exec failure: " + repr(error) + "\n").encode())
            os._exit(127)

    child_socket.close()
    os.close(binding_descriptor)
    os.close(output_write)
    os.close(error_write)
    os.close(leaked_descriptor)

    expected_failure = scenario not in {"success", "pathname-substitution"}
    try:
        if scenario not in {
            "response-file",
            "empty-argument",
            "wrong-socket-type",
            "unsealed-binding",
            "writable-binding",
            "noncanonical-binding",
            "trampoline-binding-substitution",
        }:
            hello = broker_socket.recv(HELLO_LEN + 1)
            assert len(hello) == HELLO_LEN, (scenario, len(hello))
            expected_header = frame_header(1, 384, 0)
            assert hello[:HEADER] == expected_header
            process_identity = hello[HEADER:HEADER + 16]
            child_pid, reserved, start_ticks = struct.unpack("<IIQ", process_identity)
            assert child_pid == pid
            assert reserved == 0
            assert start_ticks == child_start_ticks(pid)
            assert hello[HEADER + 16:HEADER + 352] == binding
            assert hello[HEADER + 352:HEADER + 384] == sha256(trampoline_bytes)

            manifest = struct.pack("<HHHHHHHH", 1, 1, 1, 0, 0, 0, 0, 0)
            payload = process_identity + binding_identity + bootstrap_identity + manifest
            bootstrap = frame_header(2, 96, 1) + payload

            wrapper_descriptor = sealed_memfd(wrapper_bytes, 0o555)
            descriptors = [wrapper_descriptor]
            if scenario == "malformed-magic":
                bootstrap = bytes([bootstrap[0] ^ 1]) + bootstrap[1:]
            elif scenario == "malformed-sequence":
                bootstrap = bootstrap[:16] + struct.pack("<I", 2) + bootstrap[20:]
            elif scenario == "malformed-length":
                bootstrap = bootstrap[:12] + struct.pack("<I", 95) + bootstrap[16:]
            elif scenario == "malformed-flags":
                bootstrap = bootstrap[:20] + struct.pack("<I", 1) + bootstrap[24:]
            elif scenario == "truncated-frame":
                bootstrap = bootstrap[:-1]
            elif scenario == "trailing-frame":
                bootstrap += b"\0"
            elif scenario == "binding-substitution":
                bootstrap = bootstrap[:HEADER + 16] + digest("wrong-binding") + bootstrap[HEADER + 48:]
            elif scenario == "zero-bootstrap-identity":
                bootstrap = bootstrap[:HEADER + 48] + bytes(32) + bootstrap[HEADER + 80:]
            elif scenario == "manifest-substitution":
                bad_manifest = struct.pack("<HHHHHHHH", 1, 1, 2, 0, 0, 0, 0, 0)
                bootstrap = bootstrap[:HEADER + 80] + bad_manifest
            elif scenario == "peer-process-mismatch":
                wrong_process = struct.pack("<IIQ", pid + 1, 0, start_ticks)
                bootstrap = bootstrap[:HEADER] + wrong_process + bootstrap[HEADER + 16:]
            elif scenario == "extra-descriptor":
                descriptors.append(sealed_memfd(alternate_bytes, 0o555))
            elif scenario == "missing-descriptor":
                descriptors = []
            elif scenario == "writable-descriptor":
                os.close(wrapper_descriptor)
                descriptors = [sealed_memfd(wrapper_bytes, 0o555, read_only=False)]
            elif scenario == "unsealed-descriptor":
                os.close(wrapper_descriptor)
                descriptors = [sealed_memfd(wrapper_bytes, 0o555, sealed=False)]
            elif scenario == "substituted-wrapper":
                os.close(wrapper_descriptor)
                descriptors = [sealed_memfd(alternate_bytes, 0o555)]
            elif scenario == "nonregular-descriptor":
                os.close(wrapper_descriptor)
                read_end, write_end = os.pipe2(os.O_CLOEXEC)
                os.close(write_end)
                descriptors = [read_end]

            if scenario == "timeout":
                time.sleep(0.7)
            elif scenario == "peer-death":
                rights = [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                           array.array("i", descriptors))]
                broker_socket.sendmsg([bootstrap], rights)
                broker_socket.close()
            else:
                rights = []
                if descriptors:
                    rights = [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                               array.array("i", descriptors))]
                broker_socket.sendmsg([bootstrap], rights)
                if scenario == "replayed-frame":
                    try:
                        broker_socket.sendmsg([bootstrap], rights)
                    except (BrokenPipeError, OSError):
                        pass
            for descriptor in descriptors:
                os.close(descriptor)

        exit_code = wait_child(pid)
        output = os.read(output_read, 1 << 20).decode("utf-8", "replace")
        errors = os.read(error_read, 1 << 20).decode("utf-8", "replace")
        if expected_failure:
            assert exit_code == 125, (scenario, exit_code, output, errors)
            assert "fe2o3-rustc-trampoline:" in errors, (scenario, errors)
            assert "ALTERNATE_WRAPPER_EXECUTED" not in output
        else:
            assert exit_code == 0, (scenario, exit_code, output, errors)
            assert errors == "", (scenario, errors)
            assert output == expected_success_output(binding, bootstrap_identity), (
                scenario, output, expected_success_output(binding, bootstrap_identity)
            )
        assert not os.path.exists(preload_marker), (scenario, "LD_PRELOAD survived")
    finally:
        if original_path_bytes is not None:
            os.chmod(wrapper_path, 0o755)
            with open(wrapper_path, "wb") as stream:
                stream.write(original_path_bytes)
            os.chmod(wrapper_path, 0o555)
        for descriptor in (output_read, error_read):
            try:
                os.close(descriptor)
            except OSError:
                pass
        try:
            broker_socket.close()
        except OSError:
            pass


if __name__ == "__main__":
    if len(sys.argv) != 7:
        raise SystemExit("usage: broker-harness scenario trampoline wrapper alternate preload marker")
    verify_rust_codec_golden_identity()
    run_scenario(*sys.argv[1:])
PY
chmod 0555 "${HARNESS}"

run_watchdog "${BUILD}" "${PRODUCTION_ONE}"
run_watchdog "${BUILD}" "${PRODUCTION_TWO}"
cmp --silent -- "${PRODUCTION_ONE}" "${PRODUCTION_TWO}" ||
  fail 'production trampoline build is not reproducible across output names'
run_watchdog "${BUILD}" --verify "${PRODUCTION_ONE}"

mkdir -- "${PUBLIC_OUTPUT_DIRECTORY}"
chmod 0755 "${PUBLIC_OUTPUT_DIRECTORY}"
expect_build_failure 'candidate directory must be caller-owned mode 0700' \
  "${BUILD}" "${PUBLIC_OUTPUT_DIRECTORY}/trampoline"

printf '%s\n' 'SYMLINK_VICTIM_MUST_NOT_CHANGE' >"${SYMLINK_VICTIM}"
chmod 0600 "${SYMLINK_VICTIM}"
ln -s -- "${SYMLINK_VICTIM}" "${REPLACED_SYMLINK}"
run_watchdog "${BUILD}" "${REPLACED_SYMLINK}"
[[ ! -L "${REPLACED_SYMLINK}" ]] ||
  fail 'atomic trampoline installation retained an attacker symlink'
[[ "$(<"${SYMLINK_VICTIM}")" == 'SYMLINK_VICTIM_MUST_NOT_CHANGE' ]] ||
  fail 'atomic trampoline installation followed and modified a symlink victim'
cmp --silent -- "${PRODUCTION_ONE}" "${REPLACED_SYMLINK}" ||
  fail 'atomic trampoline installation did not install the verified object'

ln -s -- "${PRODUCTION_ONE}" "${VERIFY_SYMLINK}"
expect_build_failure 'rustc trampoline verification rejects symlinks' \
  "${BUILD}" --verify "${VERIFY_SYMLINK}"

if compgen -G "${TEST_ROOT}/.fe2o3-rustc-trampoline.*" >/dev/null; then
  fail 'private trampoline staging directory survived successful installation'
fi
compile_test_trampoline

run_watchdog /usr/bin/cc \
  -std=c11 -O0 -fanalyzer -c -march=x86-64 -mtune=generic \
  -Wall -Wextra -Werror -Wconversion -Wformat=2 -Wshadow \
  -Wstack-protector -fstack-protector-strong -D_FORTIFY_SOURCE=3 \
  "${SOURCE}" -o "${TEST_ROOT}/analyzer.o"

readonly SCENARIOS=(
  success
  pathname-substitution
  response-file
  empty-argument
  wrong-socket-type
  unsealed-binding
  writable-binding
  noncanonical-binding
  trampoline-binding-substitution
  malformed-magic
  malformed-sequence
  malformed-length
  malformed-flags
  truncated-frame
  trailing-frame
  binding-substitution
  zero-bootstrap-identity
  manifest-substitution
  peer-process-mismatch
  extra-descriptor
  missing-descriptor
  writable-descriptor
  unsealed-descriptor
  substituted-wrapper
  nonregular-descriptor
  replayed-frame
  peer-death
  timeout
)

for scenario in "${SCENARIOS[@]}"; do
  rm -f -- "${PRELOAD_MARKER}"
  run_watchdog /usr/bin/python3 "${HARNESS}" "${scenario}" \
    "${TEST_TRAMPOLINE}" "${WRAPPER}" "${ALTERNATE_WRAPPER}" \
    "${PRELOAD}" "${PRELOAD_MARKER}"
done

if command -v shellcheck >/dev/null 2>&1; then
  run_watchdog shellcheck \
    "${REPO_ROOT}/scripts/fe2o3-rustc-trampoline-build.sh" \
    "${REPO_ROOT}/scripts/tests/fe2o3-rustc-trampoline.sh"
fi

printf '%s\n' \
  'fe2o3 rustc trampoline non-authoritative foundation tests passed'
