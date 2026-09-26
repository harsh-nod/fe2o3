#!/usr/bin/env python3
import gzip
import hashlib
from pathlib import Path
import runpy
import unittest

VERIFY = runpy.run_path(str(Path(__file__).with_name("verify.py")))


class ElfJoinTests(unittest.TestCase):
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
