#!/usr/bin/env python3
"""Collect fail-closed V3 evidence for the direct LLVM link release gate."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

from common import (
    EvidenceError,
    decode_canonical_text,
    read_regular_file,
    require_commit,
    require_reason,
    require_target,
    require_typed_identity,
    typed_file_identity,
    typed_identity,
)
from reproduce import (
    FINAL_ARTIFACT_DOMAIN,
    LINKED_ARTIFACT_DOMAIN,
    RECORD_DOMAIN as REPRO_RECORD_DOMAIN,
    REQUEST_DOMAIN,
    TOOLCHAIN_DOMAIN,
    WORKER_DOMAIN,
    parse_result,
)

SCHEMA_VERSION = "3"
RECORD_DOMAIN = "fe2o3-direct-link-evidence-v3"
WORKER_EXECUTABLE_DOMAIN = "fe2o3-worker-executable-v1"
SCALAR_FIELDS = (
    "schema_version",
    "git_commit",
    "target",
    "worker_executable_identity",
    "worker_identity",
    "llvm_toolchain_identity",
    "request_identity",
    "linked_artifact_identity",
    "final_artifact_identity",
    "reproducibility_identity",
    "release_gate",
)
REQUIRED_SUITES = (
    ("clean-build-reproducibility", "reproducibility"),
    ("compile", "compile"),
    ("direct-llvm-link", "worker"),
    ("hardware-execution", "hardware"),
    ("static-checks", "static"),
)
STATUSES = frozenset(("pass", "fail", "unavailable"))


@dataclass(frozen=True)
class SuiteOutcome:
    name: str
    evidence_class: str
    status: str
    reason: str
    provenance_identity: str


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
                        suite.provenance_identity,
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
    if statuses != {"pass"}:
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
        raise EvidenceError("schema_version must be exactly 3")
    require_commit(scalars["git_commit"])
    require_target(scalars["target"])
    for field, domain in (
        ("worker_executable_identity", WORKER_EXECUTABLE_DOMAIN),
        ("worker_identity", WORKER_DOMAIN),
        ("llvm_toolchain_identity", TOOLCHAIN_DOMAIN),
        ("request_identity", REQUEST_DOMAIN),
        ("linked_artifact_identity", LINKED_ARTIFACT_DOMAIN),
        ("final_artifact_identity", FINAL_ARTIFACT_DOMAIN),
        ("reproducibility_identity", REPRO_RECORD_DOMAIN),
    ):
        require_typed_identity(scalars[field], domain, field)

    for name, expected_class in REQUIRED_SUITES:
        suite = record.suites[name]
        if suite.name != name or suite.evidence_class != expected_class:
            raise EvidenceError(f"suite {name} has the wrong evidence class")
        if suite.status not in STATUSES:
            raise EvidenceError(f"suite {name} has an unknown status")
        if suite.status == "pass":
            if suite.reason != "-":
                raise EvidenceError(f"passing suite {name} must use reason '-'")
            raise EvidenceError(
                f"suite {name} cannot pass until its authenticated provenance parser exists"
            )
        else:
            require_reason(suite.reason)
            if (
                suite.provenance_identity != "none"
                and name != "clean-build-reproducibility"
            ):
                raise EvidenceError(
                    f"non-passing suite {name} must not assert unvalidated provenance"
                )

    reproduction = record.suites["clean-build-reproducibility"]
    if reproduction.provenance_identity != scalars["reproducibility_identity"]:
        raise EvidenceError("reproducibility suite does not bind its V3 record")
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
            if len(fields) != 6:
                raise EvidenceError(
                    f"suite line {line_number} must contain exactly six fields"
                )
            _, name, evidence_class, status, reason, provenance = fields
            if name in suites:
                raise EvidenceError(f"duplicate suite: {name}")
            suites[name] = SuiteOutcome(
                name, evidence_class, status, reason, provenance
            )
        elif key == "record_identity":
            if len(fields) != 2 or record_identity is not None:
                raise EvidenceError("duplicate or malformed record_identity")
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
    record = EvidenceRecord(scalars, suites)
    validate_record(record)
    require_typed_identity(record_identity, RECORD_DOMAIN, "record_identity")
    if record.identity() != record_identity:
        raise EvidenceError(
            "record_identity does not authenticate the canonical record"
        )
    if data != record.canonical_bytes():
        raise EvidenceError("evidence record uses noncanonical field order or encoding")
    return record


def validate_reproduction_bindings(
    reproduction: object,
    commit: str,
    target: str,
    toolchain: str,
    worker: str,
    request: str,
    linked_artifact: str,
    final_artifact: str,
) -> None:
    expected = {
        "git_commit": commit,
        "target": target,
        "llvm_toolchain_identity": toolchain,
        "worker_identity": worker,
        "request_identity": request,
    }
    for field, value in expected.items():
        if getattr(reproduction, field) != value:
            raise EvidenceError(f"reproducibility {field} does not match evidence")
    for field, actual in (
        ("first_linked_artifact_identity", linked_artifact),
        ("second_linked_artifact_identity", linked_artifact),
        ("first_final_artifact_identity", final_artifact),
        ("second_final_artifact_identity", final_artifact),
    ):
        recorded = getattr(reproduction, field)
        if recorded != "none" and recorded != actual:
            raise EvidenceError(f"reproducibility {field} does not match evidence")


def build_record(args: argparse.Namespace) -> EvidenceRecord:
    commit = require_commit(args.git_commit)
    target = require_target(args.target)
    for identity, domain, name in (
        (args.worker_identity, WORKER_DOMAIN, "worker_identity"),
        (args.llvm_toolchain_identity, TOOLCHAIN_DOMAIN, "llvm_toolchain_identity"),
        (args.request_identity, REQUEST_DOMAIN, "request_identity"),
    ):
        require_typed_identity(identity, domain, name)
    worker_executable = typed_file_identity(
        WORKER_EXECUTABLE_DOMAIN, args.worker_executable
    )
    linked_artifact = typed_file_identity(LINKED_ARTIFACT_DOMAIN, args.linked_artifact)
    final_artifact = typed_file_identity(FINAL_ARTIFACT_DOMAIN, args.final_artifact)
    reproduction = parse_result(args.repro_result)
    validate_reproduction_bindings(
        reproduction,
        commit,
        target,
        args.llvm_toolchain_identity,
        args.worker_identity,
        args.request_identity,
        linked_artifact,
        final_artifact,
    )

    reproduction_status = reproduction.status
    reproduction_reason = reproduction.reason
    if reproduction_status == "pass":
        reproduction_status = "unavailable"
        reproduction_reason = "unauthenticated-reproducibility"
    suites = {
        "clean-build-reproducibility": SuiteOutcome(
            "clean-build-reproducibility",
            "reproducibility",
            reproduction_status,
            reproduction_reason,
            reproduction.identity(),
        ),
        "compile": SuiteOutcome(
            "compile", "compile", "unavailable", "missing-g2-provenance", "none"
        ),
        "direct-llvm-link": SuiteOutcome(
            "direct-llvm-link",
            "worker",
            "unavailable",
            "missing-g5-g6-provenance",
            "none",
        ),
        "hardware-execution": SuiteOutcome(
            "hardware-execution",
            "hardware",
            "unavailable",
            "missing-g7-hardware-provenance",
            "none",
        ),
        "static-checks": SuiteOutcome(
            "static-checks",
            "static",
            "unavailable",
            "missing-static-runner-provenance",
            "none",
        ),
    }
    scalars = {
        "schema_version": SCHEMA_VERSION,
        "git_commit": commit,
        "target": target,
        "worker_executable_identity": worker_executable,
        "worker_identity": args.worker_identity,
        "llvm_toolchain_identity": args.llvm_toolchain_identity,
        "request_identity": args.request_identity,
        "linked_artifact_identity": linked_artifact,
        "final_artifact_identity": final_artifact,
        "reproducibility_identity": reproduction.identity(),
        "release_gate": derive_release_gate(suites),
    }
    record = EvidenceRecord(scalars, suites)
    validate_record(record)
    return record


def collect(args: argparse.Namespace) -> int:
    record = build_record(args)
    sys.stdout.buffer.write(record.canonical_bytes())
    return 0 if record.scalars["release_gate"] == "pass" else 1


def inspect(args: argparse.Namespace) -> int:
    record = parse_record(args.record)
    print(
        f"direct-link evidence is structurally canonical: {record.identity()} "
        f"gate={record.scalars['release_gate']}"
    )
    return 0


def validate(args: argparse.Namespace) -> int:
    record = parse_record(args.record)
    expected = build_record(args)
    for field in SCALAR_FIELDS[:-1]:
        if record.scalars[field] != expected.scalars[field]:
            raise EvidenceError(
                f"{field} mismatch: expected {expected.scalars[field]}, "
                f"found {record.scalars[field]}"
            )
    if record.suites != expected.suites:
        raise EvidenceError("suite provenance does not match canonical runner records")
    print(
        f"direct-link evidence gate: {record.identity()} "
        f"gate={record.scalars['release_gate']}"
    )
    return 0 if record.scalars["release_gate"] == "pass" else 1


def add_collection_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--git-commit", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--worker-executable", required=True, type=Path)
    parser.add_argument("--worker-identity", required=True)
    parser.add_argument("--llvm-toolchain-identity", required=True)
    parser.add_argument("--request-identity", required=True)
    parser.add_argument("--linked-artifact", required=True, type=Path)
    parser.add_argument("--final-artifact", required=True, type=Path)
    parser.add_argument("--repro-result", required=True, type=Path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    collect_parser = subparsers.add_parser(
        "collect", help="emit fail-closed evidence from canonical runner records"
    )
    add_collection_arguments(collect_parser)
    collect_parser.set_defaults(handler=collect)

    inspect_parser = subparsers.add_parser(
        "inspect", help="validate structure without asserting a passing gate"
    )
    inspect_parser.add_argument("record", type=Path)
    inspect_parser.set_defaults(handler=inspect)

    validate_parser = subparsers.add_parser(
        "validate", help="validate pinned provenance and require a passing gate"
    )
    validate_parser.add_argument("record", type=Path)
    add_collection_arguments(validate_parser)
    validate_parser.set_defaults(handler=validate)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        return args.handler(args)
    except EvidenceError as error:
        print(f"direct-link evidence: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
