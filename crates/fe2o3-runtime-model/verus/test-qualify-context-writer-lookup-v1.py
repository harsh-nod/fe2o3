"""Qualifier orchestration controls with simulated process results; never runs Verus."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import types
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "writer_lookup_qualification", HERE / "qualify-context-writer-lookup-v1.py")
QUALIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUALIFIER)
CASES = (
    "expected_writer_erased", "no_versions_accepts_reference", "missing_writer_accepts_reference",
    "empty_absence_rejected", "reader_check_after_missing_writer", "reader_error_is_reference",
    "submission_reader_ignored", "producer_reader_ignored", "record_reader_ignored",
    "record_producer_reader_ignored", "domain_check_omitted", "generated_stream_ignored",
    "generated_hold_ignored", "generated_shell_ignored", "expected_reference_ignored",
    "key_generation_ignored", "key_local_ignored", "key_kind_ignored",
    "return_none_for_writer", "return_slot_zero",
)


def record(path):
    raw = path.read_bytes()
    return dict(bytes=len(raw), mode=stat.S_IMODE(path.stat().st_mode),
                sha256=hashlib.sha256(raw).hexdigest())


def tree(root):
    return {str(path.relative_to(root)): record(path)
            for path in sorted(root.rglob("*")) if path.is_file()}


def simulated_unittest_output(count):
    rows = [f"test_{number} (__main__.FixtureControls.test_{number}) ... ok\n"
            for number in range(count)]
    return "".join(rows) + "\n" + "-" * 70 + f"\nRan {count} tests in 0.125s\n\nOK\n"


class OutputControls(unittest.TestCase):
    def test_exact_executed_control_counts(self):
        for count in (47, 32):
            with self.subTest(count=count):
                self.assertTrue(QUALIFIER.unittest_success(
                    0, "", simulated_unittest_output(count), count))

    def test_incomplete_or_nonpassing_controls_fail_closed(self):
        good = simulated_unittest_output(47)
        cases = [
            (1, "", good), (0, "unexpected\n", good), (0, "", ""),
            (0, "", simulated_unittest_output(46)), (0, "", simulated_unittest_output(48)),
            (0, "", good.replace(" ... ok", " ... skipped 'disabled'", 1)),
            (0, "", good.replace(" ... ok", " ... expected failure", 1)),
            (0, "", good.replace(" ... ok", " ... unexpected success", 1)),
            (0, "", good.replace(" ... ok", " ... ERROR", 1)),
            (0, "", good.replace("FixtureControls.test_1)", "FixtureControls.test_0)", 1)),
            (0, "", good.replace("test_1 (__main__.FixtureControls.test_1)",
                                "test_0 (__main__.FixtureControls.test_0)", 1)),
            (0, "", good.replace("Ran 47 tests", "Ran 48 tests")),
            (0, "", good.replace("\nOK\n", "\nFAILED (errors=1)\n")),
            (0, "", good + "trailing\n"), (0, "", good[:-1]),
        ]
        for status, stdout, stderr in cases:
            with self.subTest(status=status, stdout=stdout, stderr=stderr[-120:]):
                self.assertFalse(QUALIFIER.unittest_success(status, stdout, stderr, 47))

    def test_phase_roster_is_exactly_twenty_eight(self):
        self.assertEqual(QUALIFIER.phase_names(CASES),
            ("source-signature", "source-controls", "classifier-controls", "closure-before",
             "proof-before", "relocated-proof", *CASES, "proof-after", "closure-after"))

    def test_incomplete_duplicate_and_reserved_mutant_rosters_fail(self):
        for names in (CASES[:-1], (*CASES, "extra"), (*CASES[:-1], CASES[0]),
                      ("proof-before", *CASES[1:])):
            with self.subTest(names=names):
                with self.assertRaises(ValueError):
                    QUALIFIER.phase_names(names)

    def test_proof_argv_preserves_whole_crate_and_bounds(self):
        source = types.SimpleNamespace(PROOF="proof.rs")
        self.assertEqual(QUALIFIER.proof_command(Path("/verus/verus"), Path("/case"), source),
            ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120",
             "/verus/verus", "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
             "--output-json", "--error-format=json", "--num-threads", "4", "/case/proof.rs"])


class CampaignControls(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="writer-qualifier-unit-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.root = self.directory / "repo"
        self.base = self.root / "crates/fe2o3-runtime-model/verus"
        self.base.mkdir(parents=True)
        self.out = self.directory / "out"
        self.body = "crates/fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs"
        self.proof = "crates/fe2o3-runtime-model/verus/context_writer_lookup_v1.rs"
        self.selector = self.base / "pins/CONTEXT_WRITER_LOOKUP_TOOLCHAIN.toml"
        self.controller = self.base / "check-compute-pipeline-publication.py"
        for path, raw in [(self.root / self.body, b"original body\n"),
                          (self.root / self.proof, b"unchanged proof\n"),
                          (self.selector, QUALIFIER.SELECTOR),
                          (self.controller, b"# inert unit controller\n")]:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            path.chmod(0o644)
        self.verus = self.directory / "tools/verus"
        self.verus.parent.mkdir()
        self.verus.write_text("not executable: unit fixture\n")
        self.rows = {name: dict(text=f"mutant {name}\n", macro="lookup_body",
                               before="original", after=name) for name in CASES}
        inputs = {self.body: record(self.root / self.body),
                  self.proof: record(self.root / self.proof),
                  "rust-toolchain.toml": record(self.selector)}
        self.policy = {"positive": {"inputs": inputs}, "cases": {name: {} for name in CASES}}
        self.policy_path = self.base / "pins/CONTEXT_WRITER_LOOKUP_DIAGNOSTICS_V1.json"
        self.policy_path.write_text(json.dumps(self.policy))
        self.source = types.SimpleNamespace(BODY=self.body, PROOF=self.proof,
            inputs=mock.Mock(return_value={"source": "fixed"}),
            audit=mock.Mock(), support_check=mock.Mock(return_value={"support": "fixed"}),
            ordinary=lambda path: (path.read_bytes(), stat.S_IMODE(path.stat().st_mode)),
            record=lambda raw, mode: dict(bytes=len(raw), mode=mode,
                                         sha256=hashlib.sha256(raw).hexdigest()))
        self.diagnostics = types.SimpleNamespace(CASES=CASES, SCOPE="unit-conditional-scope",
            parse=json.loads, policy_check=mock.Mock(), classify=mock.Mock(side_effect=self.classify))
        self.tools = types.SimpleNamespace(GIT=Path("/pinned/git"), TOOL_ENV={"PATH": "/pinned"},
                                            authenticate=mock.Mock())
        self.owner = types.SimpleNamespace(SIGNALS=(), interrupted=mock.Mock(), run_owned=self.run_owned)
        self.prior = types.SimpleNamespace(CONTROLLER=Path("owner.py"), CONTROLLER_SHA="owner-pin",
            SIGNER="unit signer\n", SIGNATURE="unit signature\n", clean_source=mock.Mock(),
            source_tools=mock.Mock(return_value=self.tools), signature_tool=mock.Mock(),
            git=mock.Mock(return_value=b"a" * 40 + b"\n"), snapshot=lambda: tree(self.root),
            save=lambda path, value: path.write_text(json.dumps(value)))
        self.leaf = types.SimpleNamespace(PRIOR=Path("prior.py"), PRIOR_SHA="prior-pin",
            tree=tree, bind_signed_blobs=mock.Mock(return_value={"bound": True}),
            load=lambda name, path, pin: self.owner if name.endswith("process_owner") else self.prior)
        self.classifier = types.SimpleNamespace(LEAF=Path("leaf.py"), inherited=lambda: self.leaf)
        self.modules = {
            "writer_lookup_sources": self.source,
            "writer_lookup_mutations": types.SimpleNamespace(mutations=lambda _: self.rows),
            "writer_lookup_diagnostics": self.diagnostics,
            "writer_lookup_base": types.SimpleNamespace(inherited=lambda: self.classifier),
        }
        self.calls, self.override, self.after_phase = [], {}, None
        self.previous_cwd = Path.cwd()
        self.addCleanup(os.chdir, self.previous_cwd)
        self.patches = [
            mock.patch.object(QUALIFIER, "ROOT", self.root),
            mock.patch.object(QUALIFIER, "BASE", self.base),
            mock.patch.object(QUALIFIER, "CONTROLLER", self.controller),
            mock.patch.object(QUALIFIER, "CONTROLLER_SHA", hashlib.sha256(self.controller.read_bytes()).hexdigest()),
            mock.patch.object(QUALIFIER, "load", side_effect=lambda name, path: self.modules[name]),
            mock.patch.object(QUALIFIER, "resources"),
        ]
        for patch in self.patches:
            patch.start()
            self.addCleanup(patch.stop)

    def classify(self, status, stdout, stderr, root, policy, name):
        self.assertIs(policy, self.loaded_policy)
        self.assertEqual((root / self.proof).read_bytes(), b"unchanged proof\n")
        self.assertEqual((root / "rust-toolchain.toml").read_bytes(), QUALIFIER.SELECTOR)
        expected = "original body\n" if name == "positive" else self.rows[name]["text"]
        return (status == (0 if name == "positive" else 1) and stdout == name and not stderr
                and (root / self.body).read_text() == expected)

    def run_owned(self, command, deadline, prefix, environment):
        name = prefix.name
        self.calls.append(dict(name=name, command=command, deadline=deadline,
                               cwd=Path.cwd(), environment=environment))
        if name == "source-signature":
            value = (0, "", self.prior.SIGNATURE)
        elif name in ("source-controls", "classifier-controls"):
            count = 47 if name == "source-controls" else 32
            value = (0, "", simulated_unittest_output(count))
        elif name in ("closure-before", "closure-after"):
            value = (0, "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n", "")
        else:
            value = (1, name, "") if name in CASES else (0, "positive", "")
        value = self.override.get(name, value)
        prefix.mkdir()
        (prefix / "stdout.log").write_text(value[1])
        (prefix / "stderr.log").write_text(value[2])
        if self.after_phase:
            self.after_phase(name)
        return value

    def run_campaign(self):
        self.loaded_policy = None
        def check(policy):
            self.loaded_policy = policy
        if self.diagnostics.policy_check.side_effect is None:
            self.diagnostics.policy_check.side_effect = check
        with mock.patch.object(sys, "argv", ["qualifier", "--verus", str(self.verus),
                                            "--output", str(self.out)]), \
             contextlib.redirect_stdout(io.StringIO()):
            QUALIFIER.main()

    def refused(self):
        with self.assertRaises((ValueError, AssertionError)):
            self.run_campaign()
        self.assertFalse((self.out / "scope.json").exists())
        self.assertFalse((self.out / "results.json").exists())

    def test_complete_campaign_has_exact_phase_order_and_scope(self):
        self.run_campaign()
        self.assertEqual([row["name"] for row in self.calls], list(QUALIFIER.phase_names(CASES)))
        self.assertEqual(len(self.calls), 28)
        scope = json.loads((self.out / "scope.json").read_text())
        self.assertEqual({key: scope[key] for key in (
            "key_law_premises", "positives", "logical_negatives", "verification_obligations",
            "full_context", "native", "unwind", "trusted_vstd")},
            dict(key_law_premises=1, positives=3, logical_negatives=20,
                 verification_obligations=16, full_context=False, native=False,
                 unwind=False, trusted_vstd=True))
        self.assertEqual(self.source.inputs.call_count, 2)
        self.assertEqual(self.source.audit.call_count, 2)
        self.assertEqual(self.diagnostics.classify.call_count, 23)
        self.assertEqual(tree(self.out / "nominal-source"), tree(self.out / "relocated-source"))
        self.leaf.bind_signed_blobs.assert_called_once()
        self.tools.authenticate.assert_called_once_with(self.tools.GIT)
        self.assertEqual(self.prior.signature_tool.call_count, 2)

    def test_exact_commands_cwds_and_raw_control_outputs(self):
        self.run_campaign()
        for row in self.calls:
            name = row["name"]
            self.assertEqual(row["deadline"], 130)
            if name in CASES or name in ("proof-before", "relocated-proof", "proof-after"):
                self.assertEqual(row["command"], QUALIFIER.proof_command(self.verus, row["cwd"], self.source))
                self.assertEqual((row["cwd"] / self.proof).read_bytes(), b"unchanged proof\n")
                self.assertEqual(row["environment"]["VERUS_Z3_PATH"], str(self.verus.parent / "z3"))
        for name, count, filename in [
            ("source-controls", 47, "test-context-writer-lookup-v1.py"),
            ("classifier-controls", 32, "test-context-writer-lookup-diagnostics-v1.py")]:
            row = next(row for row in self.calls if row["name"] == name)
            self.assertEqual(row["command"], [sys.executable, "-I", "-B", str(self.base / filename)])
            self.assertEqual((self.out / name / "stderr.log").read_text(), simulated_unittest_output(count))
        self.assertEqual([call.args[-1] for call in self.diagnostics.classify.call_args_list],
                         ["positive", "positive", *CASES, "positive"])
        self.assertTrue(all(len(call.args) == 6 and not call.kwargs
                            for call in self.diagnostics.classify.call_args_list))

    def test_every_phase_failure_prevents_success_outputs(self):
        for index, name in enumerate(QUALIFIER.phase_names(CASES)):
            with self.subTest(phase=name):
                self.out = self.directory / f"failure-{index}"
                self.calls.clear()
                self.override = {name: (2, "", "unit injected failure\n")}
                self.refused()
                self.assertEqual([row["name"] for row in self.calls],
                                 list(QUALIFIER.phase_names(CASES)[:index + 1]))

    def test_missing_mutant_fails_before_execution(self):
        del self.rows[CASES[0]]
        self.refused()
        self.assertEqual(self.calls, [])

    def test_additional_mutant_fails_before_execution(self):
        self.rows["extra"] = self.rows[CASES[0]]
        self.refused()
        self.assertEqual(self.calls, [])

    def test_reordered_mutants_fail_before_execution(self):
        self.rows = dict(reversed(list(self.rows.items())))
        self.refused()
        self.assertEqual(self.calls, [])

    def test_inactive_policy_fails_before_execution(self):
        self.diagnostics.policy_check.side_effect = ValueError("inactive policy")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_source_audit_fails_before_execution(self):
        self.source.audit.side_effect = ValueError("source mismatch")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_wrong_selector_fails_even_with_matching_policy_record(self):
        self.selector.write_bytes(b'[toolchain]\nchannel = "nightly"\n')
        self.policy["positive"]["inputs"]["rust-toolchain.toml"] = record(self.selector)
        self.policy_path.write_text(json.dumps(self.policy))
        self.refused()
        self.assertEqual(self.calls, [])

    def test_selector_mode_mismatch_fails(self):
        self.selector.chmod(0o600)
        self.refused()
        self.assertEqual(self.calls, [])

    def test_controller_hash_mismatch_fails_before_execution(self):
        self.controller.write_text("changed controller\n")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_tool_admission_failure_prevents_execution(self):
        self.prior.source_tools.side_effect = ValueError("tool mismatch")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_signature_tool_failure_prevents_execution(self):
        self.prior.signature_tool.side_effect = ValueError("signature tool mismatch")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_signed_blob_failure_prevents_proofs(self):
        self.leaf.bind_signed_blobs.side_effect = ValueError("unsigned source blob")
        self.refused()
        self.assertEqual([row["name"] for row in self.calls], ["source-signature"])

    def test_clean_source_failure_prevents_execution(self):
        self.prior.clean_source.side_effect = ValueError("dirty source")
        self.refused()
        self.assertEqual(self.calls, [])

    def test_changed_support_fails_before_first_phase(self):
        self.source.support_check.side_effect = [{"support": "fixed"}, {"support": "changed"}]
        self.refused()
        self.assertEqual(self.calls, [])

    def test_source_change_after_phase_prevents_success(self):
        def change(name):
            if name == "source-signature":
                (self.root / self.body).write_text("changed live source\n")
        self.after_phase = change
        self.refused()
        self.assertEqual([row["name"] for row in self.calls], ["source-signature"])

    def test_mutated_theorem_is_refused(self):
        original = shutil.copytree
        def changed(source, destination, *args, **kwargs):
            result = original(source, destination, *args, **kwargs)
            destination = Path(destination)
            if destination.name == CASES[0] + "-source":
                (destination / self.proof).write_text("changed theorem\n")
            return result
        with mock.patch.object(QUALIFIER.shutil, "copytree", side_effect=changed):
            self.refused()
        self.assertNotIn(CASES[0], [row["name"] for row in self.calls])

    def test_unchanged_body_is_not_a_mutation(self):
        self.rows[CASES[0]]["text"] = "original body\n"
        self.refused()
        self.assertNotIn(CASES[0], [row["name"] for row in self.calls])

    def test_changed_nominal_proof_prevents_final_positive(self):
        def change(name):
            if name == CASES[-1]:
                (self.out / "nominal-source" / self.proof).write_text("changed nominal proof\n")
        self.after_phase = change
        self.refused()
        self.assertNotIn("proof-after", [row["name"] for row in self.calls])

    def test_existing_output_is_refused(self):
        self.out.mkdir()
        self.refused()
        self.assertEqual(self.calls, [])

    def test_output_inside_repository_is_refused(self):
        self.out = self.root / "output"
        self.refused()
        self.assertEqual(self.calls, [])

    def test_noncanonical_verifier_is_refused(self):
        alias = self.directory / "alias-verus"
        alias.symlink_to(self.verus)
        self.verus = alias
        self.refused()
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
