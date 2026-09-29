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


def cell_fixture(ordinal):
    return dict(schema=checker.CELL_SCHEMA, unique_id=checker.UNIQUE_ID,
                case=fixture()["cases"][ordinal], owner_threads=1,
                process_scope="one-cell-per-process", cleanup="complete", physical_overlap="unmeasured")


def endpoint_fixture():
    """Synthetic complete strict observation, never live-host evidence."""
    ticks = iter(range(30_000_000_000, 30_000_000_100))
    def stamp():
        return {"utc": "2026-09-29T00:00:00.000000001Z", "monotonic_ns": next(ticks)}
    def snapshot():
        return {"started": stamp(), "finished": stamp(), "path": "/sys/bus/pci/devices/" + runner.endpoint.BDF,
                "values": {key: runner.endpoint.UID[2:] if key == "unique_id" else "0"
                           for key in runner.observer.METRICS}, "errors": {}}
    def capture(arguments, stdout):
        return {"command": ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi", *arguments],
                "started": stamp(), "finished": stamp(), "exit": 0, "error": None, "stdout": stdout, "stderr": ""}
    value = {"schema": runner.observer.SCHEMA, "record": "observation", "index": 0, "gpu_index": 1,
             "pci_bdf": runner.endpoint.BDF, "unique_id": runner.endpoint.UID, "started": stamp()}
    before = snapshot()
    value["status"] = capture(["--showuse", "--showmeminfo", "vram", "--showuniqueid", "--showbus", "--json"],
                              json.dumps({"card1": {"Unique ID": runner.endpoint.UID, "PCI Bus": runner.endpoint.BDF,
                                                     "GPU use (%)": "0", "VRAM Total Used Memory (B)": "0"}}))
    between = snapshot()
    value["pids"] = capture(["--showpidgpus"], "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\n"
                            "PID 9999 is using 0 DRM device(s)\n===\n=== End of ROCm SMI Log ===\n")
    value.update(sysfs=[before, between, snapshot()], finished=stamp(), endpoint_admitted=True, reasons=[], selected_pids=[],
                 scope="sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
                 visibility_filters="removed-for-cli", vram_limit_exclusive=512 * 1024 * 1024)
    complete = {"schema": runner.observer.SCHEMA, "record": "complete", "observations": 1, "refused": 0,
                "all_endpoints_admitted": True, "performance_accepted": False}
    return value, complete


