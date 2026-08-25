#!/usr/bin/env python3
"""Executable, bounded fe2o3-vs-HIP/CPU differential conformance harness."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
FIXTURE_DIR = REPO_ROOT / "tests/fixtures/differential"
sys.path.insert(0, str(SCRIPT_DIR))

from compare import ComparisonError, compare_cases, parse_results  # noqa: E402

SCHEMA = "fe2o3.differential.conformance.v1"
MAX_COMMAND_OUTPUT = 64 * 1024
MAX_GPU_IDENTITY_OUTPUT = 256 * 1024
DEFAULT_ARTIFACT_MAX = 512 * 1024
MIN_ARTIFACT_MAX = 16 * 1024
MAX_ARTIFACT_MAX = 1024 * 1024
TARGET_RE = re.compile(r"^gfx[0-9a-f]+(?::[A-Za-z0-9_+\-]+)*$")
HOST_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.@-]*$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


class HarnessError(RuntimeError):
    pass


@dataclass(frozen=True)
class Settings:
    hardware: bool
    target: str | None
    timeout_seconds: int
    artifact_path: Path
    artifact_max_bytes: int
    expected_commit: str | None


@dataclass
class StreamCapture:
    data: bytes
    byte_count: int
    sha256: str
    truncated: bool


@dataclass
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: StreamCapture
    stderr: StreamCapture
    timed_out: bool

    @property
    def succeeded(self) -> bool:
        return self.returncode == 0 and not self.timed_out


class _Drain(threading.Thread):
    def __init__(self, stream: object, limit: int):
        super().__init__(daemon=True)
        self.stream = stream
        self.limit = limit
        self.buffer = bytearray()
        self.byte_count = 0
        self.digest = hashlib.sha256()

    def run(self) -> None:
        while True:
            chunk = self.stream.read(8192)  # type: ignore[attr-defined]
            if not chunk:
                return
            self.byte_count += len(chunk)
            self.digest.update(chunk)
            remaining = self.limit - len(self.buffer)
            if remaining > 0:
                self.buffer.extend(chunk[:remaining])

    def capture(self) -> StreamCapture:
        return StreamCapture(
            data=bytes(self.buffer),
            byte_count=self.byte_count,
            sha256=self.digest.hexdigest(),
            truncated=self.byte_count > len(self.buffer),
        )


def run_command(
    argv: Sequence[str],
    *,
    cwd: Path,
    env: Mapping[str, str] | None = None,
    timeout_seconds: float,
    output_limit: int = MAX_COMMAND_OUTPUT,
) -> CommandResult:
    if not argv or any("\x00" in argument for argument in argv):
        raise HarnessError("invalid empty command or NUL-containing argument")
    try:
        process = subprocess.Popen(
            list(argv),
            cwd=cwd,
            env=dict(env) if env is not None else None,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError:
        empty = StreamCapture(b"", 0, hashlib.sha256(b"").hexdigest(), False)
        return CommandResult(tuple(argv), 127, empty, empty, False)

    assert process.stdout is not None and process.stderr is not None
    stdout_drain = _Drain(process.stdout, output_limit)
    stderr_drain = _Drain(process.stderr, output_limit)
    stdout_drain.start()
    stderr_drain.start()
    timed_out = False
    try:
        returncode = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        returncode = process.wait()
    stdout_drain.join()
    stderr_drain.join()
    process.stdout.close()
    process.stderr.close()
    return CommandResult(
        tuple(argv),
        returncode,
        stdout_drain.capture(),
        stderr_drain.capture(),
        timed_out,
    )


def validate_environment(environ: Mapping[str, str]) -> Settings:
    opt_in = environ.get("FE2O3_ALLOW_GPU_SMOKE", "")
    if opt_in not in {"", "0", "1"}:
        raise HarnessError("FE2O3_ALLOW_GPU_SMOKE must be unset, 0, or 1")
    hardware = opt_in == "1"
    target = environ.get("FE2O3_TARGET") or None
    if target is not None and TARGET_RE.fullmatch(target) is None:
        raise HarnessError(f"malformed FE2O3_TARGET: {target!r}")
    if hardware and target is None:
        raise HarnessError("hardware differential runs require explicit FE2O3_TARGET")

    timeout_text = environ.get("FE2O3_DIFFERENTIAL_TIMEOUT_SECONDS", "900")
    if not timeout_text.isascii() or not timeout_text.isdecimal():
        raise HarnessError("FE2O3_DIFFERENTIAL_TIMEOUT_SECONDS must be an integer")
    timeout_seconds = int(timeout_text)
    if not 1 <= timeout_seconds <= 3600:
        raise HarnessError("FE2O3_DIFFERENTIAL_TIMEOUT_SECONDS must be in 1..=3600")

    maximum_text = environ.get(
        "FE2O3_DIFFERENTIAL_ARTIFACT_MAX_BYTES", str(DEFAULT_ARTIFACT_MAX)
    )
    if not maximum_text.isascii() or not maximum_text.isdecimal():
        raise HarnessError("FE2O3_DIFFERENTIAL_ARTIFACT_MAX_BYTES must be an integer")
    artifact_max_bytes = int(maximum_text)
    if not MIN_ARTIFACT_MAX <= artifact_max_bytes <= MAX_ARTIFACT_MAX:
        raise HarnessError(
            "FE2O3_DIFFERENTIAL_ARTIFACT_MAX_BYTES must be between "
            f"{MIN_ARTIFACT_MAX} and {MAX_ARTIFACT_MAX}"
        )

    artifact_text = environ.get("FE2O3_DIFFERENTIAL_ARTIFACT")
    artifact_path = (
        Path(artifact_text).expanduser()
        if artifact_text
        else REPO_ROOT / "target/differential/conformance-v1.json"
    )
    expected_commit = environ.get("FE2O3_EXPECT_COMMIT") or None
    if expected_commit is not None and COMMIT_RE.fullmatch(expected_commit) is None:
        raise HarnessError(
            "FE2O3_EXPECT_COMMIT must be 40 lowercase hexadecimal digits"
        )
    return Settings(
        hardware=hardware,
        target=target,
        timeout_seconds=timeout_seconds,
        artifact_path=artifact_path,
        artifact_max_bytes=artifact_max_bytes,
        expected_commit=expected_commit,
    )


def _capture_record(capture: StreamCapture, include_text: bool) -> dict[str, object]:
    record: dict[str, object] = {
        "bytes": capture.byte_count,
        "sha256": capture.sha256,
        "truncated": capture.truncated,
    }
    if include_text or capture.truncated:
        record["captured"] = capture.data.decode("utf-8", "backslashreplace")
    return record


def command_phase(index: int, name: str, result: CommandResult) -> dict[str, object]:
    status = "PASS" if result.succeeded else "FAIL"
    include_text = not result.succeeded
    return {
        "argv": list(result.argv),
        "index": index,
        "name": name,
        "returncode": result.returncode,
        "status": status,
        "stderr": _capture_record(result.stderr, include_text),
        "stdout": _capture_record(result.stdout, include_text),
        "timed_out": result.timed_out,
    }


def skip_phase(index: int, name: str, reason: str) -> dict[str, object]:
    return {"index": index, "name": name, "reason": reason, "status": "SKIP"}


def _identity(argv: Sequence[str], cwd: Path) -> dict[str, object]:
    result = run_command(argv, cwd=cwd, timeout_seconds=30, output_limit=32 * 1024)
    combined = result.stdout.data + result.stderr.data
    return {
        "argv": list(argv),
        "bytes": result.stdout.byte_count + result.stderr.byte_count,
        "returncode": result.returncode,
        "sha256": hashlib.sha256(combined).hexdigest(),
        "text": combined.decode("utf-8", "backslashreplace"),
        "truncated": result.stdout.truncated or result.stderr.truncated,
    }


def collect_identities(settings: Settings) -> dict[str, object]:
    git = _identity(["git", "rev-parse", "HEAD"], REPO_ROOT)
    commit = str(git["text"]).strip()
    if git["returncode"] != 0 or COMMIT_RE.fullmatch(commit) is None:
        raise HarnessError("could not resolve exact Git commit")
    if settings.expected_commit is not None and commit != settings.expected_commit:
        raise HarnessError(
            f"wrong Git commit: expected {settings.expected_commit}, found {commit}"
        )
    dirty = _identity(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], REPO_ROOT
    )
    if settings.expected_commit is not None and str(dirty["text"]).strip():
        raise HarnessError("commit-pinned differential runs require a clean checkout")
    identities: dict[str, object] = {
        "cargo": _identity(["cargo", "-Vv"], REPO_ROOT),
        "git_commit": commit,
        "git_status": dirty,
        "python": {
            "executable": sys.executable,
            "version": sys.version,
        },
        "rustc": _identity(["rustc", "-Vv"], REPO_ROOT),
        "target": settings.target or "not-configured",
    }
    rocm_version = None
    for candidate in [
        Path(os.environ.get("ROCM_PATH", "/opt/rocm")) / ".info/version",
        Path("/opt/rocm/.info/version-dev"),
    ]:
        try:
            data = candidate.read_bytes()
        except OSError:
            continue
        rocm_version = {
            "bytes": len(data),
            "path": str(candidate),
            "sha256": hashlib.sha256(data).hexdigest(),
            "text": data[:4096].decode("utf-8", "backslashreplace"),
            "truncated": len(data) > 4096,
        }
        break
    identities["rocm_version"] = rocm_version or "unavailable"
    return identities


def find_executable(explicit: str | None, candidates: Sequence[str]) -> str | None:
    if explicit:
        if "\x00" in explicit or any(character.isspace() for character in explicit):
            raise HarnessError("compiler environment variable must name one executable")
        return shutil.which(explicit)
    for candidate in candidates:
        resolved = shutil.which(candidate)
        if resolved is not None:
            return resolved
    return None


def _checked_phase(
    phases: list[dict[str, object]],
    name: str,
    argv: Sequence[str],
    *,
    cwd: Path,
    env: Mapping[str, str] | None,
    timeout_seconds: float,
    output_limit: int = MAX_COMMAND_OUTPUT,
) -> CommandResult:
    result = run_command(
        argv,
        cwd=cwd,
        env=env,
        timeout_seconds=timeout_seconds,
        output_limit=output_limit,
    )
    phases.append(command_phase(len(phases) + 1, name, result))
    if result.stdout.truncated or result.stderr.truncated:
        raise HarnessError(f"phase {name} exceeded bounded command output")
    if not result.succeeded:
        detail = result.stderr.data.decode("utf-8", "backslashreplace").strip()
        raise HarnessError(f"phase {name} failed: {detail or result.returncode}")
    return result


def _reference(
    settings: Settings,
    phases: list[dict[str, object]],
    work_root: Path,
) -> tuple[bytes, dict[str, object]]:
    use_hip = settings.hardware
    compiler: str | None = None
    if use_hip:
        rocm_path = Path(os.environ.get("ROCM_PATH", "/opt/rocm"))
        explicit_hip = os.environ.get("HIPCXX")
        compiler = find_executable(
            explicit_hip,
            [str(rocm_path / "bin/hipcc"), "hipcc"],
        )
        if explicit_hip and compiler is None:
            raise HarnessError(f"HIPCXX executable is unavailable: {explicit_hip}")
    if compiler is None:
        use_hip = False
        explicit_cxx = os.environ.get("CXX")
        compiler = find_executable(explicit_cxx, ["c++", "clang++", "g++"])
        if explicit_cxx and compiler is None:
            raise HarnessError(f"CXX executable is unavailable: {explicit_cxx}")
    if compiler is None:
        raise HarnessError(
            "no C++ compiler is available for the independent CPU oracle"
        )

    executable = work_root / "reference"
    argv = [compiler, "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror"]
    if use_hip:
        assert settings.target is not None
        argv.extend(
            [
                "-DFE2O3_REFERENCE_HIP=1",
                f"--offload-arch={settings.target.split(':', 1)[0]}",
            ]
        )
    argv.extend([str(SCRIPT_DIR / "reference.cpp"), "-o", str(executable)])
    _checked_phase(
        phases,
        "reference-build-hip" if use_hip else "reference-build-cpu",
        argv,
        cwd=REPO_ROOT,
        env=os.environ,
        timeout_seconds=settings.timeout_seconds,
    )
    result = _checked_phase(
        phases,
        "reference-run-hip" if use_hip else "reference-run-cpu",
        [str(executable)],
        cwd=REPO_ROOT,
        env=os.environ,
        timeout_seconds=settings.timeout_seconds,
    )
    parse_results(result.stdout.data)
    return result.stdout.data, {
        "compiler": _identity([compiler, "--version"], REPO_ROOT),
        "mode": "hip-gpu" if use_hip else "cpu",
        "source_sha256": hashlib.sha256(
            (SCRIPT_DIR / "reference.cpp").read_bytes()
        ).hexdigest(),
    }


def _alias_rejection(settings: Settings, phases: list[dict[str, object]]) -> None:
    _checked_phase(
        phases,
        "alias-rejection",
        [
            "cargo",
            "test",
            "--locked",
            "--quiet",
            "-p",
            "fe2o3-host",
            "argument_alias::tests::partial_write_overlap_conflicts_but_touching_boundaries_do_not",
            "--",
            "--exact",
        ],
        cwd=REPO_ROOT,
        env=os.environ,
        timeout_seconds=settings.timeout_seconds,
    )


def _gpu_identity(
    settings: Settings, phases: list[dict[str, object]]
) -> dict[str, object]:
    device = Path("/dev/kfd") if Path("/dev/kfd").exists() else Path("/dev/dxg")
    if not device.exists():
        raise HarnessError("hardware opt-in requires /dev/kfd or /dev/dxg")
    if not os.access(device, os.R_OK | os.W_OK):
        raise HarnessError(f"GPU device is not readable and writable: {device}")
    if device == Path("/dev/dxg") and os.environ.get("HSA_ENABLE_DXG_DETECTION") != "1":
        raise HarnessError("WSL hardware runs require HSA_ENABLE_DXG_DETECTION=1")
    rocminfo = find_executable(None, ["rocminfo", "/opt/rocm/bin/rocminfo"])
    if rocminfo is None:
        raise HarnessError("rocminfo is required for hardware identity")
    result = _checked_phase(
        phases,
        "gpu-identity",
        [rocminfo],
        cwd=REPO_ROOT,
        env=os.environ,
        timeout_seconds=60,
        output_limit=MAX_GPU_IDENTITY_OUTPUT,
    )
    text = result.stdout.data.decode("utf-8", "backslashreplace")
    assert settings.target is not None
    processor = settings.target.split(":", 1)[0]
    if (
        re.search(rf"^\s*Name:\s+{re.escape(processor)}\s*$", text, re.MULTILINE)
        is None
    ):
        raise HarnessError(
            f"configured target {processor} was not reported by rocminfo"
        )
    selected = [
        line.strip()
        for line in text.splitlines()
        if any(
            marker in line
            for marker in ["Name:", "Marketing Name:", "Uuid:", "Vendor Name:"]
        )
    ][:128]
    return {
        "device_node": str(device),
        "rocminfo_bytes": result.stdout.byte_count,
        "rocminfo_identity_lines": selected,
        "rocminfo_sha256": result.stdout.sha256,
    }


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _prepare_fixture(kernel: str, work_root: Path) -> Path:
    package_root = work_root / f"fixture-{kernel}"
    source_root = package_root / "src"
    source_root.mkdir(parents=True)
    shutil.copyfile(FIXTURE_DIR / f"{kernel}.rs", source_root / "main.rs")
    shutil.copyfile(FIXTURE_DIR / "common.rs", source_root / "common.rs")
    manifest = f"""[package]
