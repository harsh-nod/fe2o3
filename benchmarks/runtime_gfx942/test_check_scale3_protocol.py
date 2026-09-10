import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("scale3_protocol_tests", pathlib.Path(__file__).with_name("check-scale3-protocol.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def identity(index):
    return hashlib.sha256(f"synthetic-parser-fixture-{index}".encode()).hexdigest()


def jsonl(records):
    return "".join(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n" for record in records)


def plan_fixture():
    return {"schema": checker.PLAN_SCHEMA, "campaign_id": identity(1), "source_commit": "1" * 40,
            "host_boot_id": "11111111-2222-3333-4444-555555555555",
            "gpu": {"unique_id": "1234567890abcdef", "pci_bdf": "0000:26:00.0", "kfd_gpu_id": 7,
                    "target": "gfx942:xnack-", "topology_sha256": identity(2), "numa_node": 1, "measurement_cpus": [4, 5]},
            "producers": {backend: {"binary_sha256": identity(10 + index), "toolchain_closure_sha256": identity(20 + index),
                                     "runtime_version": "synthetic-fixture-v1"} for index, backend in enumerate(checker.BACKENDS)},
            "semantics": dict(checker.SEMANTICS),
            "workload": {"name": "synthetic-parser-fixture", "artifact_sha256": identity(30),
                         "abi_sha256": identity(31), "effects_sha256": identity(32), "oracle_sha256": identity(33),
                         "argument_template_sha256": identity(34),
                         "kernel": "fixture", "grid": [1024, 1, 1], "workgroup": [256, 1, 1], "kernarg_bytes": 16,
                         "buffers": [{"name": name, "memory": memory, "bytes": size, "initial_sha256": identity(40 + index),
                                      "expected_sha256": identity(50 + index)} for index, (name, memory, size) in enumerate(
                                          (("compute", "device-local", 4096), ("upload", "host-visible", 2048), ("copy", "device-local", 2048)))],
                         "kernel_bindings": [0], "copy": {"direction": "h2d", "source": 1, "destination": 2,
                                                            "source_offset": 128, "destination_offset": 128, "bytes": 1024}},
            "policy": {"rounds": 3, "warmups_per_block": 1, "samples_per_block": 2, "rotation": list(checker.BACKENDS),
                       "publication_order": "compute-first", "phase_deadline_ns": 10000},
            "resource_limits": {"requested_allocation_bytes": 8192, "process_peak_rss_bytes": 1 << 30,
                                "resident_backing_bytes": None, "native_published_slots": None}}


def captures(plan):
    """Fabricated parser fixtures, deliberately not hardware/performance evidence."""
    counter = 100
    def fresh():
        nonlocal counter
        counter += 1
        return identity(counter)
    def header(kind):
        return {"record": "capture", "schema": checker.CAPTURE_SCHEMA, "kind": kind, "capture_id": fresh(),
                "plan_sha256": checker.sha256(jsonl([plan])), "campaign_id": plan["campaign_id"],
                "source_commit": plan["source_commit"], "host_boot_id": plan["host_boot_id"], "physical_overlap": "unmeasured"}
    def matching(backend):
        return {"backend": backend, "producer": copy.deepcopy(plan["producers"][backend]),
                "gpu": copy.deepcopy(plan["gpu"]), "workload_sha256": checker.object_sha256(plan["workload"])}
    correctness = [header("correctness")]
    for backend in checker.BACKENDS:
        correctness.append({"record": "correctness", **matching(backend), "invocation_id": fresh(),
                            "initial_buffer_sha256": [buffer["initial_sha256"] for buffer in plan["workload"]["buffers"]],
                            "output_buffer_sha256": [buffer["expected_sha256"] for buffer in plan["workload"]["buffers"]],
                            "completed_operations": 2, "cleanup": "complete"})
    correctness.append({"record": "complete", "validated_backends": 3, "cleanup": "complete"})
    timing = [dict(header("timing"), correctness_capture_sha256=checker.sha256(jsonl(correctness)))]
    wall, ordinal = 100, 0
    phases = list(checker.PHASES)
    if plan["policy"]["publication_order"] == "copy-first":
        phases[1:3] = ["transfer", "submission"]
    for round_index in range(plan["policy"]["rounds"]):
        backends = checker.BACKENDS[round_index % 3:] + checker.BACKENDS[:round_index % 3]
        for position, backend in enumerate(backends):
            invocation, cpu = fresh(), 0
            def interval():
                nonlocal wall, cpu
                result = {"wall_start_ns": wall, "wall_end_ns": wall + 10, "cpu_start_ns": cpu, "cpu_end_ns": cpu + 5}
                wall += 10
                cpu += 5
                return result
            timing.append({"record": "block", "round": round_index, "position": position, **matching(backend),
                           "invocation_id": invocation, "setup": interval()})
            for phase, count in (("warmup", plan["policy"]["warmups_per_block"]), ("sample", plan["policy"]["samples_per_block"])):
                for index in range(count):
                    timing.append({"record": "iteration", "round": round_index, "backend": backend, "invocation_id": invocation,
                                   "sample_id": fresh(), "phase": phase, "index": index, "ordinal": ordinal,
                                   "issued_operations": 2, "completed_operations": 2,
                                   "phases": [{"name": name, "interval": interval()} for name in phases],
                                   "end_to_end_ns": 30, "process_cpu_ns": 15,
                                   "validation": "full-requested-buffers-passed-outside-interval"})
                    ordinal += 1
            timing.append({"record": "block-complete", "round": round_index, "backend": backend, "invocation_id": invocation,
                           "warmups": plan["policy"]["warmups_per_block"], "samples": plan["policy"]["samples_per_block"],
                           "cleanup": "complete", "cleanup_cost": interval(), "resource_peaks": {
                               "requested_allocation_bytes": {"value": sum(buffer["bytes"] for buffer in plan["workload"]["buffers"]), "basis": checker.PEAK_BASES["requested_allocation_bytes"]},
                               "process_peak_rss_bytes": {"value": 1 << 20, "basis": checker.PEAK_BASES["process_peak_rss_bytes"]},
                               "resident_backing_bytes": {"value": None, "basis": "unavailable"},
                               "native_published_slots": {"value": None, "basis": "unavailable"}}})
    blocks = 3 * plan["policy"]["rounds"]
    timing.append({"record": "complete", "blocks": blocks, "warmups": blocks * plan["policy"]["warmups_per_block"],
                   "samples": blocks * plan["policy"]["samples_per_block"], "cleanup": "complete"})
    return correctness, timing


class Scale3ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.plan = plan_fixture()
        self.correctness, self.timing = captures(self.plan)

    def validate(self, plan=None, correctness=None, timing=None, pin=None):
        plan_text = jsonl([self.plan if plan is None else plan])
        return checker.validate_campaign(plan_text, jsonl(self.correctness if correctness is None else correctness),
                                         jsonl(self.timing if timing is None else timing), pin or checker.sha256(plan_text))

    def reject(self, **kwargs):
        with self.assertRaises(checker.CheckError):
            self.validate(**kwargs)

    def test_balanced_matrix_and_no_evidence_or_performance_claim(self):
        result = self.validate()
        self.assertEqual(result["blocks"], 9)
        self.assertEqual(result["samples_per_backend"], 6)
        self.assertEqual(result["hardware_authenticity"], "not-established")
        self.assertTrue(result["consistency_only"])
        self.assertFalse(result["performance_claim"])
        self.assertEqual(result["unavailable_peak_categories"], ["native_published_slots", "resident_backing_bytes"])
        for direction in ("h2d", "d2h"):
            for order in ("compute-first", "copy-first"):
                plan = plan_fixture()
                plan["policy"]["publication_order"] = order
                if direction == "d2h":
                    plan["workload"]["copy"].update(direction=direction, source=2, destination=1)
                correctness, timing = captures(plan)
                self.validate(plan=plan, correctness=correctness, timing=timing)

    def test_pin_and_every_matching_workload_coordinate(self):
        self.reject(pin="0" * 64)
        self.reject(pin=identity(999))
        mutations = []
        for key, value in (("artifact_sha256", identity(61)), ("abi_sha256", identity(62)),
                           ("effects_sha256", identity(63)), ("oracle_sha256", identity(64)), ("argument_template_sha256", identity(65)),
                           ("grid", [2048, 1, 1]), ("workgroup", [128, 1, 1]), ("kernarg_bytes", 32)):
            plan = copy.deepcopy(self.plan)
            plan["workload"][key] = value
            mutations.append(plan)
        for key in ("bytes", "source_offset", "destination_offset"):
            plan = copy.deepcopy(self.plan)
            plan["workload"]["copy"][key] += 1
            mutations.append(plan)
        for index in range(3):
            plan = copy.deepcopy(self.plan)
            plan["workload"]["buffers"][index]["bytes"] += 1
            plan["resource_limits"]["requested_allocation_bytes"] += 1
            mutations.append(plan)
        for plan in mutations:
            correctness, timing = copy.deepcopy(self.correctness), copy.deepcopy(self.timing)
            # Rebind the envelopes, deliberately retaining the old loaded-workload digest.
            pin = checker.sha256(jsonl([plan]))
            correctness[0]["plan_sha256"] = timing[0]["plan_sha256"] = pin
            timing[0]["correctness_capture_sha256"] = checker.sha256(jsonl(correctness))
            with self.assertRaisesRegex(checker.CheckError, "artifact, ABI, effects, geometry, bytes, or oracle mismatch"):
                self.validate(plan=plan, correctness=correctness, timing=timing)
        for key in checker.SEMANTICS:
            plan = copy.deepcopy(self.plan)
            plan["semantics"][key] = "changed"
            self.reject(plan=plan)

    def test_maximum_precommitted_schedule_stays_inside_parser_bounds(self):
        plan = plan_fixture()
        plan["policy"].update(rounds=12, warmups_per_block=4, samples_per_block=16)
        correctness, timing = captures(plan)
        self.assertEqual(len(timing), 794)
        text = jsonl(timing)
        self.assertLess(len(text.encode("utf-8")), checker.MAX_LOG_BYTES)
        self.assertLess(max(len(line.encode("utf-8")) for line in text.splitlines()), checker.MAX_RECORD_BYTES)
        result = self.validate(plan=plan, correctness=correctness, timing=timing)
        self.assertEqual(result["samples_per_backend"], 192)

    def test_available_native_peaks_still_do_not_close_native_budget_inventory(self):
        plan = plan_fixture()
        plan["resource_limits"].update(resident_backing_bytes=16384, native_published_slots=4)
        correctness, timing = captures(plan)
        for record in timing:
            if record["record"] == "block-complete":
                for key, value in (("resident_backing_bytes", 8192), ("native_published_slots", 4)):
                    record["resource_peaks"][key] = {"value": value, "basis": checker.PEAK_BASES[key]}
        result = self.validate(plan=plan, correctness=correctness, timing=timing)
        self.assertEqual(result["unavailable_peak_categories"], [])
        self.assertEqual(result["native_budget_closure"], "not-established")

    def test_plan_shape_bounds_and_descriptor_aliases(self):
        for key in self.plan:
            plan = copy.deepcopy(self.plan)
            del plan[key]
            self.reject(plan=plan)
        for key, value in (("rounds", 4), ("rounds", True), ("rounds", 15), ("samples_per_block", 0),
                           ("samples_per_block", 17), ("warmups_per_block", 0), ("phase_deadline_ns", -1),
                           ("rotation", ["hip", "kfd", "hsa"])):
            plan = copy.deepcopy(self.plan)
            plan["policy"][key] = value
            self.reject(plan=plan)
        for bindings in ([], [0, 0], [True], [1], [17]):
            plan = copy.deepcopy(self.plan)
            plan["workload"]["kernel_bindings"] = bindings
            self.reject(plan=plan)
        for key, value in (("source", 0), ("destination", 1), ("source_offset", 2048),
                           ("bytes", True), ("bytes", 0), ("direction", "xgmi")):
            plan = copy.deepcopy(self.plan)
            plan["workload"]["copy"][key] = value
            self.reject(plan=plan)
        for cpus in ([], [4, 4], [5, 4], [True], [65536]):
            plan = copy.deepcopy(self.plan)
            plan["gpu"]["measurement_cpus"] = cpus
            self.reject(plan=plan)

    def test_gpu_producer_and_workload_identity_mismatch_in_every_backend(self):
        for collection in ("correctness", "timing"):
            source = getattr(self, collection)
            for index, record in enumerate(source):
                if "producer" not in record:
                    continue
                for key, value in (("gpu", dict(record["gpu"], unique_id="fedcba0987654321")),
                                   ("producer", dict(record["producer"], binary_sha256=identity(777))),
                                   ("workload_sha256", identity(778))):
                    mutated = copy.deepcopy(source)
                    mutated[index][key] = value
                    self.reject(**{collection: mutated})

    def test_correctness_is_distinct_complete_and_full_buffer_checked(self):
        self.reject(correctness=self.timing)
        self.reject(timing=self.correctness)
        for index in (1, 2, 3):
            for key in ("initial_buffer_sha256", "output_buffer_sha256"):
                for buffer_index in range(3):
                    records = copy.deepcopy(self.correctness)
                    records[index][key][buffer_index] = identity(888)
                    self.reject(correctness=records)
            for key, value in (("completed_operations", 1), ("cleanup", "retained"), ("end_to_end_ns", 30)):
                records = copy.deepcopy(self.correctness)
                records[index][key] = value
                self.reject(correctness=records)
        timing = copy.deepcopy(self.timing)
        timing[0]["correctness_capture_sha256"] = identity(889)
        self.reject(timing=timing)

    def test_missing_invented_reordered_and_relabelled_samples(self):
        for index in range(len(self.timing)):
            records = copy.deepcopy(self.timing)
            del records[index]
            self.reject(timing=records)
        self.reject(timing=self.timing + [self.timing[-1]])
        records = copy.deepcopy(self.timing)
        records[2], records[3] = records[3], records[2]
        self.reject(timing=records)
        for key, value in (("ordinal", 999), ("index", True), ("phase", "sample"), ("backend", "hip"),
                           ("round", 1), ("invocation_id", identity(900)), ("issued_operations", 1),
                           ("completed_operations", 0), ("validation", "unchecked")):
            records = copy.deepcopy(self.timing)
            records[2][key] = value
            self.reject(timing=records)
        for key, value in (("round", 1), ("position", 1), ("backend", "hip")):
            records = copy.deepcopy(self.timing)
            records[1][key] = value
            self.reject(timing=records)

    def test_occurrence_reuse_including_cross_capture_relabelling(self):
        for value in (self.timing[0]["capture_id"], self.timing[1]["invocation_id"], self.correctness[1]["invocation_id"], self.timing[3]["sample_id"]):
            records = copy.deepcopy(self.timing)
            records[2]["sample_id"] = value
            self.reject(timing=records)
        records = copy.deepcopy(self.timing)
        records[0]["capture_id"] = self.correctness[0]["capture_id"]
        self.reject(timing=records)

    def test_cost_categories_raw_intervals_and_aggregate_consistency(self):
        for key, value in (("end_to_end_ns", 50), ("process_cpu_ns", 25), ("end_to_end_ns", True), ("device_overlap_ns", 1)):
            records = copy.deepcopy(self.timing)
            records[2][key] = value
            self.reject(timing=records)
        for phase_index in range(5):
            for key, value in (("wall_end_ns", 0), ("wall_end_ns", 9999999), ("cpu_start_ns", -1), ("cpu_end_ns", True)):
                records = copy.deepcopy(self.timing)
                records[2]["phases"][phase_index]["interval"][key] = value
                self.reject(timing=records)
            records = copy.deepcopy(self.timing)
            del records[2]["phases"][phase_index]
            self.reject(timing=records)
        records = copy.deepcopy(self.timing)
        records[2]["phases"][2]["interval"]["wall_start_ns"] += 1
        self.reject(timing=records)
        records = copy.deepcopy(self.timing)
        records[6]["setup"]["wall_start_ns"] = 0
        self.reject(timing=records)

    def test_resource_peaks_limits_and_unavailable_are_not_invented_measurements(self):
        for key in checker.PEAK_BASES:
            for value in ({"value": None, "basis": "invented"}, {"value": True, "basis": checker.PEAK_BASES[key]},
                          {"value": -1, "basis": checker.PEAK_BASES[key]}, {"value": 1, "basis": "caller-estimate"}):
                records = copy.deepcopy(self.timing)
                records[5]["resource_peaks"][key] = value
                self.reject(timing=records)
        for key in ("resident_backing_bytes", "native_published_slots"):
            plan = copy.deepcopy(self.plan)
            plan["resource_limits"][key] = 16384
            correctness, timing = captures(plan)
            self.reject(plan=plan, correctness=correctness, timing=timing)
        for key, peak in (("requested_allocation_bytes", 8193), ("process_peak_rss_bytes", 1 << 31), ("resident_backing_bytes", 8191)):
            records = copy.deepcopy(self.timing)
            records[5]["resource_peaks"][key] = {"value": peak, "basis": checker.PEAK_BASES[key]}
            self.reject(timing=records)
        records = copy.deepcopy(self.timing)
        records[5]["cleanup"] = "partial"
        self.reject(timing=records)

    def test_unsupported_overlap_and_uncommitted_summary_claims(self):
        for collection in ("correctness", "timing"):
            records = copy.deepcopy(getattr(self, collection))
            records[0]["physical_overlap"] = "proven"
            self.reject(**{collection: records})
            records = copy.deepcopy(getattr(self, collection))
            records[-1]["speedup"] = 1000
            self.reject(**{collection: records})

    def test_bounded_parsing_duplicate_keys_nonfinite_and_utf8(self):
        record = '{"a":1}\n'
        encoded_json = tuple('{"a":1}'.encode(encoding).decode("utf-8") + "\n"
                             for encoding in ("utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be"))
        for text in ("", record[:-1], record.replace("\n", "\r\n"), '{"a":1,"a":2}\n', '{"a":NaN}\n', "[]\n",
                     '{"a":1e10000}\n', '{"a":-1e10000}\n', '{"a":1.0}\n', *encoded_json,
                     "[" * 2000 + "\n", record * (checker.MAX_RECORDS + 1), " " * checker.MAX_RECORD_BYTES + "{}\n",
                     "\ud800\n", " " * checker.MAX_LOG_BYTES + "\n", '"' + "\u20ac" * checker.MAX_LOG_BYTES + '"\n'):
            with self.subTest(text=text[:40]), self.assertRaises(checker.CheckError):
                checker.parse_records(text)
        for payload in (b"x" * (checker.MAX_LOG_BYTES + 1), b"\xff\n"):
            stream = mock.MagicMock()
            stream.__enter__.return_value.read.return_value = payload
            with mock.patch.object(pathlib.Path, "open", return_value=stream), self.assertRaises(checker.CheckError):
                checker.load_text(pathlib.Path("unused"))
            stream.__enter__.return_value.read.assert_called_once_with(checker.MAX_LOG_BYTES + 1)

    def test_cli_requires_pin_and_reports_consistency_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            for filename, records in (("plan", [self.plan]), ("correctness", self.correctness), ("timing", self.timing)):
                (root / filename).write_text(jsonl(records))
            args = ["--plan", str(root / "plan"), "--plan-sha256", checker.sha256(jsonl([self.plan])),
                    "--correctness", str(root / "correctness"), "--timing", str(root / "timing")]
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                self.assertEqual(checker.main(args), 0)
            self.assertEqual(json.loads(stdout.getvalue())["hardware_authenticity"], "not-established")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(checker.main(args[:3] + [identity(999)] + args[4:]), 2)


if __name__ == "__main__":
    unittest.main()
