#!/usr/bin/env python3
"""Run the declared semantic suites and publish one authority-free evidence record."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
from typing import Any

import tutorial_kernel_manifest as manifest_contract


REPO_ROOT = Path(__file__).resolve().parent.parent
EVIDENCE_SCHEMA = "fe2o3-tutorial-semantic-qualification-evidence-v1"
SUITE_RESULT_SCHEMA = "fe2o3-tutorial-semantic-suite-result-v1"
ROADMAP_ISSUE = "https://github.com/harsh-nod/fe2o3/issues/272"
CORPUS_DOMAIN = b"fe2o3-tutorial-kernel-corpus-contract-v1\0"
COMMAND_DOMAIN = b"fe2o3-tutorial-semantic-command-v1\0"
MAX_STDOUT_BYTES = 64 * 1024 * 1024
MAX_STDERR_BYTES = 8 * 1024 * 1024
MAX_SUITES = 256
ENVIRONMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.+\Z")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")


class SemanticQualificationError(ValueError):
    """The declared suite set cannot produce qualification evidence."""


def _canonical(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("ascii")


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def corpus_contract_sha256(document: dict[str, Any]) -> str:
    contract = {key: value for key, value in document.items() if key != "baseline"}
    return _sha256(CORPUS_DOMAIN + _canonical(contract))


def command_sha256(command: dict[str, Any]) -> str:
    return _sha256(COMMAND_DOMAIN + _canonical(command))


def _git(*arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(REPO_ROOT), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise SemanticQualificationError(
            f"git {' '.join(arguments)} failed: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def clean_candidate() -> dict[str, Any]:
    if _git("status", "--porcelain=v1"):
        raise SemanticQualificationError("semantic evidence requires a clean compiler worktree")
    return {
        "commit": _git("rev-parse", "--verify", "HEAD"),
        "tree": _git("show", "-s", "--format=%T", "HEAD"),
        "worktreeClean": True,
    }


def _regular_repository_file(relative: str, *, executable: bool = False) -> Path:
    candidate = REPO_ROOT / relative
    resolved = candidate.resolve(strict=True)
    if not resolved.is_relative_to(REPO_ROOT):
        raise SemanticQualificationError(f"path escapes the repository: {relative}")
    current = REPO_ROOT
    for component in Path(relative).parts:
        current = current / component
        if current.is_symlink():
            raise SemanticQualificationError(f"path contains a symlink: {relative}")
    metadata = resolved.stat()
    if not resolved.is_file() or (executable and metadata.st_mode & 0o111 == 0):
        qualifier = " executable" if executable else ""
        raise SemanticQualificationError(f"path is not a regular{qualifier} file: {relative}")
    return resolved


def _validate_coverage(
    raw: Any, lessons: set[str], fixtures: set[str], label: str
) -> list[dict[str, Any]]:
    if not isinstance(raw, list) or not raw:
        raise SemanticQualificationError(f"{label} coverage must be nonempty")
    coverage: list[dict[str, Any]] = []
    previous = ""
    for index, item in enumerate(raw):
        if not isinstance(item, dict) or set(item) != {"fixtureIds", "lessonId"}:
            raise SemanticQualificationError(f"{label} coverage {index} is malformed")
        lesson = item["lessonId"]
        fixture_ids = item["fixtureIds"]
        if (
            not isinstance(lesson, str)
            or lesson not in lessons
            or lesson <= previous
            or not isinstance(fixture_ids, list)
            or not fixture_ids
            or fixture_ids != sorted(set(fixture_ids))
            or any(fixture not in fixtures for fixture in fixture_ids)
        ):
            raise SemanticQualificationError(f"{label} coverage {index} is invalid")
        previous = lesson
        coverage.append({"fixtureIds": fixture_ids, "lessonId": lesson})
    return coverage


def declared_suites(document: dict[str, Any]) -> list[dict[str, Any]]:
    entries = {entry["lessonId"]: entry for entry in document["entries"]}
    fixtures = {fixture["fixtureId"] for fixture in document["compilerFixtures"]}
    suites = document["qualification"]["suites"]
    if not isinstance(suites, list) or not 0 < len(suites) <= MAX_SUITES:
        raise SemanticQualificationError("semantic suite roster is empty or oversized")
    admitted: list[dict[str, Any]] = []
    previous = ""
    for index, suite in enumerate(suites):
        if not isinstance(suite, dict) or set(suite) != {
            "availability",
            "command",
            "coverage",
            "gate",
            "suiteId",
            "unavailableReason",
        }:
            raise SemanticQualificationError(f"suite {index} is malformed")
        suite_id = suite["suiteId"]
        if not isinstance(suite_id, str) or SLUG.fullmatch(suite_id) is None or suite_id <= previous:
            raise SemanticQualificationError("semantic suites must have sorted unique slug identities")
        previous = suite_id
        if suite["gate"] not in {"cpu-reference", "semantic-simulation"}:
            raise SemanticQualificationError(f"suite {suite_id} has an invalid gate")
        coverage = _validate_coverage(suite["coverage"], set(entries), fixtures, suite_id)
        for item in coverage:
            lesson = entries[item["lessonId"]]
            if suite["gate"] not in lesson["requiredGates"]:
                raise SemanticQualificationError(
                    f"suite {suite_id} claims an unrequired gate for {item['lessonId']}"
                )
            if any(fixture not in lesson["compilerFixtureIds"] for fixture in item["fixtureIds"]):
                raise SemanticQualificationError(
                    f"suite {suite_id} claims a fixture outside {item['lessonId']}"
                )
        if suite["availability"] == "available":
            if suite["unavailableReason"] is not None or not isinstance(suite["command"], dict):
                raise SemanticQualificationError(f"suite {suite_id} has inconsistent availability")
            manifest_contract._validate_command(suite["command"], f"suite {suite_id}.command")
            _regular_repository_file(suite["command"]["executable"], executable=True)
            admitted.append({**suite, "coverage": coverage})
        elif suite["availability"] == "unavailable":
            if suite["command"] is not None or not isinstance(suite["unavailableReason"], str):
                raise SemanticQualificationError(f"suite {suite_id} has inconsistent unavailability")
        else:
            raise SemanticQualificationError(f"suite {suite_id} has an invalid availability")
    return admitted


def _run_suite(suite: dict[str, Any]) -> dict[str, Any]:
    command = suite["command"]
    executable = _regular_repository_file(command["executable"], executable=True)
    environment = os.environ.copy()
    for assignment in command["environment"]:
        if ENVIRONMENT.fullmatch(assignment) is None:
            raise SemanticQualificationError(
                f"suite {suite['suiteId']} has an invalid environment assignment"
            )
        key, value = assignment.split("=", 1)
        environment[key] = value
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(
            [str(executable), *command["arguments"]],
            cwd=REPO_ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            status = process.wait(timeout=command["timeoutSeconds"])
        except subprocess.TimeoutExpired as error:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise SemanticQualificationError(
                f"suite {suite['suiteId']} exceeded its timeout"
            ) from error
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        if stdout_size > MAX_STDOUT_BYTES or stderr_size > MAX_STDERR_BYTES:
            raise SemanticQualificationError(f"suite {suite['suiteId']} exceeded output bounds")
        stdout.seek(0)
        stderr.seek(0)
        stdout_bytes = stdout.read()
        stderr_bytes = stderr.read()
    if status != 0:
        sys.stderr.buffer.write(stderr_bytes)
        raise SemanticQualificationError(f"suite {suite['suiteId']} exited with status {status}")
    if suite["gate"] == "semantic-simulation":
        try:
            result = json.loads(stdout_bytes)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise SemanticQualificationError(
                f"suite {suite['suiteId']} emitted invalid JSON"
            ) from error
        if (
            not isinstance(result, dict)
            or result.get("schema") != SUITE_RESULT_SCHEMA
            or result.get("status") != "passed"
            or result.get("authority") != "observation_only"
        ):
            raise SemanticQualificationError(
                f"suite {suite['suiteId']} emitted an invalid semantic result"
            )
    return {
        "commandSha256": command_sha256(command),
        "coverage": suite["coverage"],
        "exitStatus": 0,
        "gate": suite["gate"],
        "stderrBytes": len(stderr_bytes),
        "stderrSha256": _sha256(stderr_bytes),
        "stdoutBytes": len(stdout_bytes),
        "stdoutSha256": _sha256(stdout_bytes),
        "status": "passed",
        "suiteId": suite["suiteId"],
    }


def build_evidence(
    document: dict[str, Any],
    manifest_bytes: bytes,
    candidate: dict[str, Any],
    suite_results: list[dict[str, Any]],
) -> dict[str, Any]:
    return {
        "authority": {
            "compilerAuthority": False,
            "hardwareAuthority": False,
            "launchAuthority": False,
            "loadAuthority": False,
            "publicationAuthority": False,
        },
        "candidate": candidate,
        "manifest": {
            "corpusContractSha256": corpus_contract_sha256(document),
            "path": "config/tutorial-kernel-manifest-v1.json",
            "rawSha256": _sha256(manifest_bytes),
        },
        "roadmapIssue": ROADMAP_ISSUE,
        "schema": EVIDENCE_SCHEMA,
        "suites": suite_results,
    }


def _publish_new(path: Path, contents: bytes) -> None:
    path.parent.resolve(strict=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(contents)
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--digest-output", required=True, type=Path)
    options = parser.parse_args(arguments)
    output = options.output.resolve(strict=False)
    digest_output = options.digest_output.resolve(strict=False)
    if output == digest_output:
        print("semantic qualification: output paths must differ", file=sys.stderr)
        return 1
    published: list[Path] = []
    try:
        manifest_contract.validate_repository(REPO_ROOT)
        manifest_path = REPO_ROOT / "config" / manifest_contract.MANIFEST_NAME
        manifest_bytes, document = manifest_contract._load_json_unique(manifest_path)
        candidate = clean_candidate()
        suites = declared_suites(document)
        if len(suites) != len(document["qualification"]["suites"]):
            raise SemanticQualificationError(
                "qualification evidence requires every declared semantic suite to be available"
            )
        results = [_run_suite(suite) for suite in suites]
        evidence = build_evidence(document, manifest_bytes, candidate, results)
        evidence_bytes = _canonical(evidence) + b"\n"
        _publish_new(output, evidence_bytes)
        published.append(output)
        digest_record = f"{_sha256(evidence_bytes)}  {output.name}\n".encode("ascii")
        _publish_new(digest_output, digest_record)
        published.append(digest_output)
    except (OSError, SemanticQualificationError) as error:
        for path in reversed(published):
            path.unlink(missing_ok=True)
        print(f"semantic qualification: {error}", file=sys.stderr)
        return 1
    print(f"published authority-free semantic evidence: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
