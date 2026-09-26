#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("observer_packet_tests", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)


class ReplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.stdout = (HERE / "raw/gnu/stdout.log").read_text()
        cls.names = V.R.roster((HERE / "raw/gnu-roster/stdout.log").read_text())

    def rejects(self, stdout, names=None):
        with self.assertRaises(ValueError):
            V.runtime_result(stdout, self.names if names is None else names)

    def test_original_failure_remains_failure(self):
        result = V.runtime_result(self.stdout, self.names)
        self.assertEqual(result, dict(passed=1422, failed=sorted(V.FAILURES), ignored=27, status=101))

    def test_incomplete_or_duplicated_summary_rejected(self):
        self.rejects(self.stdout.replace("test result: FAILED.", "missing summary:"))
        self.rejects(self.stdout + "test result: FAILED. 1422 passed; 3 failed; 27 ignored; 0 measured; 0 filtered out;\n")

    def test_missing_duplicate_or_foreign_row_rejected(self):
        row = next(line for line in self.stdout.splitlines() if line.startswith("test ") and line.endswith(" ... ok"))
        self.rejects(self.stdout.replace(row + "\n", "", 1))
        self.rejects(self.stdout + row + "\n")
        self.rejects(self.stdout.replace(row, "test fabricated::case ... ok", 1))

    def test_error_and_failure_identity_are_required(self):
        self.rejects(self.stdout.replace("InspectSocket", "OtherError"))
        self.rejects(self.stdout.replace("code: 1, kind: PermissionDenied", "code: 2, kind: NotFound"))
        name = sorted(V.FAILURES)[0]
        self.rejects(self.stdout.replace(name, "unrelated::failure"), ["unrelated::failure" if item == name else item for item in self.names])

    def test_relabelled_pass_and_filtered_suite_rejected(self):
        self.rejects(self.stdout.replace("1422 passed; 3 failed", "1425 passed; 0 failed"))
        self.rejects(self.stdout.replace("0 filtered out;", "3 filtered out;"))
        self.rejects(self.stdout.replace(" ... FAILED", " ... ok")
                     .replace("test result: FAILED.", "test result: ok.")
                     .replace("1422 passed; 3 failed", "1425 passed; 0 failed"))

    def test_native_pass_cannot_be_claimed_from_ignore(self):
        name = next(item for item in self.names if item.endswith("::" + V.R.NATIVE_NAMES[0]))
        row = next(line for line in self.stdout.splitlines() if line.startswith("test " + name + " ... "))
        changed = self.stdout.replace(row, "test " + name + " ... ok")
        self.rejects(changed.replace("1422 passed; 3 failed; 27 ignored", "1423 passed; 3 failed; 26 ignored"))

    def test_exact_command_required(self):
        V.V.check_command(dict(command=["test-elf", "--test-threads=2"]), ["test-elf", "--test-threads=2"])
        with self.assertRaises(ValueError):
            V.V.check_command(dict(command=["test-elf", "--skip", "telemetry"]), ["test-elf", "--test-threads=2"])


if __name__ == "__main__":
    unittest.main()
