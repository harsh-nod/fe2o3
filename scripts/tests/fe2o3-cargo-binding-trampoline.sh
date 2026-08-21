#!/bin/bash

set -Eeuo pipefail
umask 077
IFS=$' \t\n'
unset CDPATH GLOBIGNORE

SCRIPT_DIR="$(cd -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd -P)"
readonly REPO_ROOT
TEST_ROOT="$(mktemp -d "${HOME}/.fe2o3-cargo-binding-trampoline.XXXXXXXXXX")"
readonly TEST_ROOT
cleanup() {
  /usr/bin/rm -rf -- "${TEST_ROOT}"
}
trap cleanup EXIT

"${REPO_ROOT}/scripts/fe2o3-rustc-trampoline-build.sh" \
  --cargo-binding "${TEST_ROOT}/trampoline"
"${REPO_ROOT}/scripts/fe2o3-rustc-trampoline-build.sh" \
  --cargo-binding "${TEST_ROOT}/trampoline-rebuilt"
/usr/bin/cmp --silent -- "${TEST_ROOT}/trampoline" "${TEST_ROOT}/trampoline-rebuilt" || {
  printf '%s\n' 'cargo binding trampoline build is not reproducible' >&2
  exit 1
}
"${REPO_ROOT}/scripts/fe2o3-rustc-trampoline-build.sh" \
  --verify-cargo-binding "${TEST_ROOT}/trampoline"

/usr/bin/python3 - "${TEST_ROOT}/trampoline" <<'PY'
import fcntl
import os
import subprocess
import sys

TRAMPOLINE = sys.argv[1]
WRAPPER_FD = 191
TRAMPOLINE_FD = 192
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL
    | fcntl.F_SEAL_SHRINK
    | fcntl.F_SEAL_GROW
    | fcntl.F_SEAL_WRITE
)


def sealed_image(path, *, mode=0o500, sealed=True):
    writable = os.memfd_create(
        "fe2o3-cargo-binding-test-image", os.MFD_ALLOW_SEALING
    )
    with open(path, "rb") as source:
        while chunk := source.read(65536):
            os.write(writable, chunk)
    os.fchmod(writable, mode)
    if sealed:
        fcntl.fcntl(writable, fcntl.F_ADD_SEALS, REQUIRED_SEALS)
    descriptor = os.open(f"/proc/self/fd/{writable}", os.O_RDONLY)
    os.close(writable)
    return descriptor


def run(loader, *, extra=None, mode=0o500, sealed=True, install=True):
    descriptor = (
        sealed_image("/usr/bin/env", mode=mode, sealed=sealed) if install else None
    )
    trampoline = sealed_image(TRAMPOLINE)
    environment = {
        "FE2O3_CARGO_BINDING_TEST_MARKER": "preserved",
        "LANG": "C",
        "LD_LIBRARY_PATH": loader,
        "PATH": "/usr/bin:/bin",
    }
    if extra:
        environment.update(extra)

    try:
        if descriptor is not None:
            os.dup2(descriptor, WRAPPER_FD, inheritable=True)
        os.dup2(trampoline, TRAMPOLINE_FD, inheritable=True)
        return subprocess.run(
            [f"/proc/self/fd/{TRAMPOLINE_FD}", "--"],
            env=environment,
            pass_fds=(
                (TRAMPOLINE_FD,)
                if descriptor is None
                else (WRAPPER_FD, TRAMPOLINE_FD)
            ),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=5,
            check=False,
        )
    finally:
        if descriptor is not None:
            os.close(WRAPPER_FD)
            os.close(descriptor)
        os.close(TRAMPOLINE_FD)
        os.close(trampoline)


valid = run("/mutable/cargo/target/debug/deps:/proc/self/fd/193")
assert valid.returncode == 0, valid
assert "FE2O3_CARGO_BINDING_TEST_MARKER=preserved\n" in valid.stdout
assert "LD_LIBRARY_PATH=" not in valid.stdout
assert "LD_PRELOAD=" not in valid.stdout

for result in [
    run("/proc/self/fd/193:/mutable/cargo/target/debug/deps"),
    run("/mutable/cargo/target/debug/deps::/proc/self/fd/193"),
    run("/mutable/cargo/target/debug/deps:relative:/proc/self/fd/193"),
    run("/mutable/cargo/target/debug/deps:/proc/self/fd/193", install=False),
    run("/mutable/cargo/target/debug/deps:/proc/self/fd/193", sealed=False),
    run("/mutable/cargo/target/debug/deps:/proc/self/fd/193", mode=0o555),
    run(
        "/mutable/cargo/target/debug/deps:/proc/self/fd/193",
        extra={"LD_PRELOAD": "/attacker/preload.so"},
    ),
    run(
        "/mutable/cargo/target/debug/deps:/proc/self/fd/193",
        extra={"GLIBC_TUNABLES": "glibc.malloc.check=1"},
    ),
]:
    assert result.returncode == 125, result
    assert "fe2o3-cargo-binding-trampoline:" in result.stderr, result

print("fe2o3 cargo binding trampoline tests passed")
PY
