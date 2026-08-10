#!/usr/bin/env python3
"""Deterministic tests for the S09 sealed-object snapshot boundary."""

from __future__ import annotations

import errno
import fcntl
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from s09_pinned_snapshot import (  # noqa: E402
    REQUIRED_SEALS,
    SnapshotError,
    create_sealed_snapshot,
)


class SealedSnapshotTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = pathlib.Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write(self, name: str, data: bytes, mode: int = 0o600) -> pathlib.Path:
        path = self.directory / name
        path.write_bytes(data)
        path.chmod(mode)
        return path

    def test_hsaco_rename_and_substitution_cannot_change_snapshot(self) -> None:
        path = self.write("alpha.hsaco", b"original-hsaco-object")
        snapshot = create_sealed_snapshot("hsaco", path)
        try:
            replacement = self.write("replacement.hsaco", b"substitute-hsaco-object")
            replacement.replace(path)
            self.assertEqual(os.pread(snapshot.descriptor, snapshot.size, 0), b"original-hsaco-object")
            self.assertEqual(fcntl.fcntl(snapshot.descriptor, fcntl.F_GET_SEALS), REQUIRED_SEALS)
            with self.assertRaises(OSError) as write_error:
                os.pwrite(snapshot.descriptor, b"X", 0)
            self.assertEqual(write_error.exception.errno, errno.EPERM)
        finally:
            snapshot.close()

    def test_facts_same_size_mutation_with_restored_mtime_is_rejected(self) -> None:
        original = b"a" * (96 * 1024)
        path = self.write("artifact.facts", original)
        original_stat = path.stat()

        def mutate_after_first_chunk() -> None:
            descriptor = os.open(path, os.O_WRONLY)
            try:
                os.pwrite(descriptor, b"b" * 4096, 64 * 1024)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            os.utime(path, ns=(original_stat.st_atime_ns, original_stat.st_mtime_ns))

        with self.assertRaisesRegex(SnapshotError, "changed while being copied"):
            create_sealed_snapshot(
                "facts", path, _after_first_chunk=mutate_after_first_chunk
            )

    def test_host_snapshot_is_the_exact_executed_image(self) -> None:
        source = pathlib.Path("/bin/true")
        host = self.directory / "s09-host"
        shutil.copyfile(source, host)
        host.chmod(0o700)
        snapshot = create_sealed_snapshot("host", host, executable=True)
        try:
            replacement = self.write("replacement-host", b"not an ELF image", 0o700)
            replacement.replace(host)
            completed = subprocess.run(
                [snapshot.proc_path],
                pass_fds=(snapshot.descriptor,),
                check=False,
            )
            self.assertEqual(completed.returncode, 0)
        finally:
            snapshot.close()

    def test_non_executable_host_is_rejected(self) -> None:
        host = self.write("host", b"host", 0o600)
        with self.assertRaisesRegex(SnapshotError, "must be executable"):
            create_sealed_snapshot("host", host, executable=True)


if __name__ == "__main__":
    unittest.main()
