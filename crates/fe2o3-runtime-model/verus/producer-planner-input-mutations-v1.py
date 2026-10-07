#!/usr/bin/env python3
"""Finite body-only controls for the actual-result/planner-observation join."""
from pathlib import Path

V = Path("crates/fe2o3-runtime-model/verus")
EXTENSION = V / "producer_planner_input_composition_v1.rs"
VALIDATION = V / "context_completion_reconciliation_validation_v1.rs"


def function_interval(text, name, helper):
    anchor = "fn " + name + "("
    if text.count(anchor) != 1:
        raise ValueError("unique intended function")
    start = text.index(anchor)
    prefix = text[text.rfind("\n", 0, start) + 1:start]
    indentation = prefix[:len(prefix) - len(prefix.lstrip())]
    opening = text.index("\n" + indentation + "{", start) + 1 + len(indentation)
    return start, helper.balanced_end(text, opening)


def construct(values, helper):
    cases = {}

    def add(name, path, method, before, after, primary):
        text = values[path]
        lo, hi = function_interval(text, method, helper)
        body = text[lo:hi]
        if body.count(before) != 1 or before == after:
            raise ValueError("exact one body mutation: " + name)
        changed = text[:lo] + body.replace(before, after) + text[hi:]
        cases[name] = {"name": name, "path": path, "method": method,
                       "before": before, "after": after, "primary": primary,
                       "text": changed}

    for source, target in (("Pending", "Success"), ("NoEffect", "Success"),
                           ("Unknown", "Success"), ("Success", "Unknown")):
        before = ("ContextProducerReadStatusV1::" + source
                  + " => completion_input::ContextProducerReadStatusV1::" + source + ",")
        after = before.replace("::" + source + ",", "::" + target + ",")
        add(source.lower() + "-coerced", EXTENSION, "project_input", before, after,
            "out == projected_input(result)")
    add("journal-error-coerced", EXTENSION, "project_input",
        "Err(_) => Err(completion_input::JournalObservationErrorV1 { code: 0 }),",
        "Err(_) => Ok(Some(completion_input::ContextProducerReadStatusV1::Success)),",
        "out == projected_input(result)")
    add("actual-reconciliation-skipped", EXTENSION, "reconcile_planner_input",
        "let result = self.reconcile();", "let result = Ok(ContextProducerReadStatusV1::Success);",
        "final(self).consumed@ == fold::reached(old(self).original(), 0)")
    start = values[EXTENSION].index("exists|actual: StatusResult|")
    end = values[EXTENSION].index(",\n            final(planner).quarantined", start)
    add("actual-result-discarded", EXTENSION, "reconcile_planner_input",
        "let result = self.reconcile();",
        "let _ = self.reconcile();\n        let result = Ok(ContextProducerReadStatusV1::Success);",
        values[EXTENSION][start:end])
    add("caller-observation-retained", EXTENSION, "install_observed_input",
        "context.submissions.nodes[index].input = input;",
        "let _ = input;",
        "input_projection(old(context).submissions, final(context).submissions, id, input)")
    add("other-node-selected", EXTENSION, "install_observed_input",
        "context.submissions.nodes[index].input = input;",
        "context.submissions.nodes[0].input = input;",
        "input_projection(old(context).submissions, final(context).submissions, id, input)")
    add("completion-state-changed", EXTENSION, "install_observed_input",
        "context.submissions.nodes[index].input = input;",
        "context.submissions.nodes[index].input = input;\n"
        "    context.submissions.nodes[index].record.quiescent = true;",
        "input_projection(old(context).submissions, final(context).submissions, id, input)")
    add("journal-error-not-quarantined", VALIDATION, "journal_result_v1<T>",
        "self.quarantined = true;", "self.quarantined = false;",
        "final(self).quarantined == (old(self).quarantined || result.is_err())")
    if len(cases) != 11:
        raise ValueError("exact eleven-control roster")
    return cases
