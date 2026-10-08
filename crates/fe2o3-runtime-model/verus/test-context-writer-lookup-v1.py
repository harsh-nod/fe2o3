"""In-memory hostile source controls only; no Rust or solver execution."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("writer_lookup_guard", ROOT / "check-context-writer-lookup-v1.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
INPUTS = guard.inputs()
mutation_spec = importlib.util.spec_from_file_location(
    "writer_lookup_mutations", ROOT / "context-writer-lookup-mutations-v1.py")
mutation = importlib.util.module_from_spec(mutation_spec)
mutation_spec.loader.exec_module(mutation)


class SourceControls(unittest.TestCase):
    def test_exact(self):
        self.assertEqual(guard.audit(INPUTS)["field_counts"], [15, 10, 7])

    def refused(self, path, before, after):
        self.assertEqual(INPUTS[path].count(before), 1)
        changed = dict(INPUTS)
        changed[path] = changed[path].replace(before, after)
        with self.assertRaises(ValueError):
            guard.audit(changed)

    def test_domain_omission(self):
        self.refused(guard.BODY, "root.domain != $domain", "false")

    def test_expected_match_some_substitution(self):
        self.refused(guard.BODY,
                     "Some(record) => record.journal_writer,",
                     "Some(record) => None,")

    def test_expected_match_none_substitution(self):
        self.refused(guard.BODY,
                     "None => None,",
                     "None => Some(ContextWriterReferenceV1 { slot: 0, key: ContextWriterKeyV1 { context_generation: 0, local: 0, kind: ContextWriterKindV1::Submission } }),")

    def test_read_match_absence_changed(self):
        before = "Some(record) => record.journal_read.is_some(),\n                    None => false,"
        self.refused(guard.BODY, before, before.replace("None => false", "None => true"))

    def test_producer_match_absence_changed(self):
        before = "Some(record) => record.journal_producer_read.is_some(),\n                    None => false,"
        self.refused(guard.BODY, before, before.replace("None => false", "None => true"))

    def test_producer_match_field_substituted(self):
        self.refused(guard.BODY,
                     "Some(record) => record.journal_producer_read.is_some(),",
                     "Some(record) => record.journal_read.is_some(),")

    def test_key_local_substitution(self):
        self.refused(guard.BODY, "writer.key.local != $id.local", "writer.key.local != 0")

    def test_reference_slot_ignored(self):
        self.refused(guard.BODY, "expected != Some(writer)", "expected.map(|r| r.key) != Some(writer.key)")

    def test_reader_order_changed(self):
        left = "versions.submission_readers.contains_key(&$id)"
        right = "versions.producer_readers.contains_key(&$id)"
        source = INPUTS[guard.BODY]
        self.assertEqual((source.count(left), source.count(right)), (1, 1))
        changed = dict(INPUTS)
        changed[guard.BODY] = source.replace(left, "__reader_swap__").replace(right, left).replace("__reader_swap__", right)
        with self.assertRaises(ValueError):
            guard.audit(changed)

    def test_wrong_absence_error(self):
        self.refused(guard.BODY, "Ok(None)", "Err(ContextVersionJournalErrorV1::InvalidState)")

    def test_writer_substituted(self):
        self.refused(guard.BODY, "Ok(Some(writer))", "Ok(None)")

    def test_effect_removed(self):
        self.refused(guard.SUBMISSIONS, "            completion_writer_effect_body!(", "            wrong_effect_body!(")

    def test_quarantine_removed(self):
        self.refused(guard.SUBMISSIONS,
                     'pub(in crate::context) fn quarantine_submission_writers_v1(&mut self) {',
                     'pub(in crate::context) fn omitted_quarantine(&mut self) {')

    def test_reborrow_panic_rejected(self):
        self.refused(guard.SUBMISSIONS,
                     "            let versions = self\n"
                     "                .versions\n"
                     "                .as_mut()\n"
                     "                .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;",
                     '            let versions = self.versions.as_mut().expect("validated retained writer");')

    def test_semantic_relation_weakened(self):
        self.refused(guard.PROOF,
                     "out == writer_lookup_selection_v1(submissions@, borrowed_versions_v1(versions), id, domain)",
                     "true")

    def test_key_law_removed(self):
        self.refused(guard.PROOF,
                     "vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>()",
                     "true")

    def test_frame_omitted(self):
        self.refused(guard.PROOF, "*final(versions) == *old(versions),", "true,")

    def test_record_field_removed(self):
        self.refused(guard.CONTEXT, "    dependency_retains: usize,\n", "")

    def test_owner_field_removed(self):
        self.refused(guard.VERSIONS, "    phases: Vec<Option<AllocationPhaseV1>>,\n", "")


class LiteralEdgeControls(unittest.TestCase):
    INCLUDE = 'include!("writer_lookup_body.rs");'
    OTHER = 'include!("different.rs");'

    def test_active_root_include(self):
        guard.root_include('use super::*;\n' + self.INCLUDE, self.INCLUDE)

    def test_inactive_include_decoys(self):
        for source in (
            "// " + self.INCLUDE + "\n" + self.OTHER,
            "/* " + self.INCLUDE + " */\n" + self.OTHER,
            'const DECOY: &str = r#"' + self.INCLUDE + '"#;\n' + self.OTHER,
            self.OTHER,
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                guard.root_include(source, self.INCLUDE)

    def test_nonroot_or_attributed_include(self):
        for source in (
            *("decoy!" + left + self.INCLUDE + right + ";" for left, right in
              (("(", ")"), ("[", "]"), ("{", "}"))),
            "#[cfg(any())]\n" + self.INCLUDE,
            '#[path = "different.rs"]\n' + self.INCLUDE,
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                guard.root_include(source, self.INCLUDE)

    def test_reserved_include_marker(self):
        with self.assertRaises(ValueError):
            guard.root_include("// __writer_lookup_include__\n" + self.INCLUDE, self.INCLUDE)

    def test_runtime_include_comment_cannot_redirect_active_include(self):
        SourceControls.refused(self, guard.LOOKUP, self.INCLUDE,
                               "// " + self.INCLUDE + "\n" + self.OTHER)

    def test_active_test_module(self):
        guard.root_test_module(guard.TEST_MODULE)
        guard.root_test_module(INPUTS[guard.LOOKUP])

    def test_redirected_test_module_path(self):
        SourceControls.refused(self, guard.LOOKUP,
                               '#[path = "writer_lookup_tests.rs"]',
                               '#[path = "different.rs"]')

    def test_test_module_literal_decoys(self):
        wrong = guard.TEST_MODULE.replace("writer_lookup_tests.rs", "different.rs")
        for source in ("/* " + guard.TEST_MODULE + " */\n" + wrong,
                       'const DECOY: &str = r#"' + guard.TEST_MODULE + '"#;\n' + wrong):
            with self.subTest(source=source), self.assertRaises(ValueError):
                guard.root_test_module(source)

    def test_test_module_boundaries_and_attributes(self):
        for source in (
            guard.TEST_MODULE.replace("mod tests", "modtests"),
            guard.TEST_MODULE.replace("cfg(test)", "cfgtest"),
            "#[cfg(any())]\n" + guard.TEST_MODULE,
            '#[path = "different.rs"]\n' + guard.TEST_MODULE,
            "mod nested {\n" + guard.TEST_MODULE + "\n}",
            "// __writer_lookup_test_module__\n" + guard.TEST_MODULE,
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                guard.root_test_module(source)


class PortableControls(unittest.TestCase):
    def test_closed_input_and_support_rosters(self):
        self.assertEqual(len(guard.FILES), 14)
        self.assertEqual(set(guard.inputs()), set(guard.FILES))
        self.assertEqual(len(guard.support_check()), len(guard.SUPPORT_FILES))

    def test_added_or_missing_input_refused(self):
        for changed in (dict(INPUTS, extra=""), {k: v for k, v in INPUTS.items() if k != guard.BODY}):
            with self.assertRaises(ValueError):
                guard.audit(changed)

    def test_duplicate_manifest_key_refused(self):
        with self.assertRaises(ValueError):
            guard.unique_json('{"same": 1, "same": 2}')

    def test_changed_predecessor_capsule_refused(self):
        original = guard.ordinary
        def read(path):
            raw, mode = original(path)
            return (raw + b" ", mode) if path == guard.REFERENCE else (raw, mode)
        with mock.patch.object(guard, "ordinary", side_effect=read):
            with self.assertRaises(ValueError):
                guard.audit(INPUTS)

    def test_source_bytes_and_mode_substitution_refused(self):
        original = guard.ordinary
        for changed_bytes in (False, True):
            def read(path):
                raw, mode = original(path)
                if path == guard.ROOT / guard.BODY:
                    return (raw + b" ", mode) if changed_bytes else (raw, 0o600)
                return raw, mode
            with mock.patch.object(guard, "ordinary", side_effect=read):
                with self.assertRaises(ValueError):
                    guard.inputs()

    def test_support_bytes_substitution_refused(self):
        original = guard.ordinary
        def read(path):
            raw, mode = original(path)
            return (raw + b" ", mode) if path.name == "context-writer-lookup-mutations-v1.py" else (raw, mode)
        with mock.patch.object(guard, "ordinary", side_effect=read):
            with self.assertRaises(ValueError):
                guard.support_check()

    def test_inactive_or_redirected_reachability_refused(self):
        for path, name in ((guard.LIB, "context"), (guard.CONTEXT, "versions"),
                           (guard.VERSIONS, "submissions"), (guard.SUBMISSIONS, "writer_lookup")):
            for attribute in ('#[cfg(any())]\n', '#[path = "different.rs"]\n'):
                changed = dict(INPUTS)
                marker = "mod " + name + ";"
                self.assertEqual(changed[path].count(marker), 1)
                changed[path] = changed[path].replace(marker, attribute + marker, 1)
                with self.subTest(path=path, attribute=attribute), self.assertRaises(ValueError):
                    guard.audit(changed)

    def test_capsule_contains_no_private_filesystem_dependency(self):
        for path in (guard.REFERENCE, guard.MANIFEST, guard.SUPPORT):
            self.assertNotIn(b"/home/", guard.ordinary(path)[0])
        self.assertEqual(set(guard.originals()), set(guard.BASE_PINS))

    def test_inactive_or_redirected_visible_modules_refused(self):
        for path, name in ((guard.LIB, "context"), (guard.CONTEXT, "versions"),
                           (guard.VERSIONS, "submissions"), (guard.SUBMISSIONS, "writer_lookup")):
            for attribute in ('#[cfg(any())]', '#[path = "different.rs"]'):
                for visibility in ("pub", "pub(crate)", "pub(in crate)"):
                    changed = dict(INPUTS)
                    marker = "mod " + name + ";"
                    self.assertEqual(changed[path].count(marker), 1)
                    changed[path] = changed[path].replace(marker, attribute + " " + visibility + " " + marker, 1)
                    with self.subTest(path=path, attribute=attribute, visibility=visibility):
                        with self.assertRaises(ValueError):
                            guard.audit(changed)


    def test_root_module_rejects_macro_token_trees_and_nonitem_tokens(self):
        for name in ("context", "versions", "submissions", "writer_lookup"):
            for opening, closing in (("(", ")"), ("[", "]"), ("{", "}")):
                source = "const _: &str = stringify!" + opening + "mod " + name + ";" + closing + ";"
                with self.subTest(name=name, opening=opening), self.assertRaises(ValueError):
                    guard.root_module(source, name)
            for source in ("not_mod " + name + ";", "const _: bool = mod " + name + ";",
                           'const _: &str = "mod ' + name + ';";'):
                with self.subTest(name=name, source=source), self.assertRaises(ValueError):
                    guard.root_module(source, name)
            for visibility in ("", "pub ", "pub(crate) ", "pub(in crate) "):
                self.assertIsNone(guard.root_module(visibility + "mod " + name + ";", name))


class MutationControls(unittest.TestCase):
    def test_ordinary_refuses_actual_symlink(self):
        with tempfile.TemporaryDirectory(prefix="writer-lookup-source-symlink-") as name:
            path = Path(name) / "link"
            path.symlink_to(guard.ROOT / guard.BODY)
            with self.assertRaises(ValueError):
                guard.ordinary(path)

    def test_file_to_fifo_substitution_opens_nonblocking_then_refuses(self):
        with tempfile.TemporaryDirectory(prefix="writer-lookup-source-fifo-") as name:
            path = Path(name) / "fifo"
            os.mkfifo(path, 0o600)
            original = os.open
            def checked_open(selected, flags):
                self.assertTrue(flags & os.O_NONBLOCK)
                self.assertTrue(flags & os.O_NOFOLLOW)
                return original(selected, flags)
            with mock.patch.object(Path, "is_file", return_value=True):
                with mock.patch.object(guard.os, "open", side_effect=checked_open) as opened:
                    with self.assertRaises(ValueError):
                        guard.ordinary(path)
                    opened.assert_called_once()

    def test_exact_twenty_distinct_body_cases_and_no_acceptance(self):
        cases = mutation.mutations(INPUTS[guard.BODY])
        self.assertEqual(len(cases), 20)
        self.assertEqual(set(cases), {name for name, _, _ in mutation.CASES})
        self.assertEqual(len({row["sha256"] for row in cases.values()}), 20)
        for name, row in cases.items():
            with self.subTest(name=name):
                self.assertEqual(row["intended_declarations"], ["preflight_settlement_writer_v1"])
                self.assertEqual(row["state"], "NOT_RUN")
                self.assertFalse(row["accepted"])
                self.assertFalse(row["compiler_validated"])
                self.assertFalse(row["solver_ran"])

    def test_each_case_changes_only_the_real_body_and_source_guard_refuses_it(self):
        for name, row in mutation.mutations(INPUTS[guard.BODY]).items():
            changed = dict(INPUTS)
            changed[guard.BODY] = row["text"]
            with self.subTest(name=name):
                self.assertEqual({p for p in INPUTS if INPUTS[p] != changed[p]}, {guard.BODY})
                self.assertEqual(changed[guard.PROOF], INPUTS[guard.PROOF])
                self.assertEqual(INPUTS[guard.BODY].count(row["before"]), 1)
                self.assertEqual(row["text"], INPUTS[guard.BODY].replace(row["before"], row["after"], 1))
                with self.assertRaises(ValueError):
                    guard.audit(changed)

    def test_unpinned_or_duplicate_macro_body_refused(self):
        original = INPUTS[guard.BODY]
        for changed in (original + "\n", original.replace("writer.key.local", "writer.key.context_generation"),
                        original + "\nmacro_rules! completion_writer_lookup_body_v1 {}"):
            with self.assertRaises(ValueError):
                mutation.mutations(changed)

    def test_missing_or_ambiguous_anchor_refused(self):
        original = INPUTS[guard.BODY]
        for before, after in (("not an executable anchor", "false"), ("return absent;", "return Ok(None);"),
                              ("Ok(Some(writer))", "Ok(Some(writer))")):
            with self.assertRaises(ValueError):
                mutation.mutate(original, before, after)

    def test_generated_domain_omits_only_selected_axis(self):
        for ignored in ("stream", "hold", "shell_key"):
            text = mutation.domain_without(ignored)
            self.assertIn("_ => root.domain != $domain", text)
            self.assertIn(ignored + ": _", text)
            for field in set(("stream", "hold", "shell_key")) - {ignored}:
                self.assertIn("left_" + field + " != right_" + field, text)
        with self.assertRaises(ValueError):
            mutation.domain_without("not_a_field")

    def test_manifest_exactly_records_constructed_unrun_bytes(self):
        cases = mutation.mutations(INPUTS[guard.BODY])
        raw, mode = guard.ordinary(ROOT / "pins/CONTEXT_WRITER_LOOKUP_MUTATIONS_V1.json")
        manifest = guard.unique_json(raw)
        self.assertEqual(mode, 0o644)
        self.assertEqual(manifest["state"], "NOT_RUN")
        self.assertEqual(manifest["body_before_sha256"], mutation.BODY_SHA)
        self.assertEqual(manifest["proof_sha256"], mutation.PROOF_SHA)
        self.assertEqual(manifest["cases"], {
            name: {key: value for key, value in row.items() if key != "text"}
            for name, row in cases.items()
        })


if __name__ == "__main__":
    unittest.main(verbosity=2)
