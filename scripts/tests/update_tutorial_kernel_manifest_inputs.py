#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "update_tutorial_kernel_manifest_inputs.py"
)
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location(
    "update_tutorial_kernel_manifest_inputs", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
UPDATER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(UPDATER)


class TutorialManifestInputRefreshTests(unittest.TestCase):
    def test_refreshes_only_repository_derived_compiler_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "example"
            (package / "src").mkdir(parents=True)
            manifest_bytes = b'[package]\nname = "fixture"\nversion = "0.1.0"\n'
            lock_bytes = b"# fixture lock\n"
            (package / "Cargo.toml").write_bytes(manifest_bytes)
            (package / "src" / "lib.rs").write_text("pub fn kernel() {}\n")
            (root / "Cargo.lock").write_bytes(lock_bytes)

            fixture = {
                "fixtureId": "fixture",
                "target": "target-neutral",
                "matrix": None,
                "compilerInput": {
                    "packageManifest": "example/Cargo.toml",
                    "packageManifestSha256": "0" * 64,
                    "sourcePaths": ["example/src/lib.rs"],
                    "sourceClosureSha256": "0" * 64,
                    "cargoTarget": {
                        "kind": "lib",
                        "name": "fixture",
                        "sourcePath": "src/lib.rs",
                    },
                    "defaultFeatures": True,
                    "features": [],
                    "kernelSymbols": ["kernel"],
                    "cargoLockPath": "stale.lock",
                    "cargoLockSha256": "0" * 64,
                    "contractSha256": "0" * 64,
                },
            }
            document = {
                "unrelated": {"authority": False},
                "compilerFixtures": [fixture],
            }
            old_root = UPDATER.REPO_ROOT
            try:
                UPDATER.REPO_ROOT = root
                refreshed = UPDATER.refreshed_document(document)
            finally:
                UPDATER.REPO_ROOT = old_root

            self.assertEqual("0" * 64, fixture["compilerInput"]["contractSha256"])
            self.assertEqual(document["unrelated"], refreshed["unrelated"])
            compiler_input = refreshed["compilerFixtures"][0]["compilerInput"]
            self.assertEqual(
                hashlib.sha256(manifest_bytes).hexdigest(),
                compiler_input["packageManifestSha256"],
            )
            self.assertEqual("Cargo.lock", compiler_input["cargoLockPath"])
            self.assertEqual(
                hashlib.sha256(lock_bytes).hexdigest(),
                compiler_input["cargoLockSha256"],
            )
            self.assertNotEqual("0" * 64, compiler_input["sourceClosureSha256"])
            self.assertEqual(
                UPDATER.manifest_contract._fixture_input_contract_sha256(
                    refreshed["compilerFixtures"][0]
                ),
                compiler_input["contractSha256"],
            )

    def test_encoding_is_ascii_deterministic_and_newline_terminated(self) -> None:
        document = {"z": "value", "a": [1, True, None]}
        encoded = UPDATER.encoded(document)
        self.assertTrue(encoded.endswith(b"\n"))
        self.assertEqual(document, json.loads(encoded.decode("ascii")))
        self.assertEqual(encoded, UPDATER.encoded(document))


if __name__ == "__main__":
    unittest.main()
