#!/usr/bin/env bash

# Shared entry hook for manifest hardware runners. Ordinary developer runs are
# unchanged; qualification runs are re-entered once under the strict adapter.
fe2o3_tutorial_hardware_entry() {
    local runner=$1
    shift

    case ${FE2O3_TUTORIAL_HARDWARE_PROTOCOL:-} in
        '') return 0 ;;
        authenticated-v1) ;;
        *)
            printf 'unsupported FE2O3 tutorial hardware protocol\n' >&2
            return 2
            ;;
    esac
    if [[ ${FE2O3_TUTORIAL_HARDWARE_INNER:-0} == 1 ]]; then
        return 0
    fi

    local helper_root
    helper_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
    exec "$helper_root/scripts/run-tutorial-authenticated-hardware.py" \
        --runner "$runner" -- "$@"
}

fe2o3_tutorial_hardware_active() {
    [[ ${FE2O3_TUTORIAL_HARDWARE_PROTOCOL:-} == authenticated-v1 && \
        ${FE2O3_TUTORIAL_HARDWARE_INNER:-0} == 1 ]]
}

# The device-side runner owns the semantic bytes. It must atomically publish a
# canonical fe2o3-tutorial-runtime-semantic-observation-v1 document here; shell
# exit status or a PASS line is deliberately insufficient qualification input.
fe2o3_tutorial_hardware_begin() {
    if ! fe2o3_tutorial_hardware_active; then
        return 0
    fi
    : "${FE2O3_TUTORIAL_HARDWARE_SCRATCH:?missing hardware scratch}"
    local raw="$FE2O3_TUTORIAL_HARDWARE_SCRATCH/runtime-semantic-observation-v1.json"
    if [[ -e $raw || -L $raw ]]; then
        printf 'runtime semantic observation path was not attempt-fresh\n' >&2
        return 1
    fi
    FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT=$raw
    export FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT
}

