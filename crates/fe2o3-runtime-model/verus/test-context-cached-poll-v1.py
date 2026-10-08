import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('cached_poll_source', HERE / 'check-context-cached-poll-v1.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class SourceControls(unittest.TestCase):
    def test_original_correspondence(self):
        self.assertEqual(gate.audit(gate.inputs())['actual_shared_bodies'], 7)

    def reject(self, path, old, new):
        data = gate.inputs()
        self.assertEqual(data[path].count(old), 1)
        data[path] = data[path].replace(old, new)
        with self.assertRaises(ValueError):
            gate.audit(data)

    def test_skip_generation_refused(self):
        self.reject(gate.BODY, '$submission.id.context_generation != $context.context_generation', 'false')

    def test_substituted_backend_refused(self):
        self.reject(gate.BODY, 'record.backend_submission != $submission.backend_submission', 'false')

    def test_missing_live_stream_refused(self):
        self.reject(gate.BODY, '$context.live_submission_record($submission)?', '$context.submission_record($submission)?')

    def test_skip_hold_refused(self):
        self.reject(gate.BODY, '$context.require_stream_unheld_v1(record.stream)?;', '')

    def test_token_cache_as_authority_refused(self):
        self.reject(gate.BODY, 'if record.status.is_terminal() {', 'if $submission.completion.is_some() {')

    def test_pending_success_refused(self):
        self.reject(gate.BODY, '            Ok(None)', '            Ok(Some(RuntimePollV1::Succeeded))')

    def test_fresh_backend_tail_change_refused(self):
        self.reject(gate.CONTEXT, '|backend, id| backend.poll_v1(id))?;\n        Ok(submission.observe_status(status))',
                    '|backend, id| backend.poll_v1(id + 1))?;\n        Ok(submission.observe_status(status))')

    def test_outer_gate_reordered_refused(self):
        self.reject(gate.CONTEXT, 'self.require_graph_access(access)?;\n        if let Some(status) = self.poll_cached_status_v1(submission)? {',
                    'if let Some(status) = self.poll_cached_status_v1(submission)? {\n            self.require_graph_access(access)?;')

    def test_omitted_opaque_context_field_refused(self):
        self.reject(gate.PROOF, '    backend: O,\n', '')

    def test_omitted_peer_frame_refused(self):
        self.reject(gate.PROOF, '    &&& before.peer_transfer == after.peer_transfer\n', '')

    def test_vacuous_precondition_refused(self):
        self.reject(gate.PROOF, '    requires key_contracts_v1(),\n    ensures',
                    '    requires key_contracts_v1(), false,\n    ensures')

    def test_assume_refused(self):
        self.reject(gate.PROOF, '    context.poll_cached_status_v1(submission)\n',
                    '    proof { assume(false); }\n    context.poll_cached_status_v1(submission)\n')

    def test_std_map_shadow_refused(self):
        self.reject(gate.PROOF, 'use std::collections::HashMap;', 'type HashMap<K, V> = ScriptedMap<K, V>;')

    def test_extra_shared_arm_refused(self):
        self.reject(gate.BODY, 'macro_rules! cached_poll_prefix_body_v1 {',
                    'macro_rules! cached_poll_prefix_body_v1 { ($anything:tt) => { unreachable!() };')

    def test_hidden_key_premise_refused(self):
        self.reject(gate.PROOF, 'spec fn key_contracts_v1() -> bool {',
                    'spec fn key_contracts_v1() -> bool { false &&')

    def test_changed_legacy_error_constant_refused(self):
        self.reject(gate.PROOF, 'const RUNTIME_CANCELLED_CODE_V1: i64 = -2;',
                    'const RUNTIME_CANCELLED_CODE_V1: i64 = -3;')

    def test_true_disjunction_postcondition_refused(self):
        self.reject(gate.PROOF, 'out == prefix_selection_v1(self, old(submission)),',
                    'true || out == prefix_selection_v1(self, old(submission)),')

    def test_omitted_submission_key_premise_refused(self):
        self.reject(gate.PROOF, '    &&& vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>()',
                    '    &&& true')

    def test_removed_whole_owner_frame_refused(self):
        self.reject(gate.PROOF, '        *final(context) == *old(context),', '')

    def test_runtime_binder_substitution_refused(self):
        self.reject(gate.UNPUBLISHED, '.map(|_held_stream| ())', '.map(|_other| ())')

    def test_proof_binder_substitution_refused(self):
        self.reject(gate.PROOF, '.map(|_held_stream| ())', '.map(|_other| ())')

    def test_ambiguous_mutable_poststate_refused(self):
        self.reject(gate.PROOF, 'token_frame_v1(*old(self), *final(self)),',
                    'token_frame_v1(*old(self), *self),')



class PortableSourceControls(unittest.TestCase):
    def test_complete_support_closure(self):
        records = gate.support_check()
        self.assertEqual(len(records), 14)
        self.assertEqual(set(records), {str(path.relative_to(gate.ROOT)) for path in gate.SUPPORT_FILES})

    def test_complete_seven_input_roster(self):
        self.assertEqual(len(gate.inputs()), 7)

    def test_manifest_extra_path_refused(self):
        rows = {str(p): {} for p in gate.FILES}
        rows['unexpected.rs'] = {}
        with self.assertRaises(ValueError):
            gate.validate_input_roster(rows)

    def test_manifest_missing_path_refused(self):
        rows = {str(p): {} for p in gate.FILES}
        rows.pop(str(gate.LIB))
        with self.assertRaises(ValueError):
            gate.validate_input_roster(rows)

    def test_duplicate_manifest_key_refused(self):
        with self.assertRaises(ValueError):
            gate.unique_json(b'{"same": 1, "same": 2}')

    def reject_module(self, path, old, new):
        data = gate.inputs()
        self.assertEqual(data[path].count(old), 1)
        data[path] = data[path].replace(old, new)
        with self.assertRaises(ValueError):
            gate.audit(data)

    def test_runtime_module_disabled_refused(self):
        self.reject_module(gate.LIB, 'mod context;', '#[cfg(any())]\nmod context;')

    def test_unpublished_path_substitution_refused(self):
        self.reject_module(gate.CONTEXT, 'mod unpublished;',
                           '#[path = "other.rs"]\nmod unpublished;')

    def test_test_owner_disabled_refused(self):
        self.reject_module(gate.CONTEXT, '#[cfg(test)]\nmod tests {',
                           '#[cfg(any())]\nmod tests {')

    def test_cached_test_leaf_disabled_refused(self):
        self.reject_module(gate.IDENTITY_TESTS, 'mod cached_poll;',
                           '#[cfg(any())]\nmod cached_poll;')

    def test_reference_tampering_refused_before_semantics(self):
        from unittest import mock
        original = gate.ordinary
        def read(path):
            raw, mode = original(path)
            return (raw + b' ', mode) if path == gate.REFERENCE else (raw, mode)
        with mock.patch.object(gate, 'ordinary', side_effect=read):
            with self.assertRaises(ValueError):
                gate.audit(gate.inputs())

    def test_actual_mutations_leave_proof_immutable_and_change_only_named_body(self):
        mutation_path = HERE / 'context-cached-poll-mutations-v1.py'
        spec = importlib.util.spec_from_file_location('cached_mutations', mutation_path)
        mutation = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mutation)
        data = gate.inputs()
        rows = mutation.mutations(data[gate.BODY])
        self.assertEqual(len(rows), 16)
        self.assertEqual(set(rows), {case[0] for case in mutation.CASES})
        for name, item in rows.items():
            with self.subTest(name=name):
                candidate = dict(data, **{})
                candidate[gate.BODY] = item['text']
                self.assertEqual({p for p in data if data[p] != candidate[p]}, {gate.BODY})
                self.assertEqual(candidate[gate.PROOF], data[gate.PROOF])
                self.assertFalse(item['accepted'])
                with self.assertRaises(ValueError):
                    gate.audit(candidate)


