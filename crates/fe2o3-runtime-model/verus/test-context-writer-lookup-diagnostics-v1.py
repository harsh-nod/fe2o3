"""Portable synthetic and calibrated-fixture controls, not fresh solver credit."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
spec = importlib.util.spec_from_file_location("writer_lookup_diagnostics", HERE / "context-writer-lookup-diagnostics-v1.py")
diag = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = diag
spec.loader.exec_module(diag)
mutator_spec = importlib.util.spec_from_file_location("writer_lookup_mutator", HERE / "context-writer-lookup-mutations-v1.py")
mutator = importlib.util.module_from_spec(mutator_spec)
mutator_spec.loader.exec_module(mutator)
POSITIVE, MUTANTS = diag.references()
ACTIVE = diag.parse((HERE / "pins/CONTEXT_WRITER_LOOKUP_DIAGNOSTICS_V1.json").read_text())
DRAFT = copy.deepcopy(ACTIVE)
DRAFT["state"] = "draft-review-required"
for row in DRAFT["cases"].values():
    row.update(state="NOT_RUN", observation=None)
CASES = mutator.mutations((REPO / diag.BODY).read_text())


class DiagnosticControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="writer-lookup-diagnostic-control-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.select("positive")

    def select(self, case):
        for name in POSITIVE["observation"]["inputs"]:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            if name == diag.BODY:
                raw = (REPO / name).read_bytes() if case == "positive" else CASES[case]["text"].encode()
            elif name == "rust-toolchain.toml":
                raw = (HERE / "pins/CONTEXT_WRITER_LOOKUP_TOOLCHAIN.toml").read_bytes()
            else:
                raw = (REPO / name).read_bytes()
            target.write_bytes(raw)
            target.chmod(0o644)

    def output(self, case, *, line=145, file=None):
        value = copy.deepcopy(POSITIVE["report"])
        rows = json.loads(json.dumps(POSITIVE["observation"]["warnings"]).replace("<CAPTURED_SOURCE>", str(self.root)))
        if case != "positive":
            value["verification-results"].update({"encountered-error": True, "success": False,
                                                   "verified": 15, "errors": 1})
            rows = rows[:1]
            rows.append({"$message_type": "diagnostic", "code": None, "level": "error",
                         "message": "postcondition not satisfied", "children": [],
                         "spans": [{"file_name": str(self.root / (file or diag.PROOF)),
                                    "is_primary": True, "line_start": line}],
                         "rendered": "synthetic fixture only"})
            rows.append({"$message_type": "diagnostic", "code": None, "level": "error",
                         "message": "aborting due to 1 previous error; 1 warning emitted",
                         "children": [], "spans": [],
                         "rendered": "synthetic fixture only"})
        return json.dumps(value), "\n".join(json.dumps(row) for row in rows)

    def inspect(self, case, output=None):
        stdout, stderr = output or self.output(case)
        return diag.inspect(0 if case == "positive" else 1, stdout, stderr, self.root, case)

    def calibrated_output(self, case):
        observation = ACTIVE["positive"] if case == "positive" else ACTIVE["cases"][case]["observation"]
        report = copy.deepcopy(POSITIVE["report"])
        report["verification-results"] = copy.deepcopy(observation["summary"])
        warnings = json.loads(json.dumps(observation["warnings"]).replace("<CAPTURED_SOURCE>", str(self.root)))

        def relocate(value):
            if isinstance(value, list):
                return [relocate(item) for item in value]
            if isinstance(value, dict):
                return {key: str(self.root / item) if key == "file_name" else relocate(item)
                        for key, item in value.items()}
            return value

        rows = warnings + relocate(observation["diagnostics"])
        return json.dumps(report), "\n".join(json.dumps(row) for row in rows)

    def synthetic_active(self):
        policy = copy.deepcopy(DRAFT)
        policy["state"] = "reviewed-calibrated"
        for case in diag.CASES:
            self.select(case)
            row = policy["cases"][case]
            row["state"] = "reviewed-calibrated"
            row["observation"] = self.inspect(case)["observation"]
        return policy

    def test_actual_positive_fixture_and_relocated_inspection_are_nonaccepting(self):
        value = self.inspect("positive")
        self.assertFalse(value["accepted"])
        self.assertEqual(value["observation"], POSITIVE["observation"])
        self.assertEqual(value["observation"]["summary"]["verified"], 16)
        self.assertEqual(len(value["observation"]["functions"]), 182)
        self.assertEqual(len(value["observation"]["application_declarations"]), 51)

    def test_derived_draft_keeps_every_case_disabled(self):
        self.assertTrue(diag.policy_check(DRAFT, active=False))
        self.assertEqual(len(DRAFT["cases"]), 20)
        for row in DRAFT["cases"].values():
            self.assertEqual(row["state"], "NOT_RUN")
            self.assertIsNone(row["observation"])
        with self.assertRaises(ValueError):
            diag.policy_check(DRAFT)
        self.assertFalse(diag.classify(0, *self.output("positive"), self.root, DRAFT, "positive"))

    def test_synthetic_exact_classifier_control_does_not_rewrite_disk_policy(self):
        policy = self.synthetic_active()
        self.assertTrue(diag.policy_check(policy))
        for case in ("positive", "return_slot_zero"):
            self.select(case)
            self.assertTrue(diag.classify(0 if case == "positive" else 1,
                                         *self.output(case), self.root, policy, case))
        self.assertEqual(diag.parse((HERE / "pins/CONTEXT_WRITER_LOOKUP_DIAGNOSTICS_V1.json").read_text()), ACTIVE)

    def test_reviewed_calibration_fixtures_classify_all_twenty_cases(self):
        self.assertTrue(diag.policy_check(ACTIVE))
        self.assertEqual(len(ACTIVE["cases"]), 20)
        for case in ("positive", *diag.CASES):
            with self.subTest(case=case):
                self.select(case)
                output = self.calibrated_output(case)
                expected = ACTIVE["positive"] if case == "positive" else ACTIVE["cases"][case]["observation"]
                observed = self.inspect(case, output)
                self.assertFalse(observed["accepted"])
                self.assertEqual(observed["observation"], expected)
                self.assertTrue(diag.classify(0 if case == "positive" else 1,
                                             *output, self.root, ACTIVE, case))

    def test_reviewed_calibration_refuses_missing_or_substituted_observation(self):
        case = "return_slot_zero"
        self.select(case)
        output = self.calibrated_output(case)
        for change in ("missing-case", "other-case", "failure-site", "diagnostic-row"):
            with self.subTest(change=change):
                policy = copy.deepcopy(ACTIVE)
                if change == "missing-case":
                    del policy["cases"][case]
                elif change == "other-case":
                    policy["cases"][case]["observation"] = copy.deepcopy(policy["cases"]["return_none_for_writer"]["observation"])
                elif change == "failure-site":
                    policy["cases"][case]["observation"]["logical_sites"][0]["line"] += 1
                    self.assertTrue(diag.policy_check(policy))
                else:
                    policy["cases"][case]["observation"]["diagnostics"].pop(0)
                self.assertFalse(diag.classify(1, *output, self.root, policy, case))

    def test_activation_requires_all_twenty_complete_observations(self):
        policy = copy.deepcopy(DRAFT)
        policy["state"] = "reviewed-calibrated"
        with self.assertRaises(ValueError):
            diag.policy_check(policy)
        for key in ("extra_case",):
            policy = copy.deepcopy(DRAFT)
            policy["cases"][key] = copy.deepcopy(next(iter(policy["cases"].values())))
            with self.assertRaises(ValueError):
                diag.policy_check(policy, active=False)

    def test_case_manifest_and_proof_identity_cannot_be_substituted(self):
        for field in ("sha256", "mode", "bytes"):
            policy = copy.deepcopy(DRAFT)
            row = policy["cases"]["return_none_for_writer"]["inputs"][diag.BODY]
            row[field] = "0" * 64 if field == "sha256" else row[field] + 1
            with self.assertRaises(ValueError):
                diag.policy_check(policy, active=False)
        policy = copy.deepcopy(DRAFT)
        policy["proof_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            diag.policy_check(policy, active=False)

    def test_wrong_case_actual_body_refused(self):
        self.select("return_none_for_writer")
        with self.assertRaises(ValueError):
            self.inspect("return_slot_zero")

    def test_actual_source_bytes_and_mode_refused(self):
        for mode_only in (False, True):
            self.select("positive")
            path = self.root / diag.BODY
            if mode_only:
                path.chmod(0o600)
            else:
                path.write_bytes(path.read_bytes() + b" ")
            with self.assertRaises(ValueError):
                self.inspect("positive")

    def test_actual_source_symlink_refused(self):
        path = self.root / diag.BODY
        path.unlink()
        path.symlink_to(REPO / diag.BODY)
        with self.assertRaises(ValueError):
            self.inspect("positive")

    def test_source_fifo_after_stale_precheck_is_nonblocking(self):
        path = self.root / diag.BODY
        path.unlink()
        os.mkfifo(path, 0o600)
        original = os.open
        def checked_open(selected, flags):
            self.assertTrue(flags & os.O_NONBLOCK)
            return original(selected, flags)
        with mock.patch.object(Path, "is_file", return_value=True), mock.patch.object(diag.os, "open", side_effect=checked_open):
            with self.assertRaises(ValueError):
                diag.source_records(self.root, POSITIVE["observation"]["inputs"])

    def test_pinned_fixture_bytes_cannot_be_replaced(self):
        pins = self.root / "pins"
        pins.mkdir()
        name = "CONTEXT_WRITER_LOOKUP_POSITIVE_V1.json"
        path = pins / name
        path.write_bytes((HERE / "pins" / name).read_bytes() + b" ")
        path.chmod(0o644)
        with mock.patch.object(diag, "HERE", self.root):
            with self.assertRaises(ValueError):
                diag.pinned_json(name, diag.POSITIVE_SHA)

    def test_duplicate_nonfinite_and_oversize_json_refused(self):
        for raw in ('{"a":1,"a":2}', '{"a":NaN}', " " * (diag.LIMIT + 1)):
            with self.assertRaises(ValueError):
                diag.parse(raw)

    def test_frontend_vir_partial_and_wrong_whole_counts_refused(self):
        for key, val in (("encountered-vir-error", True), ("is-verifying-entire-crate", False),
                         ("verified", 15), ("verified", True), ("errors", -1), ("success", False)):
            value = copy.deepcopy(POSITIVE["report"])
            value["verification-results"][key] = val
            with self.subTest(key=key, val=val), self.assertRaises(ValueError):
                self.inspect("positive", (json.dumps(value), self.output("positive")[1]))

    def test_wrong_toolchain_identity_refused(self):
        value = copy.deepcopy(POSITIVE["report"])
        value["verus"]["commit"] = "0" * 40
        with self.assertRaises(ValueError):
            self.inspect("positive", (json.dumps(value), self.output("positive")[1]))

    def test_status_timeout_signal_boolean_refused(self):
        for status in (1, 124, -9, True):
            with self.assertRaises(ValueError):
                diag.inspect(status, *self.output("positive"), self.root, "positive")

    def test_no_full_roster_omission_addition_or_substitution(self):
        for kind in ("remove", "add", "substitute"):
            value = copy.deepcopy(POSITIVE["report"])
            names = value["func-details"]
            name = next(iter(names))
            if kind != "add":
                names.pop(name)
            if kind != "remove":
                names["unexpected::function"] = {"obligation_proof_notes": [], "failed_proof_notes": []}
            with self.assertRaises(ValueError):
                self.inspect("positive", (json.dumps(value), self.output("positive")[1]))

    def test_nonempty_proof_notes_refused(self):
        for key in ("failed_proof_notes", "obligation_proof_notes"):
            value = copy.deepcopy(POSITIVE["report"])
            next(iter(value["func-details"].values()))[key] = ["unexpected"]
            with self.assertRaises(ValueError):
                self.inspect("positive", (json.dumps(value), self.output("positive")[1]))

    def test_warning_missing_added_or_changed_refused(self):
        stdout, stderr = self.output("positive")
        rows = [json.loads(line) for line in stderr.splitlines()]
        for altered in (rows[:-1], rows + rows[-1:], [dict(rows[0], message="different"), rows[1]]):
            with self.assertRaises(ValueError):
                self.inspect("positive", (stdout, "\n".join(map(json.dumps, altered))))

    def test_coded_frontend_error_cannot_replace_logical_failure(self):
        case = "expected_writer_erased"
        self.select(case)
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        rows[-2]["code"] = {"code": "E0308", "explanation": None}
        with self.assertRaises(ValueError):
            self.inspect(case, (stdout, "\n".join(map(json.dumps, rows))))

    def test_negative_needs_intended_primary_declaration(self):
        case = "expected_writer_erased"
        self.select(case)
        for line in (1, 160, 10000):
            with self.assertRaises(ValueError):
                self.inspect(case, self.output(case, line=line))
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        rows[-2]["spans"] = []
        with self.assertRaises(ValueError):
            self.inspect(case, (stdout, "\n".join(map(json.dumps, rows))))

    def test_only_exact_shared_include_alias_allowed(self):
        case = "expected_writer_erased"
        self.select(case)
        alias = next(iter(diag.INCLUDE_ALIASES))
        self.assertFalse(self.inspect(case, self.output(case, line=12, file=alias))["accepted"])
        for name in ("crates/../" + diag.PROOF, "../outside.rs", "relative.rs"):
            stdout, stderr = self.output(case)
            rows = [json.loads(line) for line in stderr.splitlines()]
            rows[-2]["spans"][0]["file_name"] = str(self.root / name) if name != "relative.rs" else name
            with self.assertRaises(ValueError):
                self.inspect(case, (stdout, "\n".join(map(json.dumps, rows))))

    def test_cancelled_symlink_component_refused(self):
        # The compiler alias traverses this directory before cancelling it.
        directory = self.root / "crates/fe2o3-runtime-model/verus"
        proof = directory / "context_writer_lookup_v1.rs"
        proof.unlink()
        directory.rmdir()
        target = self.root / "other"
        target.mkdir()
        (target / proof.name).write_bytes((REPO / diag.PROOF).read_bytes())
        directory.symlink_to(target, target_is_directory=True)
        with self.assertRaises(ValueError):
            diag.diagnostic_member(str(self.root / next(iter(diag.INCLUDE_ALIASES))),
                                   self.root, POSITIVE["observation"]["inputs"])

    def test_whole_report_and_sites_must_match_reviewed_policy(self):
        policy = self.synthetic_active()
        case = "return_slot_zero"
        self.select(case)
        stdout, stderr = self.output(case)
        changed = copy.deepcopy(policy)
        changed["cases"][case]["observation"]["logical_sites"][0]["line"] += 1
        self.assertFalse(diag.classify(1, stdout, stderr, self.root, changed, case))
        changed = copy.deepcopy(policy)
        changed["cases"][case]["observation"]["report_sha256"] = "0" * 64
        self.assertFalse(diag.classify(1, stdout, stderr, self.root, changed, case))

    def test_untrusted_relative_auxiliary_has_no_path_fallback(self):
        case = "expected_writer_erased"
        self.select(case)
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        rows[-1]["spans"].append({"file_name": "vstd/option.rs", "is_primary": False, "line_start": 154})
        with self.assertRaises(ValueError):
            self.inspect(case, (stdout, "\n".join(map(json.dumps, rows))))


    def test_negative_warning_and_abort_frame_is_nonaccepting(self):
        case = "expected_writer_erased"
        self.select(case)
        observed = self.inspect(case)
        self.assertFalse(observed["accepted"])
        self.assertEqual(observed["observation"]["warnings"],
                         POSITIVE["observation"]["warnings"][:1])
        self.assertEqual(observed["observation"]["diagnostics"][-1]["message"],
                         "aborting due to 1 previous error; 1 warning emitted")

    def test_negative_warning_missing_added_or_full_row_changed_refused(self):
        case = "expected_writer_erased"
        self.select(case)
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        changed_child = copy.deepcopy(rows)
        changed_child[0]["children"] = []
        changed_rendered = copy.deepcopy(rows)
        changed_rendered[0]["rendered"] += "changed"
        for altered in (rows[1:], rows[:1] + rows, changed_child, changed_rendered,
                        rows + rows[:1]):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                self.inspect(case, (stdout, "\n".join(map(json.dumps, altered))))

    def test_negative_abort_summary_exact_count_shape_and_order(self):
        case = "expected_writer_erased"
        self.select(case)
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        variants = [rows[:-1], rows + rows[-1:], rows[:1] + rows[-1:] + rows[1:-1]]
        for field, value in (("message", "aborting due to 2 previous errors; 1 warning emitted"),
                             ("message", "aborting due to 1 previous error; 2 warnings emitted"),
                             ("message", "aborting due to 1 previous error"),
                             ("children", [{"message": "unexpected"}]),
                             ("unexpected", True)):
            changed = copy.deepcopy(rows)
            changed[-1][field] = value
            variants.append(changed)
        for altered in variants:
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                self.inspect(case, (stdout, "\n".join(map(json.dumps, altered))))

    def test_warning_profiles_do_not_cross_positive_and_negative(self):
        case = "expected_writer_erased"
        self.select(case)
        stdout, stderr = self.output(case)
        rows = [json.loads(line) for line in stderr.splitlines()]
        summary_warning = copy.deepcopy(POSITIVE["observation"]["warnings"][1])
        with self.assertRaises(ValueError):
            self.inspect(case, (stdout, "\n".join(map(json.dumps, rows[:1] + [summary_warning] + rows[1:]))))
        self.select("positive")
        stdout, stderr = self.output("positive")
        rows = [json.loads(line) for line in stderr.splitlines()]
        with self.assertRaises(ValueError):
            self.inspect("positive", (stdout, json.dumps(rows[0])))

    def trusted_fixture(self):
        tool = self.root / "tool"
        path = tool / "vstd/fixture.rs"
        path.parent.mkdir(parents=True)
        raw = b"x\n"
        path.write_bytes(raw)
        path.chmod(0o644)
        row = {"bytes": len(raw), "mode": 0o644,
               "sha256": diag.hashlib.sha256(raw).hexdigest()}
        span = {"byte_start": 0, "byte_end": 1, "column_start": 1, "column_end": 2,
                "expansion": None, "file_name": "vstd/fixture.rs", "is_primary": False,
                "label": None, "line_start": 1, "line_end": 1, "suggested_replacement": None,
                "suggestion_applicability": None, "text": []}
        return path, span, (tool, {"vstd/fixture.rs": row})

    def test_trusted_auxiliary_regular_buffer_keeps_exact_coordinates(self):
        path, span, context = self.trusted_fixture()
        result = diag.trusted_span(span, context)
        self.assertEqual(result, {"namespace": "trusted-verus", "member": "vstd/fixture.rs",
                                  "sha256": context[1]["vstd/fixture.rs"]["sha256"]})
        span["column_end"] = 1
        with self.assertRaises(ValueError):
            diag.trusted_span(span, context)

    def test_trusted_auxiliary_stale_regular_precheck_cannot_block_on_fifo(self):
        path, span, context = self.trusted_fixture()
        before = path.lstat()
        path.unlink()
        os.mkfifo(path, 0o600)
        original_lstat, original_open = Path.lstat, os.open
        def stale_lstat(selected, *args, **kwargs):
            return before if selected == path else original_lstat(selected, *args, **kwargs)
        def checked_open(selected, flags):
            self.assertTrue(flags & os.O_NONBLOCK)
            self.assertTrue(flags & os.O_NOFOLLOW)
            return original_open(selected, flags)
        with mock.patch.object(Path, "lstat", stale_lstat), mock.patch.object(
                Path, "open", side_effect=AssertionError("blocking pathname open")), mock.patch.object(
                diag.os, "open", side_effect=checked_open) as opened:
            with self.assertRaises(ValueError):
                diag.trusted_span(span, context)
            opened.assert_called_once()

    def test_logical_declaration_uses_authenticated_snapshot_not_path_reopen(self):
        case = "expected_writer_erased"
        self.select(case)
        output = self.output(case)
        with mock.patch.object(Path, "read_text", side_effect=AssertionError("pathname reread")):
            observed = self.inspect(case, output)
        self.assertFalse(observed["accepted"])
        self.assertEqual(observed["observation"]["logical_sites"][0]["declaration"], diag.INTENDED)


if __name__ == "__main__":
    unittest.main(verbosity=2)
