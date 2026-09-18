#!/usr/bin/env python3
"""CPU-only calibration of exact example transcripts and archive seals."""

from pathlib import Path
import tempfile
import unittest
import verify as V


class HarnessTests(unittest.TestCase):
    def test_both_real_transcripts(self):
        for target in ("gnu", "musl"):
            self.assertEqual(len(V.harness((V.ARCHIVE / f"raw/{target}-example.log").read_text())), 19)

    def test_rejects_twelve_malformed_harnesses(self):
        text = (V.ARCHIVE / "raw/gnu-example.log").read_text()
        row = "test tests::all_is_an_explicit_selection ... ok"
        mutations = [
            text + "unexpected trailer\n",
            "unrecognized prelude\n" + text,
            text.replace("running 19 tests", "running 18 tests"),
            text.replace("19 passed", "18 passed"),
            text.replace("0 failed", "1 failed"),
            text.replace("0 ignored", "1 ignored"),
            text.replace("0 filtered out", "1 filtered out"),
            text.replace(row, row.replace("ok", "FAILED")),
            text.replace(row, row.replace("ok", "ignored")),
            text.replace(row, row.replace("all_is_an_explicit_selection", "unknown_test")),
            text.replace("tests::explicit_unique_ids_accept_decimal_and_hex", "tests::all_is_an_explicit_selection"),
            text[:text.index("test result:")],
        ]
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises((ValueError, AssertionError, IndexError)):
                V.harness(mutation)

    def test_final_source_and_commands(self):
        self.assertEqual(V.qualify()["example_tests_per_target"], 19)

    def test_seal_modes(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-logical-mux-seal-") as directory:
            path = Path(directory) / "SHA256SUMS"
            with self.assertRaises(ValueError):
                V.verify_seal(path, "a\n")
            V.verify_seal(path, "a\n", allow_unsealed=True)
            self.assertFalse(path.exists())
            V.verify_seal(path, "a\n", seal=True)
            V.verify_seal(path, "a\n")
            V.verify_seal(path, "a\n", allow_unsealed=True)
            with self.assertRaises(FileExistsError):
                V.verify_seal(path, "a\n", seal=True)
            for allow in (False, True):
                with self.assertRaises(ValueError):
                    V.verify_seal(path, "b\n", allow_unsealed=allow)


if __name__ == "__main__":
    unittest.main()
