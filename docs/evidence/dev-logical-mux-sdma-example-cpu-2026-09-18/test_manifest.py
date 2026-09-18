#!/usr/bin/env python3
"""Rerun the original calibrations and test exact archive closure."""

from pathlib import Path
import tempfile
import unittest
from test_verify import HarnessTests
import verify as V


class ManifestTests(unittest.TestCase):
    def test_exact_membership(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-logical-mux-manifest-") as directory:
            root = Path(directory)
            for name in V.expected_paths():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture\n")
            baseline = V.manifest(root)
            (root / "SHA256SUMS").write_text(baseline)
            self.assertEqual(V.manifest(root), baseline)
            for name in ("raw/SHA256SUMS", "nested/deeper/SHA256SUMS", "unexpected"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("unmanifested\n")
                with self.subTest(name=name), self.assertRaises(ValueError):
                    V.manifest(root)
                path.unlink()
            path = root / "raw/source-before.log"
            path.unlink()
            with self.assertRaises(ValueError):
                V.manifest(root)
            path.symlink_to(root / "raw/source-after.log")
            with self.assertRaises(ValueError):
                V.manifest(root)


if __name__ == "__main__":
    unittest.main()
