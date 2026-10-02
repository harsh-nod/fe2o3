#!/usr/bin/env python3
"""Light source/extraction controls only; no compiler or solver qualification."""
import hashlib
import json
from pathlib import Path
import re
import types
import unittest

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
VALIDATOR = "context_producer_input_validate_v1.rs"
FOLD = "context_producer_input_fold_v1.rs"
DEFINITIONS = "producer_input_validate_definitions_v1.rs"
SPEC = "producer_input_fold_spec_v1.rs"
COMPOSITION = "context_producer_input_composition_v1.rs"
NATIVE = "crates/fe2o3-runtime/src/context/versions/"
PINS = {
    "producer_readers.rs": "31312306a5fc4793465e4f662e529f244d968e85f1230bf4eeb11a6d8ffe613a",
    "producer_journal_observer_bodies.rs": "5fb7f1572c41a6f2c6dffa74f040e870bb4ae874133c127ee5ae87d52c3fce59",
    "producer_input_fold_body.rs": "701824a7cf27d45d9ec93e36401bffd988e2d6e8e27da281868507a51f74f158",
    "producer_input_fold_tests.rs": "39f37757239f9f6880caeddded618bb13952d863d8aef11dc5c53453084a2d00",
}
NATIVE_TREE_SHA = "8f4078103cd2995ba50d3b5a6e20847eeced76737a4b2cbf983eab8d789d44b3"
SCHEMA_TREE_SHA = "42cd9af10a99c1eb1d29dc3db42dac8f919fc27d2aeac963272b962333b0c291"
DECLARATIONS = tuple(Path("crates/fe2o3-runtime-model/src") / name for name in (
    "context_version_journal/declarations.rs",
    "context_version_journal/enrollment_declarations.rs",
    "context_read_leases/declarations.rs",
    "context_producer_reads/declarations.rs",
    "context_queued_writers/read_declarations.rs",
))
CHECKER_PINS = {
    "check-producer-input-validate.py": "2ea590120cbdaa88d2369f21bd63bec1ebad8aac01f3e1d2f877bc680cc4fc7b",
    "check-producer-input-fold.py": "e62ba0cdf8914b133d8408fb496b9a0949cbec087dbec5ab069c853d9bc1c1bd",
    "check-producer-input-composition.py": "126c48db6e37807382e012f4dab4527c6111f57d4223a38daf880ca2ad5b819b",
}
PARTS = (DEFINITIONS, COMPOSITION, "producer_input_runtime_declarations_v1.rs",
         "producer_input_journal_comparison_declarations_v1.rs", "producer_input_outcome_spec_v1.rs",
         "producer_input_composition_logic_v1.rs")


def extraction():
    path = HERE / "producer-input-source-extraction-v2.py"
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == "b1986ef96f58d0f50fc7fa0dfe961683f44f7382bed42a3c6b7dfc656365aa71",
         "exact reversible factoring helper")
    module = types.ModuleType("producer_input_extraction")
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def predecessor_sources():
    parts = {name: raw_source(HERE / name).decode() for name in PARTS}
    helper = extraction()
    return {DEFINITIONS: helper.definitions(parts), COMPOSITION: helper.composition(parts)}


def need(value, message):
    if not value:
        raise ValueError(message)


def raw_source(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         "ordinary canonical source path")
    return path.read_bytes()


def baseline(name, digest):
    if name == VALIDATOR:
        text = reconstruct_validator(predecessor_sources()[DEFINITIONS])
    else:
        need(name == FOLD, "known reversible source extraction")
        text = reconstruct_fold(raw_source(HERE / FOLD).decode(), raw_source(HERE / SPEC).decode())
    need(hashlib.sha256(text.encode()).hexdigest() == digest, "exact reconstructed signed source baseline")
    return text


def once(text, old, new):
    need(text.count(old) == 1, "unique source anchor: " + old)
    return text.replace(old, new)