# Publish the three typed observations required by the authenticated adapter.
# The optional artifact and LLVM arguments identify outputs rebuilt by a legacy
# example runner and must match the compiler-owned transaction inputs exactly.
fe2o3_tutorial_hardware_finish() {
    local target=$1
    local produced_artifact=${2:--}
    local produced_llvm=${3:--}
    if ! fe2o3_tutorial_hardware_active; then
        return 0
    fi

    : "${FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT:?hardware begin was not called}"
    : "${FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT:?missing hardware request}"
    : "${FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT:?missing hardware record}"
    : "${FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT:?missing compiler artifact}"
    : "${FE2O3_TUTORIAL_HARDWARE_LLVM_INPUT:?missing compiler LLVM module}"
    : "${FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT:?missing driver identity}"
    : "${FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT:?missing runtime identity}"
    : "${FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT:?missing ISA output}"
    : "${FE2O3_TUTORIAL_HARDWARE_RESOURCE_OBSERVATION_OUTPUT:?missing resource output}"
    : "${FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT:?missing result output}"

    local rocm=${ROCM_PATH:-/opt/rocm}
    local objdump=${OBJDUMP:-$rocm/llvm/bin/llvm-objdump}
    local readobj=${READOBJ:-$rocm/llvm/bin/llvm-readobj}
    local scratch=$FE2O3_TUTORIAL_HARDWARE_SCRATCH
    local disassembly="$scratch/compiler-artifact.isa"
    local metadata="$scratch/compiler-artifact.notes"
    local path
    for path in "$objdump" "$readobj"; do
        if [[ $path != /* || ! -f $path || ! -x $path || -L $path ]]; then
            printf 'hardware inspection tool is not an absolute regular executable: %s\n' \
                "$path" >&2
            return 1
        fi
    done
    objdump=$(cd -- "$(dirname -- "$objdump")" && pwd -P)/$(basename -- "$objdump")
    readobj=$(cd -- "$(dirname -- "$readobj")" && pwd -P)/$(basename -- "$readobj")
    for path in "$disassembly" "$metadata"; do
        if [[ -e $path || -L $path ]]; then
            printf 'hardware inspection output path was not attempt-fresh: %s\n' "$path" >&2
            return 1
        fi
    done

    (
        ulimit -f 65536
        "$objdump" --disassemble --mcpu="$target" \
            "$FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT" >"$disassembly"
    )
    (
        ulimit -f 4096
        "$readobj" --file-headers --notes \
            "$FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT" >"$metadata"
    )

    python3 - "$target" "$produced_artifact" "$produced_llvm" \
        "$objdump" "$readobj" "$disassembly" "$metadata" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys


SHA256 = re.compile(r"[0-9a-f]{64}\Z")
RUNTIME_SCHEMA = "fe2o3-tutorial-runtime-semantic-observation-v1"
ISA_SCHEMA = "fe2o3-tutorial-hardware-isa-observation-v1"
RESOURCE_SCHEMA = "fe2o3-tutorial-hardware-resource-observation-v1"
RESULT_SCHEMA = "fe2o3-tutorial-hardware-result-observation-v1"
COMMAND_DOMAIN = b"fe2o3-tutorial-semantic-command-v1\0"
MAX_JSON = 4 * 1024 * 1024
MAX_ISA = 64 * 1024 * 1024


def fail(message):
    raise SystemExit(f"tutorial hardware observation: {message}")


def canonical(value):
    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            ).encode("ascii")
            + b"\n"
        )
    except (TypeError, ValueError, UnicodeError) as error:
        fail(f"cannot encode canonical JSON: {error}")


def pairs(values):
    result = {}
    for key, value in values:
        if key in result:
            fail(f"JSON repeats key {key!r}")
        result[key] = value
    return result


def read_regular(path, label, limit):
    path = Path(path)
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"cannot inspect {label}: {error}")
    if (
        not path.is_absolute()
        or path != Path(os.path.normpath(str(path)))
        or path.is_symlink()
        or resolved != path
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_size > limit
    ):
        fail(f"{label} must be a bounded, normalized, non-linked regular file")
    payload = path.read_bytes()
    if len(payload) != metadata.st_size:
        fail(f"{label} changed while it was read")
    return payload


def document(path, label, limit=MAX_JSON):
    payload = read_regular(path, label, limit)
    try:
        value = json.loads(payload, object_pairs_hook=pairs)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        fail(f"cannot decode {label}: {error}")
    if not isinstance(value, dict) or payload != canonical(value):
        fail(f"{label} must be a canonical JSON object followed by one newline")
    return value


def exact(value, fields, label):
    if not isinstance(value, dict) or set(value) != set(fields):
        fail(f"{label} fields differ from the contract")


def digest(payload):
    return hashlib.sha256(payload).hexdigest()


def digest_field(value, label):
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        fail(f"{label} must be a lowercase SHA-256 identity")
    return value


def same_file_identity(path, expected, label):
    if path == "-":
        return
    actual = digest(read_regular(path, label, 512 * 1024 * 1024))
    if actual != expected:
        fail(f"{label} differs from the compiler-owned transaction input")


def metadata_records(payload, symbols, target):
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        fail(f"HSACO metadata is not UTF-8: {error}")
    records = {}
    current = None
    index = 0
    while index < len(lines):
        line = lines[index]
        match = re.match(r"^\s*[.]name:\s+['\"]?([^'\"\s]+)", line)
        if match:
            current = match.group(1)
            if current in records:
                fail(f"HSACO metadata repeats kernel {current}")
            records[current] = {}
        elif current is not None:
            scalar = re.match(
                r"^\s*[.](group_segment_fixed_size|private_segment_fixed_size|sgpr_count|vgpr_count):\s+([0-9]+)\s*$",
                line,
            )
            if scalar:
                field = scalar.group(1)
                if field in records[current]:
                    fail(f"HSACO metadata repeats {field} for {current}")
                records[current][field] = int(scalar.group(2))
            elif re.match(r"^\s*[.]reqd_workgroup_size:\s*$", line):
                if "workgroup" in records[current]:
                    fail(f"HSACO metadata repeats workgroup size for {current}")
                dimensions = []
                for offset in range(1, 4):
                    if index + offset >= len(lines):
                        break
                    dimension = re.match(r"^\s*-\s+([0-9]+)\s*$", lines[index + offset])
                    if dimension is None:
                        break
                    dimensions.append(int(dimension.group(1)))
                if len(dimensions) == 3:
                    records[current]["workgroup"] = dimensions
        index += 1
    target_pattern = re.compile(
        rf"^\s*amdhsa[.]target:\s+['\"]amdgcn-amd-amdhsa--{re.escape(target)}(?::[^'\"]+)?['\"]\s*$",
        re.MULTILINE,
    )
    if target_pattern.search(payload.decode("utf-8")) is None:
        fail(f"HSACO metadata does not bind target {target}")
    selected = []
    required = {
        "group_segment_fixed_size",
        "private_segment_fixed_size",
        "sgpr_count",
        "vgpr_count",
        "workgroup",
    }
    for symbol in symbols:
        record = records.get(symbol)
        if record is None or not required.issubset(record):
            fail(f"HSACO metadata omits complete resources for {symbol}")
        selected.append(record)
    first = selected[0]
    if any(record != first for record in selected[1:]):
        fail("one fixture's kernel symbols have incompatible resource records")
    dimensions = first["workgroup"]
    threads = dimensions[0] * dimensions[1] * dimensions[2]
    if not 0 < threads <= 1024:
        fail("required workgroup size is outside 1..=1024")
    waves = (threads + 63) // 64
    registers = waves * first["sgpr_count"] + threads * first["vgpr_count"]
    scratch = threads * first["private_segment_fixed_size"]
    return dimensions, first["group_segment_fixed_size"], registers, scratch


def exclusive_write(path, payload):
    path = Path(path)
    parent = path.parent.resolve(strict=True)
    scratch = Path(os.environ["FE2O3_TUTORIAL_HARDWARE_SCRATCH"]).resolve(strict=True)
    if path.parent != parent or parent != scratch or path.exists() or path.is_symlink():
        fail("typed observation output must be a fresh direct child of hardware scratch")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    parent_descriptor = os.open(parent, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))
    try:
        descriptor = os.open(path.name, flags, 0o600, dir_fd=parent_descriptor)
        try:
            with os.fdopen(descriptor, "wb") as output:
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
        except BaseException:
            try:
                os.unlink(path.name, dir_fd=parent_descriptor)
            except FileNotFoundError:
                pass
            raise
    finally:
        os.close(parent_descriptor)


target, produced_artifact, produced_llvm, objdump, readobj, isa_path, notes_path = sys.argv[1:]
request = document(os.environ["FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT"], "hardware request")
record = document(os.environ["FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT"], "hardware record")
runtime = document(
    os.environ["FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT"],
    "runtime semantic observation",
)
fixture = request.get("fixture")
transaction = record.get("productionTransaction")
hardware = record.get("hardware")
if not isinstance(fixture, dict) or not isinstance(transaction, dict) or not isinstance(hardware, dict):
    fail("request or record omits the fixture, transaction, or hardware identity")
compiler_input = fixture.get("compilerInput")
if not isinstance(compiler_input, dict):
    fail("request omits compiler input")
symbols = compiler_input.get("kernelSymbols")
if (
    not isinstance(symbols, list)
    or not symbols
    or symbols != sorted(set(symbols))
    or any(not isinstance(symbol, str) or not symbol for symbol in symbols)
):
    fail("request kernel symbols are not sorted, unique, and non-empty")
if fixture.get("target") != target:
    fail("runner target differs from the compiler request")

artifact_payload = read_regular(
    os.environ["FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT"],
    "compiler artifact",
    512 * 1024 * 1024,
)
llvm_payload = read_regular(
    os.environ["FE2O3_TUTORIAL_HARDWARE_LLVM_INPUT"],
    "compiler LLVM module",
    512 * 1024 * 1024,
)
artifact_sha = digest(artifact_payload)
llvm_sha = digest(llvm_payload)
same_file_identity(produced_artifact, artifact_sha, "runner-built artifact")
same_file_identity(produced_llvm, llvm_sha, "runner-built LLVM module")

isa_payload = read_regular(isa_path, "compiler artifact disassembly", MAX_ISA)
notes_payload = read_regular(notes_path, "compiler artifact metadata", MAX_JSON)
try:
    disassembly = isa_payload.decode("utf-8")
except UnicodeDecodeError as error:
    fail(f"compiler artifact disassembly is not UTF-8: {error}")
for symbol in symbols:
    if re.search(
        rf"^[0-9a-fA-F]+\s+<{re.escape(symbol)}>:\s*$", disassembly, re.MULTILINE
    ) is None:
        fail(f"compiler artifact disassembly omits {symbol}")
workgroup, lds_bytes, registers, scratch_bytes = metadata_records(
    notes_payload, symbols, target
)

checks_fields = {
    "canariesChecked",
    "completeOutputChecked",
    "inputsUnchangedChecked",
    "paddingChecked",
    "timedOut",
}
identity_fields = {
    "canaryAfterSha256",
    "canaryBeforeSha256",
    "expectedOutputSha256",
    "inputAfterSha256",
    "inputBeforeSha256",
    "observedOutputSha256",
    "paddingAfterSha256",
    "paddingBeforeSha256",
}
exact(
    runtime,
    {
        "artifactSha256",
        "checks",
        "driverIdentitySha256",
        "kernelSymbols",
        "observedIdentities",
        "runtimeIdentitySha256",
        "schema",
        "target",
    },
    "runtime semantic observation",
)
checks = runtime.get("checks")
identities = runtime.get("observedIdentities")
exact(checks, checks_fields, "runtime semantic checks")
exact(identities, identity_fields, "runtime semantic identities")
for name, value in identities.items():
    digest_field(value, f"runtime semantic identities.{name}")
for name in checks_fields - {"timedOut"}:
    if checks[name] is not True:
        fail(f"runtime semantic check {name} did not pass")
if checks["timedOut"] is not False:
    fail("runtime semantic observation timed out")
for before, after in (
    ("expectedOutputSha256", "observedOutputSha256"),
    ("inputBeforeSha256", "inputAfterSha256"),
    ("canaryBeforeSha256", "canaryAfterSha256"),
    ("paddingBeforeSha256", "paddingAfterSha256"),
):
    if identities[before] != identities[after]:
        fail(f"runtime semantic identities {before}/{after} differ")

driver_sha = digest(
    read_regular(os.environ["FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT"], "driver identity", MAX_JSON)
)
runtime_sha = digest(
    read_regular(os.environ["FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT"], "runtime identity", MAX_JSON)
)
if (
    runtime.get("schema") != RUNTIME_SCHEMA
    or runtime.get("target") != target
    or runtime.get("kernelSymbols") != symbols
    or runtime.get("artifactSha256") != artifact_sha
    or runtime.get("driverIdentitySha256") != driver_sha
    or runtime.get("runtimeIdentitySha256") != runtime_sha
    or hardware.get("driverIdentitySha256") != driver_sha
    or hardware.get("runtimeIdentitySha256") != runtime_sha
):
    fail("runtime semantic observation is stale, substituted, or cross-target")

command = fixture.get("hardwareCommand")
command_sha = digest(COMMAND_DOMAIN + canonical(command)[:-1])
isa = {
    "artifactSha256": artifact_sha,
    "disassembly": disassembly,
    "inspectionToolSha256": digest(read_regular(objdump, "ISA inspection tool", 512 * 1024 * 1024)),
    "kernelSymbols": symbols,
    "llvmModuleSha256": llvm_sha,
    "schema": ISA_SCHEMA,
    "target": target,
}
resource = {
    "artifactSha256": artifact_sha,
    "inspectionToolSha256": digest(read_regular(readobj, "resource inspection tool", 512 * 1024 * 1024)),
    "kernelSymbols": symbols,
    "ldsBytes": lds_bytes,
    "registersPerWorkgroup": registers,
    "schema": RESOURCE_SCHEMA,
    "scratchBytes": scratch_bytes,
    "target": target,
    "workgroupSize": workgroup,
}
result = {
    "artifactSha256": artifact_sha,
    "authority": "observation-only",
    "candidate": request.get("candidate"),
    "checks": checks,
    "commandSha256": command_sha,
    "driverIdentitySha256": driver_sha,
    "fixtureId": fixture.get("fixtureId"),
    "kernelSymbols": symbols,
    "lane": fixture.get("hardwareLane"),
    "observedIdentities": identities,
    "outcome": "passed",
    "reservationIdentity": fixture.get("hardwareReservation"),
    "runtimeIdentitySha256": runtime_sha,
    "schema": RESULT_SCHEMA,
    "target": target,
    "transactionSha256": transaction.get("transactionSha256"),
}
for name, value in (
    ("isa", isa),
    ("resource", resource),
    ("result", result),
):
    exclusive_write(
        os.environ[f"FE2O3_TUTORIAL_HARDWARE_{name.upper()}_OBSERVATION_OUTPUT"],
        canonical(value),
    )
PY
}

# Shared production-authority runner used by the compact gfx942 examples.
fe2o3_tutorial_hardware_authority_run_gfx942() {
    local example_dir=$1
    local binary=$2
    local timeout_seconds=${FE2O3_HARDWARE_TIMEOUT_SECONDS:-600}
    if [[ ! $timeout_seconds =~ ^[0-9]+$ ]] || \
        (( timeout_seconds < 1 || timeout_seconds > 900 )); then
        printf 'FE2O3_HARDWARE_TIMEOUT_SECONDS must be an integer in 1..900\n' >&2
        return 2
    fi

    local cargo_fe2o3=${FE2O3_CARGO_FE2O3:-$(command -v cargo-fe2o3 2>/dev/null || true)}
    local timeout_bin
    timeout_bin=$(command -v timeout 2>/dev/null || true)
    if [[ -z $cargo_fe2o3 || $cargo_fe2o3 != /* || ! -f $cargo_fe2o3 || \
        ! -x $cargo_fe2o3 || -L $cargo_fe2o3 ]]; then
        printf 'FE2O3_CARGO_FE2O3 must name an absolute regular executable\n' >&2
        return 1
    fi
    if [[ -z $timeout_bin ]]; then
        printf 'GNU timeout is required\n' >&2
        return 1
    fi

    local scratch_root=${FE2O3_RUN_SCRATCH_ROOT:-${TMPDIR:-/tmp}}
    local scratch
    scratch=$(mktemp -d "$scratch_root/fe2o3-$binary-gfx942.XXXXXX")
    chmod 700 "$scratch"
    FE2O3_TUTORIAL_GFX942_RUN_SCRATCH=$scratch
    trap 'rm -rf -- "${FE2O3_TUTORIAL_GFX942_RUN_SCRATCH:?}"' EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    fe2o3_tutorial_hardware_begin
    local environment=(-i LANG=C LC_ALL=C TZ=UTC FE2O3_TARGET=gfx942)
    local name
    for name in \
        CARGO \
        FE2O3_AUTHORITY_BACKEND_SHA256_V1 \
        FE2O3_AUTHORITY_CARGO_SHA256_V1 \
        FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1 \
        FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1 \
        FE2O3_AUTHORITY_RUSTC_PATH_V1 \
        FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1 \
        FE2O3_AUTHORITY_RUSTC_SHA256_V1 \
        FE2O3_BACKEND \
        FE2O3_PRODUCTION_BUILD_CONFIG_V1 \
        FE2O3_PRODUCTION_BUILD_CONFIG_V2 \
        FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT
    do
        if [[ -v $name ]]; then
            environment+=("$name=${!name}")
        fi
    done
    env "${environment[@]}" "$timeout_bin" --foreground --signal=TERM --kill-after=10 \
        "$timeout_seconds" "$cargo_fe2o3" authority release run --locked \
        --manifest-path "$example_dir/Cargo.toml" \
        --target-dir "$scratch/target" \
        --bin "$binary" -- --gfx942-qualification
    fe2o3_tutorial_hardware_finish gfx942
    rm -rf -- "$FE2O3_TUTORIAL_GFX942_RUN_SCRATCH"
    unset FE2O3_TUTORIAL_GFX942_RUN_SCRATCH
    trap - EXIT HUP INT TERM
}
