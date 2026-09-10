import argparse
import importlib.util
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("r65_drain_versions_tests", pathlib.Path(__file__).with_name("run-r65-drain-versions-mi300x.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class DrainVersionsRunnerTests(unittest.TestCase):
    def test_profile_rejects_old_partial_or_widened_pass(self):
        expected = runner.DrainVersionsRunner.expected_pass
        runner.owner.validate_output(expected, expected)
        r63 = ("PASS schema=fe2o3.runtime.r63-async-graph-copy.v1 bytes=1048832 "
               "owner_threads=1 streams=4 nodes=12 copies=5 joins=host "
               "canaries=complete submissions=released cleanup=complete\n")
        for output in (runner.owner.PASS, r63, expected[:-1], expected * 2,
                       expected.replace("executions=2", "executions=1"),
                       expected.replace("versions=21", "versions=20"),
                       expected.replace("version_inputs=10", "version_inputs=9"),
                       expected.replace("current_versions=13", "current_versions=12"),
                       expected.replace("occurrences=distinct", "occurrences=reused"),
                       expected.replace("idle-quiescent", "active-quiescent"),
                       expected.replace("admission=closed", "admission=open"),
                       expected.replace("released", "retained"),
                       expected.replace("cleanup=complete", "cleanup=partial")):
            with self.subTest(output=output), self.assertRaises(runner.owner.base.RunError):
                runner.owner.validate_output(output, expected)

    def test_profile_and_shared_runner_must_match_signed_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            instance = runner.DrainVersionsRunner(argparse.Namespace(), pathlib.Path(temporary))
            sources = {str(runner.owner.base.BENCH_DIR / path.name): runner.owner.base.sha256_file(path)
                       for path in (runner.HERE, runner.owner.HERE)}
            with mock.patch.object(runner.owner.base.Runner, "snapshot"):
                instance.source_hashes = sources.copy()
                instance.snapshot()
                for path in sources:
                    instance.source_hashes = dict(sources, **{path: "0" * 64})
                    with self.subTest(path=path), self.assertRaises(runner.owner.base.RunError):
                        instance.snapshot()

    def test_profile_inherits_exact_build_and_guarded_execution(self):
        profile = runner.DrainVersionsRunner
        self.assertIs(profile.build, runner.owner.OwnerRunner.build)
        self.assertIs(profile.qualify_and_measure, runner.owner.OwnerRunner.qualify_and_measure)
        self.assertIs(profile.publish, runner.owner.OwnerRunner.publish)
        self.assertEqual(runner.owner.HOST_TARGET, "x86_64-unknown-linux-musl")
        self.assertEqual(profile.example, "gfx942-runtime-r65-drain-versions")
        self.assertEqual(profile.label, "r65")
        self.assertIn("graph-local-lineage-idle-drain", profile.claim_scope)
        self.assertIn("idle-drain", profile.schema)

    def test_constructor_failure_cleans_only_r65_owned_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            sentinel = root / "unrelated"
            sentinel.touch()
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            with mock.patch.object(runner.owner.base, "parse_args", return_value=args):
                constructor = mock.Mock(side_effect=RuntimeError("constructor failed"))
                self.assertEqual(runner.owner.main(constructor, "r65"), 2)
            self.assertEqual(list(root.iterdir()), [sentinel])


if __name__ == "__main__":
    unittest.main()
