#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd -P)
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-tutorial-hardware-runner.XXXXXXXX")
trap 'rm -rf -- "$TEST_ROOT"' EXIT

artifact="$TEST_ROOT/compiler.hsaco"
llvm="$TEST_ROOT/compiler.ll"
driver="$TEST_ROOT/driver.json"
runtime="$TEST_ROOT/runtime.json"
request="$TEST_ROOT/request.json"
record="$TEST_ROOT/record.json"
objdump="$TEST_ROOT/llvm-objdump"
readobj="$TEST_ROOT/llvm-readobj"
printf 'exact compiler artifact\n' >"$artifact"
printf 'exact compiler llvm\n' >"$llvm"
printf '{"driver":"actual-test-driver"}\n' >"$driver"
printf '{"device":"actual-test-device","runtime":"actual-test-runtime"}\n' >"$runtime"

cat >"$objdump" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '0000000000000000 <qualified_kernel>:' 's_endpgm'
EOF
cat >"$readobj" <<'EOF'
#!/usr/bin/env bash
cat <<'METADATA'
  amdhsa.target: 'amdgcn-amd-amdhsa--gfx942:xnack-'
    .name: qualified_kernel
    .group_segment_fixed_size: 2048
    .private_segment_fixed_size: 4
    .sgpr_count: 16
    .vgpr_count: 8
    .reqd_workgroup_size:
      - 256
      - 1
      - 1
METADATA
EOF
chmod 700 "$objdump" "$readobj"

