import argparse
import importlib.util
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("r62_control_tests", pathlib.Path(__file__).with_name("run-r62-control-mi300x.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ControlRunnerTests(unittest.TestCase):
    def test_profile_rejects_old_or_partial_pass(self):
        expected = runner.ControlRunner.expected_pass
        runner.owner.validate_output(expected, expected)
        for output in (runner.owner.PASS, expected[:-1], expected * 2,
                       expected.replace("not_submitted", "submitted"),
                       expected.replace("retained", "lost"),
                       expected.replace("complete", "partial")):
            with self.assertRaises(runner.owner.base.RunError):
                runner.owner.validate_output(output, expected)

    def test_both_profile_and_shared_runner_must_match_signed_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            instance = runner.ControlRunner(argparse.Namespace(), pathlib.Path(temporary))
            sources = {str(runner.owner.base.BENCH_DIR / path.name): runner.owner.base.sha256_file(path)
                       for path in (runner.HERE, runner.owner.HERE)}
            with mock.patch.object(runner.owner.base.Runner, "snapshot"):
                instance.source_hashes = sources.copy()
                instance.snapshot()
                for path in sources:
                    instance.source_hashes = dict(sources, **{path: "0" * 64})
                    with self.assertRaises(runner.owner.base.RunError):
                        instance.snapshot()

    def test_profile_keeps_static_musl_and_nonperformance_scope(self):
        self.assertEqual(runner.owner.HOST_TARGET, "x86_64-unknown-linux-musl")
        self.assertEqual(runner.ControlRunner.example, "gfx942-runtime-r62-async-control")
        self.assertEqual(runner.ControlRunner.label, "r62")
        self.assertIn("pre-submit-cancel", runner.ControlRunner.claim_scope)
        self.assertIn("recoverable-timeout", runner.ControlRunner.claim_scope)

    def test_constructor_failure_cleans_only_r62_owned_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            sentinel = root / "unrelated"
            sentinel.touch()
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            with mock.patch.object(runner.owner.base, "parse_args", return_value=args):
                constructor = mock.Mock(side_effect=RuntimeError("constructor failed"))
                self.assertEqual(runner.owner.main(constructor, "r62"), 2)
            self.assertEqual(list(root.iterdir()), [sentinel])


if __name__ == "__main__":
    unittest.main()
