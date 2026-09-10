import argparse
import copy
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("r66_runner_tests", pathlib.Path(__file__).with_name("run-r66-coexistence-mi300x.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
checker = runner.checker


def empty_observation():
    return {"compute": None, "compute_membership": None, "copy": None,
            "copy_membership": None, "copy_packets": 0}


def fixture():
    """Synthetic parser fixture; not evidence of native publication."""
    cases = []
    for ordinal, (size, packets, direction, order) in enumerate(checker.PROFILES):
        both = {"compute": f"{ordinal * 4 + 1:064x}", "compute_membership": f"{ordinal * 4 + 2:064x}",
                "copy": f"{ordinal * 4 + 3:064x}", "copy_membership": f"{ordinal * 4 + 4:064x}", "copy_packets": packets}
        first = dict(both)
        if order == "compute-first":
            first.update(copy=None, copy_membership=None, copy_packets=0)
        else:
            first.update(compute=None, compute_membership=None)
        after_copy = dict(both, copy=None, copy_membership=None, copy_packets=0)
        cases.append(dict(ordinal=ordinal, bytes=size, packets=packets, direction=direction,
                          order=order, first=first, both=both, after_copy=after_copy,
                          after_compute=empty_observation(), canaries="complete", logical_credits=checker.expected_credits(ordinal),
                          **checker.expected_hashes(ordinal)))
    return dict(schema=checker.SCHEMA, cases=cases, owner_threads=1,
                cleanup="complete", physical_overlap="unmeasured")


def output(document):
    return json.dumps(document, separators=(",", ":")) + "\n"


class CoexistenceCheckerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.good = fixture()

    def reject(self, document):
        with self.assertRaises(checker.ValidationError):
            checker.validate_output(output(document))

    def test_complete_matrix(self):
        self.assertEqual(checker.validate_output(output(self.good)), self.good)
        runner.CoexistenceRunner.validate_qualifier_output(None, output(self.good))

    def test_file_read_is_byte_bounded_before_decode_and_parse(self):
        good = output(self.good).encode("utf-8")
        for encoded in (good, b" " * (checker.MAX_OUTPUT_BYTES + 20), b"\xff\n"):
            with self.subTest(size=len(encoded)), io.BytesIO(encoded) as stream:
                with mock.patch.object(pathlib.Path, "open", return_value=stream), \
                        mock.patch.object(stream, "read", wraps=stream.read) as read:
                    if encoded == good:
                        self.assertEqual(checker.read_output(pathlib.Path("unused")), good.decode("utf-8"))
                    else:
                        with self.assertRaises(checker.ValidationError):
                            checker.read_output(pathlib.Path("unused"))
                    read.assert_called_once_with(checker.MAX_OUTPUT_BYTES + 1)

    def test_framing_duplicate_nonfinite_and_unbounded_json(self):
        good = output(self.good)
        for bad in ("", good[:-1], good * 2, good + "\n", " " * 32769 + "\n",
                    good.replace('"owner_threads":1', '"owner_threads":1,"owner_threads":1'),
                    good.replace('"owner_threads":1', '"owner_threads":NaN'),
                    '"' + '\u00e9' * 17000 + '"\n', '\ud800\n',
                    "[]\n", "[" * 2000 + "\n"):
            with self.subTest(bad=bad[:70]), self.assertRaises(checker.ValidationError):
                checker.validate_output(bad)

    def test_claims_and_matrix_are_exact(self):
        for key, value in (("schema", "old"), ("cleanup", "partial"),
                           ("physical_overlap", "proven"), ("owner_threads", 2),
                           ("owner_threads", True), ("raw_gpu_address", 1)):
            self.reject(dict(self.good, **{key: value}))
        self.reject(dict(self.good, cases=self.good["cases"][:-1]))
        self.reject(dict(self.good, cases=list(reversed(self.good["cases"]))))
        for key in self.good:
            mutated = copy.deepcopy(self.good)
            del mutated[key]
            self.reject(mutated)
        for key, value in (("ordinal", True), ("bytes", 42), ("packets", True),
                           ("order", "copy-first"), ("direction", "d2h"),
                           ("canaries", "prefix-only")):
            mutated = copy.deepcopy(self.good)
            mutated["cases"][0][key] = value
            self.reject(mutated)

    def test_identity_shape_membership_and_reuse_fail_closed(self):
        for phase in ("first", "both", "after_copy", "after_compute"):
            for key in checker.OBSERVATION_KEYS:
                mutated = copy.deepcopy(self.good)
                del mutated["cases"][0][phase][key]
                self.reject(mutated)
        for key in ("compute", "compute_membership", "copy", "copy_membership"):
            for value in (None, "0" * 64, "f" * 63, "F" * 64, 12):
                mutated = copy.deepcopy(self.good)
                mutated["cases"][0]["both"][key] = value
                self.reject(mutated)
        mutated = copy.deepcopy(self.good)
        second = mutated["cases"][1]
        second["both"]["copy"] = second["first"]["copy"] = mutated["cases"][0]["both"]["copy"]
        self.reject(mutated)
        mutated = copy.deepcopy(self.good)
        both = mutated["cases"][0]["both"]
        both["copy_membership"] = both["copy"]
        self.reject(mutated)

    def test_retirement_and_order_require_exact_occurrence(self):
        for ordinal in (0, 1, 4, 5):
            for phase in ("first", "after_copy", "after_compute"):
                mutated = copy.deepcopy(self.good)
                mutated["cases"][ordinal][phase] = dict(mutated["cases"][ordinal]["both"])
                self.reject(mutated)
            for key in ("compute", "compute_membership"):
                mutated = copy.deepcopy(self.good)
                mutated["cases"][ordinal]["after_copy"][key] = "f" * 64
                self.reject(mutated)
        for count in (0, 2, 3, True):
            mutated = copy.deepcopy(self.good)
            mutated["cases"][0]["both"]["copy_packets"] = count
            self.reject(mutated)
        mutated = copy.deepcopy(self.good)
        mutated["cases"][1]["first"]["copy_packets"] = 2
        self.reject(mutated)

    def test_every_full_buffer_hash_is_checked_for_every_cell(self):
        for ordinal in range(8):
            for key in checker.expected_hashes(ordinal):
                mutated = copy.deepcopy(self.good)
                mutated["cases"][ordinal][key] = "f" * 64
                self.reject(mutated)


    def test_requested_allocation_credits_reject_scope_usage_and_cleanup_drift(self):
        for ordinal in range(8):
            for key, value in checker.expected_credits(ordinal).items():
                mutated = copy.deepcopy(self.good)
                mutated["cases"][ordinal]["logical_credits"][key] = value + 1 if type(value) is int else "changed"
                self.reject(mutated)
                mutated = copy.deepcopy(self.good)
                del mutated["cases"][ordinal]["logical_credits"][key]
                self.reject(mutated)


class CoexistenceRunnerTests(unittest.TestCase):
    def test_profile_inherits_all_guarded_execution_and_publication(self):
        profile = runner.CoexistenceRunner
        self.assertIs(profile.build, runner.owner.OwnerRunner.build)
        self.assertIs(profile.qualify_and_measure, runner.owner.OwnerRunner.qualify_and_measure)
        self.assertIs(profile.phase, runner.owner.base.Runner.phase)
        self.assertIs(profile.publish, runner.owner.OwnerRunner.publish)
        self.assertEqual(profile.cargo_features, ("fe2o3-runtime/hardware-qualification",))
        self.assertIn("not-physical-overlap", profile.claim_scope)

    def test_runner_checker_and_shared_runner_match_signed_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            instance = runner.CoexistenceRunner(argparse.Namespace(), pathlib.Path(temporary))
            paths = (runner.HERE, runner.owner.HERE, pathlib.Path(checker.__file__))
            sources = {str(runner.owner.base.BENCH_DIR / path.name): runner.owner.base.sha256_file(path) for path in paths}
            with mock.patch.object(runner.owner.base.Runner, "snapshot"):
                instance.source_hashes = sources.copy()
                instance.snapshot()
                for path in sources:
                    instance.source_hashes = dict(sources, **{path: "0" * 64})
                    with self.subTest(path=path), self.assertRaises(runner.owner.base.RunError):
                        instance.snapshot()

    def test_validation_exception_prevents_publish_and_cleans_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            sentinel = root / "unrelated"
            sentinel.touch()
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            events = []
            class Failing(runner.CoexistenceRunner):
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
            topology = {"measurement_cpu_list": "1", "numa_node": "0", "kfd_node": "1", "kfd_gpu_id": "2"}
            with mock.patch.object(runner.owner.base, "parse_args", return_value=args), \
                    mock.patch.object(runner.owner.base, "sealed_fields", return_value=topology), \
                    mock.patch.object(runner.owner.base, "validate_topology", return_value=topology):
                self.assertEqual(runner.owner.main(Failing, "r66"), 2)
            self.assertEqual(events, ["snapshot", "build", "placement", "guarded-phase"])
            self.assertTrue(sentinel.exists())
            self.assertFalse(list(root.glob("fe2o3-r66-owner.*")))
            self.assertFalse(list(root.glob("r66-owner-*")))
            rejected = list(root.glob("r66-rejected-*"))
            self.assertEqual(len(rejected), 1)
            self.assertIn("evidence rejected", (rejected[0] / "rejection.json").read_text())


if __name__ == "__main__":
    unittest.main()
