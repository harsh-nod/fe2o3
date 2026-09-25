import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("completed_input_run", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class NegativeClassification(unittest.TestCase):
    def setUp(self):
        self.error = {"reason": "compiler-message", "message": {
            "level": "error", "code": {"code": "E0382"},
            "message": "use of moved value: `result`", "spans": [
                {"is_primary": True, "file_name": "src/bin/charged_result_reuse.rs"}]}}
        self.end = {"reason": "build-finished", "success": False}

    def accepts(self, rows, status=101):
        return runner.compiler_negative(status, "\n".join(map(json.dumps, rows)),
            "E0382", "use of moved value: `result`", "charged_result_reuse.rs")

    def test_exact_error(self):
        self.assertTrue(self.accepts([self.error, self.end]))

    def test_wrong_status_or_extra_error(self):
        self.assertFalse(self.accepts([self.error, self.end], 0))
        self.assertFalse(self.accepts([self.error, self.error, self.end]))

    def test_wrong_error(self):
        self.error["message"]["code"]["code"] = "E0308"
        self.assertFalse(self.accepts([self.error, self.end]))

    def test_wrong_file_or_nonprimary(self):
        span = self.error["message"]["spans"][0]
        span["file_name"] = "wrong.rs"
        self.assertFalse(self.accepts([self.error, self.end]))
        span["file_name"] = "charged_result_reuse.rs"
        span["is_primary"] = False
        self.assertFalse(self.accepts([self.error, self.end]))

    def test_typed_terminal_outcome(self):
        for wrong in [True, 0, None]:
            self.end["success"] = wrong
            self.assertFalse(self.accepts([self.error, self.end]))

    def test_missing_or_malformed_diagnostics(self):
        self.assertFalse(self.accepts([]))
        self.error["message"]["code"] = None
        self.assertFalse(self.accepts([self.error, self.end]))
        self.assertFalse(runner.compiler_negative(101, "not JSON", "E0382", "result", "file.rs"))


if __name__ == "__main__":
    unittest.main()
