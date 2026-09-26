#!/usr/bin/env python3
import hashlib
import gzip
import json
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch

RUN = runpy.run_path(str(Path(__file__).with_name("run.py")))


class RunTests(unittest.TestCase):
    def test_exact_roster(self):
        expected = (1423, 0, 24, 0, 0)
        def output(row):
            return "test result: ok. %d passed; %d failed; %d ignored; %d measured; %d filtered out; finished\n" % row
        self.assertTrue(RUN["accepted"](0, output(expected), expected))
        for row in ((1422, 0, 24, 0, 0), (1423, 1, 24, 0, 0), (1423, 0, 23, 0, 0),
                    (1423, 0, 24, 1, 0), (1423, 0, 24, 0, 1)):
            self.assertFalse(RUN["accepted"](0, output(row), expected))
        self.assertFalse(RUN["accepted"](1, output(expected), expected))
        self.assertFalse(RUN["accepted"](0, "", expected))
        self.assertFalse(RUN["accepted"](0, output(expected) * 2, expected))

    def test_raw_git_blob_binding(self):
        raw = b"exact source\r\n"
        oid = hashlib.sha1(b"blob 14\0" + raw).hexdigest()
        self.assertEqual(RUN["bind_blob"](raw, oid), hashlib.sha256(raw).hexdigest())
        for bad in (raw.replace(b"\r\n", b"\n"), raw + b" ", b""):
            with self.assertRaises(ValueError):
                RUN["bind_blob"](bad, oid)

    def test_unsigned_staged_addition(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_bytes(b"signed input")
            def git(*args):
                return subprocess.check_output(["/usr/bin/git", "-C", directory, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("add", "Cargo.toml")
            git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
            with patch.dict(RUN["snapshot"].__globals__, ROOT=root, INPUTS=["Cargo.toml", "crates"]):
                RUN["snapshot"](root)
                (root / "crates").mkdir()
                (root / "crates/build.rs").write_bytes(b"unsigned automatic build input")
                git("add", "crates/build.rs")
                with self.assertRaises(ValueError):
                    RUN["snapshot"](root)

    def test_artifact_substitution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            executable = target / "test"
            executable.write_bytes(b"\x7fELFtest")
            row = dict(reason="compiler-artifact", package_id="path+file://" + str(root / "crates/fe2o3-runtime") + "#0.1.0",
                       target=dict(name="fe2o3_runtime", kind=["lib"], src_path=str(root / "crates/fe2o3-runtime/src/lib.rs")),
                       profile=dict(test=True), executable=str(executable))
            self.assertEqual(RUN["select_executable"](json.dumps(row), root, target), executable)
            retained = root / "test.gz"
            digest = RUN["retain_executable"](executable, retained)
            self.assertEqual(digest, hashlib.sha256(executable.read_bytes()).hexdigest())
            self.assertEqual(gzip.decompress(retained.read_bytes()), executable.read_bytes())
            for key, value in (("package_id", "other#0.1.0"), ("executable", str(root / "outside"))):
                with self.assertRaises(ValueError):
                    RUN["select_executable"](json.dumps(dict(row, **{key: value})), root, target)
            for key, value in (("kind", ["bin"]), ("src_path", str(root / "other.rs"))):
                changed = dict(row, target=dict(row["target"], **{key: value}))
                with self.assertRaises(ValueError):
                    RUN["select_executable"](json.dumps(changed), root, target)
            with self.assertRaises(ValueError):
                RUN["select_executable"]("", root, target)

    def test_roster_is_not_empty_or_partial(self):
        names = ["qualification_gfx942_mixed_duration_v1::tests::case" + str(i) for i in range(7)]
        names += ["native::native_mixed_duration_profiles_preserve_full_output_and_refund_backing",
                  "native::native_owned_later_short_completes_while_earlier_long_signal_is_pending"]
        names += ["other::case" + str(i) for i in range(1438)]
        roster = "\n".join(name + ": test" for name in names) + "\n\n1447 tests, 0 benchmarks\n"
        self.assertEqual(RUN["check_roster"](roster), names)
        for bad in ("", roster.replace(names[0], names[1]), roster.replace(names[0] + ": test", ""),
                    roster.replace("1447 tests", "1446 tests"), roster.replace("0 benchmarks", "1 benchmarks")):
            with self.assertRaises(ValueError):
                RUN["check_roster"](bad)

    def test_split_doctest_summary(self):
        expected = [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)]
        rows = ["test result: ok. %d passed; %d failed; %d ignored; %d measured; %d filtered out;\n" % row for row in expected]
        self.assertTrue(RUN["accepted"](0, "".join(rows), expected))
        for bad in (rows[0], rows[1], "".join(rows * 2), "".join(rows).replace("42 passed", "41 passed"),
                    "".join(rows).replace("0 failed", "1 failed")):
            self.assertFalse(RUN["accepted"](0, bad, expected))
        self.assertFalse(RUN["accepted"](1, "".join(rows), expected))


if __name__ == "__main__":
    unittest.main()
