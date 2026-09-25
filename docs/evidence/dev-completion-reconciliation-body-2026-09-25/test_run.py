import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("reconciliation_run", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ExtractionChecks(unittest.TestCase):
    def setUp(self):
        self.previous = runner.git("show", runner.BASELINE + ":" + str(runner.CONTEXT)).decode()
        self.shared = (runner.ROOT / runner.BODY).read_text()
        self.production = (runner.ROOT / runner.CONTEXT).read_text()

    def check(self):
        return runner.compare_sources(self.previous, self.shared, self.production)

    def test_same_body(self):
        old, new = self.check()
        self.assertEqual(old, new)

    def test_changed_status_gate(self):
        self.shared = self.shared.replace("Some(BackendPollV1::Succeeded) => {}", "Some(BackendPollV1::Pending) => {}")
        with self.assertRaisesRegex(ValueError, "planner statements changed"):
            self.check()

    def test_changed_final_identity(self):
        self.shared = self.shared.replace("submissions[&$requested].status", "submissions[&id].status")
        with self.assertRaisesRegex(ValueError, "planner statements changed"):
            self.check()

    def test_extra_production_statement(self):
        self.production = self.production.replace(runner.INVOCATION, "        let _extra = 0;\n" + runner.INVOCATION)
        with self.assertRaisesRegex(ValueError, "exact empty-hook"):
            self.check()

    def test_production_hook(self):
        self.production = self.production.replace(runner.INVOCATION, runner.INVOCATION.replace("[]", "[let _extra = 0;]"))
        with self.assertRaisesRegex(ValueError, "exact empty-hook"):
            self.check()

    def test_missing_or_moved_step_hook(self):
        self.shared = self.shared.replace("                $($on_step)*\n", "")
        with self.assertRaisesRegex(ValueError, "exact loop counter hook"):
            self.check()
        self.shared = self.shared.replace("            let mut length", "            $($on_step)*\n            let mut length")
        with self.assertRaisesRegex(ValueError, "exact loop counter hook"):
            self.check()


if __name__ == "__main__":
    unittest.main()
