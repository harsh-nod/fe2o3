#!/usr/bin/env python3
"""Run and validate clean-build direct-link reproducibility comparisons."""

from __future__ import annotations

import argparse
import os
import re
import resource
import signal
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

from common import (
    EvidenceError,
    SUPPORTED_PROCESSORS,
    decode_canonical_text,
    read_regular_file,
    require_digest,
    require_reason,
    require_target,
    sha256_file,
    typed_identity,
)

SCHEMA_VERSION = "1"
RECORD_DOMAIN = "fe2o3-direct-link-repro-v1"
FIELDS = (
    "schema_version",
    "target",
    "first_artifact_sha256",
    "second_artifact_sha256",
    "status",
    "reason",
)
STATUSES = frozenset(("pass", "fail", "skipped", "unavailable"))
MAX_LOG_BYTES = 1024 * 1024
MAX_TIMEOUT_SECONDS = 6 * 60 * 60
_RECORD_ID_RE = re.compile(r"fe2o3-direct-link-repro-v1-sha256-[0-9a-f]{64}\Z")


@dataclass(frozen=True)
class ReproducibilityResult:
    target: str
    first_artifact_sha256: str
    second_artifact_sha256: str
    status: str
    reason: str

    def values(self) -> dict[str, str]:
        return {
            "schema_version": SCHEMA_VERSION,
            "target": self.target,
            "first_artifact_sha256": self.first_artifact_sha256,
            "second_artifact_sha256": self.second_artifact_sha256,
            "status": self.status,
            "reason": self.reason,
        }

    def preimage(self) -> bytes:
        values = self.values()
        return (
            "\n".join(f"{field}\t{values[field]}" for field in FIELDS) + "\n"
        ).encode("ascii")

    def identity(self) -> str:
        return typed_identity(RECORD_DOMAIN, self.preimage())

    def canonical_bytes(self) -> bytes:
        return self.preimage() + f"record_identity\t{self.identity()}\n".encode("ascii")


def validate_result(result: ReproducibilityResult) -> None:
    require_target(result.target)
    if result.status not in STATUSES:
        raise EvidenceError("reproducibility status is unknown")
    for name, digest in (
        ("first_artifact_sha256", result.first_artifact_sha256),
        ("second_artifact_sha256", result.second_artifact_sha256),
    ):
        if digest != "none":
            require_digest(digest, name)
    if result.status == "pass":
        if result.reason != "-":
            raise EvidenceError("passing reproducibility result must use reason '-'")
        if result.first_artifact_sha256 == "none":
            raise EvidenceError(
                "passing reproducibility result must identify both artifacts"
            )
        if result.first_artifact_sha256 != result.second_artifact_sha256:
            raise EvidenceError(
                "passing reproducibility result contains different digests"
            )
    else:
        require_reason(result.reason)


def parse_result(path: Path) -> ReproducibilityResult:
    data = read_regular_file(path)
    lines = decode_canonical_text(data, "reproducibility record")
    values: dict[str, str] = {}
    record_identity: str | None = None
    for line_number, line in enumerate(lines, start=1):
        fields = line.split("\t")
        if len(fields) != 2:
            raise EvidenceError(
                f"reproducibility line {line_number} must contain exactly two fields"
            )
        key, value = fields
        if key == "record_identity":
            if record_identity is not None:
                raise EvidenceError("duplicate record_identity")
            record_identity = value
        elif key in FIELDS:
            if key in values:
                raise EvidenceError(f"duplicate reproducibility field: {key}")
            values[key] = value
        else:
            raise EvidenceError(f"unknown reproducibility field: {key}")
    missing = set(FIELDS) - set(values)
    if missing:
        raise EvidenceError(f"missing reproducibility fields: {sorted(missing)}")
    if values["schema_version"] != SCHEMA_VERSION:
        raise EvidenceError("reproducibility schema_version must be exactly 1")
    if record_identity is None or _RECORD_ID_RE.fullmatch(record_identity) is None:
        raise EvidenceError("reproducibility record_identity is missing or malformed")
    result = ReproducibilityResult(
        target=values["target"],
        first_artifact_sha256=values["first_artifact_sha256"],
        second_artifact_sha256=values["second_artifact_sha256"],
        status=values["status"],
        reason=values["reason"],
    )
    validate_result(result)
    if result.identity() != record_identity:
        raise EvidenceError("reproducibility record_identity mismatch")
    if data != result.canonical_bytes():
        raise EvidenceError(
            "reproducibility record uses noncanonical field order or encoding"
        )
    return result


