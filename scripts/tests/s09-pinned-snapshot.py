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
import time
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
PINNER = ROOT / "scripts" / "s09_pinned_snapshot.py"
sys.path.insert(0, str(ROOT / "scripts"))

from s09_pinned_snapshot import (  # noqa: E402
    REQUIRED_SEALS,
    SnapshotError,
    create_sealed_snapshot,
    export_sealed_snapshot,
    verify_sealed_path,
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
            self.assertEqual(
                os.pread(snapshot.descriptor, snapshot.size, 0),
                b"original-hsaco-object",
            )
            self.assertEqual(
                fcntl.fcntl(snapshot.descriptor, fcntl.F_GET_SEALS), REQUIRED_SEALS
            )
            with self.assertRaises(OSError) as write_error:
                os.pwrite(snapshot.descriptor, b"X", 0)
            self.assertEqual(write_error.exception.errno, errno.EPERM)
        finally:
            snapshot.close()

    def test_each_object_rejects_same_size_mutation_with_restored_mtime(self) -> None:
        for name, executable in (("hsaco", False), ("facts", False), ("host", True)):
            with self.subTest(name=name):
                path = self.write(f"{name}.input", b"a" * (96 * 1024), 0o700)
                original_stat = path.stat()

                def mutate_after_first_chunk() -> None:
                    descriptor = os.open(path, os.O_WRONLY)
                    try:
                        os.pwrite(descriptor, b"b" * 4096, 64 * 1024)
                        os.fsync(descriptor)
                    finally:
                        os.close(descriptor)
                    os.utime(
                        path,
                        ns=(original_stat.st_atime_ns, original_stat.st_mtime_ns),
                    )

                with self.assertRaisesRegex(
                    SnapshotError, "changed while being copied"
                ):
                    create_sealed_snapshot(
                        name,
                        path,
                        executable=executable,
                        _after_first_chunk=mutate_after_first_chunk,
                    )

    def test_facts_rename_and_substitution_cannot_change_snapshot(self) -> None:
        facts = self.write("artifact.facts", b"target=gfx942:xnack-\n")
        snapshot = create_sealed_snapshot("facts", facts)
        try:
            substitute = self.write("facts-substitute", b"target=gfx90a:xnack-\n")
            substitute.replace(facts)
            self.assertEqual(
                os.pread(snapshot.descriptor, snapshot.size, 0),
                b"target=gfx942:xnack-\n",
            )
        finally:
            snapshot.close()

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
                check=False,
            )
            self.assertEqual(completed.returncode, 0)
        finally:
            snapshot.close()

    def test_non_executable_host_is_rejected(self) -> None:
        host = self.write("host", b"host", 0o600)
        with self.assertRaisesRegex(SnapshotError, "must be executable"):
            create_sealed_snapshot("host", host, executable=True)

    def test_internal_boundary_rejects_unsealed_descriptors(self) -> None:
        descriptor = os.memfd_create(
            "fe2o3-s09-unsealed", os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING
        )
        try:
            os.write(descriptor, b"facts")
            path = pathlib.Path(f"/proc/{os.getpid()}/fd/{descriptor}")
            with self.assertRaisesRegex(SnapshotError, "missing required seals"):
                verify_sealed_path("facts", path)
        finally:
            os.close(descriptor)

    def test_dead_and_noncanonical_proc_paths_are_rejected(self) -> None:
        for path in (
            pathlib.Path("/proc/self/fd/3"),
            pathlib.Path("/proc/0/fd/3"),
            pathlib.Path("/proc/999999999/fd/3"),
        ):
            with self.subTest(path=path), self.assertRaises(SnapshotError):
                verify_sealed_path("facts", path)

    def test_stale_starttime_and_wrong_owner_are_rejected(self) -> None:
        source = self.write("facts", b"facts")
        snapshot = create_sealed_snapshot("facts", source)
        try:
            with self.assertRaisesRegex(SnapshotError, "PID was reused"):
                verify_sealed_path(
                    "facts",
                    pathlib.Path(snapshot.proc_path),
                    _expected_owner_start_time_ticks=snapshot.owner_start_time_ticks
                    + 1,
                )
            with self.assertRaisesRegex(SnapshotError, "wrong UID"):
                verify_sealed_path(
                    "facts",
                    pathlib.Path(snapshot.proc_path),
                    _expected_owner_uid=os.getuid() + 1,
                )
        finally:
            snapshot.close()

    def test_live_nonancestor_snapshot_owner_is_rejected(self) -> None:
        read_path, write_path = os.pipe()
        child = os.fork()
        if child == 0:
            os.close(read_path)
            try:
                source = self.write("child-facts", b"child facts")
                snapshot = create_sealed_snapshot("child-facts", source)
                os.write(write_path, snapshot.proc_path.encode("ascii") + b"\n")
                time.sleep(10)
            finally:
                os._exit(0)
        os.close(write_path)
        try:
            path = pathlib.Path(os.read(read_path, 4096).decode("ascii").strip())
            with self.assertRaisesRegex(SnapshotError, "live ancestor"):
                verify_sealed_path("facts", path)
        finally:
            os.close(read_path)
            os.kill(child, 15)
            os.waitpid(child, 0)

    def test_cli_supervisor_stays_alive_for_stable_proc_paths(self) -> None:
        source = self.write("facts", b"supervised facts")
        child_code = """
import os
import pathlib
import subprocess
import sys
import time
path = pathlib.Path(sys.argv[1])
owner = int(path.parts[2])
assert owner == os.getppid()
identity = path.stat()
assert identity.st_dev == int(sys.argv[2])
assert identity.st_ino == int(sys.argv[3])
assert identity.st_mode == int(sys.argv[4])
assert identity.st_size == int(sys.argv[5])
assert owner == int(sys.argv[6])
owner_state = pathlib.Path("/proc/" + str(owner) + "/stat").read_text(encoding="ascii")
fields = owner_state[owner_state.rfind(") ") + 2:].split()
assert int(fields[19]) == int(sys.argv[7])
assert path.read_bytes() == b"supervised facts"
time.sleep(0.05)
assert pathlib.Path("/proc/" + str(owner) + "/stat").is_file()
assert path.read_bytes() == b"supervised facts"
grandchild = "import pathlib,sys; assert pathlib.Path(sys.argv[1]).read_bytes() == b'supervised facts'"
completed = subprocess.run([sys.executable, "-c", grandchild, str(path)], close_fds=True)
assert completed.returncode == 0
"""
        completed = subprocess.run(
            [
                str(PINNER),
                "--input",
                f"facts={source}",
                "--",
                sys.executable,
                "-c",
                child_code,
                "{facts}",
                "{facts_device}",
                "{facts_inode}",
                "{facts_mode}",
                "{facts_size}",
                "{facts_owner_pid}",
                "{facts_owner_start_time_ticks}",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_grandchild_exec_reopens_supervisor_host_after_fd_close(self) -> None:
        host = self.directory / "host"
        shutil.copyfile("/bin/true", host)
        host.chmod(0o700)
        child_code = """
import pathlib
import subprocess
import sys
path = pathlib.Path(sys.argv[1])
completed = subprocess.run([str(path)], close_fds=True)
assert completed.returncode == 0
"""
        completed = subprocess.run(
            [
                str(PINNER),
                "--input",
                f"host={host}",
                "--executable",
                "host",
                "--",
                sys.executable,
                "-c",
                child_code,
                "{host}",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_inherited_sealed_snapshot_can_be_resnapshotted(self) -> None:
        source = self.write("hsaco", b"exact inherited hsaco")
        first = create_sealed_snapshot("hsaco", source)
        try:
            second = create_sealed_snapshot(
                "hsaco-nested", pathlib.Path(first.proc_path)
            )
            try:
                self.assertEqual(first.sha256, second.sha256)
                self.assertEqual(
                    os.pread(second.descriptor, second.size, 0),
                    b"exact inherited hsaco",
                )
            finally:
                second.close()
        finally:
            first.close()

    def test_export_retains_exact_read_only_host_bytes(self) -> None:
        host = self.directory / "host"
        shutil.copyfile("/bin/true", host)
        host.chmod(0o700)
        snapshot = create_sealed_snapshot("host", host, executable=True)
        retained = self.directory / "retained-host.bin"
        try:
            export_sealed_snapshot(snapshot, retained)
            self.assertEqual(retained.read_bytes(), host.read_bytes())
            self.assertEqual(retained.stat().st_mode & 0o777, 0o400)
            self.assertEqual(retained.stat().st_nlink, 1)
        finally:
            snapshot.close()

    def test_export_rejects_existing_destination_substitution(self) -> None:
        source = self.write("host", b"exact host bytes", 0o700)
        snapshot = create_sealed_snapshot("host", source, executable=True)
        destination = self.write("retained-host.bin", b"substitute")
        try:
            with self.assertRaisesRegex(SnapshotError, "must not already exist"):
                export_sealed_snapshot(snapshot, destination)
            self.assertEqual(destination.read_bytes(), b"substitute")
        finally:
            snapshot.close()

    def test_cli_export_rejects_source_path_swap_and_keeps_snapshot(self) -> None:
        source = self.write("host", b"exact retained host", 0o700)
        retained = self.directory / "retained-host.bin"
        completed = subprocess.run(
            [
                str(PINNER),
                "--input",
                f"host={source}",
                "--executable",
                "host",
                "--export",
                f"host={retained}",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        replacement = self.write("replacement-host", b"substitute retained", 0o700)
        replacement.replace(source)
        self.assertEqual(retained.read_bytes(), b"exact retained host")
        self.assertEqual(retained.stat().st_mode & 0o777, 0o400)


if __name__ == "__main__":
    unittest.main()
