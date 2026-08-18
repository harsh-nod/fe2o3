#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest


sys.dont_write_bytecode = True
CHECKER_PATH = Path(__file__).resolve().parents[1] / "pliron_dependency_policy.py"
SPEC = importlib.util.spec_from_file_location("pliron_dependency_policy", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def package(name: str, version: str = CHECKER.PLIRON_VERSION, source: str | None = None) -> dict:
    return {
        "name": name,
        "version": version,
        "source": CHECKER.PLIRON_SOURCE if source is None else source,
    }


class PlironDependencyPolicyTests(unittest.TestCase):
    def test_accepts_exact_d0_closure(self) -> None:
        violations, stats = CHECKER.check_metadata(
            {"packages": [package("pliron"), package("pliron-derive")]}
        )
        self.assertEqual([], violations)
        self.assertEqual({"pliron": 1, "pliron_derive": 1, "pliron_llvm": 0}, stats)

    def test_rejects_missing_and_duplicate_packages(self) -> None:
        violations, _ = CHECKER.check_metadata(
            {"packages": [package("pliron"), package("pliron")]}
        )
        self.assertIn("duplicate Pliron package identity: pliron (2)", violations)
        self.assertIn("missing required Pliron package: pliron-derive", violations)

    def test_rejects_mixed_revision_or_version(self) -> None:
        violations, _ = CHECKER.check_metadata(
            {
                "packages": [
                    package("pliron", version="0.16.0"),
                    package("pliron-derive", source="git+https://example.invalid/pliron"),
                ]
            }
        )
        self.assertTrue(any("wrong Pliron version: pliron" in item for item in violations))
        self.assertTrue(any("wrong Pliron source: pliron-derive" in item for item in violations))

    def test_rejects_unreviewed_llvm_or_repository_package(self) -> None:
        violations, _ = CHECKER.check_metadata(
            {
                "packages": [
                    package("pliron"),
                    package("pliron-derive"),
                    package("pliron-llvm"),
                    package("pliron-extra"),
                ]
            }
        )
        self.assertTrue(any("pliron-llvm is outside the D0 closure" in item for item in violations))
        self.assertIn("unexpected package from the Pliron repository: pliron-extra", violations)


if __name__ == "__main__":
    unittest.main()
