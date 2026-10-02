#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd -- "${script_dir}/../.." && pwd)"
root="$(mktemp -d)"
declare -a receipts=()
cleanup() {
  local receipt
  for receipt in "${receipts[@]}"; do
    [[ "${receipt##*/}" == fe2o3-source-test-json.* && -d "${receipt}" \
       && ! -L "${receipt}" && "$(stat -c %u -- "${receipt}")" == "$(id -u)" ]] || continue
    rm -rf -- "${receipt}"
  done
  rm -rf -- "${root}"
}
trap cleanup EXIT
mkdir -m 700 -- "${root}/bin" "${root}/tmp"
real_python="$(command -v python3)"

cat >"${root}/bin/cargo" <<'MOCK'
#!/usr/bin/env bash
set -Eeuo pipefail
printf '%s\n' "$@" >"${MOCK_ARGV}"
exec "${REAL_PYTHON}" -I -B - "$@" <<'PY'
import json
import os
from pathlib import Path
import sys

expected = ["test", "--locked", "--offline", "-p", "fixture-package", "--test",
            "fixture-target", "fixture::selected", "--message-format=json", "--",
            "-Z", "unstable-options", "--format=json", "--ignored", "--exact", "--test-threads=1"]
library = os.environ.get("MOCK_LIBRARY") == "1"
if library:
    expected[5:7] = ["--lib"]
assert sys.argv[1:] == expected, sys.argv
observed_tmp = Path(os.environ["TMPDIR"])
if Path(os.environ["MOCK_TMP"]).resolve().is_relative_to(Path(os.environ["MOCK_REPO"])):
    assert observed_tmp.name.startswith("fe2o3-source-test-json."), observed_tmp
    assert not observed_tmp.is_relative_to(Path(os.environ["MOCK_REPO"]))
else:
    assert os.environ["TMPDIR"] == os.environ["MOCK_TMP"]
print("original Cargo diagnostic", file=sys.stderr)
mode = os.environ["MOCK_CASE"]
if mode == "capture-failure":
    raise SystemExit(0)
if mode == "overflow":
    sys.stdout.buffer.write(b"x" * (10485760 + 1))
    raise SystemExit(0)
if mode == "malformed":
    print("not JSON")
    raise SystemExit(0)
if mode == "large-artifact":
    Path(os.environ["MOCK_ARTIFACT"]).write_bytes(b"x" * (10485760 + 1))
name = "fixture::other" if mode == "renamed" else "fixture::selected"
rows = [
    {"reason": "compiler-artifact", "target": {"name": "fixture-target", "kind": ["test"]},
     "executable": "/fixture/test"},
    {"reason": "build-finished", "success": True},
    {"type": "suite", "event": "started", "test_count": 1},
    {"type": "test", "event": "started", "name": name},
    {"type": "test", "event": "ok", "name": name},
    {"type": "suite", "event": "ok", "passed": 1, "failed": 0,
     "ignored": 0, "measured": 0, "filtered_out": 17},
]
if library:
    rows[0]["target"].update(kind=["rlib", "dylib"], crate_types=["rlib", "dylib"])
    rows[0]["profile"] = {"test": True}
    if mode == "wrong-kind":
        rows[0]["target"]["kind"] = ["dylib", "rlib"]
    elif mode == "wrong-crate-types":
        rows[0]["target"]["crate_types"] = ["rlib"]
    elif mode == "non-test-profile":
        rows[0]["profile"]["test"] = False
    elif mode == "extra-harness":
        rows.insert(1, {"reason": "compiler-artifact", "target": {
            "name": "other", "kind": ["lib"], "crate_types": ["lib"]},
            "profile": {"test": True}, "executable": "/fixture/other"})
if mode == "zero":
    rows[2]["test_count"] = rows[-1]["passed"] = 0
    del rows[3:5]
elif mode == "duplicate":
    rows.insert(4, rows[3].copy())
elif mode in ("failed", "ignored"):
    rows[4]["event"] = mode
    rows[-1][mode] = 1
elif mode == "cargo-test-failure":
    rows[4].update(event="failed", stdout="selected-test assertion: required source rejected")
    rows[-1].update(event="failed", passed=0, failed=1)
elif mode == "counter-bool":
    rows[-1]["filtered_out"] = True
elif mode in ("advisory", "advisory-cargo-failure"):
    rows.insert(4, {"type": "test", "event": "timeout", "name": name})
for row in rows:
    print(json.dumps(row))
raise SystemExit(37 if mode in ("cargo-failure", "advisory-cargo-failure", "cargo-test-failure") else 0)
PY
MOCK
cat >"${root}/bin/python3" <<'MOCK'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${MOCK_CASE}" == capture-failure && "${3:-}" == -c ]]; then
  exit 73
