"""Runner guards tested with synthetic parser fixtures, never hardware evidence."""
import argparse
import importlib.util
import pathlib
import tempfile
import unittest
from unittest import mock

from test_check_drain_capture import encoded, fixture

spec = importlib.util.spec_from_file_location(
    "drain_capture_runner_tests", pathlib.Path(__file__).with_name("run-drain-capture-mi300x.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class DrainCaptureRunnerTests(unittest.TestCase):
    def test_profile_inherits_guarded_execution_and_publication(self):
        profile = runner.DrainCaptureRunner
        self.assertIs(profile.build, runner.owner.OwnerRunner.build)
        self.assertIs(profile.qualify_and_measure, runner.owner.OwnerRunner.qualify_and_measure)
        self.assertIs(profile.phase, runner.owner.base.Runner.phase)
        self.assertIs(profile.publish, runner.owner.OwnerRunner.publish)
        self.assertEqual(profile.cargo_features, ("fe2o3-runtime/hardware-qualification",))
        self.assertEqual(profile.schema, runner.checker.SCHEMA)
        self.assertIn("not-physical-overlap", profile.claim_scope)
        self.assertIn("no-performance-claim", profile.claim_scope)
        profile.validate_qualifier_output(None, encoded(fixture()))

    def test_runner_checker_and_shared_runner_match_signed_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            instance = runner.DrainCaptureRunner(argparse.Namespace(), pathlib.Path(temporary))
            paths = (runner.HERE, runner.owner.HERE, pathlib.Path(runner.checker.__file__))
            sources = {str(runner.owner.base.BENCH_DIR / path.name):
                       runner.owner.base.sha256_file(path) for path in paths}
            with mock.patch.object(runner.owner.base.Runner, "snapshot"):
                instance.source_hashes = sources.copy()
                instance.snapshot()
                for path in sources:
                    instance.source_hashes = dict(sources, **{path: "0" * 64})
                    with self.subTest(path=path), self.assertRaises(runner.owner.base.RunError):
                        instance.snapshot()

    def test_validation_failure_prevents_publish_and_cleans_owned_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            sentinel = root / "unrelated"
            sentinel.touch()
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            events = []

            class Failing(runner.DrainCaptureRunner):
                def snapshot(self):
                    events.append("snapshot")
                    self.retained = mock.Mock()

                def build(self): events.append("build")
                def topology_record(self, _): return {}
                def run(self, *args, **kwargs): events.append("placement")

                def phase(self, *args):
                    events.append("guarded-phase")
                    return mock.Mock(read_text=lambda: "invalid\n")

                def publish(self):
                    events.append("publish")
                    raise AssertionError("validation bypassed")

            topology = {"measurement_cpu_list": "1", "numa_node": "0",
                        "kfd_node": "1", "kfd_gpu_id": "2"}
            with mock.patch.object(runner.owner.base, "parse_args", return_value=args), \
                    mock.patch.object(runner.owner.base, "sealed_fields", return_value=topology), \
                    mock.patch.object(runner.owner.base, "validate_topology", return_value=topology):
                self.assertEqual(runner.owner.main(Failing, "drn2"), 2)
            self.assertEqual(events, ["snapshot", "build", "placement", "guarded-phase"])
            self.assertTrue(sentinel.exists())
            self.assertFalse(list(root.glob("fe2o3-drn2-owner.*")))
            self.assertFalse(list(root.glob("drn2-owner-*")))
            rejected = list(root.glob("drn2-rejected-*"))
            self.assertEqual(len(rejected), 1)
            self.assertIn("evidence rejected", (rejected[0] / "rejection.json").read_text())


if __name__ == "__main__":
    unittest.main()
