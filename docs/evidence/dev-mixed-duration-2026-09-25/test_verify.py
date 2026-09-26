#!/usr/bin/env python3
import gzip
import hashlib
from pathlib import Path
import runpy
import tarfile
import unittest

VERIFY = runpy.run_path(str(Path(__file__).with_name("verify.py")))


class ElfJoinTests(unittest.TestCase):
    def test_auxiliary_command_substitution(self):
        commands = VERIFY["auxiliary_commands"]()
        self.assertEqual(set(commands), {"runner-tests", "fixture-rebuild", "rustc", "short-disassembly", "long-disassembly"})
        for expected in commands.values():
            VERIFY["check_command"](dict(command=expected), expected)
            for wrong in (["/bin/true"], expected[:-1]):
                with self.assertRaises(ValueError):
                    VERIFY["check_command"](dict(command=wrong), expected)

    def test_missing_or_duplicate_roster(self):
        rows = ["test_" + str(index) for index in range(1438)]
        rows += ["qualification_gfx942_mixed_duration_v1::tests::test_" + str(index) for index in range(7)]
        rows += ["kfd_backend::retained_release_tests::mixed_duration::" + name for name in (
            "native_mixed_duration_profiles_preserve_full_output_and_refund_backing",
            "native_owned_later_short_completes_while_earlier_long_signal_is_pending")]
        def render(names):
            return "\n".join(name + ": test" for name in names) + "\n1447 tests, 0 benchmarks\n"
        self.assertEqual(VERIFY["check_roster"](render(rows)), rows)
        for wrong in ("", render(rows[:-1]), render([*rows[:-1], rows[0]]), render(rows).replace("1447 tests", "1446 tests")):
            with self.assertRaises(ValueError):
                VERIFY["check_roster"](wrong)

    def test_archive_roster_rejects_duplicates_and_special_paths(self):
        entry = tarfile.TarInfo("crates/source.rs")
        self.assertEqual(set(VERIFY["check_members"]([entry])), {"crates/source.rs"})
        for name in ("../outside", "/absolute", "crates/../outside", "crates//source.rs"):
            with self.assertRaises(ValueError):
                VERIFY["check_members"]([tarfile.TarInfo(name)])
        with self.assertRaises(ValueError):
            VERIFY["check_members"]([entry, entry])
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE):
            entry = tarfile.TarInfo("special")
            entry.type = kind
            with self.assertRaises(ValueError):
                VERIFY["check_members"]([entry])

    def test_raw_commands_and_doctest_summaries(self):
        expected = ["cargo", "clippy", "--", "-D", "warnings"]
        VERIFY["check_command"](dict(command=expected), expected)
        for command in (["/bin/true"], expected[:-1], [*expected, "--cap-lints=allow"]):
            with self.assertRaises(ValueError):
                VERIFY["check_command"](dict(command=command), expected)
        summary = "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n" + \
                  "test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n"
        rows = [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]]
        self.assertEqual(VERIFY["raw_counts"](summary), rows)
        for altered in ("", summary.replace("42 passed", "41 passed"), summary.replace("0 failed", "1 failed"), summary * 2):
            self.assertNotEqual(VERIFY["raw_counts"](altered), rows)

    def test_retained_executed_bytes_must_match(self):
        raw = b"\x7fELFbounded calibration only"
        compressed = gzip.compress(raw, mtime=0)
        metadata = dict(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
                        gzip_sha256=hashlib.sha256(compressed).hexdigest(), path="/owned/test")
        result = dict(sha256=metadata["sha256"])
        record = dict(command=[metadata["path"], "--test-threads=2"])
        VERIFY["check_elf"](compressed, metadata, result, record)
        for candidate in (
            (compressed, metadata, dict(sha256="0" * 64), record),
            (compressed, dict(metadata, sha256="0" * 64), result, record),
            (compressed, dict(metadata, bytes=len(raw) - 1), result, record),
            (compressed, dict(metadata, bytes=len(raw) + 1), result, record),
            (compressed, dict(metadata, bytes=1024**3), result, record),
            (gzip.compress(raw + b"x"), metadata, result, record),
            (compressed, metadata, result, dict(command=["cargo", "test"])),
            (compressed, metadata, result, dict(command=[metadata["path"], "--list"])),
        ):
            with self.assertRaises(ValueError):
                VERIFY["check_elf"](*candidate)


if __name__ == "__main__":
    unittest.main()