fi
exec "${REAL_PYTHON}" "$@"
MOCK
chmod 700 -- "${root}/bin/cargo" "${root}/bin/python3"

run_case() {
  local mode="$1" expected="$2" temporary="${3:-${root}/tmp}" status=0 receipt directory
  local library="${4:-0}"
  local -a selector=()
  [[ "${library}" != 1 ]] || selector=(--lib rlib,dylib)
  timeout --signal=TERM --kill-after=2s 20s \
    env PATH="${root}/bin:${PATH}" REAL_PYTHON="${real_python}" \
      MOCK_CASE="${mode}" MOCK_ARGV="${root}/${mode}.argv" \
      MOCK_ARTIFACT="${root}/compiler-artifact" TMPDIR="${temporary}" \
      MOCK_LIBRARY="${library}" MOCK_TMP="${temporary}" MOCK_REPO="${repo}" \
      bash "${repo}/scripts/ci-cargo-test-json.sh" \
        "${selector[@]}" fixture-package fixture-target fixture::selected \
      >"${root}/${mode}.stdout" 2>"${root}/${mode}.stderr" || status=$?
  [[ "${status}" == "${expected}" ]] || {
    cat "${root}/${mode}.stderr" >&2
    printf '%s: expected status %s, got %s\n' "${mode}" "${expected}" "${status}" >&2
    return 1
  }
  receipt="$(sed -n 's/^Cargo\/libtest JSON evidence retained at //p' "${root}/${mode}.stdout")"
  [[ -n "${receipt}" && "${receipt}" != *$'\n'* && "${receipt##*/}" == events.jsonl ]]
  directory="$(dirname -- "${receipt}")"
  receipts+=("${directory}")
  [[ -d "${directory}" && "$(realpath -- "${directory}")" == "${directory}" \
     && "$(stat -c %a -- "${directory}")" == 700 \
     && "${directory}" != "${repo}" && "${directory}" != "${repo}/"* ]]
  if [[ "${mode}" != capture-failure ]]; then
    [[ -f "${receipt}" && ! -L "${receipt}" && "$(stat -c %s -- "${receipt}")" -le 10485760 ]]
  fi
  grep -Fx 'original Cargo diagnostic' "${root}/${mode}.stderr" >/dev/null
  if [[ "${mode}" == cargo-test-failure ]]; then
    grep -F 'selected-test assertion: required source rejected' "${root}/${mode}.stdout" >/dev/null
    grep -F 'selected-test assertion: required source rejected' "${receipt}" >/dev/null
  fi
  if [[ "${expected}" == 0 ]]; then
    grep -F 'verified Cargo/libtest JSON:' "${root}/${mode}.stdout" >/dev/null
  else
    ! grep -F 'verified Cargo/libtest JSON:' "${root}/${mode}.stdout" >/dev/null
  fi
}

run_case filtered 0
run_case zero 1
run_case renamed 1
run_case duplicate 1
run_case failed 1
run_case ignored 1
run_case counter-bool 1
run_case malformed 1
run_case cargo-failure 37
run_case cargo-test-failure 37
run_case advisory 0
run_case advisory-cargo-failure 37
run_case capture-failure 73
run_case overflow 75
run_case large-artifact 0
[[ "$(stat -c %s -- "${root}/compiler-artifact")" == 10485761 ]]
run_case local-fallback 0 "${repo}"
run_case library 0 "${root}/tmp" 1
run_case library-fallback 0 "${repo}" 1
for mode in zero renamed duplicate failed ignored wrong-kind wrong-crate-types non-test-profile extra-harness; do
  run_case "${mode}" 1 "${root}/tmp" 1
done
run_case cargo-test-failure 37 "${root}/tmp" 1
run_case advisory 0 "${root}/tmp" 1

status=0
env PATH="${root}/bin:${PATH}" bash "${repo}/scripts/ci-cargo-test-json.sh" \
  >"${root}/missing.stdout" 2>"${root}/missing.stderr" || status=$?
[[ "${status}" == 2 && ! -s "${root}/missing.stdout" ]]
for kinds in '' bin rlib,rlib rlib, ',rlib' 'rlib dylib'; do
  status=0
  env PATH="${root}/bin:${PATH}" bash "${repo}/scripts/ci-cargo-test-json.sh" \
    --lib "${kinds}" fixture-package fixture-target fixture::selected \
    >"${root}/invalid-lib.stdout" 2>"${root}/invalid-lib.stderr" || status=$?
  [[ "${status}" == 2 && ! -s "${root}/invalid-lib.stdout" ]]
done
printf '%s\n' 'exact Cargo JSON execution helper controls passed'