name = "fe2o3-differential-{kernel}"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
fe2o3-core = {{ path = {_toml_string(str(REPO_ROOT / "crates/fe2o3-core"))} }}
fe2o3-device = {{ path = {_toml_string(str(REPO_ROOT / "crates/fe2o3-device"))} }}
fe2o3-host = {{ path = {_toml_string(str(REPO_ROOT / "crates/fe2o3-host"))}, features = ["qualification-oracles-test-only"] }}

[workspace]
"""
    (package_root / "Cargo.toml").write_text(manifest, encoding="ascii")
    return package_root


def _fe2o3_results(
    settings: Settings,
    phases: list[dict[str, object]],
    work_root: Path,
) -> bytes:
    _checked_phase(
        phases,
        "fe2o3-tools-build",
        [
            "cargo",
            "build",
            "--locked",
            "--quiet",
            "-p",
            "cargo-fe2o3",
            "-p",
            "rustc-codegen-fe2o3",
        ],
        cwd=REPO_ROOT,
        env=os.environ,
        timeout_seconds=settings.timeout_seconds,
    )
    cargo_fe2o3 = REPO_ROOT / "target/debug/cargo-fe2o3"
    backend = REPO_ROOT / "target/debug/librustc_codegen_fe2o3.so"
    if not cargo_fe2o3.is_file() or not backend.is_file():
        raise HarnessError("fe2o3 tool build did not produce required executables")

    outputs = bytearray()
    for kernel in ["fill", "vecadd", "affine"]:
        package = _prepare_fixture(kernel, work_root)
        manifest = package / "Cargo.toml"
        _checked_phase(
            phases,
            f"fe2o3-{kernel}-lock",
            [
                "cargo",
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                str(manifest),
            ],
            cwd=package,
            env=os.environ,
            timeout_seconds=settings.timeout_seconds,
        )
        command_env = dict(os.environ)
        command_env.update(
            {
                "FE2O3_BACKEND": str(backend),
                "FE2O3_CODEGEN_PIPELINE": "legacy-v1",
                "FE2O3_TARGET": settings.target or "",
                "RUSTFLAGS": "-Zalways-encode-mir",
            }
        )
        result = _checked_phase(
            phases,
            f"fe2o3-{kernel}-run",
            [
                str(cargo_fe2o3),
                "run",
                "--locked",
                "--quiet",
                "--manifest-path",
                str(manifest),
            ],
            cwd=package,
            env=command_env,
            timeout_seconds=settings.timeout_seconds,
        )
        outputs.extend(result.stdout.data)
    return bytes(outputs)


def canonical_artifact_bytes(artifact: Mapping[str, object]) -> bytes:
    return (
        json.dumps(artifact, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def write_bounded_artifact(
    path: Path, artifact: Mapping[str, object], maximum: int
) -> tuple[int, bool]:
    data = canonical_artifact_bytes(artifact)
    fallback = False
    if len(data) > maximum:
        fallback = True
        data = canonical_artifact_bytes(
            {
                "authority": {"launch": False, "proof": False},
                "error": "full artifact exceeded configured byte bound",
                "full_artifact_bytes": len(data),
                "full_artifact_sha256": hashlib.sha256(data).hexdigest(),
                "schema": SCHEMA,
                "status": "FAIL",
            }
        )
    if len(data) > maximum:
        raise HarnessError("artifact bound is too small for the failure envelope")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(data)
    os.replace(temporary, path)
    return len(data), fallback


def _reference_case_summaries(reference: bytes) -> list[dict[str, object]]:
    summaries: list[dict[str, object]] = []
    for case in sorted(parse_results(reference).values(), key=lambda value: value.key):
        summaries.append(
            {
                "kernel": case.kernel,
                "length": case.length,
                "record_sha256": hashlib.sha256(
                    case.canonical.encode("ascii")
                ).hexdigest(),
                "seed": f"{case.seed:016x}",
                "status": "ORACLE",
            }
        )
    return summaries


def _source_hashes() -> dict[str, str]:
    paths = [
        SCRIPT_DIR / "reference.cpp",
        FIXTURE_DIR / "common.rs",
        FIXTURE_DIR / "fill.rs",
        FIXTURE_DIR / "vecadd.rs",
        FIXTURE_DIR / "affine.rs",
    ]
    return {
        str(path.relative_to(REPO_ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }


def run(settings: Settings) -> int:
    work_root = REPO_ROOT / "target/differential/work-v1"
    if work_root.is_symlink():
        raise HarnessError(f"refusing symlinked differential work root: {work_root}")
    shutil.rmtree(work_root, ignore_errors=True)
    work_root.mkdir(parents=True)
    phases: list[dict[str, object]] = []
    artifact: dict[str, object] = {
        "authority": {"launch": False, "proof": False},
        "cases": [],
        "hardware_enabled": settings.hardware,
        "phases": phases,
        "policy": {
            "canaries": "exact-32-bit",
            "f32": "abs=1e-6;rel=1e-6;nan=both-nan;infinity=exact-sign",
            "bits32": "exact-bits",
        },
        "schema": SCHEMA,
        "source_sha256": _source_hashes(),
        "status": "FAIL",
    }
    exit_code = 1
    try:
        artifact["identities"] = collect_identities(settings)
        if settings.hardware:
            artifact["device"] = _gpu_identity(settings, phases)
        reference, reference_identity = _reference(settings, phases, work_root)
        artifact["reference"] = reference_identity
        artifact["cases"] = _reference_case_summaries(reference)
        _alias_rejection(settings, phases)
        if not settings.hardware:
            phases.append(
                skip_phase(
                    len(phases) + 1,
                    "fe2o3-hardware",
                    "FE2O3_ALLOW_GPU_SMOKE is not 1",
                )
            )
        else:
            actual = _fe2o3_results(settings, phases, work_root)
            artifact["cases"] = compare_cases(reference, actual)
            phases.append(
                {
                    "index": len(phases) + 1,
                    "name": "differential-compare",
                    "status": "PASS",
                }
            )
        artifact["status"] = "PASS"
        exit_code = 0
    except (ComparisonError, HarnessError, OSError) as error:
        artifact["error"] = str(error)
        artifact["status"] = "FAIL"

    byte_count, fallback = write_bounded_artifact(
        settings.artifact_path, artifact, settings.artifact_max_bytes
    )
    if fallback:
        exit_code = 1
    print(
        f"differential status={artifact['status']} artifact={settings.artifact_path} "
        f"bytes={byte_count} hardware={'enabled' if settings.hardware else 'skipped'}"
    )
    if artifact["status"] != "PASS":
        print(
            f"differential error: {artifact.get('error', 'unknown failure')}",
            file=sys.stderr,
        )
    return exit_code


def prepare_remote(args: argparse.Namespace) -> int:
    if HOST_RE.fullmatch(args.host) is None:
        raise HarnessError(f"malformed SSH host: {args.host!r}")
    if args.target not in {"gfx942", "gfx950"}:
        raise HarnessError("remote preparation target must be gfx942 or gfx950")
    if not args.checkout or "\x00" in args.checkout or "\n" in args.checkout:
        raise HarnessError("malformed remote checkout path")
    commit = args.commit
    if commit is None:
        identity = _identity(["git", "rev-parse", "HEAD"], REPO_ROOT)
        commit = str(identity["text"]).strip()
    if COMMIT_RE.fullmatch(commit or "") is None:
        raise HarnessError("remote commit must be 40 lowercase hexadecimal digits")
    remote = " && ".join(
        [
            f"cd -- {shlex.quote(args.checkout)}",
            f'test "$(git rev-parse HEAD)" = {shlex.quote(commit)}',
            " ".join(
                [
                    f"FE2O3_EXPECT_COMMIT={shlex.quote(commit)}",
                    "FE2O3_ALLOW_GPU_SMOKE=1",
                    f"FE2O3_TARGET={shlex.quote(args.target)}",
                    "scripts/differential/run.sh",
                ]
            ),
        ]
    )
    print(shlex.join(["ssh", "--", args.host, "bash", "-lc", remote]))
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run bounded fe2o3 differential conformance"
    )
    subparsers = parser.add_subparsers(dest="command")
    remote = subparsers.add_parser(
        "prepare-remote", help="print a credential-free gfx942/gfx950 SSH invocation"
    )
    remote.add_argument("--host", required=True)
    remote.add_argument("--target", required=True)
    remote.add_argument("--checkout", required=True)
    remote.add_argument("--commit")
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare-remote":
            return prepare_remote(args)
        if args.command is not None:
            raise HarnessError(f"unknown command: {args.command}")
        return run(validate_environment(os.environ))
    except HarnessError as error:
        print(f"differential configuration error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
