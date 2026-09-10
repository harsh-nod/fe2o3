#!/usr/bin/env python3
"""Synthetic CPU protocol tests. Fixtures are not native qualification evidence."""
import copy
import hashlib
import json
import pathlib
import tempfile
import unittest

import check_drain_capture as checker


def identity(label):
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def fixture():
    cases = []
    for ordinal, (cutoff, streams, observers) in enumerate(checker.CELLS):
        stream_rows = [{"runtime": 10 + index, "backend": 110 + index}
                       for index in range(streams)]
        allocations = [{"runtime": 20 + index, "backend": 120 + index,
                        "role": role, "bytes": size}
                       for index, (role, size) in enumerate(zip(
                           ("input", "device", "output"),
                           (checker.DATA, checker.DATA, checker.CAPTURE)))]
        baseline = {"publication_ids": [201, 202], "native_retained_copies": 0, "copies": []}
        copies = []
        for index, (source, destination, source_offset, destination_offset, byte_len) in enumerate((
                (120, 121, 131, 137, checker.BODY),
                (121, 122, 137, 139, checker.BODY),
                (121, 122, 0, checker.VERIFY_OFFSET, checker.DATA))):
            copies.append({"submission": 203 + index,
                           "stream": 110 + int(index > 0 and streams == 2),
                           "source": source, "destination": destination,
                           "source_offset": source_offset, "destination_offset": destination_offset,
                           "byte_len": byte_len,
                           "dependencies": [202 + index] if index and streams == 1 else [],
                           "phase": "directional-published", "native_packets": 1,
                           "native_receipt": identity(f"synthetic-native-{ordinal}-{index}"),
                           "runtime_membership": identity(f"synthetic-runtime-{ordinal}-{index}")})
            copies[-1]["runtime_membership"] = checker.expected_membership(copies[-1])
        at_cutoff = copy.deepcopy(baseline)
        if cutoff == "native-retained":
            at_cutoff = {"publication_ids": [201, 202, 203], "native_retained_copies": 1,
                         "copies": [copy.deepcopy(copies[0])]}
        final = {"publication_ids": [201, 202, 203, 204, 205],
                 "native_retained_copies": 0, "copies": []}
        retained = 3 if streams == 1 else 0
        case = {"ordinal": ordinal, "cutoff": cutoff, "streams": streams,
                "observers": observers, "profile": dict(checker.PROFILE),
                "context": identity(f"synthetic-context-{ordinal}"),
                "resources": {"streams": stream_rows, "allocations": allocations},
                "baseline": baseline, "at_cutoff": at_cutoff, "capture_before": final,
                "capture_after": copy.deepcopy(final), "copies": copies,
                "drain": {"outcome": "quiescent", "ticks": 11,
                          "total_submissions": retained, "succeeded": retained, "pending": 0,
                          "queued_commands_exhausted": True, "operations_remaining": 0,
                          "graph_active": False},
                "completed_observers": (3 if streams == 1 else 1) if observers == "retained" else 0,
                "graph_sha256": identity(f"synthetic-graph-{ordinal}") if streams == 2 else None,
                "credits": checker.expected_credits(), "cleanup": "complete"}
        case.update(checker.expected_hashes(ordinal))
        cases.append(case)
    return {"schema": checker.SCHEMA, "owner_threads": 1, "physical_overlap": "unmeasured",
            "performance": "unmeasured", "cases": cases}


def encoded(document):
    return json.dumps(document, separators=(",", ":")) + "\n"


class DrainCaptureProtocolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.valid = fixture()

    def reject(self, mutate):
        document = copy.deepcopy(self.valid)
        mutate(document)
        with self.assertRaises(checker.ValidationError):
            checker.validate_output(encoded(document))

    def test_complete_eight_cell_synthetic_matrix(self):
        self.assertEqual(checker.validate_output(encoded(self.valid)), self.valid)

    def test_missing_duplicate_reordered_or_relabelled_cell(self):
        for mutation in (
                lambda doc: doc["cases"].pop(),
                lambda doc: doc["cases"].append(copy.deepcopy(doc["cases"][0])),
                lambda doc: doc["cases"].reverse(),
                lambda doc: doc["cases"][0].update(ordinal=1),
                lambda doc: doc["cases"][0].update(cutoff="native-retained"),
                lambda doc: doc["cases"][0].update(streams=2),
                lambda doc: doc["cases"][0].update(observers="dropped")):
            with self.subTest(mutation=mutation):
                self.reject(mutation)

    def test_every_profile_coordinate_is_fixed(self):
        for key in checker.PROFILE:
            with self.subTest(key=key):
                self.reject(lambda doc: doc["cases"][0]["profile"].__setitem__(key, checker.PROFILE[key] + 1))

    def test_every_copy_geometry_and_dependency_is_bound(self):
        for index in range(3):
            for field in ("submission", "stream", "source", "destination", "source_offset",
                          "destination_offset", "byte_len", "native_packets"):
                with self.subTest(index=index, field=field):
                    self.reject(lambda doc: doc["cases"][0]["copies"][index].__setitem__(
                        field, doc["cases"][0]["copies"][index][field] + 1))
            for dependencies in ([999], [999, 999], [203 + index], list(range(300, 309))):
                with self.subTest(index=index, dependencies=dependencies):
                    self.reject(lambda doc: doc["cases"][0]["copies"][index].update(dependencies=dependencies))
        self.reject(lambda doc: doc["cases"][2]["copies"][1].update(dependencies=[203]))

    def test_all_resources_roles_extents_and_ids_are_bound(self):
        for index in range(3):
            for field in ("bytes", "backend"):
                with self.subTest(index=index, field=field):
                    self.reject(lambda doc: doc["cases"][0]["resources"]["allocations"][index].__setitem__(
                        field, doc["cases"][0]["resources"]["allocations"][index][field] + 1))
        self.reject(lambda doc: doc["cases"][0]["resources"]["allocations"][1].update(role="input"))
        self.reject(lambda doc: doc["cases"][2]["resources"]["streams"][1].update(runtime=10))
        self.reject(lambda doc: doc["cases"][0]["resources"]["allocations"][0].update(runtime=0))
        self.reject(lambda doc: doc["cases"][0]["resources"]["allocations"][0].update(runtime=21))

    def test_full_body_device_guards_and_output_hashes_are_independent(self):
        for field in ("output_sha256", "device_sha256", "body_sha256"):
            with self.subTest(field=field):
                self.reject(lambda doc: doc["cases"][0].__setitem__(field, identity("wrong-bytes")))
                self.reject(lambda doc: doc["cases"][0].__setitem__(field, doc["cases"][1][field]))

    def test_receipts_cannot_be_missing_queued_reused_or_relabelled(self):
        for index in range(3):
            for field in ("native_receipt", "runtime_membership"):
                with self.subTest(index=index, field=field):
                    for value in (None, "0" * 64, "f" * 63, "F" * 64):
                        self.reject(lambda doc: doc["cases"][0]["copies"][index].__setitem__(field, value))
            self.reject(lambda doc: doc["cases"][0]["copies"][index].update(phase="ready"))
        self.reject(lambda doc: doc["cases"][0]["copies"].pop())
        self.reject(lambda doc: doc["cases"][0]["copies"].reverse())
        self.reject(lambda doc: doc["cases"][1]["copies"][0].update(
            native_receipt=doc["cases"][0]["copies"][0]["native_receipt"]))
        self.reject(lambda doc: doc["cases"][0]["copies"][0].update(
            native_receipt=doc["cases"][0]["copies"][0]["runtime_membership"]))

    def test_cutoff_is_real_native_retention_not_pending_or_a_counter(self):
        self.reject(lambda doc: doc["cases"][0].__setitem__("at_cutoff", copy.deepcopy(doc["cases"][4]["at_cutoff"])))
        self.reject(lambda doc: doc["cases"][4]["at_cutoff"].update(copies=[], native_retained_copies=0))
        self.reject(lambda doc: doc["cases"][4]["at_cutoff"].update(publication_ids=[201, 202]))
        self.reject(lambda doc: doc["cases"][4]["at_cutoff"]["copies"][0].update(
            phase="ready", native_receipt=None, runtime_membership=None, native_packets=0))
        self.reject(lambda doc: doc["cases"][4]["at_cutoff"]["copies"][0].update(
            native_receipt=identity("a-different-receipt")))
        self.reject(lambda doc: doc["cases"][4]["at_cutoff"]["copies"].append(
            copy.deepcopy(doc["cases"][4]["copies"][1])))

    def test_capture_preserves_empty_roster_and_exact_history(self):
        for phase in ("baseline", "capture_before", "capture_after"):
            self.reject(lambda doc: doc["cases"][0][phase]["publication_ids"].append(999))
            self.reject(lambda doc: doc["cases"][0][phase]["publication_ids"].append(201))
            self.reject(lambda doc: doc["cases"][0][phase].update(native_retained_copies=1))
            self.reject(lambda doc: doc["cases"][0][phase]["copies"].append(copy.deepcopy(doc["cases"][0]["copies"][0])))
        def missing_publication(doc):
            for phase in ("capture_before", "capture_after"):
                doc["cases"][0][phase]["publication_ids"].remove(204)
        self.reject(missing_publication)
        def reversed_publication(doc):
            for phase in ("capture_before", "capture_after"):
                doc["cases"][0][phase]["publication_ids"] = [201, 202, 203, 205, 204]
        self.reject(reversed_publication)

    def test_drain_credits_observer_and_cleanup_claims_are_exact(self):
        for field, value in (("ticks", 0), ("ticks", 129), ("outcome", "pending"),
                             ("total_submissions", 2), ("succeeded", 2), ("pending", 1),
                             ("operations_remaining", 1), ("queued_commands_exhausted", False),
                             ("graph_active", True)):
            self.reject(lambda doc: doc["cases"][0]["drain"].__setitem__(field, value))
        for field in checker.expected_credits():
            self.reject(lambda doc: doc["cases"][0]["credits"].__setitem__(
                field, doc["cases"][0]["credits"][field] + 1))
        self.reject(lambda doc: doc["cases"][0].update(completed_observers=0))
        self.reject(lambda doc: doc["cases"][1].update(completed_observers=3))
        self.reject(lambda doc: doc["cases"][2].update(graph_sha256=None))
        self.reject(lambda doc: doc["cases"][0].update(graph_sha256=identity("wrong-graph")))
        self.reject(lambda doc: doc["cases"][0].update(cleanup="retained"))
        self.reject(lambda doc: doc.update(physical_overlap="proved"))
        self.reject(lambda doc: doc.update(performance="faster"))

    def test_strict_shape_and_bool_is_not_an_integer(self):
        self.reject(lambda doc: doc.update(extra="ignored"))
        self.reject(lambda doc: doc["cases"][0]["copies"][0].update(extra="ignored"))
        self.reject(lambda doc: doc["cases"][0].update(context=doc["cases"][1]["context"]))
        self.reject(lambda doc: doc.update(owner_threads=True))
        self.reject(lambda doc: doc["cases"][0].update(streams=True))
        self.reject(lambda doc: doc["cases"][0]["drain"].update(ticks=True))
        self.reject(lambda doc: doc["cases"][0]["copies"][0].update(native_packets=True))
        self.reject(lambda doc: doc["cases"][0]["credits"].update(after_disposal=False))

    def test_duplicate_fields_float_overflow_nonfinite_and_trailing_data(self):
        text = encoded(self.valid)
        malformed = [text.replace('"owner_threads":1', '"owner_threads":1,"owner_threads":1', 1),
                     text[:-1], text + "\n", text + "{}\n", "[]\n", "null\n"]
        for token in ("1.0", "1e0", "1e10000", "NaN", "Infinity", "-Infinity", "-1", "0"):
            malformed.append(text.replace('"owner_threads":1', f'"owner_threads":{token}', 1))
        for value in malformed:
            with self.subTest(prefix=value[:80]), self.assertRaises(checker.ValidationError):
                checker.validate_output(value)

    def test_bounded_utf8_file_read(self):
        text = encoded(self.valid)
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "evidence.json"
            path.write_bytes(text.encode("utf-8"))
            self.assertEqual(checker.validate_output(checker.read_output(path)), self.valid)
            for data in (b"x" * (checker.MAX_OUTPUT_BYTES + 1), b"\xff\n", text.encode("utf-16-le"),
                         text.encode("utf-16-be"), text.encode("utf-32-le"), text.encode("utf-32-be")):
                path.write_bytes(data)
                with self.subTest(bytes=len(data)), self.assertRaises(checker.ValidationError):
                    checker.validate_output(checker.read_output(path))
        with self.assertRaises(checker.ValidationError):
            checker.validate_output("[" * 2000 + "]" * 2000 + "\n")
        with self.assertRaises(checker.ValidationError):
            checker.validate_output("\u00e9" * checker.MAX_OUTPUT_BYTES + "\n")

    def test_coherent_synthetic_relabelling_is_not_producer_authentication(self):
        document = copy.deepcopy(self.valid)
        case = document["cases"][0]
        for item in case["resources"]["streams"] + case["resources"]["allocations"]:
            item["backend"] += 1000
        for row in case["copies"]:
            for field in ("stream", "source", "destination"):
                row[field] += 1000
        with self.assertRaises(checker.ValidationError):
            checker.validate_output(encoded(document))
        for row in case["copies"]:
            row["runtime_membership"] = checker.expected_membership(row)
        # The checker establishes consistency only. Native identities are checked
        # by the live observer and bound to the actual ELF/capture by the runner.
        self.assertEqual(checker.validate_output(encoded(document)), document)


if __name__ == "__main__":
    unittest.main()
