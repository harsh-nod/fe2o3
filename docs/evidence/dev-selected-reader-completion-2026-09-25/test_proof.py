#!/usr/bin/env python3
"""Calibrate the selected-reader mutation inventory and source-policy adapter."""

from pathlib import Path
import runpy
import tempfile
import unittest


CHECK = runpy.run_path(str(Path(__file__).with_name("proof.py")))
ROOT = CHECK["ROOT"]


class ProofTests(unittest.TestCase):
    def test_mutations_are_distinct_and_scoped(self):
        source = (ROOT / CHECK["BODY"]).read_text()
        mutations = CHECK["mutations"](source)
        self.assertEqual(len(mutations), 7)
        self.assertEqual(len({data for data, _ in mutations.values()}), 7)
        for data, function in mutations.values():
            self.assertNotEqual(data, source.encode())
            self.assertTrue(function.startswith("SelectedReadersV1::release_"))

    def test_missing_and_duplicate_anchors_refuse(self):
        source = (ROOT / CHECK["BODY"]).read_text()
        for malformed in ("", source + source):
            with self.assertRaisesRegex(ValueError, "unique mutation anchor"):
                CHECK["mutations"](malformed)

    def test_source_policy_scans_both_new_proof_leaves(self):
        prior = runpy.run_path(str(ROOT / CHECK["PREVIOUS"]))
        loaded = prior["load"](ROOT)
        _, owner, _, _, deps, *_, paths = loaded
        paths = paths | {CHECK["SELECTED"], CHECK["WITNESSES"]}
        original = {path: (ROOT / path).read_bytes() for path in paths}
        with tempfile.TemporaryDirectory(prefix="fe2o3-selected-proof-test-") as directory:
            stage = Path(directory)
            owner.materialize(stage, original)
            CHECK["scan"](stage, deps)
            for path in (CHECK["SELECTED"], CHECK["WITNESSES"]):
                owner.materialize(stage, original | {path: original[path] + b"\nverus! { proof fn forbidden() { assume(false); } }\n"})
                with self.assertRaises(Exception):
                    CHECK["scan"](stage, deps)


if __name__ == "__main__":
    unittest.main()
