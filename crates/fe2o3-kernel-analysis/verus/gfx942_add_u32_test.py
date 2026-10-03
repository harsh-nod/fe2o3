#!/usr/bin/env python3
"""Synthetic runner controls; these unit tests do not execute the solver."""

import copy
import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("gfx942_add_u32_check.py")
SPEC = importlib.util.spec_from_file_location("gfx942_add_proof_check", PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class AddProofControls(unittest.TestCase):
    def setUp(self):
        self.proof = CHECK.ROOT / CHECK.PROOF
        self.summary = {"verus": copy.deepcopy(CHECK.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": 4, "is-verifying-entire-crate": True,
        }}

    def positive(self, status=0, stderr=""):
        return CHECK.classify(status, json.dumps(self.summary), stderr, self.proof)

    def negative(self, focus=CHECK.FOCUS):
        lines = self.proof.read_text().splitlines()
        macro, path = ((CHECK.MACRO, CHECK.BODY) if focus == CHECK.FOCUS
                       else (CHECK.STATE_MACRO, CHECK.EXECUTE))
        first = next(i for i, line in enumerate(lines) if line.strip().startswith("fn " + focus + "("))
        contract = next(i + 1 for i, line in enumerate(lines)
                        if i > first and "result.value as int ==" in line)
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(macro + "!("))
        body = CHECK.ROOT / path
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + macro + " {"))
        diagnostic = {"level": "error", "message": "postcondition not satisfied", "spans": [
            {"file_name": str(self.proof), "line_start": contract, "is_primary": True},
            {"file_name": str(body), "expansion": {
                "macro_decl_name": macro + "!",
                "span": {"file_name": str(self.proof), "line_start": call},
                "def_site_span": {"file_name": str(body), "line_start": definition},
            }},
        ]}
        self.summary["verification-results"] = {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False,
        }
        return [diagnostic, {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]

    def changed_sources(self, path, before, after):
        sources = CHECK.source_snapshot()
        self.assertEqual(sources[str(path)].count(before.encode("ascii")), 1)
        sources[str(path)] = sources[str(path)].replace(before.encode("ascii"), after.encode("ascii"))
        return sources

    def test_sources_and_eight_mutations_are_closed(self):
        sources = CHECK.source_snapshot()
        mutations = CHECK.mutations(sources[str(CHECK.BODY)].decode("ascii"))
        self.assertEqual(len(mutations), 3)
        self.assertEqual(len(set(mutations.values())), 3)
        states = CHECK.state_mutations(sources[str(CHECK.EXECUTE)].decode("ascii"))
        self.assertEqual(len(states), 5)
        self.assertEqual(len(set(states.values())), 5)
        self.assertTrue(set(mutations).isdisjoint(states))

    def test_positive_requires_exact_exit_identity_and_whole_proof(self):
        self.assertTrue(self.positive())
        self.assertFalse(self.positive(status=1))
        for key, value in [("verified", 0), ("verified", 1), ("verified", True), ("errors", False),
                           ("errors", 1), ("success", False),
                           ("is-verifying-entire-crate", False), ("encountered-vir-error", True)]:
            original = self.summary["verification-results"][key]
            self.summary["verification-results"][key] = value
            self.assertFalse(self.positive())
            self.summary["verification-results"][key] = original
        self.summary["verus"]["version"] = "other"
        self.assertFalse(self.positive())

    def test_positive_rejects_warning_and_malformed_json(self):
        self.assertFalse(self.positive(stderr=json.dumps({"level": "warning", "message": "unproved"})))
        self.assertFalse(CHECK.classify(0, "{", "", self.proof))
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":1,"x":2}')
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":NaN}')

    def test_negative_requires_contract_and_shared_body_diagnostics(self):
        for focus in (CHECK.FOCUS, CHECK.STATE_FOCUS):
            diagnostics = self.negative(focus)
            encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
            self.assertTrue(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True, focus))
            original = copy.deepcopy(diagnostics)
            for path, value in (
                ((0, "spans", 0, "line_start"), 1),
                ((0, "spans", 0, "file_name"), "/tmp/foreign-proof.rs"),
                ((0, "spans", 0, "is_primary"), False),
                ((0, "spans", 1, "file_name"), "/tmp/foreign-body.rs"),
                ((0, "spans", 1, "expansion", "macro_decl_name"), "other!"),
                ((0, "spans", 1, "expansion", "span", "file_name"), "/tmp/foreign-proof.rs"),
                ((0, "spans", 1, "expansion", "span", "line_start"), 1),
                ((0, "spans", 1, "expansion", "def_site_span", "file_name"), "/tmp/foreign-body.rs"),
                ((0, "spans", 1, "expansion", "def_site_span", "line_start"), 0),
                ((0, "spans", 1, "expansion"), None),
            ):
                with self.subTest(focus=focus, path=path):
                    diagnostics[:] = copy.deepcopy(original)
                    current = diagnostics
                    for key in path[:-1]:
                        current = current[key]
                    current[path[-1]] = value
                    self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True, focus))
            diagnostics[:] = original
            other = CHECK.STATE_FOCUS if focus == CHECK.FOCUS else CHECK.FOCUS
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True, other))

    def test_negative_rejects_timeout_translation_and_unrelated_errors(self):
        for focus in (CHECK.FOCUS, CHECK.STATE_FOCUS):
            diagnostics = self.negative(focus)
            encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
            for status in [0, 2, 124, -9]:
                self.assertFalse(CHECK.classify(status, json.dumps(self.summary), encode(), self.proof, True, focus))
            diagnostics[0]["message"] = "arithmetic underflow/overflow"
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True, focus))
            diagnostics[0]["message"] = "postcondition not satisfied"
            diagnostics.append({"level": "note", "message": "unknown unverified item"})
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True, focus))

    def test_exact_state_instruction_operand_and_result_schemas(self):
        for path in (CHECK.RUST, CHECK.PROOF):
            for before, after in (
                ("pub value: u32,", "pub value: u64,"),
                ("pub scc: bool,", "pub scc: u32,"),
                ("Sgpr(u8),", "Sgpr(u16),"),
                ("Constant(u32),", "Constant(i32),"),
                ("    registers: [u32; GFX942_ORDINARY_SGPR_COUNT_V1],", "    registers: [u32; 101],"),
                ("    scc: bool,", "    scc: u8,"),
                ("destination: u8,", "destination: u16,"),
                ("sources: [Gfx942U32SourceV1; 2],", "sources: [Gfx942U32SourceV1; 3],"),
                ("bytes: [u8; 8],", "bytes: [u8; 4],"),
                ("byte_len: usize,", "byte_len: u8,"),
            ):
                with self.subTest(path=path, before=before), self.assertRaises(ValueError):
                    CHECK.validate_sources(self.changed_sources(path, before, after))

    def test_exact_register_count_in_both_models(self):
        for path in (CHECK.RUST, CHECK.PROOF):
            for count in ("101", "103", "102 + 0"):
                with self.subTest(path=path, count=count), self.assertRaises(ValueError):
                    CHECK.validate_sources(self.changed_sources(path, "usize = 102;", "usize = " + count + ";"))

    def test_shared_includes_macros_and_compilation_are_closed(self):
        for path in (CHECK.RUST, CHECK.PROOF):
            prefix = "" if path == CHECK.RUST else "../src/"
            for name in ("add_u32_body", "execute_body"):
                include = 'include!("' + prefix + 'gfx942_integer_semantics_v1/' + name + '.rs");'
                for replacement in ("", include + "\n" + include, include.replace(name, "other")):
                    with self.subTest(path=path, replacement=replacement), self.assertRaises(ValueError):
                        CHECK.validate_sources(self.changed_sources(path, include, replacement))
        for path in (CHECK.BODY, CHECK.EXECUTE):
            for prefix in ('include!("other.rs");\n', "#[cfg(feature = \"alternate\")]\n",
                           "#[verifier::external_body]\n", "macro_rules! other { () => { 0 }; }\n"):
                sources = CHECK.source_snapshot()
                sources[str(path)] = prefix.encode("ascii") + sources[str(path)]
                with self.subTest(path=path, prefix=prefix), self.assertRaises(ValueError):
                    CHECK.validate_sources(sources)
        for path in (CHECK.RUST, CHECK.PROOF):
            sources = CHECK.source_snapshot()
            sources[str(path)] += b"\nmacro_rules! gfx942_s_add_u32_execute_body_v1 { () => { 0 }; }\n"
            with self.subTest(path=path), self.assertRaises(ValueError):
                CHECK.validate_sources(sources)

    def test_actual_production_forwarding_is_exact_and_unique(self):
        for before, after in (
            ("gfx942_add_u32_body_v1!(a, b)", "gfx942_add_u32_body_v1!(b, a)"),
            ("gfx942_s_add_u32_execute_body_v1!(self, state)", "gfx942_s_add_u32_execute_body_v1!(self, state); state.scc = false; unreachable!()"),
            ("pub fn execute(", "#[cfg(feature = \"other\")]\n    pub fn execute("),
            ("impl Gfx942SAddU32V1 {", "impl OtherInstruction {"),
            ("gfx942_s_add_u32_execute_body_v1!(self, state)", "other!(self, state)"),
        ):
            with self.subTest(before=before), self.assertRaises(ValueError):
                CHECK.validate_sources(self.changed_sources(CHECK.RUST, before, after))
        sources = CHECK.source_snapshot()
        sources[str(CHECK.RUST)] += b"\npub fn execute() {}\n"
        with self.assertRaises(ValueError):
            CHECK.validate_sources(sources)

    def test_theorem_contract_cannot_be_weakened_or_assumed(self):
        for before, after in (
            ("self.destination < GFX942_ORDINARY_SGPR_COUNT_V1,", "self.destination == 0,"),
            ("valid_source(self.sources@[1]),", "false,"),
            ("final(state).scc == result.scc,", "true,"),
            ("old(state).registers@.update(self.destination as int, result.value)", "final(state).registers@"),
            ("source_value(instruction.sources@[1], state) as int", "0int"),
            ("gfx942_s_add_u32_execute_body_v1!(self, state)", "assume(false); gfx942_s_add_u32_execute_body_v1!(self, state)"),
        ):
            with self.subTest(before=before), self.assertRaises(ValueError):
                CHECK.validate_sources(self.changed_sources(CHECK.PROOF, before, after))

    def test_comments_strings_and_nested_comments_do_not_fake_forwarding(self):
        call = "gfx942_s_add_u32_execute_body_v1!(self, state)"
        for decoy in ("// " + call + "\n        other!(self, state)",
                      'let decoy = "' + call + '"; other!(self, state)'):
            with self.subTest(decoy=decoy), self.assertRaises(ValueError):
                CHECK.validate_sources(self.changed_sources(CHECK.RUST, call, decoy))
        sources = CHECK.source_snapshot()
        sources[str(CHECK.PROOF)] += b"\n// harmless comment\n/* ordinary comment */\n"
        CHECK.validate_sources(sources)
        sources[str(CHECK.PROOF)] += b"/* nested /* comment */ still nested */\n"
        with self.assertRaises(ValueError):
            CHECK.validate_sources(sources)

    def test_logical_mutants_pass_source_shape_but_require_solver_rejection(self):
        sources = CHECK.source_snapshot()
        for path, generator in ((CHECK.BODY, CHECK.mutations), (CHECK.EXECUTE, CHECK.state_mutations)):
            original = sources[str(path)].decode("ascii")
            for name, mutant in generator(original).items():
                mutated = dict(sources)
                mutated[str(path)] = mutant.encode("ascii")
                with self.subTest(name=name):
                    CHECK.validate_sources(mutated)
            with self.assertRaises(ValueError):
                generator("")
            with self.assertRaises(ValueError):
                generator(original + original)


if __name__ == "__main__":
    unittest.main()
