#!/usr/bin/env python3
import hashlib
from pathlib import Path
import runpy
import unittest

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


if __name__ == "__main__":
    unittest.main()
