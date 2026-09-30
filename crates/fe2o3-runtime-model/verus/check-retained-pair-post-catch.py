#!/usr/bin/env python3
"""Conditional retained-pair post-catch controller, not native quarantine/unwind.

The complete KFD Rust source roster binds the reviewed concrete Result adapters,
enum schemas, getter/account routes and immediate settle-to-resume wrapper. The
proof replays actual getter results at their individual observation points. It
does not prove that an arbitrary replay is an observation of a real native owner.
The sole returning-quarantine contract is explicit; it can abort/non-return, and
the uninterpreted relation grants no native effect, release or completion fact.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-kfd/src")
BODY = SRC / "sdma/retained_pair_operation_body.rs"
PROOF = V / "retained_pair_post_catch_v1.rs"
FILES = [PROOF, BODY]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "4787ce99de22e2d06dc9973c23e55cbd082227126741ba5c0c4f2a07fdfb6aee"
PROOF_SHA = "943963c9a992594541382ba163cc178a6425655bc69d0d1e987a8d88d6327e80"
# Measured by the full, unfiltered development discovery on the reviewed source.
EXPECTED_VERIFIED = 14


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = {PROOF} | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact retained source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF}, "exact complete KFD Rust roster plus proof")
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "reviewed complete KFD Rust roster and bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "reviewed exact contracts, payload projections and full owner frame")
    need(sources[PROOF].count('include!("../../fe2o3-kfd/src/sdma/retained_pair_operation_body.rs");') == 1,
         "actual post-catch body is the one proof include")
    need(len(re.findall(r'\binclude!\(', sources[PROOF])) == 1
         and not re.search(r'\binclude!\(', sources[BODY]), "exact two-file executable proof closure")
    need(sources[PROOF].count("#[verifier::external_body]") == 1
         and sources[PROOF].count("pub uninterp spec fn returning_quarantine_effect") == 1,
         "only disclosed returning-quarantine trust")
    need("requires" not in sources[PROOF] and "assume(" not in sources[PROOF]
         and "admit(" not in sources[PROOF], "no healthy-callback or owner-validity precondition")


def change(body, macro, before, after):
    anchor = "macro_rules! " + macro + " {"
    need(body.count(anchor) == 1, "unique changed shared macro")
    start = body.index(anchor)
    end = body.find("\nmacro_rules! ", start + len(anchor))
    end = len(body) if end < 0 else end
    selected = body[start:end]
    need(selected.count(before) == 1 and before != after, "one meaningful shared-body mutation")
    return body[:start] + selected.replace(before, after) + body[end:]


def mutations(body):
    rows = []

    def add(name, macro, focus, before, after):
        rows.append((name, macro, focus, before, after))

    terminal = "retained_pair_terminal_body"
    for name, expression in (
        ("queue", "$pair.queue.require_live_queue_state_v1().is_err()"),
        ("source", "$pair.source.phase() != SharedMemorySessionPhaseV1::Active"),
        ("destination", "$pair.destination.phase() != SharedMemorySessionPhaseV1::Active"),
    ):
        add("terminal-ignores-" + name, terminal, "PostCallbackCustody::terminal", expression,
            "(false && " + expression + ")")

    settle = "retained_pair_settle_body"
    selected = "settle_given_returning_quarantine_contract"
    add("settle-inverts-observation", settle, selected, "if $context.terminal()", "if !$context.terminal()")
    add("settle-terminal-no-quarantine", settle, selected,
        "$context.quarantine();\n                    Settled::Return(value.refuse_terminal_success())",
        "Settled::Return(value.refuse_terminal_success())")
    add("settle-terminal-double-quarantine", settle, selected,
        "$context.quarantine();\n                    Settled::Return(value.refuse_terminal_success())",
        "$context.quarantine();\n                    $context.quarantine();\n                    Settled::Return(value.refuse_terminal_success())")
    add("settle-terminal-keeps-success", settle, selected, "Settled::Return(value.refuse_terminal_success())",
        "Settled::Return(value)")
    add("settle-live-normalizes", settle, selected, "Settled::Return(value)\n", "Settled::Return(value.refuse_terminal_success())\n")
    add("settle-live-quarantines", settle, selected, "Settled::Return(value)\n",
        "$context.quarantine();\n                    Settled::Return(value)\n")
    add("settle-panic-no-quarantine", settle, selected,
        "$context.quarantine();\n                Settled::Resume(payload)", "Settled::Resume(payload)")

    unit = "retained_pair_unit_outcome_body"
    add("unit-terminal-success-kept", unit, "normalize_unit", "Ok(()) => Err(terminal_success_error())", "Ok(()) => Ok(())")
    add("unit-existing-cause-replaced", unit, "normalize_unit", "failure => failure",
        "failure => { let _ = failure; Err(terminal_success_error()) }")

    tickets = "retained_pair_tickets_outcome_body"
    add("tickets-terminal-success-kept", tickets, "normalize_tickets",
        "Ok(tickets) => Err(Gfx942XgmiBatchSubmissionFailureV1::Retained {\n                error: terminal_success_error(), tickets,\n            })",
        "Ok(tickets) => Ok(tickets)")
    add("tickets-success-roster-lost", tickets, "normalize_tickets", "error: terminal_success_error(), tickets,",
        "error: terminal_success_error(), tickets: { let _ = tickets; Vec::new() },")
    for variant, payload in (("Recoverable", "requests"), ("Retained", "tickets")):
        add("tickets-existing-" + variant.lower() + "-cause-replaced", tickets, "normalize_tickets", "failure => failure",
            "Err(Gfx942XgmiBatchSubmissionFailureV1::" + variant + " { error: _, " + payload + " }) => "
            "Err(Gfx942XgmiBatchSubmissionFailureV1::" + variant + " { error: terminal_success_error(), " + payload + " }),\n"
            "            failure => failure")

    completed = "retained_pair_completed_outcome_body"
    add("completed-terminal-success-kept", completed, "normalize_completed",
        "Ok(completed) => Err(Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {\n                error: terminal_success_error(), completed,\n            })",
        "Ok(completed) => Ok(completed)")
    add("completed-success-mappings-lost", completed, "normalize_completed", "error: terminal_success_error(), completed,",
        "error: terminal_success_error(), completed: { let _ = completed; Vec::new() },")
    for variant, payload in (("Retained", "tickets"), ("CompletedCurrentnessIndeterminate", "completed")):
        add("completed-existing-" + payload + "-cause-replaced", completed, "normalize_completed", "failure => failure",
            "Err(Gfx942XgmiBatchWaitFailureV1::" + variant + " { error: _, " + payload + " }) => "
            "Err(Gfx942XgmiBatchWaitFailureV1::" + variant + " { error: terminal_success_error(), " + payload + " }),\n"
            "            failure => failure")

    close = "retained_pair_close_post_body"
    selected = "close_post_given_returning_quarantine_contract"
    add("close-quarantines-success", close, selected, "$result.is_err()", "$result.is_ok()")
    add("close-failure-no-quarantine", close, selected, "$scope.context.quarantine();", "let _ = &$scope.context;")
    add("close-keeps-unfinished", close, selected, "$scope.finished = true;", "$scope.finished = false;")
    add("close-discards-first-cause", close, selected, "        $result\n", "        let _ = $result; Ok(())\n")

    finish = "retained_pair_finish_terminal_body"
    selected = "finish_terminal_given_returning_quarantine_contract"
    add("terminal-finish-no-quarantine", finish, selected, "$scope.context.quarantine();", "let _ = &$scope.context;")
    add("terminal-finish-unfinished", finish, selected, "$scope.finished = true;", "$scope.finished = false;")

    drop = "retained_pair_drop_body"
    selected = "drop_once_given_returning_quarantine_contract"
    add("drop-inverts-finished", drop, selected, "if !$scope.finished", "if $scope.finished")
    add("drop-abandoned-no-quarantine", drop, selected, "$scope.context.quarantine();", "let _ = &$scope.context;")
    add("drop-changes-finished", drop, selected, "            $scope.context.quarantine();",
        "            $scope.context.quarantine();\n            $scope.finished = true;")

    result = {name: (change(body, macro, before, after), "*" + focus)
              for name, macro, focus, before, after in rows}
    need(len(result) == len(rows) == 29 and len(set(result.values())) == 29, "29 distinct logical controls")
    return result


def selection_notes(leaf, focus):
    selectors = {row[1] for row in mutations((ROOT / BODY).read_text()).values()}
    need(focus in selectors, "exact retained post-catch selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function retained_pair_post_catch_v1::" + focus[1:] + " (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated owned-process campaign")
    source = raw.decode("utf-8")
    # Preserve the strict classifier, timeouts, source/ELF closure and ownership
    # controller. Explicit trust forbids advertising this root as no-cheating.
    for before, after in (
        ('"--triggers-mode", "silent", "--no-cheating", "--output-json"',
         '"--triggers-mode", "silent", "--output-json"'),
        ('"--multiple-errors", "0"', '"--multiple-errors", "1"'),
    ):
        need(source.count(before) == 1, "exact bounded campaign adaptation")
        source = source.replace(before, after)
    return source


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "measured accepted positive count is required")
    module = types.ModuleType("retained_post_catch_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-retained-pair-post-catch.py"))
    campaign().main()