class ReachabilityControls(unittest.TestCase):
    EDGES = (('LIB', 'context'), ('CONTEXT', 'unpublished'),
             ('CONTEXT', 'submission_identity_tests'), ('IDENTITY_TESTS', 'cached_poll'))

    def reject_edge(self, owner, name, replacement):
        data = gate.inputs()
        path = getattr(gate, owner)
        old = 'mod ' + name + ';'
        self.assertEqual(data[path].count(old), 1)
        data[path] = data[path].replace(old, replacement)
        with self.assertRaises(ValueError):
            gate.audit(data)

    def test_root_module_preserves_lexical_tokens_and_visibility(self):
        for visibility in ('', 'pub ', 'pub(crate) ', 'pub(super) ', 'pub(in crate::scope) '):
            with self.subTest(visibility=visibility):
                gate.root_module('use other::item;\n' + visibility + 'mod /* gap */ context;', 'context')

    def test_root_module_requires_actual_token_and_unique_declaration(self):
        for text in ('modcontext;', 'notmod context;', 'mod context_suffix;',
                     '// mod context;', '"mod context;"', 'mod context; mod context;'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                gate.root_module(text, 'context')

    def test_all_actual_module_edges_reject_macro_delimiter_decoys(self):
        for owner, name in self.EDGES:
            old = 'mod ' + name + ';'
            for wrapper in ('stringify!({})', 'stringify![{}]', 'stringify!{{{}}}'):
                with self.subTest(owner=owner, name=name, wrapper=wrapper):
                    self.reject_edge(owner, name, wrapper.format(old) + ';')

    def test_all_actual_module_edges_reject_attributes_hidden_by_visibility(self):
        for owner, name in self.EDGES:
            for attribute, visibility in (('#[cfg(any())]', 'pub(crate) '),
                                          ('#[path = "other.rs"]', 'pub ')):
                with self.subTest(owner=owner, name=name, attribute=attribute):
                    self.reject_edge(owner, name, attribute + '\n' + visibility + 'mod ' + name + ';')

    def test_inline_owner_keeps_original_code_token_boundaries(self):
        gate.module_edges(gate.inputs())
        values = {gate.LIB: 'mod context;', gate.IDENTITY_TESTS: 'mod cached_poll;'}
        for owner in ('#[cfg(test)]\nmod tests { mod submission_identity_tests; }',
                      '# [ cfg ( test ) ] mod /* gap */ tests { pub(super) mod submission_identity_tests; }'):
            with self.subTest(owner=owner):
                gate.module_edges(values | {gate.CONTEXT: 'mod unpublished;\n' + owner})

    def test_inline_owner_rejects_macro_nesting_and_extra_attributes(self):
        owner = '#[cfg(test)] mod tests { mod submission_identity_tests; }'
        values = {gate.LIB: 'mod context;', gate.IDENTITY_TESTS: 'mod cached_poll;'}
        candidates = [wrapper.format(owner) + ';' for wrapper in
                      ('stringify!({})', 'stringify![{}]', 'stringify!{{{}}}')]
        candidates += ['#[cfg(any())]\n' + owner, '#[path = "other.rs"]\n' + owner,
                       owner.replace('mod tests', 'pub(crate) mod tests'), owner + owner]
        for candidate in candidates:
            with self.subTest(candidate=candidate), self.assertRaises(ValueError):
                gate.module_edges(values | {gate.CONTEXT: 'mod unpublished;\n' + candidate})

    def test_actual_runtime_and_proof_includes_remain_root_items(self):
        data = gate.inputs()
        for path, include in ((gate.CONTEXT, 'include!("context/cached_poll_body.rs");'),
                              (gate.PROOF, 'include!("../../fe2o3-runtime/src/context/cached_poll_body.rs");')):
            with self.subTest(path=path):
                gate.root_include(data[path], include)

    def test_shared_include_rejects_nested_expression_attributes_and_decoys(self):
        include = 'include!("context/cached_poll_body.rs");'
        for wrapper in ('stringify!({})', 'stringify![{}]', 'stringify!{{{}}}',
                        'const _: () = ({})', '#[cfg(any())] {}', '#[path = "other.rs"] {}'):
            with self.subTest(wrapper=wrapper):
                data = gate.inputs()
                self.assertEqual(data[gate.CONTEXT].count(include), 1)
                data[gate.CONTEXT] = data[gate.CONTEXT].replace(include, wrapper.format(include) + ';')
                with self.assertRaises(ValueError):
                    gate.audit(data)
        for text in ('// ' + include, 'not__cached_poll_include__;' + include, include + include):
            with self.subTest(text=text), self.assertRaises(ValueError):
                gate.root_include(text, include)


class ReaderControls(unittest.TestCase):
    def test_stale_regular_precheck_cannot_block_on_fifo_substitution(self):
        import os
        import stat
        import tempfile
        from unittest import mock
        original_is_file = Path.is_file
        original_open = os.open
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'input.rs'
            path.write_bytes(b'original regular input')
            opens = []

            def stale_precheck(selected):
                if selected == path:
                    self.assertTrue(original_is_file(selected))
                    selected.unlink()
                    os.mkfifo(selected)
                    return True
                return original_is_file(selected)

            def checked_open(selected, flags, *args, **kwargs):
                self.assertTrue(flags & os.O_NONBLOCK)
                opens.append(str(selected))
                return original_open(selected, flags, *args, **kwargs)

            with mock.patch.object(Path, 'is_file', side_effect=stale_precheck, autospec=True), \
                    mock.patch.object(os, 'open', side_effect=checked_open):
                with self.assertRaisesRegex(ValueError, 'bounded regular input'):
                    gate.ordinary(path)
            self.assertEqual(opens, [str(path)])
            self.assertTrue(stat.S_ISFIFO(path.lstat().st_mode))


if __name__ == '__main__':
    unittest.main()
