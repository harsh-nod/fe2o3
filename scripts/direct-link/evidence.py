#!/usr/bin/env python3
"""Collect and validate canonical direct-LLVM-link release evidence."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

from common import (
    EvidenceError,
    decode_canonical_text,
    read_regular_file,
    require_bounded_text,
    require_commit,
    require_digest,
    require_reason,
    require_target,
    sha256_file,
    typed_identity,
)

SCHEMA_VERSION = "1"
RECORD_DOMAIN = "fe2o3-direct-link-evidence-v1"
MAX_RECORD_IDENTITY_BYTES = 128
SCALAR_FIELDS = (
    "schema_version",
    "git_commit",
    "target",
    "worker_executable_sha256",
    "worker_build_id",
    "llvm_build_identity",
    "request_identity",
    "artifact_identity",
    "hardware_execution_identity",
    "release_gate",
)
REQUIRED_SUITES = (
    ("clean-build-reproducibility", "reproducibility"),
    ("compile", "compile"),
    ("direct-llvm-link", "worker"),
    ("hardware-execution", "hardware"),
    ("static-checks", "static"),
)
STATUSES = frozenset(("pass", "fail", "skipped", "unavailable"))
_WORKER_BUILD_RE = re.compile(r"fe2o3-worker-v1-sha256-[0-9a-f]{64}\Z")
_HARDWARE_ID_RE = re.compile(r"fe2o3-hardware-v1-sha256-[0-9a-f]{64}\Z")
_RECORD_ID_RE = re.compile(r"fe2o3-direct-link-evidence-v1-sha256-[0-9a-f]{64}\Z")


@dataclass(frozen=True)
class SuiteOutcome:
    name: str
    evidence_class: str
    status: str
    reason: str


@dataclass(frozen=True)
class EvidenceRecord:
    scalars: dict[str, str]
    suites: dict[str, SuiteOutcome]

    def preimage(self) -> bytes:
        lines = [f"{field}\t{self.scalars[field]}" for field in SCALAR_FIELDS]
        for name, _ in REQUIRED_SUITES:
            suite = self.suites[name]
            lines.append(
                "\t".join(
                    (
                        "suite",
                        suite.name,
                        suite.evidence_class,
                        suite.status,
                        suite.reason,
                    )
                )
            )
        return ("\n".join(lines) + "\n").encode("ascii")

    def identity(self) -> str:
        return typed_identity(RECORD_DOMAIN, self.preimage())

    def canonical_bytes(self) -> bytes:
        return self.preimage() + f"record_identity\t{self.identity()}\n".encode("ascii")


def derive_release_gate(suites: dict[str, SuiteOutcome]) -> str:
    statuses = {suite.status for suite in suites.values()}
    if "fail" in statuses:
        return "fail"
    if statuses & {"skipped", "unavailable"}:
        return "blocked"
    return "pass"


def validate_record(record: EvidenceRecord) -> None:
    if set(record.scalars) != set(SCALAR_FIELDS):
        missing = sorted(set(SCALAR_FIELDS) - set(record.scalars))
        extra = sorted(set(record.scalars) - set(SCALAR_FIELDS))
        raise EvidenceError(f"wrong scalar field set; missing={missing}, extra={extra}")
    required_suite_map = dict(REQUIRED_SUITES)
    if set(record.suites) != set(required_suite_map):
        missing = sorted(set(required_suite_map) - set(record.suites))
        extra = sorted(set(record.suites) - set(required_suite_map))
        raise EvidenceError(f"wrong suite set; missing={missing}, extra={extra}")

    scalars = record.scalars
    if scalars["schema_version"] != SCHEMA_VERSION:
        raise EvidenceError("schema_version must be exactly 1")
    require_commit(scalars["git_commit"])
    require_target(scalars["target"])
    require_digest(scalars["worker_executable_sha256"], "worker_executable_sha256")
    if _WORKER_BUILD_RE.fullmatch(scalars["worker_build_id"]) is None:
        raise EvidenceError("worker_build_id is not a canonical V1 worker identity")
    require_bounded_text(scalars["llvm_build_identity"], "llvm_build_identity", 192)
    require_digest(scalars["request_identity"], "request_identity")
    require_digest(scalars["artifact_identity"], "artifact_identity")

    for name, expected_class in REQUIRED_SUITES:
        suite = record.suites[name]
        if suite.name != name or suite.evidence_class != expected_class:
            raise EvidenceError(f"suite {name} has the wrong evidence class")
        if suite.status not in STATUSES:
            raise EvidenceError(f"suite {name} has an unknown status")
        if suite.status == "pass":
            if suite.reason != "-":
                raise EvidenceError(f"passing suite {name} must use reason '-' ")
        else:
            require_reason(suite.reason)

    hardware = record.suites["hardware-execution"]
    hardware_identity = scalars["hardware_execution_identity"]
    if hardware.status == "pass":
        if _HARDWARE_ID_RE.fullmatch(hardware_identity) is None:
            raise EvidenceError(
                "hardware pass requires a canonical hardware execution identity"
            )
    elif hardware_identity != "none":
        raise EvidenceError(
            "hardware execution identity must be none when hardware did not pass"
        )

    expected_gate = derive_release_gate(record.suites)
    if scalars["release_gate"] != expected_gate:
        raise EvidenceError(
            f"release_gate overclaims outcomes: expected {expected_gate}, "
            f"found {scalars['release_gate']}"
        )


def parse_record(path: Path) -> EvidenceRecord:
    data = read_regular_file(path)
    lines = decode_canonical_text(data, "evidence record")
    scalars: dict[str, str] = {}
    suites: dict[str, SuiteOutcome] = {}
    record_identity: str | None = None

    for line_number, line in enumerate(lines, start=1):
        fields = line.split("\t")
        key = fields[0]
        if key == "suite":
            if len(fields) != 5:
                raise EvidenceError(
                    f"suite line {line_number} must contain exactly five fields"
                )
            _, name, evidence_class, status, reason = fields
            if name in suites:
                raise EvidenceError(f"duplicate suite: {name}")
            suites[name] = SuiteOutcome(name, evidence_class, status, reason)
        elif key == "record_identity":
            if len(fields) != 2:
                raise EvidenceError("record_identity must contain exactly two fields")
            if record_identity is not None:
                raise EvidenceError("duplicate record_identity")
            record_identity = fields[1]
        elif key in SCALAR_FIELDS:
            if len(fields) != 2:
                raise EvidenceError(
                    f"scalar line {line_number} must contain exactly two fields"
                )
            if key in scalars:
                raise EvidenceError(f"duplicate scalar field: {key}")
            scalars[key] = fields[1]
        else:
            raise EvidenceError(f"unknown field: {key}")

    if record_identity is None:
        raise EvidenceError("missing record_identity")
    if (
        len(record_identity) > MAX_RECORD_IDENTITY_BYTES
        or _RECORD_ID_RE.fullmatch(record_identity) is None
    ):
        raise EvidenceError("record_identity is malformed")

    record = EvidenceRecord(scalars, suites)
    validate_record(record)
    if record.identity() != record_identity:
        raise EvidenceError(
            "record_identity does not authenticate the canonical record"
        )
    if data != record.canonical_bytes():
        raise EvidenceError("evidence record uses noncanonical field order or encoding")
    return record


def parse_suite_argument(value: str) -> SuiteOutcome:
    if "=" not in value:
        raise argparse.ArgumentTypeError("suite must use NAME=STATUS[:REASON]")
    name, outcome = value.split("=", 1)
    if ":" in outcome:
        status, reason = outcome.split(":", 1)
    else:
        status, reason = outcome, "-"
    expected_classes = dict(REQUIRED_SUITES)
    if name not in expected_classes:
        raise argparse.ArgumentTypeError(f"unknown suite: {name}")
    suite = SuiteOutcome(name, expected_classes[name], status, reason)
    if status not in STATUSES:
        raise argparse.ArgumentTypeError(f"unknown status for {name}: {status}")
    if status == "pass" and reason != "-":
        raise argparse.ArgumentTypeError("passing suites cannot carry a reason")
    if status != "pass":
        try:
            require_reason(reason)
        except EvidenceError as error:
            raise argparse.ArgumentTypeError(str(error)) from error
    return suite


def collect(args: argparse.Namespace) -> int:
    suites: dict[str, SuiteOutcome] = {}
    for suite in args.suite:
        if suite.name in suites:
            raise EvidenceError(f"duplicate suite: {suite.name}")
        suites[suite.name] = suite
    scalars = {
        "schema_version": SCHEMA_VERSION,
        "git_commit": args.git_commit,
        "target": args.target,
        "worker_executable_sha256": sha256_file(args.worker_executable),
        "worker_build_id": args.worker_build_id,
        "llvm_build_identity": args.llvm_build_identity,
        "request_identity": args.request_identity,
        "artifact_identity": sha256_file(args.artifact),
        "hardware_execution_identity": args.hardware_execution_identity,
        "release_gate": derive_release_gate(suites),
    }
    record = EvidenceRecord(scalars, suites)
    validate_record(record)
    sys.stdout.buffer.write(record.canonical_bytes())
    return 0


def validate(args: argparse.Namespace) -> int:
    record = parse_record(args.record)
    expected = {
        "git_commit": args.expect_commit,
        "target": args.expect_target,
        "worker_executable_sha256": sha256_file(args.worker_executable),
        "worker_build_id": args.expect_worker_build_id,
        "llvm_build_identity": args.expect_llvm_build_identity,
        "request_identity": args.expect_request_identity,
        "artifact_identity": sha256_file(args.artifact),
    }
    for field, value in expected.items():
        if record.scalars[field] != value:
            raise EvidenceError(
                f"{field} mismatch: expected {value}, found {record.scalars[field]}"
            )

    from reproduce import parse_result

    reproduction = parse_result(args.repro_result)
    suite = record.suites["clean-build-reproducibility"]
    if reproduction.target != record.scalars["target"]:
        raise EvidenceError("reproducibility target does not match the evidence target")
    if reproduction.status != suite.status or reproduction.reason != suite.reason:
        raise EvidenceError("reproducibility outcome does not match its suite row")
    if reproduction.status == "pass":
        artifact_identity = record.scalars["artifact_identity"]
        if (
            reproduction.first_artifact_sha256 != artifact_identity
            or reproduction.second_artifact_sha256 != artifact_identity
        ):
            raise EvidenceError(
                "reproducibility result does not bind the recorded artifact identity"
            )
    print(
        f"direct-link evidence is canonical and pinned: {record.identity()} "
        f"gate={record.scalars['release_gate']}"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    collect_parser = subparsers.add_parser("collect", help="emit canonical evidence")
    collect_parser.add_argument("--git-commit", required=True)
    collect_parser.add_argument("--target", required=True)
    collect_parser.add_argument("--worker-executable", required=True, type=Path)
    collect_parser.add_argument("--worker-build-id", required=True)
    collect_parser.add_argument("--llvm-build-identity", required=True)
    collect_parser.add_argument("--request-identity", required=True)
    collect_parser.add_argument("--artifact", required=True, type=Path)
    collect_parser.add_argument("--hardware-execution-identity", default="none")
    collect_parser.add_argument(
        "--suite", required=True, action="append", type=parse_suite_argument
    )
    collect_parser.set_defaults(handler=collect)

    validate_parser = subparsers.add_parser(
        "validate", help="validate canonical evidence against authenticated CI inputs"
    )
    validate_parser.add_argument("record", type=Path)
    validate_parser.add_argument("--expect-commit", required=True)
    validate_parser.add_argument("--expect-target", required=True)
    validate_parser.add_argument("--worker-executable", required=True, type=Path)
    validate_parser.add_argument("--expect-worker-build-id", required=True)
    validate_parser.add_argument("--expect-llvm-build-identity", required=True)
    validate_parser.add_argument("--expect-request-identity", required=True)
    validate_parser.add_argument("--artifact", required=True, type=Path)
    validate_parser.add_argument("--repro-result", required=True, type=Path)
    validate_parser.set_defaults(handler=validate)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        return args.handler(args)
    except EvidenceError as error:
        print(f"direct-link evidence: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
