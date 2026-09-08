#!/usr/bin/env python3
"""Install the fail-closed 47-fixture capability-simulation roster."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST = REPO_ROOT / "config" / "tutorial-kernel-manifest-v1.json"
DIGEST = MANIFEST.with_suffix(".sha256")
REASON_CODE = "capability-path-simulation-fixture-not-produced"


CPU_REFERENCE_ADDITIONS = {
    "gfx942-fill-simulation": (
        "cpu-reference-fill",
        ["examples/fill/Cargo.toml", "test", "capabilities"],
    ),
    "gfx942-wave64-collectives": (
        "cpu-reference-wave64-collectives",
        ["examples/wave64_collectives_v1/Cargo.toml", "test", "oracle"],
    ),
    "gfx942-workgroup-collectives": (
        "cpu-reference-workgroup-collectives",
        ["examples/workgroup_sync_v1/Cargo.toml", "test", "oracles"],
    ),
}


def _load_runner():
    path = Path(__file__).with_name("run-tutorial-semantic-simulation.py")
    specification = importlib.util.spec_from_file_location(
        "fe2o3_tutorial_capability_simulation", path
    )
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


RUNNER = _load_runner()


def migrated(document: dict[str, Any]) -> dict[str, Any]:
    result = json.loads(json.dumps(document))
    roster = RUNNER.manifest_simulation_roster(result)
    cpu_reference_lessons = {
        item["lessonId"]
        for fixture_id in CPU_REFERENCE_ADDITIONS
        for item in roster[fixture_id]["coverage"]
    }
    suites = [
        suite
        for suite in result["qualification"]["suites"]
        if suite.get("gate") != "semantic-simulation"
        and suite.get("suiteId")
        not in {suite_id for suite_id, _arguments in CPU_REFERENCE_ADDITIONS.values()}
    ]
    for fixture_id, (suite_id, arguments) in CPU_REFERENCE_ADDITIONS.items():
        suites.append(
            {
                "suiteId": suite_id,
                "gate": "cpu-reference",
                "availability": "available",
                "command": {
                    "executable": "scripts/run-tutorial-cpu-reference.sh",
                    "arguments": arguments,
                    "environment": [],
                    "workingDirectory": ".",
                    "timeoutSeconds": 1200,
                },
                "unavailableReason": None,
                "coverage": roster[fixture_id]["coverage"],
            }
        )
    for fixture_id, record in roster.items():
        suites.append(
            {
                "suiteId": record["suiteId"],
                "gate": "semantic-simulation",
                "availability": "available",
                "command": record["command"],
                "unavailableReason": None,
                "coverage": record["coverage"],
            }
        )
    result["qualification"]["suites"] = sorted(
        suites, key=lambda suite: suite["suiteId"]
    )
    for entry in result["entries"]:
        gates = set(entry["requiredGates"])
        gates.add("semantic-simulation")
        if entry["lessonId"] in cpu_reference_lessons:
            gates.add("cpu-reference")
        entry["requiredGates"] = sorted(gates)
    for kernel in result["capabilityKernels"]:
        target = next(
            fixture["target"]
            for fixture in result["compilerFixtures"]
            if fixture["fixtureId"] == kernel["fixtureId"]
        )
        kernel["simulatorCommand"] = {
            "status": "unavailable",
            "target": target,
            "command": None,
            "subjectSha256": None,
            "evidenceSha256": None,
            "reasonCode": REASON_CODE,
        }
    return result


def _encoded(document: dict[str, Any]) -> bytes:
    return (json.dumps(document, ensure_ascii=True, indent=2, allow_nan=False) + "\n").encode(
        "ascii"
    )


def _replace(path: Path, payload: bytes) -> None:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"refusing to replace non-regular file {path}")
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        path.stat().st_mode & 0o777,
    )
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
        document = json.loads(MANIFEST.read_bytes())
        payload = _encoded(migrated(document))
        digest = hashlib.sha256(payload).hexdigest()
        digest_payload = f"{digest}  config/{MANIFEST.name}\n".encode("ascii")
        if not options.write:
            if MANIFEST.read_bytes() != payload or DIGEST.read_bytes() != digest_payload:
                raise ValueError("semantic-simulation manifest roster is stale; rerun with --write")
        else:
            _replace(MANIFEST, payload)
            _replace(DIGEST, digest_payload)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"tutorial semantic simulation manifest: {error}", file=sys.stderr)
        return 1
    print("tutorial semantic simulation manifest roster is current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
