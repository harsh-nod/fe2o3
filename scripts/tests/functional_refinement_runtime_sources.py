#!/usr/bin/env python3
"""Authority-free coverage of pinned runtime source selection."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "functional-refinement-verus-runtime-v1.sh"
HOST_LIBRARIES = {
    "libc.so.6": "libc.so.6",
    "libdl.so.2": "libdl.so.2",
    "libgcc_s.so.1": "libgcc_s.so.1",
    "libm.so.6": "libm.so.6",
    "libpthread.so.0": "libpthread.so.0",
    "librt.so.1": "librt.so.1",
    "libstdc++.so.6": "libstdc++.so.6.0.33",
    "libz.so.1": "libz.so.1.3",
}


class RuntimeSourceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def call(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", "-c", 'source "$1"\nshift\n"$@"', "runtime-test", str(SCRIPT), *arguments],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )

    def source(self, relative: str, staged: str = "") -> subprocess.CompletedProcess[str]:
        return self.call("source_for_file", relative, "/verus", "/rust", staged)

    def test_default_sources_remain_the_frozen_host_paths(self) -> None:
        for name, host_name in HOST_LIBRARIES.items():
            with self.subTest(name=name):
                result = self.source(f"system-lib/{name}")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), f"/usr/lib/x86_64-linux-gnu/{host_name}")

    def test_staging_uses_manifest_basenames_without_host_fallback(self) -> None:
        for name in HOST_LIBRARIES:
            with self.subTest(name=name):
                result = self.source(f"system-lib/{name}", str(self.root))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(self.root / name))

    def test_staging_cannot_redirect_verus_or_toolchain(self) -> None:
        for relative, expected in [
            ("dist/rust_verify", "/verus/rust_verify"),
            ("toolchain/lib/librustc_driver.so", "/rust/lib/librustc_driver.so"),
        ]:
            result = self.source(relative, str(self.root))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), expected)

    def test_unknown_dso_and_interpreter_override_are_rejected(self) -> None:
        for name in ("libother.so", "ld-linux-x86-64.so.2", "../libc.so.6", "libz.so.1.3"):
            with self.subTest(name=name):
                result = self.source(f"system-lib/{name}", str(self.root))
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("unsupported system DSO", result.stderr)

    def test_staging_directory_must_be_absolute_existing_and_not_a_symlink(self) -> None:
        link = self.root / "link"
        link.symlink_to(self.root, target_is_directory=True)
        regular = self.root / "file"
        regular.write_bytes(b"not a directory")
        for path in ("relative", str(link), str(regular), str(self.root / "missing")):
            with self.subTest(path=path):
                result = self.source("system-lib/libz.so.1", path)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("SYSTEM_LIB_DIRECTORY", result.stderr)

    def test_staged_file_pins_remain_mandatory(self) -> None:
        payload = b"inert fixture, not an executable library"
        path = self.root / "libz.so.1"
        path.write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        selected = self.source("system-lib/libz.so.1", str(self.root)).stdout.strip()
        for size, expected_digest, valid in [
            (len(payload), digest, True),
            (len(payload) + 1, digest, False),
            (len(payload), "0" * 64, False),
        ]:
            with self.subTest(size=size, digest=expected_digest):
                result = self.call("verify_file", selected, "0444", str(size), expected_digest)
                self.assertEqual(result.returncode == 0, valid, result.stderr)
        path.unlink()
        result = self.call("verify_file", selected, "0444", str(len(payload)), digest)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a no-follow regular file", result.stderr)

    def test_staged_symlinks_and_hardlinks_are_rejected(self) -> None:
        payload = b"inert fixture"
        original = self.root / "original"
        original.write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        path = self.root / "libz.so.1"
        path.symlink_to(original)
        result = self.call("verify_file", str(path), "0444", str(len(payload)), digest)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a no-follow regular file", result.stderr)
        path.unlink()
        os.link(original, path)
        result = self.call("verify_file", str(path), "0444", str(len(payload)), digest)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("multiple hard links", result.stderr)

    def test_installed_audit_and_cli_arity_do_not_accept_source_overrides(self) -> None:
        for arguments in [
            ["audit-installed", str(self.root), str(self.root)],
            ["audit-source", "/verus", "/rust", "/rustup", ""],
            ["audit-source", "/verus", "/rust", "/rustup", str(self.root), "extra"],
            ["provision", "/verus", "/rust", "/rustup", "/destination", ""],
        ]:
            with self.subTest(arguments=arguments):
                result = subprocess.run(
                    ["bash", str(SCRIPT), *arguments],
                    capture_output=True,
                    text=True,
                    timeout=10,
                    check=False,
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("usage:", result.stderr)


if __name__ == "__main__":
    unittest.main()
