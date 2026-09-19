#!/usr/bin/env python3
"""Reuse the sealed adversarial CPU-packet calibrations against this verifier."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run calibration with python3 -I")

import hashlib
import importlib.util
from pathlib import Path
import shutil
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PRIOR = ROOT / "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18"
for name, digest in {
    "test_accept.py": "6466afb68e827f398a0f0694e330aba88f49f4f16545739586abf4f5c1abe56e",
    "accept.py": "be5998bbd3bde933c48d4ae3d6e12ae8231da62cffd254abdb7c7ebe3f7dc8bd",
}.items():
    path = PRIOR / name
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise RuntimeError("unauthenticated calibration helper: " + name)
spec = importlib.util.spec_from_file_location(
    "retirement_calibration", PRIOR / "test_accept.py"
)
T = importlib.util.module_from_spec(spec)
spec.loader.exec_module(T)
spec = importlib.util.spec_from_file_location("retirement_verifier", HERE / "verify.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
T.V, T.HERE = V, HERE


class Calibration(T.Calibration):
    def test_offline_verification_from_another_checkout(self):
        root = Path(self.temp.name) / "relocated"
        archive = root / V.PREFIX
        shutil.copytree(HERE, archive)
        for name in set(V.TOOLS) | {V.ACCEPT}:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                shutil.copyfile(ROOT / name, path)
        spec = importlib.util.spec_from_file_location(
            "relocated_retirement_verifier", archive / "verify.py"
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertNotEqual(module.ROOT, ROOT)
        self.assertEqual(module.EXECUTION_ROOT, V.EXECUTION_ROOT)
        self.assertTrue(module.verify_bundle()["cpu_qualification"])
        if not (archive / "SHA256SUMS").exists():
            module.seal(archive, True)
        module.seal(archive, False)


if __name__ == "__main__":
    unittest.main()