export TEST_ARTIFACT="$artifact" TEST_DRIVER="$driver" TEST_LLVM="$llvm"
export TEST_RECORD="$record" TEST_REQUEST="$request" TEST_RUNTIME="$runtime"
python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path, value):
    Path(path).write_text(
        json.dumps(value, allow_nan=False, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n",
        encoding="ascii",
    )


command = {
    "arguments": [],
    "environment": ["FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1"],
    "executable": "examples/test/run-gfx942.sh",
    "timeoutSeconds": 10,
    "workingDirectory": ".",
}
write(
    os.environ["TEST_REQUEST"],
    {
        "candidate": {"compilerCommit": "1" * 40, "compilerTree": "2" * 40, "worktreeClean": True},
        "fixture": {
            "compilerInput": {"kernelSymbols": ["qualified_kernel"]},
            "fixtureId": "qualified-fixture",
            "hardwareCommand": command,
            "hardwareLane": "test-mi300x",
            "hardwareReservation": "test-reservation",
            "target": "gfx942",
        },
    },
)
write(
    os.environ["TEST_RECORD"],
    {
        "hardware": {
            "driverIdentitySha256": digest(os.environ["TEST_DRIVER"]),
            "runtimeIdentitySha256": digest(os.environ["TEST_RUNTIME"]),
        },
        "productionTransaction": {"transactionSha256": "a" * 64},
    },
)
PY

# shellcheck source=scripts/tutorial-hardware-runner.sh
source "$REPO_ROOT/scripts/tutorial-hardware-runner.sh"

configure_scratch() {
    scratch="$TEST_ROOT/scratch-$1"
    mkdir "$scratch"
    export FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1
    export FE2O3_TUTORIAL_HARDWARE_INNER=1
    export FE2O3_TUTORIAL_HARDWARE_SCRATCH="$scratch"
    export FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT="$request"
    export FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT="$record"
    export FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT="$artifact"
    export FE2O3_TUTORIAL_HARDWARE_LLVM_INPUT="$llvm"
    export FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT="$driver"
    export FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT="$runtime"
    export FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT="$scratch/isa-observation-v1.json"
    export FE2O3_TUTORIAL_HARDWARE_RESOURCE_OBSERVATION_OUTPUT="$scratch/resource-observation-v1.json"
    export FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT="$scratch/result-observation-v1.json"
    export OBJDUMP="$objdump"
    export READOBJ="$readobj"
    unset FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT
    fe2o3_tutorial_hardware_begin
}

write_runtime_observation() {
    local mode=${1:-complete}
    TEST_MODE="$mode" python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


same = hashlib.sha256(b"unchanged actual bytes").hexdigest()
output = hashlib.sha256(b"complete expected and observed output").hexdigest()
observed = output if os.environ["TEST_MODE"] == "complete" else hashlib.sha256(b"truncated").hexdigest()
value = {
    "artifactSha256": digest(os.environ["FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT"]),
    "checks": {
        "canariesChecked": True,
        "completeOutputChecked": True,
        "inputsUnchangedChecked": True,
        "paddingChecked": True,
        "timedOut": False,
    },
    "driverIdentitySha256": digest(os.environ["FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT"]),
    "kernelSymbols": ["qualified_kernel"],
    "observedIdentities": {
        "canaryAfterSha256": same,
        "canaryBeforeSha256": same,
        "expectedOutputSha256": output,
        "inputAfterSha256": same,
        "inputBeforeSha256": same,
        "observedOutputSha256": observed,
        "paddingAfterSha256": same,
        "paddingBeforeSha256": same,
    },
    "runtimeIdentitySha256": digest(os.environ["FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT"]),
    "schema": "fe2o3-tutorial-runtime-semantic-observation-v1",
    "target": "gfx942",
}
Path(os.environ["FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT"]).write_text(
    json.dumps(value, allow_nan=False, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
    + "\n",
    encoding="ascii",
)
PY
}

expect_failure() {
    local pattern=$1
    shift
    local errors="$TEST_ROOT/expected-error"
    if "$@" 2>"$errors"; then
        printf 'expected command to fail: %s\n' "$*" >&2
        exit 1
    fi
    if ! grep -Fq -- "$pattern" "$errors"; then
        printf 'missing expected failure %q in:\n' "$pattern" >&2
        cat "$errors" >&2
        exit 1
    fi
}

configure_scratch success
write_runtime_observation
fe2o3_tutorial_hardware_finish gfx942 "$artifact" "$llvm"
TEST_SCRATCH="$scratch" python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path


root = Path(os.environ["TEST_SCRATCH"])
isa = json.loads((root / "isa-observation-v1.json").read_bytes())
resource = json.loads((root / "resource-observation-v1.json").read_bytes())
result = json.loads((root / "result-observation-v1.json").read_bytes())
assert isa["kernelSymbols"] == ["qualified_kernel"]
assert "<qualified_kernel>:" in isa["disassembly"]
assert resource["workgroupSize"] == [256, 1, 1]
assert resource["ldsBytes"] == 2048
assert resource["scratchBytes"] == 1024
assert resource["registersPerWorkgroup"] == 2112
assert result["checks"]["completeOutputChecked"] is True
assert result["observedIdentities"]["expectedOutputSha256"] == result["observedIdentities"]["observedOutputSha256"]
for path in root.glob("*-observation-v1.json"):
    assert path.stat().st_mode & 0o777 == 0o600
    raw = path.read_bytes()
    assert raw == (
        json.dumps(json.loads(raw), allow_nan=False, ensure_ascii=True, separators=(",", ":"), sort_keys=True).encode("ascii")
        + b"\n"
    )
PY

configure_scratch missing-runtime
expect_failure 'cannot inspect runtime semantic observation' \
    fe2o3_tutorial_hardware_finish gfx942 "$artifact" "$llvm"
[[ ! -e $FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT ]]

configure_scratch substituted-artifact
write_runtime_observation
printf 'different artifact\n' >"$TEST_ROOT/substituted.hsaco"
expect_failure 'runner-built artifact differs from the compiler-owned transaction input' \
    fe2o3_tutorial_hardware_finish gfx942 "$TEST_ROOT/substituted.hsaco" "$llvm"

configure_scratch partial-output
write_runtime_observation partial
expect_failure 'expectedOutputSha256/observedOutputSha256 differ' \
    fe2o3_tutorial_hardware_finish gfx942 "$artifact" "$llvm"

configure_scratch stale-output
write_runtime_observation
printf 'stale\n' >"$FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT"
expect_failure 'typed observation output must be a fresh direct child' \
    fe2o3_tutorial_hardware_finish gfx942 "$artifact" "$llvm"
[[ ! -e $FE2O3_TUTORIAL_HARDWARE_RESOURCE_OBSERVATION_OUTPUT ]]
[[ ! -e $FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT ]]

printf 'tutorial hardware runner tests: PASS\n'
