#!/usr/bin/env python3
"""Check or execute all canonical tutorial negative fixtures."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

import tutorial_kernel_manifest as manifest_contract
import tutorial_negative_fixtures as negative_contract


REPO_ROOT = Path(__file__).resolve().parent.parent


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--output", type=Path)
    options = parser.parse_args(arguments)
    if bool(options.output) != options.execute:
        print("negative fixtures: --execute and --output must be used together", file=sys.stderr)
        return 2
    try:
        _, document = manifest_contract._load_json_unique(
            REPO_ROOT / "config" / manifest_contract.MANIFEST_NAME
        )
        declaration, validated = negative_contract.load_declarations(REPO_ROOT, document)
        if not options.execute:
            case_count = sum(len(fixture["cases"]) for fixture in validated.fixtures.values())
            print(
                "validated tutorial negative declarations: "
                f"{len(validated.fixtures)} fixtures, {case_count} cases, "
                f"{len(validated.shared_prerequisites)} shared prerequisites"
            )
            return 0
        evidence = negative_contract.execute(REPO_ROOT, document)
        payload = negative_contract._canonical(evidence) + b"\n"
        output = options.output.resolve(strict=False)
        if output.exists() or output.is_symlink():
            raise negative_contract.NegativeFixtureError(
                f"refusing to replace negative evidence output: {output}"
            )
        output.parent.resolve(strict=True)
        descriptor = output.open("xb")
        with descriptor:
            descriptor.write(payload)
            descriptor.flush()
        print(
            "published unavailable observation-only negative evidence: "
            f"{output}"
        )
        return 0
    except (OSError, manifest_contract.ManifestError, negative_contract.NegativeFixtureError) as error:
        print(f"negative fixtures: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