def reconstruct_validator(definitions):
    text = once(definitions, "use std::collections::{HashMap, HashSet};",
                "#![allow(unused_macros)] // The include also defines the separately proved fold.\n"
                "use std::collections::{HashMap, HashSet};")
    text = once(text, "returns: Returns, calls: Ghost<Seq<Call>>", "returns: Returns, calls: Vec<Call>")
    for call, answer in (
        ("ActiveLookup(reference)", "active_lookup"), ("ActiveStatus(reference)", "active_status"),
        ("QueuedLookup(reference)", "queued_lookup"), ("QueuedStatus(reference)", "queued_status"),
        ("Credit(allocation, device, bytes)", "credit"), ("Live(allocation, *record)", "live"),
    ):
        text = once(text, "{ proof { self.calls@ = self.calls@.push(Call::" + call + "); } self.returns." + answer + " }",
                    "{ self.calls.push(Call::" + call + "); self.returns." + answer + " }")
    return text


def reconstruct_fold(root, spec):
    shared = spec.replace("pub(crate) ", "").replace("open spec fn", "spec fn")
    need(shared.count("verus! {\n") == 1 and shared.count("spec fn active_before<E>") == 1,
         "unique complete shared type/spec sections")
    start = shared.index("verus! {\n") + len("verus! {\n")
    split = shared.index("spec fn active_before<E>")
    declarations = once(shared[start:split], "enum CreditObservation {",
        "// These are observations, not a persistent credit_ok field of the owner.\n"
        "enum CreditObservation {")
    specifications = shared[split:shared.rindex("\n}")].rstrip("\n") + "\n\n"
    text = once(root, "mod producer_input_fold_spec_v1;\nuse producer_input_fold_spec_v1::*;\n", "")
    text = once(text, "verus! {\n", "verus! {\n" + declarations)
    return once(text, "impl<C, E> Observations<C, E> {", specifications + "impl<C, E> Observations<C, E> {")


