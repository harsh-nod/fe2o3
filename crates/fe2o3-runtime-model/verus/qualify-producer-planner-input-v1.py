#!/usr/bin/env python3
"""Whole-root positives and eleven source-bound planner-input logical controls.

An explicit frozen input manifest is required. A candidate manifest is only an
input capture: it never constitutes proof execution or a signed qualification.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
PINS = BASE / "pins"
VERIFIED = 260
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f",
            "platform": {"os": "linux", "arch": "x86_64"},
            "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
            "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}
SUPPORT = (
    "check-producer-planner-input-composition.py", "producer-planner-input-mutations-v1.py",
    "qualify-producer-planner-input-v1.py", "test-producer-planner-input-composition.py",
    "check-journal-issuance.py", "producer-input-diagnostics-v1.py",
    "producer-journal-composition-diagnostics-v1.py",
)
EXTRA_PRIMARY = {
    "actual-reconciliation-skipped": "final(self).receipts@ == old(self).original().take(final(self).consumed@ as int)",
    "caller-observation-retained": "final(context).submissions.node(id).input == input",
    "other-node-selected": "final(context).submissions.node(id).input == input",
}
RECOMMENDATION_EXPRESSIONS = (
    "old(self).root.references@.len()",
    "old(self).root.queued_references@.len()",
)
RECOMMENDATION_CASES = {"actual-reconciliation-skipped", "actual-result-discarded"}
RANGE_RECOMMENDATION = ("recommendation not met: value may be out of range of the target type "
                        "(use `#[verifier::truncate]` on the cast to silence this warning)")
BODY_ENUMERATION = ("function body check: not all errors may have been reported; rerun with a higher "
                    "value for --multiple-errors to find other potential errors in this function")


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), BASE / name)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def capture(guard, values, closure, cases):
    return {"source": guard.manifest(values, closure),
            "support": {name: guard.sha(guard.ordinary(BASE / name)) for name in SUPPORT},
            "controls": {name: {"path": str(row["path"]), "method": row["method"],
                         "before": row["before"], "after": row["after"],
                         "primary": row["primary"], "sha256": guard.sha(row["text"].encode())}
                         for name, row in sorted(cases.items())}}


def report(guard, parser, base, status, stdout, stderr, staged, values, case):
    need = guard.need
    need(type(status) is int and status == (0 if case is None else 1),
         "normal positive or logical negative exit only")
    need(stdout.endswith("\n"), "complete verifier output")
    result = base.parse(parser, stdout)
    need(type(result) is dict and set(result) == {"verus", "verification-results", "func-details"}
         and result["verus"] == VERIFIER, "exact pinned verifier schema")
    expected = {"encountered-error": case is not None, "encountered-vir-error": False,
                "success": case is None, "verified": VERIFIED - (case is not None),
                "errors": 0 if case is None else 1, "is-verifying-entire-crate": True}
    actual = result["verification-results"]
    need(type(actual) is dict and set(actual) == set(expected)
         and all(type(actual[key]) is type(value) and actual[key] == value
                 for key, value in expected.items()), "exact whole-root result, no resource/frontend failures")
    details = result["func-details"]
    need(type(details) is dict and 0 < len(details) <= 1024
         and all(type(name) is str and value == {"obligation_proof_notes": [], "failed_proof_notes": []}
                 for name, value in details.items()), "complete finite function schema")
    if case is None:
        need(stderr == "", "positive without diagnostics")
        return expected
    need(stderr.endswith("\n"), "complete negative diagnostics")
    mapping = {str(staged / path): text for path, text in values.items()}
    text = values[case["path"]]
    lo, hi = load("producer-planner-input-mutations-v1.py").function_interval(text, case["method"], parser)
    ranges = {str(staged / case["path"]): (lo, hi)}
    primary_expressions = [case["primary"]]
    if case["name"] in EXTRA_PRIMARY:
        primary_expressions.append(EXTRA_PRIMARY[case["name"]])

    def position(expression):
        begin, end = base.unique(text[lo:hi], expression)
        return str(staged / case["path"]), lo + begin, lo + end

    primaries_expected = [position(expression) for expression in primary_expressions]
    recommendations_expected = ([position(expression) for expression in RECOMMENDATION_EXPRESSIONS]
                                if case["name"] in RECOMMENDATION_CASES else [])
    signature = None
    if case["name"] in EXTRA_PRIMARY:
        prefix = text[text.rfind("\n", 0, lo) + 1:lo]
        indentation = prefix[:len(prefix) - len(prefix.lstrip())]
        header_end = text.index("\n" + indentation + "    requires", lo)
        if case["name"] == "actual-reconciliation-skipped":
            need(text[lo:header_end].endswith("-> (out: PlannerInputResult)"),
                 "exact calibrated named-result signature")
            # This pinned Verus note excludes the named return binder's final ')'.
            header_end -= 1
        signature = (str(staged / case["path"]), lo, header_end)
    boundaries = {
        "project_input": "project_input",
        "install_observed_input": "install_observed_input",
        "reconcile_planner_input": "Composition::reconcile_planner_input",
        "journal_result_v1<T>": "completion_input::CompletionProjectionV1::journal_result_v1",
    }
    need(guard.PROOF.stem + "::concrete_composition::" + boundaries[case["method"]] in details,
         "exact intended function in complete verifier report")
    rows = [base.parse(parser, line) for line in stderr.splitlines()]
    errors, summaries, enumerations, recommendations = [], [], [], []
    for row in rows:
        need(not summaries and type(row) is dict and set(row) == {
            "$message_type", "message", "code", "level", "spans", "children", "rendered"}
            and row["$message_type"] == "diagnostic" and type(row["message"]) is str
            and type(row["rendered"]) is str and row["code"] is None and row["children"] == []
            and type(row["spans"]) is list, "exact ordered diagnostic schema")
        positions = [parser.span_source(span, mapping, str(staged / guard.PROOF))
                     for span in row["spans"]]
        if row["level"] == "note" and row["message"] == BODY_ENUMERATION:
            need(case["name"] in EXTRA_PRIMARY and len(row["spans"]) == 1
                 and all(span["is_primary"] and span["expansion"] is None for span in row["spans"])
                 and positions == [signature],
                 "exact enumeration note for the intended multiply-failing function")
            enumerations.append(row)
        elif row["level"] == "note" and row["message"] == RANGE_RECOMMENDATION:
            need(len(row["spans"]) == 1 and row["spans"][0]["is_primary"]
                 and row["spans"][0]["expansion"] is None
                 and len(recommendations) < len(recommendations_expected)
                 and positions[0] == recommendations_expected[len(recommendations)],
                 "exact source-bound cast recommendation in a composed-result control")
            recommendations.append(row)
        elif row["level"] == "error" and row["message"] == "postcondition not satisfied":
            need(row["spans"] and all(parser.in_family(position, ranges) for position in positions),
                 "logical failure only in intended function")
            primaries = [span for span in row["spans"] if span["is_primary"]]
            need(len(primaries) == 1 and primaries[0]["label"] == "failed this postcondition"
                 and primaries[0]["expansion"] is None
                 and len(errors) < len(primaries_expected)
                 and parser.span_source(primaries[0], mapping, str(staged / guard.PROOF))
                    == primaries_expected[len(errors)],
                 "exact intended failed contract expression")
            errors.append(row)
        elif row["level"] == "error" and row["message"] == (
                "aborting due to " + str(len(primaries_expected)) + " previous error"
                + ("s" if len(primaries_expected) != 1 else "")):
            need(row["spans"] == [] and len(errors) == len(primaries_expected),
                 "one exact final logical-error summary")
            summaries.append(row)
        else:
            raise ValueError("unreviewed diagnostic: " + row["message"])
    need(len(errors) == len(primaries_expected) and len(summaries) == 1
         and len(enumerations) == (case["name"] in EXTRA_PRIMARY)
         and len(recommendations) == len(recommendations_expected),
         "exact calibrated logical diagnostics and complete summary")
    return {**expected, "control": case["name"], "primary_expressions": primary_expressions,
            "logical_messages": len(errors), "enumeration_notes": len(enumerations),
            "range_recommendations": len(recommendations)}


def campaign(args, guard, values, closure, cases, expected):
    output, verus = args.output, args.verus
    guard.need(output.is_absolute() and output.resolve() == output and not output.exists()
               and output.parent.is_dir(), "fresh canonical owned output")
    output.mkdir()
    guard.need(verus.is_absolute() and verus.resolve() == verus and verus.name == "verus",
               "canonical pinned verifier")
    owned = load("check-journal-issuance.py")
    owned.check_pin(verus, PINS / "VERUS_SHA256")
    owned.check_pin(BASE / "check-journal-issuance.py", PINS / "JOURNAL_ISSUANCE_CHECKER_SHA256")
    owned.check_pin(PINS / "VERUS_CLOSURE_MANIFEST", PINS / "VERUS_CLOSURE_MANIFEST_SHA256")
    checker = ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"
    guard.need(guard.sha(guard.ordinary(checker)) ==
               "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c", "exact closure checker")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent"), "TMPDIR": str(output),
                   "VERUS_Z3_PATH": str(verus.parent / "z3")}
    environment["CARGO_HOME"] = os.environ.get("CARGO_HOME", environment["HOME"] + "/.cargo")
    environment["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", environment["HOME"] + "/.rustup")
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    tools = {str(path): guard.sha(guard.ordinary(path)) for path in (
        verus, verus.parent / "rust_verify", verus.parent / "z3", checker,
        PINS / "VERUS_CLOSURE_MANIFEST", Path(sys.executable).resolve(), args.inputs)}
    write_json(output / "inputs.json", expected)
    write_json(output / "tools.json", tools)
    closure_command = ["sh", str(checker), str(verus.parent), str(PINS / "VERUS_CLOSURE_MANIFEST")]
    code, stdout, stderr = owned.run_owned(closure_command, 120, output / "closure-before", environment)
    guard.need(code == 0 and stderr == "", "complete pinned verifier closure check")
    base = load("producer-journal-composition-diagnostics-v1.py")
    parser = base.parser(BASE / "producer-input-diagnostics-v1.py")
    result = {"complete": False, "scope": "present-root actual producer/planner input projection",
              "whole_context_refinement": False, "dag_settlement_refinement": False,
              "hardware_execution": False, "signed_qualification": False, "cases": []}
    started = time.monotonic()
    for name, case in [("positive-before", None), *sorted(cases.items()), ("positive-after", None)]:
        guard.need(guard.sources() == (values, closure), "source closure and adapter bindings unchanged")
        guard.need(capture(guard, values, closure, cases) == expected, "all support/controls unchanged")
        guard.need(all(guard.sha(guard.ordinary(Path(path))) == sha for path, sha in tools.items()),
                   "original tools/input manifest unchanged")
        memory = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
        guard.need(int(memory["MemAvailable"].split()[0]) * 1024 >= 16 * 1024**3,
                   "unchanged 16 GiB available-memory gate")
        guard.need(sum(path.stat().st_size for path in output.rglob("*") if path.is_file()) < 128 * 1024**2,
                   "128 MiB owned proof scratch cap")
        stage = output / name
        stage.mkdir()
        selected = dict(values)
        if case is not None:
            selected[case["path"]] = case["text"]
        source = guard.stage(stage / "source", selected, closure)
        staged = {path: guard.augmented(text) if path == guard.PROOF else text
                  for path, text in selected.items() if path in closure}
        write_json(stage / "source-hashes.json", {str(path): guard.sha(text.encode())
                                                 for path, text in staged.items()})
        command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120",
                   str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                   "--output-json", "--error-format=json", "--num-threads", "4", str(source)]
        print("running " + name, flush=True)
        status, stdout, stderr = owned.run_owned(command, 130, stage / "solver", environment)
        guard.need(all(guard.ordinary(stage / "source" / path).decode() == text
                       for path, text in staged.items()), "exact staged proof bytes unchanged")
        accepted = report(guard, parser, base, status, stdout, stderr, stage / "source", staged, case)
        result["cases"].append({"name": name, "result": accepted})
        write_json(output / "result.json", result)
        print("passed " + name, flush=True)
    code, stdout, stderr = owned.run_owned(closure_command, 120, output / "closure-after", environment)
    guard.need(code == 0 and stderr == "", "complete final verifier closure check")
    guard.need(guard.sources() == (values, closure)
               and capture(guard, values, closure, cases) == expected
               and all(guard.sha(guard.ordinary(Path(path))) == sha for path, sha in tools.items()),
               "entire campaign source/tool closure unchanged at completion")
    result.update(complete=True, duration_seconds=time.monotonic() - started)
    write_json(output / "result.json", result)
    print(json.dumps(result, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture-inputs", type=Path)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--verus", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    guard = load("check-producer-planner-input-composition.py")
    base = load("producer-journal-composition-diagnostics-v1.py")
    helper = base.parser(BASE / "producer-input-diagnostics-v1.py")
    values, closure = guard.sources()
    cases = load("producer-planner-input-mutations-v1.py").construct(values, helper)
    expected = capture(guard, values, closure, cases)
    if args.capture_inputs is not None:
        guard.need(not args.capture_inputs.exists() and args.inputs is None and args.output is None
                   and args.verus is None, "capture is separate from proof execution")
        write_json(args.capture_inputs, expected)
        print("Captured candidate inputs only; no proof executed.")
        return
    guard.need(args.inputs is not None and args.output is not None and args.verus is not None,
               "frozen inputs, pinned verifier and fresh output required")
    guard.need(json.loads(guard.ordinary(args.inputs)) == expected, "exact frozen campaign inputs")
    campaign(args, guard, values, closure, cases, expected)


if __name__ == "__main__":
    main()
