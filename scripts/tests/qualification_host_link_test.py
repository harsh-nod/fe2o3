#!/usr/bin/python3
"""LLD capture rejection controls; no linker/runtime authority is synthesized."""

import hashlib
import contextlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualification_host_link as link


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-host-link-test-")
        self.path = Path(self.temporary.name) / "inputs.tar"

    def tearDown(self):
        self.temporary.cleanup()

    @unittest.skipUnless(os.environ.get("FE2O3_TEST_HOST_LINK_PROXY"), "requires prepared private linker namespace")
    def test_live_proxy_rejects_exact_controls_before_linking(self):
        proxy = os.environ["FE2O3_TEST_HOST_LINK_PROXY"]
        gcc_ld = link.read_record(link.ROOT / "gcc-ld", 4096).decode().strip()
        route = ["-B" + gcc_ld, "-fuse-ld=lld", "-o", "/run/application-target/invalid-control"]
        for control in (["@response"], ["-specs=foreign"], ["-Wl,--reproduce=elsewhere"],
                        ["-Wl,-Map,elsewhere"], ["-Wl,--Map=elsewhere"], ["-Wl,-M"],
                        ["-Wl,--print-map"], ["-Xlinker", "-Map"], ["-Wl,@response"],
                        ["-Wl,--output=foreign"], ["-Wl,-o,foreign"], ["--"]):
            with self.subTest(control=control):
                result = subprocess.run(["cc", *control, *route], executable=proxy,
                    env={"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"},
                    capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 125)
                self.assertEqual(result.stderr, "qualification host link: unreviewed response/specs/observation control\n")

    def archive(self, entries=None):
        entries = entries if entries is not None else [
            ("inputs/response.txt", b"--chroot .\n-o application\nusr/lib/crt1.o\n"),
            ("inputs/version.txt", b"LLD tested\n"),
            ("inputs/usr/lib/crt1.o", b"ELF"),
            ("inputs/usr/lib/libpthread.a", b"!<arch>\n"),
        ]
        with tarfile.open(self.path, "w") as archive:
            for name, content in entries:
                item = name if isinstance(name, tarfile.TarInfo) else tarfile.TarInfo(name)
                item.size = len(content)
                archive.addfile(item, io.BytesIO(content))

    def test_reproducer_preserves_binary_bytes_and_empty_archives(self):
        self.archive()
        inputs, response, version = link.read_reproducer(self.path)
        self.assertEqual(version, b"LLD tested\n")
        self.assertEqual(link.response_arguments(response)[:2], ["--chroot", "."])
        self.assertEqual(inputs["/usr/lib/libpthread.a"], {
            "bytes": 8, "sha256": hashlib.sha256(b"!<arch>\n").hexdigest()})

    def test_duplicate_missing_and_noncanonical_entries_reject(self):
        for entries in [
            [("inputs/response.txt", b"a"), ("inputs/response.txt", b"b")],
            [("inputs/usr/lib/a", b"bad"), ("inputs/usr/lib/a", b"approved")],
            [("inputs/usr/lib/a", b"a")],
            [("inputs/../escape", b"a")],
            [("/inputs/absolute", b"a")],
            [("inputs//duplicate-slash", b"a")],
            [("foreign/usr/lib/a", b"a")],
        ]:
            with self.subTest(entries=entries):
                self.archive(entries)
                with self.assertRaises(link.HardeningError):
                    link.read_reproducer(self.path)

    def test_aliases_and_special_entries_reject(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE, tarfile.DIRTYPE):
            with self.subTest(kind=kind):
                entry = tarfile.TarInfo("inputs/usr/lib/alias")
                entry.type = kind
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    entry.linkname = "target"
                self.archive([(entry, b"")])
                with self.assertRaises(link.HardeningError):
                    link.read_reproducer(self.path)

    def test_truncated_tar_and_missing_terminator_reject(self):
        self.archive()
        data = self.path.read_bytes()
        for truncated in (data[:513], data[:2048], data[:-1024] + b"x" * 1024):
            with self.subTest(size=len(truncated)):
                self.path.write_bytes(truncated)
                with self.assertRaises((link.HardeningError, tarfile.TarError)):
                    link.read_reproducer(self.path)

    def test_member_count_and_byte_bounds_reject(self):
        self.archive()
        for variable, bound in (("MAX_FILE", 1), ("MAX_ENTRIES", 1), ("MAX_TAR", 1), ("MAX_RESPONSE", 1)):
            with self.subTest(variable=variable), patch.object(link, variable, bound):
                with self.assertRaises(link.HardeningError):
                    link.read_reproducer(self.path)

    def test_hidden_archive_tail_and_invalid_middle_reject(self):
        self.archive()
        original = self.path.read_bytes()
        for tail in (original, b"bad-header" + bytes(2048)):
            self.path.write_bytes(original + tail + bytes(1024))
            with self.assertRaisesRegex(link.HardeningError, "nonzero bytes"):
                link.read_reproducer(self.path)

    def test_bounded_records_reject_fifo_symlink_and_large_file(self):
        import os
        path = self.path.parent / "record"
        os.mkfifo(path)
        with self.assertRaises(link.HardeningError):
            link.read_record(path, 64)
        path.unlink()
        path.symlink_to(self.path)
        with self.assertRaises(OSError):
            link.read_record(path, 64)
        path.unlink()
        path.write_bytes(b"x" * 65)
        with self.assertRaises(link.HardeningError):
            link.read_record(path, 64)

    def test_metadata_read_is_bounded_before_allocation(self):
        reader = link.BoundedTarReader(io.BytesIO(b"data"))
        with self.assertRaises(link.HardeningError):
            reader.read(link.MAX_RESPONSE + 1)
        with self.assertRaises(link.HardeningError):
            reader.read(-1)

    def test_oversized_pax_metadata_rejects_before_allocation(self):
        entry = tarfile.TarInfo("inputs/pax")
        entry.type = tarfile.XHDTYPE
        entry.size = link.MAX_RESPONSE + 1
        self.path.write_bytes(entry.tobuf() + bytes(2048))
        with self.assertRaisesRegex(link.HardeningError, "metadata read"):
            link.read_reproducer(self.path)

    def test_exact_extensionless_version_script_and_generated_objects(self):
        args = ["cc", "-Wl,--version-script=/tmp/rustcAb123/list",
                "/tmp/rustcAb123/symbols.o", "/run/application-target/unit.o",
                "/run/application-target/not-an-object.txt", "/foreign/object.o"]
        self.assertEqual(link.generated_inputs(args, {}, Path("/run/application-target/application")), {
            "/tmp/rustcAb123/list", "/tmp/rustcAb123/symbols.o", "/run/application-target/unit.o"})
        with self.assertRaises(link.HardeningError):
            link.generated_inputs(["cc", "-Wl,--version-script=/tmp/rustcAb123/../list"], {}, Path("/run/application-target/application"))

    def test_copied_proxy_retains_independent_pin_even_if_transport_changes(self):
        root = self.path.parent
        proxy = root / "proxy"
        proxy.write_bytes(b"approved-proxy")
        gcc = root / "cc"
        gcc.write_bytes(b"trusted-cc")
        pin = hashlib.sha256(proxy.read_bytes()).hexdigest()
        original_identity = link.identity
        calls = 0

        def changed_transport(path):
            nonlocal calls
            if path == proxy:
                calls += 1
                if calls == 2:
                    proxy.write_bytes(b"substituted-after-initial-pin-check")
            return original_identity(path)

        with patch.object(link, "ROOT", root / "setup"), patch.object(link, "GCC", gcc), \
             patch.object(link.os, "geteuid", return_value=0), patch.object(link.os, "getegid", return_value=0), \
             patch.object(link, "identity", side_effect=changed_transport), \
             patch.object(link, "measure") as measure:
            with self.assertRaisesRegex(link.HardeningError, "independent pin"):
                link.prepare(root, proxy, pin, None)
            measure.assert_not_called()

    def test_setup_copies_expected_proxy_into_independent_inode(self):
        root = self.path.parent
        proxy = root / "proxy"
        proxy.write_bytes(b"approved-proxy")
        gcc = root / "cc"
        gcc.write_bytes(b"trusted-cc")
        pin = hashlib.sha256(proxy.read_bytes()).hexdigest()
        with patch.object(link, "ROOT", root / "setup"), patch.object(link, "GCC", gcc), \
             patch.object(link.os, "geteuid", return_value=0), patch.object(link.os, "getegid", return_value=0), \
             patch.object(link, "measure", return_value={"scope": "test-only"}), \
             contextlib.redirect_stdout(io.StringIO()):
            link.prepare(root, proxy, pin, None)
            self.assertEqual((link.ROOT / "proxy").read_bytes(), proxy.read_bytes())
            self.assertNotEqual((link.ROOT / "proxy").stat().st_ino, proxy.stat().st_ino)

    def test_response_rejects_nested_and_changed_profiles(self):
        for response in (b"--chroot elsewhere", b"--chroot . @foreign", b"--chroot . --reproduce=foreign",
                         b"--chroot . \0", b"--chroot . 'unterminated"):
            with self.subTest(response=response), self.assertRaises((link.HardeningError, ValueError)):
                link.response_arguments(response)

    def test_only_exact_premeasured_or_individually_generated_inputs_pass(self):
        measured = {"bytes": 3, "sha256": hashlib.sha256(b"abc").hexdigest()}
        inputs = {"/usr/lib/crt1.o": measured}
        records = dict(inputs, **{"/run/application-target/one.o": measured})
        self.assertEqual(link.classify_inputs(records, inputs, {"/run/application-target/one.o"}), inputs)
        for path in ("/foreign.a", "/run/application-target/../foreign.a", "/run/application-target/unlisted.o"):
            with self.subTest(path=path), self.assertRaisesRegex(link.HardeningError, "unmeasured"):
                link.classify_inputs({path: measured}, inputs, {"/run/application-target/one.o"})
        with self.assertRaisesRegex(link.HardeningError, "changed external"):
            link.classify_inputs({"/usr/lib/crt1.o": dict(measured, bytes=4)}, inputs, set())


if __name__ == "__main__":
    unittest.main()