def tree_hash(inventory):
    return hashlib.sha256(json.dumps(inventory, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def native_inventory():
    root = ROOT / "crates/fe2o3-runtime/src"
    need(all(not path.is_symlink() for path in root.rglob("*")), "no symlink source boundaries")
    return {str(path.relative_to(ROOT)): hashlib.sha256(raw_source(path)).hexdigest()
            for path in root.rglob("*.rs")}


def expected_definitions(old):
    text = once(old, "#![allow(unused_macros)] // The include also defines the separately proved fold.\n", "")
    text = once(text, "returns: Returns, calls: Vec<Call>", "returns: Returns, calls: Ghost<Seq<Call>>")
    for call, value in (
        ("ActiveLookup(reference)", "active_lookup"),
        ("ActiveStatus(reference)", "active_status"),
        ("QueuedLookup(reference)", "queued_lookup"),
        ("QueuedStatus(reference)", "queued_status"),
        ("Credit(allocation, device, bytes)", "credit"),
        ("Live(allocation, *record)", "live"),
    ):
        text = once(text, "{ self.calls.push(Call::" + call + "); self.returns." + value + " }",
                    "{ proof { self.calls@ = self.calls@.push(Call::" + call + "); } self.returns." + value + " }")
    return text


def composition_invalid_invariant_preimage(text):
    return once(text,
        "                invalid_reference == ContextVersionJournalErrorV1::InvalidReference,\n", "")


def composition_error_proof_preimage(text):
    text = composition_invalid_invariant_preimage(text)
    text = once(text,
        "proof fn fold_step_error(history: Seq<Receipt>, index: int,\n"
        "    aggregate: fold::ContextProducerReadStatusV1, active: usize, queued: usize,\n"
        "    invalid: ContextVersionJournalErrorV1)\n"
        "    requires 0 <= index < history.len(),\n"
        "    ensures forall|error: ContextVersionJournalErrorV1|\n"
        "        #[trigger] fold_status_result(Err(error)) == history[index].result ==>\n"
        "            fold_status_result(Err(error))\n"
        "                == fold::fold_result(history, index, aggregate, active, queued, invalid),\n"
        "{\n"
        "    reveal_with_fuel(fold::fold_result, 2);\n"
        "}\n\n", "")
    return once(text,
        "                fold_step_error(before.original(), index as int, fold_status(aggregate),\n"
        "                    before.owner.root.references@.len() as usize,\n"
        "                    before.owner.root.queued_references@.len() as usize, invalid_reference);\n", "")


def composition_proof_preimage(text):
    text = composition_error_proof_preimage(text)
    text = once(text,
        "            prefix_facts(self.owner, self.id, self.consumer, self.launch,\n"
        "                self.returns@, self.owner.root.inputs@.len() as int);\n"
        "            prefix_facts(self.owner, self.id, self.consumer, self.launch, self.returns@, index as int);",
        "            prefix_facts(self.owner, self.id, self.consumer, self.launch, self.returns@, index as int);")
    text = once(text,
        "        proof {\n"
        "            vstd::std_specs::vec::axiom_spec_len(&self.owner.root.references);\n"
        "            vstd::std_specs::vec::axiom_spec_len(&self.owner.root.queued_references);\n"
        "            prefix_facts(self.owner, self.id, self.consumer, self.launch,\n"
        "                self.returns@, self.owner.root.inputs@.len() as int);\n"
        "            prefix_extends(self.owner, self.id, self.consumer, self.launch,\n"
        "                self.returns@, 0, self.owner.root.inputs@.len() as int);\n"
        "            assert(self.receipts@ == Seq::<Receipt>::empty());\n"
        "            assert(before.original().take(0) =~= Seq::<Receipt>::empty());\n"
        "        }\n", "")
    return once(text,
        "                vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),\n"
        "                vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),\n"
        "                self.owner.root.references@.len() <= usize::MAX,\n"
        "                self.owner.root.queued_references@.len() <= usize::MAX,\n", "")


def composition_representation_preimage(text):
    text = composition_proof_preimage(text)
    text = once(text,
        "consumed: Ghost<nat>, receipts: Ghost<Seq<Receipt>>, calls: Ghost<Seq<Call>>",
        "ghost consumed: nat, ghost receipts: Seq<Receipt>, ghost calls: Seq<Call>")
    text = once(text, "consumed: Ghost(initial_consumed)", "consumed: initial_consumed")
    text = once(text, "receipts: Ghost(Seq::empty())", "receipts: Seq::empty()")
    need(text.count("calls: Ghost(Seq::empty())") == 2, "two exact empty call-trace constructors")
    text = text.replace("calls: Ghost(Seq::empty())", "calls: Seq::empty()")
    text = re.sub(r"((?:self|out|old\(self\)|final\(self\))\.(?:consumed|receipts|calls))@", r"\1", text)
    need(text.count("validation.calls@") == 2, "two actual leaf call-trace view reads")
    return text.replace("validation.calls@", "validation.calls")


def definitions_representation_preimage(text):
    text = once(text, "returns: Returns, calls: Ghost<Seq<Call>>", "returns: Returns, ghost calls: Seq<Call>")
    return re.sub(r"((?:self|old\(self\)|final\(self\))\.calls)@", r"\1", text)


def composition_audit(text):
    for status in ("Pending", "Success", "NoEffect", "Unknown"):
        need(text.count("ContextProducerReadStatusV1::" + status
                        + " => fold::ContextProducerReadStatusV1::" + status + ",") == 1,
             "exact constructor mapping")
    required = (
        "match result { Ok(status) => Ok(fold_status(status)), Err(error) => Err(error) }",
        "ProducerReadRequestV1::Active(_) => (fold::Family::Active, active_after != active_before)",
        "ProducerReadRequestV1::Queued(_) => (fold::Family::Queued, queued_after != queued_before)",
        "Call::Credit(allocation, device, byte_len) => fold::CreditObservation::Returned",
        "allocation_generation: allocation.context_generation,",
        "allocation_local: allocation.local,",
        "device_generation: device.context_generation,",
        "device_local: device.local,",
        "byte_len, matched,",
        "credit: credit_from_calls(calls, credit),",
        "returns: &'a [Returns],",
        "consumed: Ghost<nat>, receipts: Ghost<Seq<Receipt>>, calls: Ghost<Seq<Call>>",
        "let result = validation.validate(self.id, self.consumer, self.launch, index, active, queued);",
        "active_before, queued_before, *active, *queued, result, validation.calls@, answers.credit));",
        "self.calls@ = self.calls@ + validation.calls@;",
        "{ self.owner.root.references.len() }",
        "{ self.owner.root.queued_references.len() }",
        "producer_input_fold_body!(verus_exec_expr, self, invalid_reference,",
        "final(self).consumed@ == fold::reached(old(self).original(), 0),",
        "final(self).receipts@ == old(self).original().take(final(self).consumed@ as int),",
    )
    for value in required:
        need(text.count(value) == 1, "exact composition bridge: " + value)
    for forbidden in ("result.is_ok()", "status as ", "Vec<Call>", "Vec<Option<", "records:"):
        need(forbidden not in text, "no lossy or executable receipt shortcut: " + forbidden)


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.old_validator = baseline(VALIDATOR, "8f0b816a5d9e4e08e5598a538273a9f25bb918633543a2b71a8f778ae0727c4d")
        cls.old_fold = baseline(FOLD, "0588fd557956b177b7b7be56fda006f25a82e509e23c40920907fecb78830184")
        cls.sources = {name: (HERE / name).read_text() for name in (VALIDATOR, FOLD, DEFINITIONS, SPEC, COMPOSITION)}
        cls.sources.update(predecessor_sources())

    def test_current_factoring_is_reversible_not_qualified(self):
        parts = {name: raw_source(HERE / name).decode() for name in PARTS}
        helper = extraction()
        self.assertNotEqual(parts[DEFINITIONS], self.sources[DEFINITIONS])
        self.assertNotEqual(parts[COMPOSITION], self.sources[COMPOSITION])
        for name, inverse in ((DEFINITIONS, helper.definitions), (COMPOSITION, helper.composition)):
            self.assertEqual(inverse(parts), self.sources[name])
        for name, old, new, inverse in (
            ("producer_input_runtime_declarations_v1.rs", "backend_submission: u64", "backend_submission: u32", helper.definitions),
            ("producer_input_outcome_spec_v1.rs", "if !local", "if local", helper.definitions),
            ("producer_input_composition_logic_v1.rs", "step.active, step.queued", "step.queued, step.active", helper.composition),
        ):
            changed = dict(parts)
            changed[name] = once(changed[name], old, new)
            with self.assertRaises(ValueError):
                inverse(changed)

    def test_reviewed_native_production_and_actual_tests(self):
        for name, digest in PINS.items():
            self.assertEqual(hashlib.sha256((ROOT / (NATIVE + name)).read_bytes()).hexdigest(), digest)
        native = native_inventory()
        self.assertEqual(len(native), 335)
        self.assertEqual(tree_hash(native), NATIVE_TREE_SHA)
        with_schemas = {**native, **{str(path): hashlib.sha256(raw_source(ROOT / path)).hexdigest()
                                   for path in DECLARATIONS}}
        self.assertEqual(len(with_schemas), 340)
        self.assertEqual(tree_hash(with_schemas), SCHEMA_TREE_SHA)

    def test_reconstruction_and_roster_reject_unreviewed_edits(self):
        changed = reconstruct_validator(self.sources[DEFINITIONS] + "\n")
        self.assertNotEqual(hashlib.sha256(changed.encode()).hexdigest(),
                            hashlib.sha256(self.old_validator.encode()).hexdigest())
        changed = reconstruct_fold(self.sources[FOLD].replace("self.records[index].take()", "self.records[0].take()"),
                                   self.sources[SPEC])
        self.assertNotEqual(hashlib.sha256(changed.encode()).hexdigest(),
                            hashlib.sha256(self.old_fold.encode()).hexdigest())
        native = native_inventory()
        key = next(iter(native))
        changed = dict(native, **{key: "0" * 64})
        self.assertNotEqual(tree_hash(changed), NATIVE_TREE_SHA)
        changed = dict(native)
        del changed[key]
        self.assertNotEqual(tree_hash(changed), NATIVE_TREE_SHA)
        changed["crates/fe2o3-runtime/src/unreviewed.rs"] = "0" * 64
        self.assertNotEqual(tree_hash(changed), NATIVE_TREE_SHA)

    def test_validator_extraction_is_exact_except_ghost_storage(self):
        self.assertEqual(self.sources[DEFINITIONS], expected_definitions(self.old_validator))
        self.assertEqual(self.sources[VALIDATOR],
                         "// Standalone conditional proof of the actual per-input validation body.\n"
                         "// The shared definitions also serve the conditional fold/validator composition.\n"
                         "#![allow(unused_macros)]\ninclude!(\"producer_input_validate_definitions_v1.rs\");\n")
        self.assertEqual(self.sources[DEFINITIONS].count("producer_input_validate_body!("), 1)
        self.assertEqual(self.sources[DEFINITIONS].count("proof { self.calls@ = self.calls@.push(Call::"), 6)

    def test_carrier_views_preserve_entire_prior_semantic_source(self):
        for name, inverse, digest in (
            (DEFINITIONS, definitions_representation_preimage,
             "4400ab49992b33119f95c5ab7669871eb0d895acd1aefc4c24b02d8a2b167e34"),
            (COMPOSITION, composition_representation_preimage,
             "c84ae2b71fa02869c9442f1c743fba92a10e6f825e1d229d7508b1242019c166"),
        ):
            self.assertEqual(hashlib.sha256(inverse(self.sources[name]).encode()).hexdigest(), digest)
        changed = once(self.sources[COMPOSITION], "Ghost(initial_consumed)", "Ghost(initial_consumed + 1)")
        with self.assertRaises(ValueError):
            composition_representation_preimage(changed)

    def test_only_exact_proof_annotations_extend_V5(self):
        recovered = composition_proof_preimage(self.sources[COMPOSITION])
        self.assertEqual(hashlib.sha256(recovered.encode()).hexdigest(),
                         "82aa80cfdd8dfa4ee392095383c7c11ff6606607aa4f3da22558e810a2297a6b")
        self.assertIn("requires old(self).wf(), old(self).consumed@ == 0,", recovered)
        changed = once(self.sources[COMPOSITION],
            "                self.owner.root.references@.len() <= usize::MAX,",
            "                self.owner.root.references@.len() < usize::MAX,")
        with self.assertRaises(ValueError):
            composition_proof_preimage(changed)

    def test_only_exact_error_lemma_extends_V6(self):
        recovered = composition_error_proof_preimage(self.sources[COMPOSITION])
        self.assertEqual(hashlib.sha256(recovered.encode()).hexdigest(),
                         "a040d749dc735846e82720915b926962ff9402bd2d5a09f5f77a72fce6a1749f")

    def test_only_exact_invalid_reference_invariant_extends_V7(self):
        recovered = composition_invalid_invariant_preimage(self.sources[COMPOSITION])
        self.assertEqual(hashlib.sha256(recovered.encode()).hexdigest(),
                         "a1c7a633f3db9434b951652e372c00d7f8b84e0650730fd988bc008966f46116")

    def test_fold_extraction_preserves_all_generic_contracts_and_bodies(self):
        old = self.old_fold
        types_start = old.index("#[derive(Clone, Copy, PartialEq, Eq)]")
        types_end = old.index("// C and E are intentionally opaque")
        specs_start = old.index("spec fn active_before<E>")
        specs_end = old.index("impl<C, E> Observations<C, E>")
        expected = old[:types_start] + old[types_end:specs_start] + old[specs_end:]
        expected = once(expected, "use vstd::prelude::*;\n",
                        "use vstd::prelude::*;\nmod producer_input_fold_spec_v1;\nuse producer_input_fold_spec_v1::*;\n")
        self.assertEqual(self.sources[FOLD], expected)
        shared = self.sources[SPEC].replace("pub(crate) ", "").replace("open spec fn", "spec fn")
        shared_types = shared[shared.index("#[derive(Clone, Copy, PartialEq, Eq)]"):shared.index("spec fn active_before<E>")]
        old_types = old[types_start:types_end].replace("// These are observations, not a persistent credit_ok field of the owner.\n", "")
        self.assertEqual(shared_types.strip(), old_types.strip())
        shared_specs = shared[shared.index("spec fn active_before<E>"):shared.rindex("\n}")]
        self.assertEqual(shared_specs.strip(), old[specs_start:specs_end].strip())
        self.assertNotRegex(shared, r"E\s*:\s*(Copy|Clone)")
        self.assertNotRegex(self.sources[FOLD], r"C\s*:\s*(Copy|Clone)")

    def test_composition_bridges_actual_bodies_and_ghost_receipts(self):
        composition_audit(self.sources[COMPOSITION])
        self.assertIn('include!("producer_input_validate_definitions_v1.rs");', self.sources[COMPOSITION])
        self.assertIn('include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");', self.sources[DEFINITIONS])

    def test_source_mutants_are_rejected_not_logically_qualified(self):
        original = self.sources[COMPOSITION]
        mutations = (
            ("active_after != active_before", "result.is_ok()"),
            ("queued_after != queued_before", "result.is_ok()"),
            ("credit: credit_from_calls(calls, credit)", "credit: fold::CreditObservation::NotReached"),
            ("allocation_local: allocation.local", "allocation_local: allocation.context_generation"),
            ("device_local: device.local", "device_local: device.context_generation"),
            ("ContextProducerReadStatusV1::Pending => fold::ContextProducerReadStatusV1::Pending",
             "ContextProducerReadStatusV1::Pending => fold::ContextProducerReadStatusV1::Success"),
            ("Err(error) => Err(error)", "Err(_error) => Err(ContextVersionJournalErrorV1::InvalidReference)"),
            ("consumed: Ghost<nat>", "consumed: usize"),
            ("calls: Ghost<Seq<Call>>", "calls: Vec<Call>"),
            ("{ self.owner.root.references.len() }", "{ self.owner.root.inputs.len() }"),
            ("{ self.owner.root.queued_references.len() }", "{ self.owner.root.queued_requests.len() }"),
            ("self.calls@ = self.calls@ + validation.calls@;", "self.calls@ = Seq::empty();"),
            ("let result = validation.validate(self.id, self.consumer, self.launch, index, active, queued);",
             "let result = Ok(ContextProducerReadStatusV1::Success);"),
        )
        self.assertEqual(len(mutations), 13)
        for old, new in mutations:
            with self.subTest(anchor=old):
                changed = once(original, old, new)
                with self.assertRaises(ValueError):
                    composition_audit(changed)

    def test_no_new_semantic_assumptions(self):
        for name, text in self.sources.items():
            code = re.sub(r"//[^\n]*", "", text)
            self.assertNotRegex(code, r"\b(?:assume|admit|external_body|external_fn_specification)\b", name)
            self.assertNotIn("verifier::axiom", code, name)

    def test_prior_proof_packets_cannot_be_claimed_for_changed_roots(self):
        self.assertNotEqual(self.sources[VALIDATOR], self.old_validator)
        self.assertNotEqual(self.sources[FOLD], self.old_fold)
        self.assertNotIn("EXPECTED_VERIFIED", self.sources[COMPOSITION])
        for name, digest in CHECKER_PINS.items():
            self.assertEqual(hashlib.sha256(raw_source(HERE / name)).hexdigest(), digest)


if __name__ == "__main__":
    unittest.main()
