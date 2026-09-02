#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/generate-formal-compiler-v3-spec.py"
MODULE_SPEC = importlib.util.spec_from_file_location("formal_compiler_v3_spec", GENERATOR)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
generator = importlib.util.module_from_spec(MODULE_SPEC)
MODULE_SPEC.loader.exec_module(generator)


class FormalCompilerV3SpecTests(unittest.TestCase):
    def setUp(self) -> None:
        self.spec = generator.load_spec()

    def write(self, value: dict[str, object]) -> Path:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "spec.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def assert_rejected(self, value: dict[str, object]) -> None:
        with self.assertRaises(ValueError):
            generator.load_spec(self.write(value))

    def test_checked_in_outputs_are_exactly_generated(self) -> None:
        self.assertEqual(
            generator.RUST.read_text(encoding="utf-8"), generator.render_rust(self.spec)
        )
        self.assertEqual(
            generator.VERUS.read_text(encoding="utf-8"), generator.render_verus(self.spec)
        )

    def test_unknown_and_missing_axes_are_rejected(self) -> None:
        unknown = dict(self.spec)
        unknown["unreviewed"] = True
        self.assert_rejected(unknown)
        missing = dict(self.spec)
        del missing["byte_order"]
        self.assert_rejected(missing)
        injected = dict(self.spec)
        injected["claim_name"] = 'claim"; pub const INJECTED: bool = true;'
        self.assert_rejected(injected)

    def test_width_extent_and_loop_substitutions_are_rejected(self) -> None:
        wrong_width = dict(self.spec)
        wrong_width["word_bits"] = 64
        self.assert_rejected(wrong_width)
        static_extent = dict(self.spec)
        static_extent["dynamic_extent"] = False
        self.assert_rejected(static_extent)
        reversed_loop = dict(self.spec)
        reversed_loop["modeled_minimum_loop_trip_count"] = 5
        self.assert_rejected(reversed_loop)

    def test_operation_roster_must_be_sorted_unique_and_nonempty(self) -> None:
        duplicate = dict(self.spec)
        duplicate["production_scalar_operations"] = ["bitxor", "bitxor"]
        self.assert_rejected(duplicate)
        reordered = dict(self.spec)
        reordered["production_scalar_operations"] = list(
            reversed(self.spec["production_scalar_operations"])
        )
        self.assert_rejected(reordered)
        empty = dict(self.spec)
        empty["production_scalar_operations"] = []
        self.assert_rejected(empty)

        overlap = dict(self.spec)
        overlap["modeled_only_scalar_operations"] = ["bitxor"]
        self.assert_rejected(overlap)


if __name__ == "__main__":
    unittest.main()
