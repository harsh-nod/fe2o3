#!/usr/bin/env python3
"""Generate the bounded, typed semantic-simulation fixtures from the manifest."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from typing import Any, Callable


ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "config" / "tutorial-kernel-manifest-v1.json"
FIXTURE_ROOT = ROOT / "scripts" / "tutorial-semantic-fixtures-v1"
SCHEMA = "fe2o3-tutorial-capability-simulation-fixture-v1"
ABI_SCHEMA = "fe2o3-physical-kernel-abi-v1"
REQUEST_SCHEMA = "fe2o3-simulation-request-v1"
SENTINEL_F32 = 12_345.0
SENTINEL_U32 = 0xA5A5A5A5
PRODUCER_TIMEOUT_SECONDS = 3600
PRODUCER_ERROR = re.compile(r"(FE2O3-TUTORIAL-(?:PROBE|TXN)-[0-9]{3}): (.+)\Z")
DIAGNOSTIC_IDENTITY_DOMAIN = b"fe2o3-tutorial-production-export-diagnostic-v1\0"
PRODUCER_STAGES = {
    "FE2O3-TUTORIAL-PROBE-001": "exporter-build",
    "FE2O3-TUTORIAL-PROBE-002": "exporter-build",
    "FE2O3-TUTORIAL-TXN-001": "request-io",
    "FE2O3-TUTORIAL-TXN-002": "request-decode",
    "FE2O3-TUTORIAL-TXN-003": "request-schema",
    "FE2O3-TUTORIAL-TXN-004": "request-binding",
    "FE2O3-TUTORIAL-TXN-005": "source-preflight",
    "FE2O3-TUTORIAL-TXN-006": "target-preflight",
    "FE2O3-TUTORIAL-TXN-007": "protected-completion",
    "FE2O3-TUTORIAL-TXN-008": "production-result-validation",
    "FE2O3-TUTORIAL-TXN-009": "artifact-validation",
    "FE2O3-TUTORIAL-TXN-010": "evidence-presence",
    "FE2O3-TUTORIAL-TXN-011": "evidence-validation",
    "FE2O3-TUTORIAL-TXN-012": "hardware-receipt",
    "FE2O3-TUTORIAL-TXN-013": "output-preflight",
    "FE2O3-TUTORIAL-TXN-014": "publication",
    "FE2O3-TUTORIAL-TXN-015": "qualification-schema",
}
RUST_DIAGNOSTIC = re.compile(r"(?:^|: )error(?:\[[A-Z][A-Z0-9]*\])?: .+")
PRODUCTION_EXPORT_IDENTITY_CONTRACT = {
    "rawContentField": "evidenceFiles[optimized-kir-v13].sha256",
    "rawContentIdentity": "SHA-256(raw optimized KIR V13 bytes)",
    "verifiedCanonicalKernelIrV13Fields": [
        "productionEvidence.finalOptimizedKirSha256",
        "graph.productionKirIdentitySha256",
        "simulator.subjectSha256",
    ],
    "verifiedCanonicalKernelIrV13Identity": (
        "domain-separated VerifiedCanonicalKernelIrV13 identity"
    ),
}


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, allow_nan=False, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("ascii")


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class ProducerBuildBlocked(ValueError):
    def __init__(self, export: dict[str, Any]) -> None:
        super().__init__(export["diagnostic"])
        self.export = export


def _build_failure_export(code: str, component: str, stderr: bytes) -> dict[str, Any]:
    text = stderr.decode("utf-8", errors="replace")
    lines = [line.strip().replace(f"{ROOT}/", "") for line in text.splitlines()]
    detail = next(
        (
            line
            for line in lines
            if RUST_DIAGNOSTIC.search(line) is not None
            and "could not compile" not in line
        ),
        None,
    )
    if detail is None:
        raise ValueError(f"{component} failed without a structured compiler diagnostic")
    return {
        "coordinates": None,
        "diagnostic": f"{code}: {component} failed: {detail}",
        "diagnosticCode": code,
        "diagnosticIdentitySha256": None,
        "identityContract": PRODUCTION_EXPORT_IDENTITY_CONTRACT,
        "stage": PRODUCER_STAGES[code],
        "status": "blocked",
    }


def _load_producer_contract():
    path = ROOT / "scripts" / "produce-tutorial-capability-qualification.py"
    specification = importlib.util.spec_from_file_location(
        "tutorial_capability_producer_for_fixture_probe", path
    )
    if specification is None or specification.loader is None:
        raise ValueError(f"cannot load production transaction contract from {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _run(
    command: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    timeout: int = PRODUCER_TIMEOUT_SECONDS,
) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"production exporter probe exceeded {timeout} seconds") from error


def _git_snapshot(
    temporary: Path,
) -> tuple[dict[str, str], dict[str, Any], Path]:
    git_directory = temporary / "git"
    initialized = _run(
        ["git", "init", "--bare", "--quiet", str(git_directory)],
        cwd=ROOT,
        environment=os.environ.copy(),
        timeout=30,
    )
    if initialized.returncode != 0:
        detail = initialized.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(f"cannot initialize producer probe snapshot: {detail}")
    environment = os.environ.copy()
    environment.update(
        {
            "GIT_AUTHOR_EMAIL": "fixture-probe@invalid",
            "GIT_AUTHOR_NAME": "fe2o3 fixture probe",
            "GIT_COMMITTER_EMAIL": "fixture-probe@invalid",
            "GIT_COMMITTER_NAME": "fe2o3 fixture probe",
            "GIT_DIR": str(git_directory),
            "GIT_WORK_TREE": str(ROOT),
        }
    )
    commands = (
        ["git", "add", "-A"],
        ["git", "commit", "--quiet", "--message", "fixture probe snapshot"],
    )
    for command in commands:
        result = _run(command, cwd=ROOT, environment=environment, timeout=300)
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", errors="replace").strip()
            raise ValueError(f"cannot create clean producer probe snapshot: {detail}")
    def git_value(*arguments: str) -> str:
        result = _run(["git", *arguments], cwd=ROOT, environment=environment, timeout=30)
        if result.returncode != 0:
            raise ValueError("cannot identify producer probe snapshot")
        return result.stdout.decode("ascii").strip()
    if git_value("status", "--porcelain=v1", "--untracked-files=all"):
        raise ValueError("producer probe snapshot is not clean")
    candidate = {
        "compilerCommit": git_value("rev-parse", "--verify", "HEAD"),
        "compilerTree": git_value("show", "-s", "--format=%T", "HEAD"),
        "worktreeClean": True,
    }
    repository = temporary / "repository"
    checkout_environment = os.environ.copy()
    checkout = _run(
        [
            "git",
            f"--git-dir={git_directory}",
            "worktree",
            "add",
            "--detach",
            "--quiet",
            str(repository),
            candidate["compilerCommit"],
        ],
        cwd=temporary,
        environment=checkout_environment,
        timeout=300,
    )
    if checkout.returncode != 0:
        detail = checkout.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(f"cannot check out producer probe snapshot: {detail}")
    environment["GIT_WORK_TREE"] = str(repository)
    return environment, candidate, repository


def _trust_policy(producer: Any, path: Path) -> None:
    public_key = ROOT / "scripts/tests/fixtures/evidence-test-attestor-public.pem"
    document = producer.hardware_receipt_contract.trust_policy_document(
        [
            {
                "attestorIdentity": "fixture-probe-attestor-v1",
                "lane": "mi300x",
                "publicKeyPath": str(public_key),
                "reservationIdentity": "fixture-probe-mi300x",
                "target": "gfx942",
            },
            {
                "attestorIdentity": "fixture-probe-attestor-v1",
                "lane": "mi350",
                "publicKeyPath": str(public_key),
                "reservationIdentity": "fixture-probe-mi350",
                "target": "gfx950",
            },
        ]
    )
    path.write_bytes(producer._canonical(document) + b"\n")
    path.chmod(0o600)


def _build_producer(
    producer: Any, target_directory: Path, repository: Path
) -> tuple[Path, Path]:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise ValueError("cargo is unavailable for the production exporter probe")
    environment = producer._transaction_environment()
    target_libdir = _run(
        ["rustc", "--print", "target-libdir"],
        cwd=repository,
        environment=environment,
        timeout=30,
    )
    if target_libdir.returncode != 0:
        raise ValueError("cannot identify the Rust target library directory")
    rust_runtime = target_libdir.stdout.decode("utf-8").strip()
    if not Path(rust_runtime).is_dir():
        raise ValueError("Rust target library directory is unavailable")
    environment.update(
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_TARGET_DIR": str(target_directory),
            "CARGO_TERM_COLOR": "never",
            "RUSTFLAGS": f"-C prefer-dynamic -C link-arg=-Wl,-rpath,{rust_runtime}",
        }
    )
    result = _run(
        [
            cargo,
            "rustc",
            "--offline",
            "--locked",
            "--package",
            "rustc-codegen-fe2o3",
            "--bin",
            producer.TRANSACTION_EXPORT_BINARY,
            "--message-format=short",
            "--",
            "-Zcrate-attr=feature(rustc_private)",
        ],
        cwd=repository,
        environment=environment,
    )
    if result.returncode != 0:
        raise ProducerBuildBlocked(
            _build_failure_export(
                "FE2O3-TUTORIAL-PROBE-001", "production exporter", result.stderr
            )
        )
    binary = target_directory / "debug" / producer.TRANSACTION_EXPORT_BINARY
    if not binary.is_file() or binary.is_symlink():
        raise ValueError("production exporter probe build omitted its binary")
    result = _run(
        [
            cargo,
            "build",
            "--offline",
            "--locked",
            "--package",
            "cargo-fe2o3",
            "--bin",
            "cargo-fe2o3",
            "--message-format=short",
        ],
        cwd=repository,
        environment=environment,
    )
    if result.returncode != 0:
        raise ProducerBuildBlocked(
            _build_failure_export(
                "FE2O3-TUTORIAL-PROBE-002", "cargo-fe2o3", result.stderr
            )
        )
    cargo_fe2o3 = target_directory / "debug" / "cargo-fe2o3"
    if not cargo_fe2o3.is_file() or cargo_fe2o3.is_symlink():
        raise ValueError("production exporter probe build omitted cargo-fe2o3")
    return binary, cargo_fe2o3


def _available_export(export_root: Path, fixture_id: str, target: str) -> dict[str, Any]:
    envelope = json.loads((export_root / "transaction-export-v1.json").read_bytes())
    record = envelope["record"]
    graph = record["graph"]
    evidence = record["productionEvidence"]
    simulator = record["simulator"]
    files = record["evidenceFiles"]
    verified = (
        evidence["finalOptimizedKirSha256"],
        graph["productionKirIdentitySha256"],
        simulator["subjectSha256"],
    )
    raw = files["optimized-kir-v13"]["sha256"]
    if len(set(verified)) != 1:
        raise ValueError(f"producer export {fixture_id} conflates distinct canonical KIR subjects")
    if raw == verified[0]:
        raise ValueError(f"producer export {fixture_id} conflates canonical KIR and raw content identities")
    coordinates = {
        "bundleContentIdentitySha256": graph["bundleContentIdentitySha256"],
        "bundleSubjectIdentitySha256": graph["bundleSubjectIdentitySha256"],
        "bundleVersion": 8,
        "finalOptimizedKirSha256": verified[0],
        "kirVersion": 13,
        "optimizedKirV13ContentSha256": raw,
        "productionKirIdentitySha256": verified[1],
        "simulationBundleV8ContentSha256": files["simulation-bundle-v8"]["sha256"],
        "simulatorSubjectSha256": verified[2],
        "target": target,
    }
    return {
        "coordinates": coordinates,
        "diagnostic": None,
        "diagnosticCode": None,
        "diagnosticIdentitySha256": None,
        "identityContract": PRODUCTION_EXPORT_IDENTITY_CONTRACT,
        "stage": "production-transaction",
        "status": "available",
    }


def _blocked_export(stderr: bytes) -> dict[str, Any]:
    text = stderr.decode("utf-8", errors="replace").strip()
    prefix = "fe2o3 tutorial production transaction: "
    diagnostic = next(
        (line[len(prefix) :] for line in reversed(text.splitlines()) if line.startswith(prefix)),
        text,
    )
    match = PRODUCER_ERROR.fullmatch(diagnostic)
    if match is None:
        raise ValueError(f"production exporter emitted an unstructured diagnostic: {diagnostic}")
    code = match.group(1)
    return {
        "coordinates": None,
        "diagnostic": diagnostic,
        "diagnosticCode": code,
        "diagnosticIdentitySha256": None,
        "identityContract": PRODUCTION_EXPORT_IDENTITY_CONTRACT,
        "stage": PRODUCER_STAGES[code],
        "status": "blocked",
    }


def diagnostic_identity_sha256(
    fixture: dict[str, Any], kernel: dict[str, Any], export: dict[str, Any]
) -> str:
    subject = {
        "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
        "diagnostic": export["diagnostic"],
        "diagnosticCode": export["diagnosticCode"],
        "fixtureId": fixture["fixtureId"],
        "identityContract": export["identityContract"],
        "kernelSymbol": kernel["kernelSymbol"],
        "sourceClosureSha256": fixture["compilerInput"]["sourceClosureSha256"],
        "stage": export["stage"],
        "status": export["status"],
        "target": fixture["target"],
    }
    return sha256(DIAGNOSTIC_IDENTITY_DOMAIN + canonical(subject))


def bind_production_export(
    fixture: dict[str, Any], kernel: dict[str, Any], export: dict[str, Any]
) -> dict[str, Any]:
    bound = copy.deepcopy(export)
    if bound["status"] == "blocked":
        bound["diagnosticIdentitySha256"] = diagnostic_identity_sha256(
            fixture, kernel, bound
        )
    elif bound["status"] == "available":
        bound["diagnosticIdentitySha256"] = None
    else:
        raise ValueError("production export probe returned an invalid status")
    return bound


def probe_production_exports(
    manifest: dict[str, Any],
    fixtures: dict[str, dict[str, Any]],
    kernels: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    producer = _load_producer_contract()
    manifest_payload = MANIFEST.read_bytes()
    manifest_identity = producer._manifest_identity(manifest, manifest_payload)
    simulator_reference = producer._reference(b"tutorial-fixture-producer-probe-v1\n")
    ordered = sorted(fixtures)
    with tempfile.TemporaryDirectory(prefix="fe2o3-fixture-producer-probe-") as raw_temporary:
        temporary = Path(raw_temporary)
        target_directory = Path(
            os.environ.get(
                "FE2O3_TUTORIAL_PRODUCER_PROBE_TARGET",
                str(temporary / "cargo-target"),
            )
        ).resolve()
        if target_directory.is_relative_to(ROOT):
            raise ValueError("production exporter probe target must be outside the repository")
        environment, candidate, repository = _git_snapshot(temporary)
        try:
            binary, cargo_fe2o3 = _build_producer(
                producer, target_directory, repository
            )
        except ProducerBuildBlocked as blocked:
            return {fixture_id: dict(blocked.export) for fixture_id in ordered}
        clean_environment = producer._transaction_environment()
        clean_environment.update(
            {name: value for name, value in environment.items() if name.startswith("GIT_")}
        )
        sysroot = _run(
            ["rustc", "--print", "sysroot"],
            cwd=ROOT,
            environment=clean_environment,
            timeout=30,
        )
        if sysroot.returncode != 0:
            raise ValueError("cannot identify rustc sysroot for production exporter")
        loader = [
            str(target_directory / "debug" / "deps"),
            str(Path(sysroot.stdout.decode("utf-8").strip()) / "lib"),
        ]
        if clean_environment.get("LD_LIBRARY_PATH"):
            loader.append(clean_environment["LD_LIBRARY_PATH"])
        clean_environment["LD_LIBRARY_PATH"] = os.pathsep.join(loader)
        environment = clean_environment
        policy = temporary / "hardware-policy-v1.json"
        _trust_policy(producer, policy)

        def probe(fixture_id: str) -> dict[str, Any]:
            fixture = fixtures[fixture_id]
            reservation = f"fixture-probe-{'mi300x' if fixture['target'] == 'gfx942' else 'mi350'}"
            request = producer.transaction_request(
                fixture,
                kernels[fixture_id],
                candidate,
                manifest_identity,
                simulator_reference,
                reservation,
                "11" * 32,
            )
            unit = temporary / fixture_id
            unit.mkdir()
            request_path = unit / "request-v1.json"
            request_path.write_bytes(producer._canonical(request) + b"\n")
            export_root = unit / "export"
            result = _run(
                [
                    str(binary),
                    "--cargo-fe2o3",
                    str(cargo_fe2o3),
                    "--hardware-trust-policy",
                    str(policy),
                    "--request",
                    str(request_path),
                    "--output-directory",
                    str(export_root),
                ],
                cwd=repository,
                environment=environment,
            )
            if result.returncode == 0:
                return _available_export(export_root, fixture_id, fixture["target"])
            return _blocked_export(result.stderr)

        first_id = ordered[0]
        first = probe(first_id)
        exports = {first_id: first}
        exports.update({fixture_id: probe(fixture_id) for fixture_id in ordered[1:]})
        return exports


def encoded(values: list[int | float], element: str) -> str:
    formats = {"u8": "B", "u16": "H", "u32": "I", "u64": "Q", "i32": "i", "f32": "f"}
    try:
        payload = b"".join(struct.pack("<" + formats[element], value) for value in values)
    except (KeyError, struct.error) as error:
        raise ValueError(f"cannot encode {element} values: {error}") from error
    return "0x" + payload.hex()


def scalar(value: int | float, ty: str) -> dict[str, Any]:
    return {"bits": encoded([value], ty), "kind": "scalar", "type": ty}


def buffer(values: list[int | float], element: str, access: str) -> dict[str, Any]:
    alignment = {"u8": 1, "u16": 2, "u32": 4, "u64": 8, "i32": 4, "f32": 4}[element]
    return {
        "access": access,
        "alignment": alignment,
        "bytes": encoded(values, element),
        "element": element,
        "kind": "buffer",
    }


def expected(argument: dict[str, Any], values: list[int | float] | None = None) -> dict[str, Any]:
    if argument["kind"] == "scalar":
        return {"bits": argument["bits"], "kind": "scalar", "type": argument["type"]}
    return {
        "bytes": argument["bytes"] if values is None else encoded(values, argument["element"]),
        "element": argument["element"],
        "kind": "buffer",
    }


def f32(value: float) -> float:
    return struct.unpack("<f", struct.pack("<f", value))[0]


def bf16(value: float) -> int:
    bits = struct.unpack("<I", struct.pack("<f", f32(value)))[0]
    return ((bits + 0x7FFF + ((bits >> 16) & 1)) >> 16) & 0xFFFF


def byte_range(argument: int, element_offset: int, values: list[int | float], element: str) -> dict[str, Any]:
    width = {"u8": 1, "u16": 2, "u32": 4, "u64": 8, "i32": 4, "f32": 4}[element]
    return {"argument": argument, "bytes": encoded(values, element), "offset": element_offset * width}


def split_parameters(parameters: str) -> list[str]:
    result: list[str] = []
    start = 0
    depth = 0
    for index, character in enumerate(parameters):
        if character in "<([":
            depth += 1
        elif character in ">)]":
            depth -= 1
        elif character == "," and depth == 0:
            value = parameters[start:index].strip()
            if value:
                result.append(value)
            start = index + 1
    value = parameters[start:].strip()
    if value:
        result.append(value)
    return result


def function_parameters(source: str, symbol: str) -> str | None:
    marker = f"pub fn {symbol}("
    start = source.find(marker)
    if start < 0:
        return None
    start += len(marker)
    depth = 1
    cursor = start
    while cursor < len(source) and depth:
        if source[cursor] == "(":
            depth += 1
        elif source[cursor] == ")":
            depth -= 1
        cursor += 1
    if depth:
        raise ValueError(f"unterminated signature for {symbol}")
    return source[start : cursor - 1]


FEATURE_SOURCE = {
    "kernel-attnres-aggregate-explicit-reuse-v1": "src/ablation.rs",
    "kernel-compressed-hybrid-attention-division-baseline-v1": "src/ablation.rs",
    "kernel-content-sparse-attention-reciprocal-reuse-v1": "src/ablation.rs",
    "kernel-four-branch-residual-explicit-v1": "src/ablation.rs",
    "kernel-mhc-sinkhorn-mix-scalar-v1": "src/ablation.rs",
    "kernel-kda-decode-baseline-v1": "src/kda_baseline.rs",
    "kernel-kda-prefill-baseline-v1": "src/kda_baseline.rs",
    "kernel-gpt-oss-decode-held-fragments": "src/kernel_held_fragments.rs",
    "kernel-gpt-oss-decode-interleaved-stores": "src/kernel_interleaved_stores.rs",
    "kernel-gpt-oss-decode-router-serial": "src/kernel_router_serial.rs",
    "kernel-gpt-oss-attention-component": "src/kernel_components.rs",
    "kernel-gpt-oss-expert-component": "src/kernel_components.rs",
    "kernel-gpt-oss-router-component": "src/kernel_components.rs",
}


def physical_abi(fixture: dict[str, Any], symbol: str) -> dict[str, Any]:
    compiler_input = fixture["compilerInput"]
    preferred = next(
        (suffix for feature, suffix in FEATURE_SOURCE.items() if feature in compiler_input["features"]),
        "src/kernel.rs",
    )
    candidates: list[tuple[str, str]] = []
    for relative in compiler_input["sourcePaths"]:
        path = ROOT / relative
        if path.suffix != ".rs":
            continue
        parameters = function_parameters(path.read_text(encoding="utf-8"), symbol)
        if parameters is not None:
            candidates.append((relative, parameters))
    selected = [item for item in candidates if item[0].endswith(preferred)]
    if len(selected) != 1:
        if len(candidates) != 1:
            raise ValueError(
                f"{fixture['fixtureId']} cannot select one physical signature for {symbol}: {candidates!r}"
            )
        selected = candidates
    source_path, parameters = selected[0]
    arguments: list[dict[str, Any]] = []
    for raw in split_parameters(parameters):
        raw = re.sub(r"^mut\s+", "", raw.strip())
        name, separator, ty = raw.partition(":")
        if not separator:
            raise ValueError(f"cannot parse {symbol} parameter {raw!r}")
        name, ty = name.strip(), " ".join(ty.split())
        if ty.startswith("KernelContext<"):
            continue
        global_match = re.fullmatch(r"Global<'_,\s*(u8|u16|u32|u64|i32|f32),\s*(.+)>", ty)
        if global_match:
            element, capability = global_match.groups()
            arguments.append(
                {
                    "access": "read_only" if capability == "ReadOnly" else "read_write",
                    "element": element,
                    "kind": "buffer",
                    "name": name,
                }
            )
        elif ty in {"u32", "u64", "i32", "f32"}:
            arguments.append({"kind": "scalar", "name": name, "type": ty})
        else:
            raise ValueError(f"unsupported physical type in {symbol}: {ty}")
    normalized = f"pub fn {symbol}({','.join(' '.join(item.split()) for item in split_parameters(parameters))})"
    return {
        "arguments": arguments,
        "schema": ABI_SCHEMA,
        "signatureSha256": sha256(normalized.encode("utf-8")),
        "sourcePath": source_path,
    }


def make_args(abi: dict[str, Any], values: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    names = [item["name"] for item in abi["arguments"]]
    if set(names) != set(values):
        raise ValueError(f"ABI values differ: missing={set(names)-set(values)} extra={set(values)-set(names)}")
    result = [values[name] for name in names]
    for specification, argument in zip(abi["arguments"], result):
        for key in ("kind", "element", "access", "type"):
            if key in specification and argument.get(key) != specification[key]:
                raise ValueError(f"argument {specification['name']} differs at {key}")
    return result


class Built:
    def __init__(
        self,
        values: dict[str, dict[str, Any]],
        outputs: dict[str, list[int | float]],
        *,
        grid: list[int],
        workgroup: list[int],
        policy: dict[str, Any] | None = None,
        canaries: list[tuple[str, int, list[int | float], str]] | None = None,
        padding: list[tuple[str, int, list[int | float], str]] | None = None,
        oracle: str,
    ) -> None:
        self.values = values
        self.outputs = outputs
        self.grid = grid
        self.workgroup = workgroup
        self.policy = policy or {"mode": "exact-bits"}
        self.canaries = canaries or []
        self.padding = padding or []
        self.oracle = oracle


def fill_fixture(_: str) -> Built:
    initial = [SENTINEL_F32] * 6
    return Built(
        {"out": buffer(initial, "f32", "read_write")},
        {"out": [42.5] * 4 + initial[4:]},
        grid=[4, 1, 1], workgroup=[64, 1, 1],
        canaries=[("out", 4, initial[4:], "f32")],
        oracle="fe2o3_fill::fill_cpu_reference",
    )


def vecadd_fixture(_: str) -> Built:
    left = [1.0, -2.0, 3.5, 4.0, 91.0, 92.0]
    right = [0.5, 2.0, -0.5, 4.0, 81.0, 82.0]
    initial = [SENTINEL_F32] * 6
    return Built(
        {"a": buffer(left, "f32", "read_only"), "b": buffer(right, "f32", "read_only"), "c": buffer(initial, "f32", "read_write")},
        {"c": [1.5, 0.0, 3.0, 8.0, SENTINEL_F32, SENTINEL_F32]},
        grid=[4, 1, 1], workgroup=[64, 1, 1],
        canaries=[("c", 4, initial[4:], "f32")],
        oracle="fe2o3_vecadd::vecadd_cpu_oracle",
    )


def wave_fixture(_: str) -> Built:
    inputs = [float((index % 7) - 3) for index in range(64)]
    mask = 0xB6DB6DB6DB6DB6DB
    contributions = [value if mask & (1 << lane) else 0.0 for lane, value in enumerate(inputs)]
    reduction = sum(contributions)
    inclusive: list[float] = []
    running = 0.0
    for value in contributions:
        running += value
        inclusive.append(running)
    exclusive = [0.0] + inclusive[:-1]
    active = [bool(mask & (1 << lane)) for lane in range(64)]
    return Built(
        {
            "input": buffer(inputs, "f32", "read_only"), "active_mask": scalar(mask, "u64"),
            "reduction_output": buffer([SENTINEL_F32] * 64, "f32", "read_write"),
            "inclusive_output": buffer([SENTINEL_F32] * 64, "f32", "read_write"),
            "exclusive_output": buffer([SENTINEL_F32] * 64, "f32", "read_write"),
        },
        {
            "reduction_output": [reduction if value else 0.0 for value in active],
            "inclusive_output": [inclusive[i] if active[i] else 0.0 for i in range(64)],
            "exclusive_output": [exclusive[i] if active[i] else 0.0 for i in range(64)],
        },
        grid=[64, 1, 1], workgroup=[64, 1, 1], oracle="wave64_collectives_oracle_v1",
    )


def workgroup_fixture(_: str) -> Built:
    values = [index - 32 for index in range(64)]
    return Built(
        {"values": buffer(values, "i32", "read_only"), "output": buffer([-999], "i32", "read_write")},
        {"output": [sum(values)]}, grid=[64, 1, 1], workgroup=[64, 1, 1],
        oracle="lds_reduction_oracle_v1",
    )


def gemm_fixture(symbol: str) -> Built:
    rows = columns = reduction = 16
    stride = 17
    lhs = [bf16(1.0 if row == depth else 0.0) for row in range(rows) for depth in range(reduction)]
    rhs_values = [float((depth + column) % 5 - 2) for depth in range(reduction) for column in range(columns)]
    rhs = [bf16(value) for value in rhs_values]
    initial = [1.0 if column < columns else SENTINEL_F32 for _row in range(rows) for column in range(stride)]
    output = initial.copy()
    for row in range(rows):
        for column in range(columns):
            output[row * stride + column] = rhs_values[row * columns + column] + 1.0
    padding = [("c", row * stride + columns, [SENTINEL_F32], "f32") for row in range(rows)]
    return Built(
        {
            "a": buffer(lhs, "u16", "read_only"), "b": buffer(rhs, "u16", "read_only"),
            "c": buffer(initial, "f32", "read_write"), "m": scalar(rows, "u32"),
            "n": scalar(columns, "u32"), "k": scalar(reduction, "u32"),
            "lda": scalar(reduction, "u32"), "ldb": scalar(columns, "u32"),
            "ldc": scalar(stride, "u32"), "alpha": scalar(1.0, "f32"), "beta": scalar(1.0, "f32"),
        },
        {"c": output}, grid=[64, 1, 1], workgroup=[64, 1, 1], padding=padding,
        policy={"absoluteTolerance": 1.0e-6, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 1.0e-6},
        oracle=("fe2o3_gemm_autoresearch_v1" if "autoresearch" in symbol else "fe2o3_tiled_gemm_general_v1") + "::reference::evaluate_reference_v1",
    )


def flash_fixture(_: str) -> Built:
    rows, keys, output_stride = 16, 16, 2
    output = [0.0 if column == 0 else SENTINEL_F32 for _row in range(rows) for column in range(output_stride)]
    output[0] = output[2] = 8.5
    padding = [("output", row * output_stride + 1, [SENTINEL_F32], "f32") for row in range(rows)]
    values = {
        "q": buffer([bf16(0.0)] * rows, "u16", "read_only"),
        "k_transposed": buffer([bf16(0.0)] * keys, "u16", "read_only"),
        "v": buffer([float(index + 1) for index in range(keys)], "f32", "read_only"),
        "additive_mask": buffer([0.0] * (rows * keys), "f32", "read_only"),
        "output": buffer([SENTINEL_F32] * (rows * output_stride), "f32", "read_write"),
        "batch_heads": scalar(1, "u32"), "query_rows": scalar(2, "u32"),
        "query_rows_padded": scalar(rows, "u32"), "keys": scalar(keys, "u32"),
        "keys_padded": scalar(keys, "u32"), "depth": scalar(1, "u32"),
        "value_dimension": scalar(1, "u32"), "q_stride": scalar(1, "u32"),
        "k_depth_stride": scalar(keys, "u32"), "k_head_stride": scalar(keys, "u32"),
        "v_stride": scalar(1, "u32"), "v_head_stride": scalar(keys, "u32"),
        "mask_stride": scalar(keys, "u32"), "output_stride": scalar(output_stride, "u32"),
        "output_rows": scalar(rows, "u32"), "scale": scalar(1.0, "f32"),
    }
    return Built(values, {"output": output}, grid=[64, 1, 1], workgroup=[64, 1, 1], padding=padding,
        policy={"absoluteTolerance": 1.0e-5, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 1.0e-5},
        oracle="fe2o3_flash_attention_general_v1::reference::evaluate_reference_v1")


def grouped_fixture(_: str) -> Built:
    rows = columns = reduction = 16
    stride = 17
    gates = [row / 16.0 for row in range(rows)]
    initial = [SENTINEL_F32] * (rows * stride)
    output = initial.copy()
    for row in range(rows):
        for column in range(columns):
            output[row * stride + column] = gates[row]
    padding = [("routed_output", row * stride + columns, [SENTINEL_F32], "f32") for row in range(rows)]
    return Built(
        {
            "routed_tokens": buffer([bf16(0.0)] * (rows * reduction), "u16", "read_only"),
            "expert_weights": buffer([bf16(0.0)] * (reduction * columns), "u16", "read_only"),
            "route_gates": buffer(gates, "f32", "read_only"),
            "expert_bias": buffer([1.0] * columns, "f32", "read_only"),
            "routed_output": buffer(initial, "f32", "read_write"),
            "rows_padded": scalar(rows, "u32"), "output_columns": scalar(columns, "u32"),
            "reduction": scalar(reduction, "u32"), "token_stride": scalar(reduction, "u32"),
            "weight_stride": scalar(columns, "u32"), "expert_weight_stride": scalar(reduction * columns, "u32"),
            "bias_stride": scalar(columns, "u32"), "output_stride": scalar(stride, "u32"),
            "expert": scalar(0, "u32"), "expert_count": scalar(1, "u32"),
        },
        {"routed_output": output}, grid=[64, 1, 1], workgroup=[64, 1, 1], padding=padding,
        policy={"absoluteTolerance": 1.0e-6, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 1.0e-6},
        oracle="fe2o3_moe_grouped_expert_general_v1::reference::evaluate_reference_v1",
    )


def top2_outputs(logits: list[float]) -> dict[str, list[int]]:
    selected: list[int] = []
    requested = [0] * 4
    for token in range(8):
        order = sorted(range(4), key=lambda expert: (-logits[token * 4 + expert], expert))[:2]
        selected.extend(order)
        for expert in order:
            requested[expert] += 1
    admitted = [min(value, 4) for value in requested]
    offsets = [0]
    for value in admitted:
        offsets.append(offsets[-1] + value)
    drop = 0xFFFFFFFF
    slots = [drop] * 16
    permutation = [drop] * 16
    seen = [0] * 4
    for route, expert in enumerate(selected):
        rank = seen[expert]
        seen[expert] += 1
        if rank < 4:
            slot = offsets[expert] + rank
            slots[route] = slot
            permutation[slot] = route
    return {
        "top2_experts": selected, "requested_counts": requested, "admitted_counts": admitted,
        "expert_offsets": offsets, "route_slots": slots, "permutation": permutation,
        "inverse": slots.copy(),
    }


def top2_fixture(_: str) -> Built:
    logits = [float(value) for token in range(8) for value in (token, 7 - token, token % 3, -1)]
    outputs = top2_outputs(logits)
    values = {"logits": buffer(logits, "f32", "read_only")}
    sizes = {"top2_experts": 16, "requested_counts": 4, "admitted_counts": 4, "expert_offsets": 5, "route_slots": 16, "permutation": 16, "inverse": 16}
    values.update({name: buffer([SENTINEL_U32] * size, "u32", "read_write") for name, size in sizes.items()})
    return Built(values, outputs, grid=[64, 1, 1], workgroup=[64, 1, 1], oracle="moe_top2_oracle_v1")


def softmax_fixture(_: str) -> Built:
    initial = [SENTINEL_F32] * 6
    return Built(
        {"input": buffer([0.0, 0.0, 0.0, 0.0, 91.0, 92.0], "f32", "read_only"),
         "output": buffer(initial, "f32", "read_write"), "rows": scalar(1, "u32"),
         "columns": scalar(4, "u32"), "input_stride": scalar(6, "u32"), "output_stride": scalar(6, "u32")},
        {"output": [0.25] * 4 + initial[4:]}, grid=[64, 1, 1], workgroup=[64, 1, 1],
        canaries=[("output", 4, initial[4:], "f32")], padding=[("output", 4, initial[4:], "f32")],
        policy={"absoluteTolerance": 1.0e-6, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 1.0e-6},
        oracle="fe2o3_row_softmax_general_v1::reference::row_softmax_reference_v1",
    )


def advanced_attention_fixture(symbol: str) -> Built:
    grid, workgroup = [1024, 1, 1], [256, 1, 1]
    zero_out = lambda count: buffer([SENTINEL_F32] * count, "f32", "read_write")
    tolerance = {"absoluteTolerance": 2.0e-5, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 2.0e-5}
    if symbol == "gfx950_kda_decode":
        batches, state = 4, 256
        values = {
            "query": buffer([0.0] * (batches * 16), "f32", "read_only"), "key": buffer([0.0] * (batches * 16), "f32", "read_only"),
            "value": buffer([0.0] * (batches * 16), "f32", "read_only"), "alpha": buffer([1.0] * (batches * 16), "f32", "read_only"),
            "beta": buffer([0.5] * batches, "f32", "read_only"), "initial_state": buffer([0.0] * (batches * state), "f32", "read_only"),
            "final_state": zero_out(batches * state), "output": zero_out(batches * state),
        }
        return Built(values, {"final_state": [0.0] * (batches * state), "output": [0.0] * (batches * state)}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="kda_decode_reference_v2")
    if symbol == "gfx950_kda_chunkwise_prefill":
        batches, state, tokens = 4, 256, 8
        values = {
            "query": buffer([0.0] * (batches * tokens * 16), "f32", "read_only"), "key": buffer([0.0] * (batches * tokens * 16), "f32", "read_only"),
            "value": buffer([0.0] * (batches * tokens * 16), "f32", "read_only"), "alpha": buffer([1.0] * (batches * tokens * 16), "f32", "read_only"),
            "beta": buffer([0.5] * (batches * tokens), "f32", "read_only"), "initial_state": buffer([0.0] * (batches * state), "f32", "read_only"),
            "final_state": zero_out(batches * state), "output_chunk0": zero_out(batches * state), "output_chunk1": zero_out(batches * state),
        }
        outputs = {name: [0.0] * (batches * state) for name in ("final_state", "output_chunk0", "output_chunk1")}
        return Built(values, outputs, grid=grid, workgroup=workgroup, policy=tolerance, oracle="kda_prefill_reference_v2")
    if symbol in {"gfx950_content_sparse_attention", "gfx950_compressed_hybrid_attention"}:
        batches = 16
        values = {
            "q": buffer([0] * (batches * 16 * 128), "u8", "read_only"),
            "k": buffer([0] * (batches * 16 * 128), "u8", "read_only"),
            "v": buffer([0] * (batches * 16 * 16), "u8", "read_only"),
        }
        if symbol == "gfx950_content_sparse_attention":
            scores = [0.10, 0.82, -0.20, 0.35, 0.61, 0.55, 0.14, 0.92, 0.73, -0.10, 0.48, 0.31, 0.41, 0.67, 0.22, 0.05]
            values.update({"content_scores": buffer(scores * batches, "f32", "read_only"), "output": zero_out(batches * 16), "selected_output": buffer([SENTINEL_U32] * (batches * 3), "u32", "read_write")})
            outputs = {"output": [0.0] * (batches * 16), "selected_output": [value for _ in range(batches) for value in (7, 1, 4)]}
            oracle = "content_sparse_attention_reference_v1"
        else:
            values.update({"token_bias": buffer([float((index % 7) - 3) / 8.0 for index in range(batches * 16)], "f32", "read_only"), "output": zero_out(batches * 16)})
            outputs = {"output": [0.0] * (batches * 16)}
            oracle = "compressed_hybrid_attention_reference_v1"
        return Built(values, outputs, grid=grid, workgroup=workgroup, policy=tolerance, oracle=oracle)
    if symbol == "gfx950_deepseek_sparse_attention":
        batches = 64
        values = {
            "q": buffer([0.0] * (batches * 128), "f32", "read_only"), "k": buffer([0.0] * (batches * 16 * 128), "f32", "read_only"),
            "v": buffer([0.0] * (batches * 16 * 16), "f32", "read_only"), "index0": scalar(0, "u32"), "index1": scalar(1, "u32"),
            "index2": scalar(2, "u32"), "index3": scalar(3, "u32"), "output": zero_out(batches * 16),
            "softmax_maximum_output": zero_out(batches * 16), "softmax_normalizer_output": zero_out(batches * 16),
        }
        outputs = {"output": [0.0] * (batches * 16), "softmax_maximum_output": [0.0] * (batches * 16), "softmax_normalizer_output": [4.0] * (batches * 16)}
        return Built(values, outputs, grid=grid, workgroup=workgroup, policy=tolerance, oracle="deepseek_sparse_attention_reference_v1")
    if symbol in {"gfx950_attnres_aggregate", "gfx950_four_branch_residual"}:
        batches = 64
        if symbol == "gfx950_attnres_aggregate":
            values = {"depth_values": buffer([0.0] * (batches * 4 * 16), "f32", "read_only"), "depth_logits": buffer([float((i % 5) - 2) / 4 for i in range(batches * 4 * 16)], "f32", "read_only"), "output": zero_out(batches * 16)}
            oracle = "attnres_aggregate_reference_v1"
        else:
            values = {"residual": buffer([0.0] * (batches * 16), "f32", "read_only"), "branches": buffer([0.0] * (batches * 4 * 16), "f32", "read_only"), "gate_logits": buffer([float((i % 5) - 2) / 4 for i in range(batches * 4 * 16)], "f32", "read_only"), "output": zero_out(batches * 16)}
            oracle = "four_branch_residual_reference_v1"
        return Built(values, {"output": [0.0] * (batches * 16)}, grid=grid, workgroup=workgroup, policy=tolerance, oracle=oracle)
    if symbol == "gfx950_mhc_sinkhorn_mix":
        batches = 16
        values = {"streams": buffer([0.0] * (batches * 4 * 16), "f32", "read_only"), "mixing_logits": buffer([float((i % 5) - 2) / 4 for i in range(batches * 16)], "f32", "read_only"), "output": zero_out(batches * 4 * 16)}
        return Built(values, {"output": [0.0] * (batches * 4 * 16)}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="mhc_sinkhorn_mix_reference_v1")
    raise ValueError(f"no advanced-attention builder for {symbol}")


def systems_fixture(symbol: str) -> Built:
    grid, workgroup = [1024, 1, 1], [256, 1, 1]
    batches, tokens, hidden, columns = 16, 16, 128, 16
    tolerance = {"absoluteTolerance": 2.0e-5, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 2.0e-5}
    if symbol == "gfx950_moe_route_fp4_t16_e4_k2_v1":
        top = [value for _batch in range(batches) for _token in range(tokens) for value in (0, 1)]
        weights = [0.5] * (batches * tokens * 2)
        counts = [value for _batch in range(batches) for value in (16, 16, 0, 0)]
        dispatch: list[int] = []
        for _batch in range(batches):
            dispatch.extend(list(range(0, 32, 2)) + [-1] * 16)
            dispatch.extend(list(range(1, 32, 2)) + [-1] * 16)
            dispatch.extend([-1] * 64)
        values = {
            "activations": buffer([0] * (batches * tokens * hidden), "u8", "read_only"),
            "router_weights": buffer([0.0] * (batches * 4 * hidden), "f32", "read_only"),
            "top_experts": buffer([SENTINEL_U32] * len(top), "u32", "read_write"),
            "top_weights": buffer([SENTINEL_F32] * len(weights), "f32", "read_write"),
            "expert_counts": buffer([SENTINEL_U32] * len(counts), "u32", "read_write"),
            "dispatch": buffer([-999] * len(dispatch), "i32", "read_write"),
        }
        return Built(values, {"top_experts": top, "top_weights": weights, "expert_counts": counts, "dispatch": dispatch}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="batched_moe_routing_reference")
    if symbol == "gfx950_moe_expert_rank_fp4_fp8_v1":
        output_size = batches * tokens * columns
        values = {
            "activations": buffer([0] * (batches * tokens * hidden), "u8", "read_only"),
            "expert_weights": buffer([0] * (batches * 5 * hidden * columns), "u8", "read_only"),
            "top_experts": buffer([value for _ in range(batches * tokens) for value in (0, 1)], "u32", "read_only"),
            "top_weights": buffer([0.5] * (batches * tokens * 2), "f32", "read_only"),
            "first_expert": scalar(0, "u32"), "include_shared_expert": scalar(1, "u32"),
            "output": buffer([SENTINEL_F32] * output_size, "f32", "read_write"),
        }
        return Built(values, {"output": [0.0] * output_size}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="batched_moe_rank_reference")
    if symbol == "gfx950_combine_expert_ranks_v1":
        count = 4 * tokens * columns
        rank0 = [index * 0.25 for index in range(count)]
        rank1 = [-index * 0.125 for index in range(count)]
        values = {"rank0": buffer(rank0, "f32", "read_only"), "rank1": buffer(rank1, "f32", "read_only"), "output": buffer([SENTINEL_F32] * count, "f32", "read_write")}
        return Built(values, {"output": [left + right for left, right in zip(rank0, rank1)]}, grid=grid, workgroup=workgroup, oracle="combine_expert_ranks_reference")
    if symbol == "gfx950_speculative_transaction_v1":
        candidates, steps, state = 8, 4, 8
        base = [float(element) for _batch in range(batches) for element in range(state)]
        delta_count = batches * candidates * steps * state
        values = {
            "draft_tokens": buffer([0] * (batches * candidates * steps), "i32", "read_only"),
            "target_tokens": buffer([0] * (batches * steps), "i32", "read_only"),
            "draft_scores": buffer([1.0] * (batches * candidates * steps), "f32", "read_only"),
            "thresholds": buffer([0.5] * (batches * steps), "f32", "read_only"),
            "base_state": buffer(base, "f32", "read_only"), "proposed_deltas": buffer([0.25] * delta_count, "f32", "read_only"),
            "accepted_steps": buffer([SENTINEL_U32] * (batches * candidates), "u32", "read_write"),
            "committed": buffer([SENTINEL_U32] * (batches * candidates), "u32", "read_write"),
            "output_state": buffer([SENTINEL_F32] * (batches * candidates * state), "f32", "read_write"),
        }
        state_output = [float(element) + 1.0 for _batch in range(batches) for _candidate in range(candidates) for element in range(state)]
        return Built(values, {"accepted_steps": [4] * (batches * candidates), "committed": [1] * (batches * candidates), "output_state": state_output}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="batched_speculative_reference")
    if symbol == "gfx950_qwen_ngram_gather_v1":
        values = {
            "queries": buffer([0] * (batches * 8 * 3), "i32", "read_only"), "table_hashes": buffer([0] * (batches * 16), "u64", "read_only"),
            "table_grams": buffer([1] * (batches * 16 * 3), "i32", "read_only"), "table_values": buffer(list(range(batches * 16)), "i32", "read_only"),
            "priorities": buffer([0] * (batches * 16), "i32", "read_only"), "output": buffer([-999] * (batches * 8), "i32", "read_write"),
        }
        return Built(values, {"output": [-1] * (batches * 8)}, grid=grid, workgroup=workgroup, oracle="batched_ngram_reference")
    if symbol == "gfx950_stage_gradient_shard_v1":
        values_in = [f32(0.5 + index / 4096.0) for index in range(batches * 16)]
        return Built({"input": buffer(values_in, "f32", "read_only"), "output": buffer([SENTINEL_F32] * len(values_in), "f32", "read_write")}, {"output": values_in}, grid=grid, workgroup=workgroup, oracle="stage_gradient_shard_reference")
    if symbol == "gfx950_muon_update_4x4_v1":
        values = {"shards": buffer([0.0] * (batches * 2 * 16), "f32", "read_only"), "output": buffer([SENTINEL_F32] * (batches * 16), "f32", "read_write"), "output_norm": buffer([SENTINEL_F32] * batches, "f32", "read_write")}
        return Built(values, {"output": [0.0] * (batches * 16), "output_norm": [0.0] * batches}, grid=grid, workgroup=workgroup, policy=tolerance, oracle="batched_muon_reference")
    raise ValueError(f"no systems builder for {symbol}")


def low_precision_fixture(symbol: str) -> Built:
    grid, workgroup, batches = [1024, 1, 1], [256, 1, 1], 16
    tolerance = {"absoluteTolerance": 2.0e-3, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 2.0e-3}
    if "gemm" in symbol:
        values = {"lhs": buffer([0] * (batches * 16 * 128), "u8", "read_only"), "rhs": buffer([0] * (batches * 128 * 16), "u8", "read_only")}
        count = batches * 16 * 16
        oracle = "batched_gemm_reference"
    else:
        values = {"query": buffer([0] * (batches * 16 * 128), "u8", "read_only"), "key": buffer([0] * (batches * 16 * 128), "u8", "read_only"), "value": buffer([0] * (batches * 16 * 16), "u8", "read_only")}
        count = batches * 16 * 16
        oracle = "batched_attention_reference"
    initial = [SENTINEL_F32] * (count + 4)
    values["output"] = buffer(initial, "f32", "read_write")
    return Built(values, {"output": [0.0] * count + initial[count:]}, grid=grid, workgroup=workgroup, policy=tolerance, canaries=[("output", count, initial[count:], "f32")], oracle=oracle)


def gpt_fixture(symbol: str) -> Built:
    grid, workgroup = [1024, 1, 1], [256, 1, 1]
    items, hidden, experts = 16, 2880, 128
    attention_count = expert_count = items * 16 * 16
    packed_count = items * 64
    packed = 0 | (1 << 7) | (2 << 14) | (3 << 21)
    all_values: dict[str, dict[str, Any]] = {
        "hidden_f32": buffer([0.0] * (items * hidden), "f32", "read_only"),
        "router_f32": buffer([0.0] * (experts * hidden), "f32", "read_only"),
        "query_bf16": buffer([0] * (items * 16 * 64), "u16", "read_only"),
        "key_transposed_bf16": buffer([0] * (items * 64 * 16), "u16", "read_only"),
        "value_f32": buffer([0.0] * (items * 16 * 16), "f32", "read_only"),
        "sinks_f32": buffer([0.0] * (items * 16), "f32", "read_only"),
        "expert_activation_blocks_fp4": buffer([0] * (items * 4 * 16 * 128), "u8", "read_only"),
        "expert_weight_blocks_fp4": buffer([0] * (experts * 4 * 128 * 16), "u8", "read_only"),
        "activation_scales": buffer([1.0] * (items * 4), "f32", "read_only"),
        "expert_weight_scales": buffer([1.0] * (experts * 4 * 16), "f32", "read_only"),
        "packed_top4": buffer([SENTINEL_U32] * (packed_count + 4), "u32", "read_write"),
        "attention_output": buffer([SENTINEL_F32] * (attention_count + 4), "f32", "read_write"),
        "expert_output": buffer([SENTINEL_F32] * (expert_count + 4), "f32", "read_write"),
    }
    selected_names = {
        "gfx950_gpt_oss_120b_router_v1": ["hidden_f32", "router_f32", "packed_top4"],
        "gfx950_gpt_oss_120b_attention_v1": ["query_bf16", "key_transposed_bf16", "value_f32", "sinks_f32", "attention_output"],
        "gfx950_gpt_oss_120b_expert_v1": ["expert_activation_blocks_fp4", "expert_weight_blocks_fp4", "activation_scales", "expert_weight_scales", "packed_top4", "expert_output"],
    }.get(symbol, ["hidden_f32", "router_f32", "query_bf16", "key_transposed_bf16", "value_f32", "sinks_f32", "expert_activation_blocks_fp4", "expert_weight_blocks_fp4", "activation_scales", "expert_weight_scales", "attention_output", "expert_output", "packed_top4"])
    values = {name: all_values[name] for name in selected_names}
    if symbol == "gfx950_gpt_oss_120b_expert_v1":
        values["packed_top4"] = buffer([packed] * packed_count, "u32", "read_only")
    outputs: dict[str, list[int | float]] = {}
    canaries: list[tuple[str, int, list[int | float], str]] = []
    padding: list[tuple[str, int, list[int | float], str]] = []
    if "attention_output" in values:
        outputs["attention_output"] = [0.0] * attention_count + [SENTINEL_F32] * 4
        canaries.append(("attention_output", attention_count, [SENTINEL_F32] * 4, "f32"))
        padding.extend(("attention_output", item * 256 + 128, [0.0] * 128, "f32") for item in range(items))
    if "expert_output" in values:
        outputs["expert_output"] = [0.0] * expert_count + [SENTINEL_F32] * 4
        canaries.append(("expert_output", expert_count, [SENTINEL_F32] * 4, "f32"))
        padding.extend(("expert_output", item * 256 + 16, [0.0] * 240, "f32") for item in range(items))
    if "packed_top4" in values and symbol != "gfx950_gpt_oss_120b_expert_v1":
        outputs["packed_top4"] = [packed] * packed_count + [SENTINEL_U32] * 4
        canaries.append(("packed_top4", packed_count, [SENTINEL_U32] * 4, "u32"))
    tolerance = {"absoluteTolerance": 2.0e-3, "allowNaN": False, "mode": "f32-tolerance", "relativeTolerance": 2.0e-3}
    return Built(values, outputs, grid=grid, workgroup=workgroup, policy=tolerance, canaries=canaries, padding=sorted(padding, key=lambda item: (item[0], item[1])), oracle="reference_batch")


BUILDERS: dict[str, Callable[[str], Built]] = {
    "fill": fill_fixture,
    "vecadd": vecadd_fixture,
    "wave64_collectives_v1": wave_fixture,
    "lds_publish_read_reduce_i32_v1": workgroup_fixture,
    "flash_attention_general_v1": flash_fixture,
    "gemm_autoresearch_v1": gemm_fixture,
    "moe_grouped_expert_general_v1": grouped_fixture,
    "moe_top2_route_f32_t8_e4_k2_c4_v1": top2_fixture,
    "row_softmax_general_v1": softmax_fixture,
    "tiled_gemm_general_v1": gemm_fixture,
}


def builder_for(symbol: str) -> Callable[[str], Built]:
    if symbol in BUILDERS:
        return BUILDERS[symbol]
    if symbol.startswith("gfx950_gpt_oss_120b_"):
        return gpt_fixture
    if symbol.startswith("gfx950_fp4_") or symbol.startswith("gfx950_fp8_"):
        return low_precision_fixture
    if symbol in {
        "gfx950_kda_decode", "gfx950_kda_chunkwise_prefill", "gfx950_content_sparse_attention",
        "gfx950_deepseek_sparse_attention", "gfx950_compressed_hybrid_attention",
        "gfx950_attnres_aggregate", "gfx950_four_branch_residual", "gfx950_mhc_sinkhorn_mix",
    }:
        return advanced_attention_fixture
    return systems_fixture


def check_ranges(built: Built, arguments: list[dict[str, Any]], abi: dict[str, Any], raw: list[tuple[str, int, list[int | float], str]]) -> list[dict[str, Any]]:
    positions = {item["name"]: index for index, item in enumerate(abi["arguments"])}
    ranges = [byte_range(positions[name], offset, values, element) for name, offset, values, element in raw]
    return sorted(ranges, key=lambda item: (item["argument"], item["offset"]))


def fixture_record(
    manifest: dict[str, Any],
    fixture: dict[str, Any],
    kernel: dict[str, Any],
    production_export: dict[str, Any],
) -> dict[str, Any]:
    symbol = kernel["kernelSymbol"]
    abi = physical_abi(fixture, symbol)
    built = builder_for(symbol)(symbol)
    arguments = make_args(abi, built.values)
    expected_arguments = [expected(argument, built.outputs.get(specification["name"])) for specification, argument in zip(abi["arguments"], arguments)]
    roles = ["scalar" if item["kind"] == "scalar" else ("input" if item["access"] == "read_only" else "output") for item in abi["arguments"]]
    canaries = check_ranges(built, arguments, abi, built.canaries)
    padding = check_ranges(built, arguments, abi, built.padding)
    oracle_suite = next(
        suite for suite in manifest["qualification"]["suites"]
        if suite["gate"] == "cpu-reference" and suite["availability"] == "available"
        and any(fixture["fixtureId"] in coverage["fixtureIds"] for coverage in suite["coverage"])
    )
    oracle_input = {"arguments": arguments, "grid": built.grid, "kernel": symbol, "workgroup": built.workgroup}
    return {
        "argumentRoles": roles,
        "canaries": canaries,
        "checkApplicability": {
            "canaries": {"reason": None if canaries else "kernel requires an exact output extent with no admissible tail", "status": "checked" if canaries else "not-applicable"},
            "padding": {"reason": None if padding else "physical ABI has no logical padding region", "status": "checked" if padding else "not-applicable"},
        },
        "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
        "expectedArguments": expected_arguments,
        "fixtureId": fixture["fixtureId"],
        "kernelSymbol": symbol,
        "numericalPolicy": built.policy,
        "oracle": {"function": built.oracle, "inputSha256": sha256(canonical(oracle_input)), "kind": "existing-rust-cpu-reference-v1"},
        "oracleSuiteId": oracle_suite["suiteId"],
        "padding": padding,
        "physicalAbi": abi,
        "productionExport": bind_production_export(fixture, kernel, production_export),
        "request": {"arguments": arguments, "grid": built.grid, "kernel": symbol, "schema": REQUEST_SCHEMA, "workgroup": built.workgroup},
        "schema": SCHEMA,
        "sourceClosureSha256": fixture["compilerInput"]["sourceClosureSha256"],
        "target": fixture["target"],
    }


def records(
    production_exports: dict[str, dict[str, Any]] | None = None,
) -> dict[str, dict[str, Any]]:
    manifest = json.loads(MANIFEST.read_bytes())
    kernels = {item["fixtureId"]: item for item in manifest["capabilityKernels"]}
    fixtures = {item["fixtureId"]: item for item in manifest["compilerFixtures"]}
    if len(fixtures) != 47 or set(fixtures) != set(kernels):
        raise ValueError("manifest must contain the exact 47-row issue #272 roster")
    production_exports = production_exports or probe_production_exports(
        manifest, fixtures, kernels
    )
    if set(production_exports) != set(fixtures):
        raise ValueError("production export probe must cover all 47 fixtures exactly")
    return {
        identity: fixture_record(
            manifest,
            fixture,
            kernels[identity],
            copy.deepcopy(production_exports[identity]),
        )
        for identity, fixture in sorted(fixtures.items())
    }


def write_regular(path: Path, payload: bytes) -> None:
    if path.exists() and (path.is_symlink() or not path.is_file()):
        raise ValueError(f"refusing to replace non-regular path {path}")
    path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o644)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    options = parser.parse_args(arguments)
    try:
        generated = records()
        expected_paths = {FIXTURE_ROOT / f"{identity}.json" for identity in generated}
        actual_paths = set(FIXTURE_ROOT.glob("*.json")) if FIXTURE_ROOT.exists() else set()
        if actual_paths - expected_paths:
            raise ValueError(f"unexpected fixture records: {sorted(path.name for path in actual_paths - expected_paths)!r}")
        for identity, record in generated.items():
            path = FIXTURE_ROOT / f"{identity}.json"
            payload = json.dumps(record, ensure_ascii=True, indent=2, allow_nan=False).encode("ascii") + b"\n"
            if options.write:
                write_regular(path, payload)
            elif not path.is_file() or path.is_symlink() or path.read_bytes() != payload:
                raise ValueError(f"fixture {identity} is missing or stale; rerun with --write")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"tutorial semantic fixtures: {error}", file=sys.stderr)
        return 1
    print(f"tutorial semantic fixtures: {len(generated)} records are current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
