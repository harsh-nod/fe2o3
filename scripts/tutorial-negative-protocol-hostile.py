#!/usr/bin/env python3
"""Exercise script-owned negative-evidence rejection boundaries."""

from __future__ import annotations

import argparse
from copy import deepcopy
from pathlib import Path
import sys

import tutorial_kernel_manifest as manifest_contract
import tutorial_negative_fixtures as negative_contract


REPO_ROOT = Path(__file__).resolve().parent.parent


def synthetic_evidence(
    declaration: dict,
    document: dict,
    validated: negative_contract.ValidatedDeclarations,
) -> dict:
    results = []
    for prerequisite_id in sorted(validated.shared_prerequisites):
        prerequisite = validated.shared_prerequisites[prerequisite_id]
        expected = prerequisite["expectedObservation"]
        results.append(
            {
                "commandSha256": negative_contract.command_sha256(prerequisite["command"]),
                "exitStatus": expected["exitStatus"],
                "prerequisiteId": prerequisite_id,
                "stderrBytes": expected["stderrBytes"],
                "stderrSha256": expected["stderrSha256"],
                "stdoutBytes": expected["stdoutBytes"],
                "stdoutSha256": expected["stdoutSha256"],
                "status": "passed",
            }
        )
    fixture_results = negative_contract.execute_fixture_cases(
        REPO_ROOT, document, validated
    )
    return negative_contract.build_evidence(
        declaration, validated, results, fixture_results
    )


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("case", choices=("evidence", "stale-output"))
    options = parser.parse_args(arguments)
    _, document = manifest_contract._load_json_unique(
        REPO_ROOT / "config" / manifest_contract.MANIFEST_NAME
    )
    declaration, validated = negative_contract.load_declarations(REPO_ROOT, document)
    evidence = synthetic_evidence(declaration, document, validated)
    expected = {
        "evidence": "negative evidence has stale status, authority, or declaration binding",
        "stale-output": "negative evidence shared prerequisite transcript is stale",
    }[options.case]
    if options.case == "evidence":
        evidence["declarationSha256"] = "f" * 64
    else:
        evidence["sharedPrerequisiteResults"][0]["stdoutSha256"] = "f" * 64
    try:
        negative_contract.validate_evidence(declaration, validated, evidence)
    except negative_contract.NegativeFixtureError as error:
        if str(error) != expected:
            print(f"unexpected negative protocol diagnostic: {error}", file=sys.stderr)
            return 1
        print(f"negative protocol passed: {options.case}")
        return 0
    print("hostile negative protocol fixture was accepted", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