def emit(result: ReproducibilityResult) -> int:
    validate_result(result)
    sys.stdout.buffer.write(result.canonical_bytes())
    return 0 if result.status == "pass" else 1


def validate_artifact_path(value: str) -> Path:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise argparse.ArgumentTypeError(
            "artifact path must be a nonempty relative path without traversal"
        )
    return Path(*path.parts)


def limited_child() -> None:
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_LOG_BYTES, MAX_LOG_BYTES))


def sanitized_environment(target: str, source_date_epoch: int) -> dict[str, str]:
    environment = {
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
        "FE2O3_TARGET": target,
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
    }
    for name in ("HOME", "CARGO_HOME", "RUSTUP_HOME"):
        value = os.environ.get(name)
        if value:
            environment[name] = value
    return environment


def expanded_command(
    command: list[str], build_dir: Path, source_dir: Path, target: str
) -> list[str]:
    replacements = {
        "{build_dir}": str(build_dir),
        "{source_dir}": str(source_dir),
        "{target}": target,
    }
    expanded: list[str] = []
    for argument in command:
        for token, replacement in replacements.items():
            argument = argument.replace(token, replacement)
        expanded.append(argument)
    return expanded


def run_once(
    command: list[str],
    build_dir: Path,
    source_dir: Path,
    target: str,
    timeout: int,
    source_date_epoch: int,
) -> tuple[str, str]:
    log_path = build_dir / "build.log"
    argv = expanded_command(command, build_dir, source_dir, target)
    try:
        with log_path.open("wb") as log:
            process = subprocess.Popen(
                argv,
                cwd=build_dir,
                env=sanitized_environment(target, source_date_epoch),
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                preexec_fn=limited_child,
                start_new_session=True,
            )
            try:
                return_code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                return "fail", "build-timeout"
    except (FileNotFoundError, PermissionError):
        return "unavailable", "build-command-unavailable"
    if log_path.stat().st_size > MAX_LOG_BYTES:
        return "fail", "build-log-limit"
    if return_code == -signal.SIGXFSZ:
        return "fail", "build-log-limit"
    if return_code != 0:
        return "fail", "build-command-failed"
    return "pass", "-"


