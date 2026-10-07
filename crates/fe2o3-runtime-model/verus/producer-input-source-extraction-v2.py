#!/usr/bin/env python3
"""Recover exact predecessor proof sources for extraction and historical fixtures.

These text inverses do not qualify the changed closure. Current proof discovery
and fresh negative qualification remain separate from historical fixture replay.
"""
import hashlib


def need(value, message):
    if not value:
        raise ValueError(message)


def once(text, old, new):
    need(text.count(old) == 1, "unique extraction anchor: " + old)
    return text.replace(old, new)


def body(text):
    need(text.count("\nverus! {\n") == 1, "single extracted Verus block")
    return text[text.index("verus! {\n") + len("verus! {\n"):text.rindex("\n}")].rstrip("\n") + "\n"


def definitions(parts):
    runtime = parts["producer_input_runtime_declarations_v1.rs"].split("\n", 1)[1]
    runtime = once(runtime, "pub(super) use super::ContextAllocationWriteV1;", "pub use super::ContextAllocationWriteV1;")
    journal = body(parts["producer_input_journal_comparison_declarations_v1.rs"])
    marker = "struct ContextAllocationEnrollmentV1"
    need(journal.count(marker) == 1, "one compared enrollment declaration")
    journal, enrollment = journal.split(marker)
    enrollment = marker + enrollment
    runtime = once(runtime, "#[derive(Clone, Copy, PartialEq, Eq)]\nstruct AllocationRecordV1",
                   journal + "#[derive(Clone, Copy, PartialEq, Eq)]\nstruct AllocationRecordV1")
    conditional = parts["producer_input_validate_definitions_v1.rs"]
    observation = conditional[conditional.index("struct Observations"):conditional.index("impl<'a")]
    runtime = once(runtime, "\n}\n\n// This is the same",
                   "\n" + observation.rstrip() + "\n}\n\n// This is the same").rstrip() + "\n\n"
    outcome = "verus! {\n" + body(parts["producer_input_outcome_spec_v1.rs"]).rstrip() + "\n\n"
    outcome = once(outcome, "fn enrollment(", enrollment + "fn enrollment(")
    result = conditional[:conditional.index("include!(")] + runtime + outcome + conditional[conditional.index("impl<'a"):]
    need(hashlib.sha256(result.encode()).hexdigest()
         == "18628bbcab588eeae7302fe39f2ade8bbc27f8506cc9ee440bc35381e17aa58b",
         "exact qualified predecessor definitions; not current qualification")
    return result


def composition(parts):
    shared = body(parts["producer_input_composition_logic_v1.rs"])
    shared = shared.replace("answers: Source", "answers: Seq<Returns>").replace("source_len(answers)", "answers.len()")
    shared = once(shared,
        "        let observed_answers = source_answers(owner, answers, index, before.active, before.queued);\n", "")
    shared = once(shared, "observed_answers.credit", "answers[end - 1].credit")
    shared = once(shared, "observed_answers)", "answers[end - 1])")
    shared = once(shared,
        "source_answers(owner, answers, (end - 1) as usize, before.active, before.queued)", "answers[end - 1]")
    current = parts["context_producer_input_composition_v1.rs"]
    result = current[:current.index("verus! {\n")] + "verus! {\n" + shared.rstrip() + "\n\n" + current[current.index("struct Composition"):]
    need(hashlib.sha256(result.encode()).hexdigest()
         == "bca4d7dfa5611cd993d810dd36be7d15225b7b97c524cc68760be71596a37b60",
         "exact qualified predecessor composition; not current qualification")
    return result
