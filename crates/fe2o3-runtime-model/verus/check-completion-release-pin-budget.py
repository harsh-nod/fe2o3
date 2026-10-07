#!/usr/bin/env python3
"""Qualify shared release-budget bodies, not allocation or batch composition."""

import hashlib
from pathlib import Path
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/release_pin_budget_body.rs")
PROOF = V / "completion_release_pin_budget_v1.rs"
FUNCTIONS = ("validate_single_release_pin_budget", "validate_reserved_release_pin_budgets")


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique release-budget mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    single, reserved = FUNCTIONS
    add("single-accepts-zero", "Some((_, 0)) => Err($insufficient)",
        "Some((_, 0)) => Ok(())", single)
    add("single-rejects-one", "Some((_, 0))", "Some((_, 1))", single)
    add("reserved-no-debit", ".checked_sub(1)", ".checked_sub(0)", reserved)
    add("reserved-double-debit", ".checked_sub(1)", ".checked_sub(2)", reserved)
    add("reserved-keeps-budget", "*$available = next;", "let _ = next;", reserved)
    add("reserved-aliases-keys", ".entry($index)", ".entry(0u32)", reserved)
    add("reserved-zero-default", ".or_insert($pins)", ".or_insert(0u32)", reserved)
    add("exhausted-returns-error", "return Ok(());", "return Err($insufficient);", reserved)
    add("shortage-returns-success", "return Err($insufficient);", "return Ok(());", reserved)
    need(len(cases) == len(set(cases.values())) == 9
         and all(text != body for text, _ in cases.values()), "distinct executable mutants")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in FUNCTIONS}, "exact budget mutation function")
    functions = FUNCTIONS if focus is None else (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function completion_release_pin_budget_v1::"
          + name + " (selected functions)" for name in functions},
    })


def campaign():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("release_budget_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES = [BODY, PROOF]
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=8)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-release-pin-budget.py"))
    campaign().main()
