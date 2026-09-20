#!/usr/bin/env python3
"""CPU plan, signal-chain, and optional C++/Rust differential qualification."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class SegmentTests(unittest.TestCase):
    def compile(self, folder, sanitizer=False):
        binary = Path(folder) / "segments-test"
        command = ["/usr/bin/g++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-pedantic"]
        if sanitizer:
            command += ["-fsanitize=undefined", "-fno-sanitize-recover=all"]
        subprocess.run(command + [str(HERE / "xgmi_peer_segments_test.cpp"), "-o", str(binary)], check=True)
        return binary

    def test_plan_and_chain_controls(self):
        for sanitizer in [False, True]:
            with self.subTest(sanitizer=sanitizer), tempfile.TemporaryDirectory(prefix="fe2o3-segments-") as tmp:
                binary = self.compile(tmp, sanitizer)
                cp = subprocess.run([str(binary)], capture_output=True, text=True, check=True)
                self.assertEqual(cp.stderr, "")
                self.assertEqual(cp.stdout, "ordered segment plans, independent bands, and chain custody: pass\n")

    @unittest.skipUnless(os.environ.get("FE2O3_SEGMENTS_RUST_BINARY"), "set FE2O3_SEGMENTS_RUST_BINARY for cross-language qualification")
    def test_rust_cpp_plans_and_data_match(self):
        rust = os.environ["FE2O3_SEGMENTS_RUST_BINARY"]
        with tempfile.TemporaryDirectory(prefix="fe2o3-segments-") as tmp:
            cpp = self.compile(tmp)
            for size in [65536, 2097152]:
                for count in [1, 65, 256, 4096]:
                    args = [str(size), str(count), "1", "2"]
                    with self.subTest(size=size, count=count):
                        a = subprocess.run([str(cpp), *args], capture_output=True, check=True)
                        b = subprocess.run([rust, "--describe-plan", *args], capture_output=True, check=True)
                        self.assertEqual(a.stderr, b"")
                        self.assertEqual(b.stderr, b"")
                        self.assertEqual(json.loads(a.stdout), json.loads(b.stdout))
            for args in [["0", "1", "0", "1"], ["1", "1", "64", "1"], ["1", "1", "0", "0"], ["1", "4097", "0", "1"], ["+1", "1", "0", "1"]]:
                self.assertNotEqual(subprocess.run([str(cpp), *args], capture_output=True).returncode, 0)
                self.assertNotEqual(subprocess.run([rust, "--describe-plan", *args], capture_output=True).returncode, 0)


if __name__ == "__main__":
    unittest.main()