def run_comparison(args: argparse.Namespace) -> int:
    target = require_target(args.target)
    try:
        source_dir = args.source_dir.resolve(strict=True)
        work_root = args.work_root.resolve(strict=True)
    except OSError as error:
        raise EvidenceError(f"cannot resolve clean-build directory: {error}") from error
    if not source_dir.is_dir() or not work_root.is_dir():
        raise EvidenceError("source-dir and work-root must be directories")
    command = list(args.build_command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise EvidenceError("run requires a build command after '--'")

    with tempfile.TemporaryDirectory(
        prefix=f"fe2o3-repro-{target.split(':', 1)[0]}-", dir=work_root
    ) as temporary:
        root = Path(temporary)
        build_dirs = (root / "first", root / "second")
        digests: list[str] = []
        for build_dir in build_dirs:
            build_dir.mkdir(mode=0o700)
            status, reason = run_once(
                command,
                build_dir,
                source_dir,
                target,
                args.timeout,
                args.source_date_epoch,
            )
            if status != "pass":
                while len(digests) < 2:
                    digests.append("none")
                return emit(
                    ReproducibilityResult(
                        target, digests[0], digests[1], status, reason
                    )
                )
            try:
                digest = sha256_file(build_dir / args.artifact)
            except EvidenceError:
                while len(digests) < 2:
                    digests.append("none")
                return emit(
                    ReproducibilityResult(
                        target,
                        digests[0],
                        digests[1],
                        "fail",
                        "artifact-unmeasurable",
                    )
                )
            digests.append(digest)

        status = "pass" if digests[0] == digests[1] else "fail"
        reason = "-" if status == "pass" else "artifact-mismatch"
        return emit(
            ReproducibilityResult(target, digests[0], digests[1], status, reason)
        )


def compare_existing(args: argparse.Namespace) -> int:
    target = require_target(args.target)
    first = sha256_file(args.first)
    second = sha256_file(args.second)
    status = "pass" if first == second else "fail"
    reason = "-" if status == "pass" else "artifact-mismatch"
    return emit(ReproducibilityResult(target, first, second, status, reason))


def validate_command(args: argparse.Namespace) -> int:
    result = parse_result(args.record)
    if args.expect_target is not None and result.target != args.expect_target:
        raise EvidenceError(
            f"reproducibility target mismatch: expected {args.expect_target}, "
            f"found {result.target}"
        )
    print(
        f"direct-link reproducibility record is canonical: {result.identity()} "
        f"status={result.status}"
    )
    return 0


def validate_matrix(args: argparse.Namespace) -> int:
    records: dict[str, ReproducibilityResult] = {}
    for path in args.record:
        result = parse_result(path)
        if ":" in result.target or result.target not in SUPPORTED_PROCESSORS:
            raise EvidenceError(
                f"matrix record must use an exact base target: {result.target}"
            )
        if result.target in records:
            raise EvidenceError(f"duplicate matrix target: {result.target}")
        records[result.target] = result
    if set(records) != set(SUPPORTED_PROCESSORS):
        missing = sorted(set(SUPPORTED_PROCESSORS) - set(records))
        extra = sorted(set(records) - set(SUPPORTED_PROCESSORS))
        raise EvidenceError(
            f"wrong reproducibility target matrix; missing={missing}, extra={extra}"
        )
    for target in sorted(records):
        result = records[target]
        print(f"{target}\t{result.status}\t{result.reason}\t{result.identity()}")
    return 0 if all(result.status == "pass" for result in records.values()) else 1


def bounded_timeout(value: str) -> int:
    try:
        timeout = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be an integer") from error
    if not 1 <= timeout <= MAX_TIMEOUT_SECONDS:
        raise argparse.ArgumentTypeError(
            f"timeout must be between 1 and {MAX_TIMEOUT_SECONDS} seconds"
        )
    return timeout


def nonnegative_epoch(value: str) -> int:
    try:
        epoch = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "source-date-epoch must be an integer"
        ) from error
    if epoch < 0:
        raise argparse.ArgumentTypeError("source-date-epoch must not be negative")
    return epoch


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run", help="run two isolated clean builds")
    run_parser.add_argument("--target", required=True)
    run_parser.add_argument("--artifact", required=True, type=validate_artifact_path)
    run_parser.add_argument("--source-dir", required=True, type=Path)
    run_parser.add_argument("--work-root", required=True, type=Path)
    run_parser.add_argument("--timeout", type=bounded_timeout, default=1800)
    run_parser.add_argument("--source-date-epoch", type=nonnegative_epoch, default=0)
    run_parser.add_argument("build_command", nargs=argparse.REMAINDER)
    run_parser.set_defaults(handler=run_comparison)

    compare_parser = subparsers.add_parser(
        "compare", help="compare two existing direct-link artifacts"
    )
    compare_parser.add_argument("--target", required=True)
    compare_parser.add_argument("--first", required=True, type=Path)
    compare_parser.add_argument("--second", required=True, type=Path)
    compare_parser.set_defaults(handler=compare_existing)

    validate_parser = subparsers.add_parser("validate", help="validate one result")
    validate_parser.add_argument("record", type=Path)
    validate_parser.add_argument("--expect-target")
    validate_parser.set_defaults(handler=validate_command)

    matrix_parser = subparsers.add_parser(
        "matrix", help="validate the required gfx1151/gfx942/gfx950 result set"
    )
    matrix_parser.add_argument("record", nargs="+", type=Path)
    matrix_parser.set_defaults(handler=validate_matrix)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        return args.handler(args)
    except EvidenceError as error:
        print(f"direct-link reproducibility: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