def publication_fixture(root):
    """Synthetic completed controller records; this never runs a child process."""
    stage, destination = root / "stage", root / "published"
    stage.mkdir()
    destination.mkdir()
    instance = runner.CoexistenceRunner(argparse.Namespace(output_dir=destination), stage)
    instance.commit, instance.snapshot_input_hashes, instance.tool_hashes = "1" * 40, {}, {}
    instance.topology = {"measurement_cpu_list": "0-47", "numa_node": "0", "kfd_gpu_id": "12345", "observer_cpu": "95"}
    instance.guard = stage / "r26-host-guard.py"
    instance.retained = runner.owner.base.load_module(runner.HERE.with_name("check-parity.py"), "r66_publication_fixture_checker")
    binary = stage / "binary"
    binary.write_bytes(b"synthetic-only-not-an-executable")
    instance.binaries = {"kfd": binary}
    instance.binary_hashes = {"kfd": runner.owner.base.sha256_file(binary)}
    instance.retain_owner_binary()
    (instance.evidence / "source.tar").write_bytes(b"synthetic-source")
    cells = []
    for index in range(16):
        repetition, ordinal = divmod(index, 8)
        label = f"owner-{repetition}-cell-{ordinal}"
        observed = instance.evidence / (label + ".jsonl")
        observed.write_text(output(cell_fixture(ordinal)))
        argv = ["/usr/bin/python3", instance.guard, "monitor", "--gpu-id", "12345", "--observer-cpu", "95",
                "--target-output", observed, "--", *instance.phase_command("kfd", [runner.owner.base.UNIQUE_ID, "--case", str(ordinal)])]
        monitor = instance.evidence / (label + "-monitor.stdout")
        values = dict(zip(instance.retained.R26_MONITOR_SEALED_FIELDS, (
            "fe2o3.r26-kfd-queue-monitor.v2", "clean", "selected-kfd-gpu-process-tree-census-v2",
            "absolute-monotonic-raw-deadline-v1", "12345", str(100 + index), str(100 + index), "95",
            "2000", "10000", "3000", "10", "5", "0", "0", "0", "1", "1",
            str(observed.stat().st_size), runner.owner.base.sha256_file(observed),
        )))
        text = "monitor " + " ".join(f"{key}={values[key]}" for key in instance.retained.R26_MONITOR_SEALED_FIELDS)
        digest = runner.hashlib.sha256((text + "\n").encode()).hexdigest()
        monitor.write_text(text + " monitor_sha256=" + digest + "\n")
        instance.commands.append({"argv": [str(part) for part in argv], "returncode": 0,
                                  "stdout": str(monitor.relative_to(stage))})
        cells.append({"repetition": repetition, "ordinal": ordinal, "phase": label, "output": observed.name,
                      "output_sha256": runner.owner.base.sha256_file(observed)})
    runner.owner.base.write_json(instance.evidence / "isolated-cells.json", {
        "schema": instance.schema, "qualification_runs": 16, "repetitions": 2, "cells_per_repetition": 8,
        "cells": cells, "process_scope": "one-cell-per-monitored-process", "occurrence_scope": "process-local",
        "strict_endpoint_timing": "sequential-not-target-t0-relative", "modern_endpoint_acceptance": False,
        "physical_overlap": "unmeasured", "exclusive_reservation": False, "performance_acceptance": False,
    })
    return instance


class CoexistenceCheckerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.good = fixture()

    def reject(self, document):
        with self.assertRaises(checker.ValidationError):
            checker.validate_output(output(document))

    def test_complete_matrix(self):
        self.assertEqual(checker.validate_output(output(self.good)), self.good)

    def test_all_isolated_cells_and_historical_schema_are_distinct(self):
        for ordinal in range(8):
            document = cell_fixture(ordinal)
            self.assertEqual(checker.validate_cell_output(output(document), ordinal), document)
            self.assertEqual(runner.CoexistenceRunner.validate_qualifier_output(None, output(document), ordinal), document)
            with self.assertRaises(checker.ValidationError):
                checker.validate_output(output(document))
        with self.assertRaises(checker.ValidationError):
            checker.validate_cell_output(output(self.good), 0)

    def test_cell_requires_exact_external_selector_and_all_fields(self):
        good = cell_fixture(0)
        for ordinal in (1, -1, 8, True, "0", None):
            with self.subTest(ordinal=ordinal), self.assertRaises(checker.ValidationError):
                checker.validate_cell_output(output(good), ordinal)
        for key in good:
            changed = copy.deepcopy(good)
            del changed[key]
            with self.subTest(missing=key), self.assertRaises(checker.ValidationError):
                checker.validate_cell_output(output(changed), 0)
        for key, value in (("unique_id", 1), ("owner_threads", True), ("process_scope", "all-cells"),
                           ("cleanup", "partial"), ("physical_overlap", "measured"), ("extra", True)):
            with self.subTest(key=key), self.assertRaises(checker.ValidationError):
                checker.validate_cell_output(output(dict(good, **{key: value})), 0)

    def test_isolated_identity_uniqueness_is_process_local(self):
        first, another = cell_fixture(0), cell_fixture(2)
        for phase in ("first", "both", "after_copy", "after_compute"):
            another["case"][phase] = copy.deepcopy(first["case"][phase])
        checker.validate_cell_output(output(first), 0)
        checker.validate_cell_output(output(another), 2)
        historical = copy.deepcopy(self.good)
        historical["cases"][2] = another["case"]
        self.reject(historical)
        duplicate = copy.deepcopy(first)
        both = duplicate["case"]["both"]
        both["copy"] = both["compute"]
        with self.assertRaises(checker.ValidationError):
            checker.validate_cell_output(output(duplicate), 0)

    def test_every_isolated_cell_retains_full_hash_and_credit_checks(self):
        for ordinal in range(8):
            good = cell_fixture(ordinal)
            for key in checker.expected_hashes(ordinal):
                changed = copy.deepcopy(good)
                changed["case"][key] = "f" * 64
                with self.subTest(ordinal=ordinal, key=key), self.assertRaises(checker.ValidationError):
                    checker.validate_cell_output(output(changed), ordinal)
            changed = copy.deepcopy(good)
            changed["case"]["logical_credits"]["after_cleanup"] = "retained"
            with self.assertRaises(checker.ValidationError):
                checker.validate_cell_output(output(changed), ordinal)

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
        self.assertIs(profile.telemetry, runner.owner.OwnerRunner.telemetry)
        self.assertIs(profile.publish, runner.owner.OwnerRunner.publish)
        self.assertEqual(profile.cargo_features, ("fe2o3-runtime/hardware-qualification",))
        self.assertIn("not-physical-overlap", profile.claim_scope)
        self.assertIn("no-modern-t0-20s-acceptance", profile.claim_scope)
        self.assertIn("development", profile.schema)
        self.assertEqual(runner.owner.OwnerRunner.qualification_runs, 2)
        self.assertEqual(profile.qualification_runs, 16)

    def test_runner_checker_and_shared_runner_match_signed_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            instance = runner.CoexistenceRunner(argparse.Namespace(), pathlib.Path(temporary))
            paths = (runner.HERE, runner.owner.HERE, pathlib.Path(checker.__file__), runner.ENDPOINT_PATH, runner.OBSERVER_PATH)
            sources = {str(path.relative_to(runner.HERE.parents[2])): runner.owner.base.sha256_file(path) for path in paths}
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
                    path = self.evidence / "invalid.jsonl"
                    path.write_text("invalid\n")
                    return path
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

    def test_two_exact_ordered_eight_process_matrices_and_stop_on_bad_cell(self):
        topology = {"measurement_cpu_list": "1", "numa_node": "0", "kfd_node": "1", "kfd_gpu_id": "2"}
        for bad_cell in (None, 3, 8, 15):
            with self.subTest(bad_cell=bad_cell), tempfile.TemporaryDirectory() as temporary:
                instance = runner.CoexistenceRunner(argparse.Namespace(), pathlib.Path(temporary))
                instance.retained = mock.Mock()
                instance.topology_record = mock.Mock(return_value="synthetic-topology")
                instance.run = mock.Mock()
                instance.verify_source = mock.Mock()
                phases = []
                def phase(label, backend, args):
                    index = len(phases)
                    phases.append((label, backend, args))
                    path = instance.evidence / (label + ".jsonl")
                    ordinal = index % 8
                    document = cell_fixture((ordinal + 1) % 8 if index == bad_cell else ordinal)
                    path.write_text(output(document))
                    return path
                instance.phase = phase
                with mock.patch.object(runner.owner.base, "sealed_fields", return_value=topology), \
                        mock.patch.object(runner.owner.base, "validate_topology", return_value=topology):
                    if bad_cell is None:
                        instance.qualify_and_measure()
                    else:
                        with self.assertRaises(runner.owner.base.RunError):
                            instance.qualify_and_measure()
                count = 16 if bad_cell is None else bad_cell + 1
                self.assertEqual(phases, [(f"owner-{index // 8}-cell-{index % 8}", "kfd",
                                          [runner.owner.base.UNIQUE_ID, "--case", str(index % 8)]) for index in range(count)])
                manifest = instance.evidence / "isolated-cells.json"
                self.assertEqual(manifest.exists(), bad_cell is None)
                if bad_cell is None:
                    document = json.loads(manifest.read_text())
                    self.assertEqual(len(document["cells"]), 16)
                    self.assertEqual((document["qualification_runs"], document["repetitions"], document["cells_per_repetition"]),
                                     (16, 2, 8))
                    self.assertFalse(document["modern_endpoint_acceptance"])
                    self.assertEqual(document["occurrence_scope"], "process-local")
                    self.assertEqual(document["strict_endpoint_timing"], "sequential-not-target-t0-relative")
                    self.assertFalse(document["performance_acceptance"])
                    instance.verify_source.assert_called_once_with()

    def test_publication_counts_actual_sixteen_cells_not_two_matrices(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            try:
                instance = publication_fixture(root)
                published = instance.publish()
                provenance = json.loads((published / "provenance.json").read_text())
                self.assertEqual((provenance["qualification_runs"], provenance["repetitions"], provenance["cells_per_repetition"]),
                                 (16, 2, 8))
                self.assertFalse(provenance["modern_endpoint_acceptance"])
                self.assertEqual(provenance["isolated_cells_sha256"], runner.owner.base.sha256_file(published / "isolated-cells.json"))
                self.assertEqual(runner.owner.OwnerRunner.qualification_metadata(mock.Mock(qualification_runs=2)),
                                 {"qualification_runs": 2})
            finally:
                runner.owner.base.cleanup_tree(root)

    def test_publication_rejects_stale_configuration_and_incomplete_actual_rosters(self):
        for mutation in ("inherited-count", "manifest-count", "missing-cell", "duplicate-cell", "reordered-cell",
                         "missing-monitor", "duplicate-monitor", "wrong-command", "failed-monitor", "changed-output"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary)
                try:
                    instance = publication_fixture(root)
                    path = instance.evidence / "isolated-cells.json"
                    document = json.loads(path.read_text())
                    if mutation == "inherited-count":
                        instance.qualification_runs = runner.owner.OwnerRunner.qualification_runs
                    elif mutation == "manifest-count":
                        document["qualification_runs"] = 2
                    elif mutation == "missing-cell":
                        document["cells"].pop()
                    elif mutation == "duplicate-cell":
                        document["cells"][-1] = document["cells"][-2]
                    elif mutation == "reordered-cell":
                        document["cells"][0], document["cells"][1] = document["cells"][1], document["cells"][0]
                    elif mutation == "missing-monitor":
                        instance.commands.pop()
                    elif mutation == "duplicate-monitor":
                        instance.commands[-1] = instance.commands[-2]
                    elif mutation == "wrong-command":
                        instance.commands[-1]["argv"][-1] = "6"
                    elif mutation == "failed-monitor":
                        instance.commands[-1]["returncode"] = 1
                    elif mutation == "changed-output":
                        (instance.evidence / document["cells"][-1]["output"]).write_text(output(cell_fixture(6)))
                    path.write_text(output(document))
                    with self.assertRaises(runner.owner.base.RunError):
                        instance.publish()
                    self.assertFalse(list(instance.args.output_dir.iterdir()))
                finally:
                    runner.owner.base.cleanup_tree(root)

    def test_publication_rejects_replayed_monitor_content_at_distinct_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            try:
                instance = publication_fixture(root)
                for ordinal in range(instance.cells_per_repetition):
                    original = instance.stage / instance.commands[ordinal]["stdout"]
                    replayed = instance.stage / instance.commands[ordinal + instance.cells_per_repetition]["stdout"]
                    self.assertNotEqual(original, replayed)
                    self.assertNotEqual(original.read_bytes(), replayed.read_bytes())
                    replayed.write_bytes(original.read_bytes())
                self.assertEqual(len({row["stdout"] for row in instance.commands}), 16)
                with self.assertRaisesRegex(runner.owner.base.RunError, "distinct sealed monitor records"):
                    instance.publish()
                self.assertFalse(list(instance.args.output_dir.iterdir()))
            finally:
                runner.owner.base.cleanup_tree(root)

    def test_strict_phase_preserves_original_guard_and_postflight_on_failure(self):
        for failure_at in (None, "before", "guard", "after", "guard-and-after"):
            with self.subTest(failure_at=failure_at), tempfile.TemporaryDirectory() as temporary:
                instance = runner.CoexistenceRunner(argparse.Namespace(), pathlib.Path(temporary))
                events = []
                def strict(label):
                    events.append(label)
                    if label == "cell-before" and failure_at == "before":
                        raise runner.owner.base.RunError("before refused")
                    if label == "cell-after" and failure_at in ("after", "guard-and-after"):
                        raise runner.owner.base.RunError("after refused")
                def guard(label, backend, args):
                    events.append("unchanged-guard")
                    if failure_at in ("guard", "guard-and-after"):
                        raise runner.owner.base.RunError("guard refused")
                    return "observed-output"
                instance.strict_endpoint = strict
                with mock.patch.object(runner.owner.OwnerRunner, "phase", side_effect=guard) as original:
                    if failure_at is None:
                        self.assertEqual(instance.phase("cell", "kfd", ["uid", "--case", "0"]), "observed-output")
                    else:
                        with self.assertRaises(runner.owner.base.RunError):
                            instance.phase("cell", "kfd", ["uid", "--case", "0"])
                    if failure_at == "before":
                        original.assert_not_called()
                    else:
                        original.assert_called_once_with("cell", "kfd", ["uid", "--case", "0"])
                self.assertEqual(events, ["cell-before"] if failure_at == "before"
                                 else ["cell-before", "unchanged-guard", "cell-after"])

    def test_strict_endpoint_replays_raw_data_and_does_not_relax_busy_policy(self):
        for mutation in (None, "gpu", "memory", "vram", "pid", "complete", "stderr", "helper"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary)
                instance = runner.CoexistenceRunner(argparse.Namespace(), root)
                instance.bench = root / "bench"
                instance.bench.mkdir()
                helper = instance.bench / runner.OBSERVER_PATH.name
                helper.write_bytes(runner.OBSERVER_PATH.read_bytes() if mutation != "helper" else b"changed")
                (root / "stderr").write_bytes(b"diagnostic" if mutation == "stderr" else b"")
                instance.commands = [{"stderr": "stderr"}]
                value, complete = endpoint_fixture()
                if mutation in ("gpu", "memory", "vram"):
                    field = {"gpu": "gpu_busy_percent", "memory": "mem_busy_percent", "vram": "mem_info_vram_used"}[mutation]
                    value["sysfs"][0]["values"][field] = str(512 * 1024 * 1024) if mutation == "vram" else "1"
                elif mutation == "pid":
                    value["pids"]["stdout"] = value["pids"]["stdout"].replace("0 DRM device(s)", "1 DRM device(s):\n1")
                elif mutation == "complete":
                    complete["all_endpoints_admitted"] = False
                instance.run = mock.Mock(return_value=output(value) + output(complete))
                if mutation is None:
                    instance.strict_endpoint("cell-before")
                    self.assertEqual(instance.run.call_args.kwargs, {"label": "strict-cell-before", "timeout": 100,
                                                                   "limit": 4 * 1024 * 1024})
                    self.assertEqual(instance.run.call_args.args[0], ["/usr/bin/python3", "-I", "-B", helper,
                                                                     "--gpu-index", "1", "--pci-bdf", runner.endpoint.BDF,
                                                                     "--unique-id", runner.endpoint.UID])
                else:
                    with self.assertRaises(runner.owner.base.RunError):
                        instance.strict_endpoint("cell-before")


if __name__ == "__main__":
    unittest.main()
