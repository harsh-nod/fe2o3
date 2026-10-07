#!/usr/bin/env python3
"""Qualify complete logical enrollment, not production Rust or native refinement."""

import argparse
import copy
import functools
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
PINS = HERE / "pins"
PREFIX = (
    "// Enrollment execution development: custody and issuance over exact logical contents.\n"
    "// Production sorting/search correspondence and native integration remain separate.\n"
)
INCLUDE = 'include!("context_producer_journal_issuance_v1.rs");\n'
COUNT = 263
DEPENDENCIES = {
    "context_producer_journal_issuance_v1.rs": "CONTEXT_PRODUCER_JOURNAL_ISSUANCE_SHA256",
    "context_producer_read_invariant_v1.rs": "CONTEXT_PRODUCER_READ_INVARIANT_SHA256",
    "context_read_invariant_v1.rs": "CONTEXT_READ_INVARIANT_SHA256",
    "context_read_commit_v1.rs": "CONTEXT_READ_COMMIT_SHA256",
    "context_read_preflight_v1.rs": "CONTEXT_READ_PREFLIGHT_SHA256",
    "context_version_journal_issuance_v1.rs": "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@functools.cache
def inherited():
    path = HERE / "check-producer-journal-issuance.py"
    data = path.read_bytes()
    need(
        hashlib.sha256(data).hexdigest()
        == (PINS / "PRODUCER_JOURNAL_ISSUANCE_CHECKER_SHA256").read_text().strip(),
        "pinned inherited checker",
    )
    spec = importlib.util.spec_from_file_location("enrollment_issuance", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    exec(compile(data, str(path), "exec"), module.__dict__)
    return module


def mutations(base):
    header = "enrollment_header_exec_v1"
    entry = "enrollment_entry_error_exec_v1"
    commit = "enrollment_commit_journal_exec_v1"
    cases = [
        base.Mutation(
            "foreign_precedence",
            entry,
            "entry.key.context_generation != context || entry.device.context_generation != context",
            "entry.key.context_generation != context && entry.device.context_generation != context",
            "enrollment_entry_error_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "allocation_error_identity",
            entry,
            "} else if !issuable_id_exec_v1(entry.key.local) {\n        Some(EnrollmentErrorV1::InvalidAllocationId)",
            "} else if !issuable_id_exec_v1(entry.key.local) {\n        Some(EnrollmentErrorV1::InvalidDeviceId)",
            "enrollment_entry_error_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "shape_error_identity",
            header,
            "if entries.len() > capacity || output.len() != entries.len() {\n        return Err(EnrollmentErrorV1::RosterCapacity);",
            "if entries.len() > capacity || output.len() != entries.len() {\n        return Err(EnrollmentErrorV1::InvalidState);",
            "enrollment_header_decision_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "empty_batch_precedence",
            header,
            "if entries.len() == 0 { return Ok(()); }",
            "if entries.len() == 0 { return Err(EnrollmentErrorV1::InvalidState); }",
            "enrollment_header_decision_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "rejection_output_mutation",
            header,
            "if output[index].is_some() { return Err(EnrollmentErrorV1::InvalidState); }",
            "if output[index].is_some() { output[index] = None; return Err(EnrollmentErrorV1::InvalidState); }",
            "final(output)@ == old(output)@",
            COUNT - 1,
        ),
        base.Mutation(
            "retained_free_prefix",
            commit,
            "contents.allocation_free.truncate(remaining);",
            "contents.allocation_free.truncate(0);\n    return;",
            "final(contents).allocation_free@ == old(contents).allocation_free@.subrange(0, remaining as int)",
            COUNT - 1,
        ),
        base.Mutation(
            "writer_watermark_frame",
            commit,
            "contents.allocation_free.truncate(remaining);",
            "contents.allocation_free.truncate(remaining);\n    contents.registration_watermark = 0;\n    return;",
            "enrollment_untouched_journal_v1",
            COUNT - 1,
        ),
    ]

    def add(name, function, before, after, postcondition):
        cases.append(base.Mutation(name, function, before, after, postcondition, COUNT - 1))

    add("key_comparison", "enrollment_key_less_exec_v1",
        "left.local < right.local", "left.local > right.local", "enrollment_key_less_v1")
    add("key_search_full_identity", "enrollment_contains_key_exec_v1",
        "    found\n}", "    found || entries[lo].key.local == key.local\n}", "found ==")
    add("slot_search_result", "enrollment_contains_slot_exec_v1",
        "    found\n}", "    !found\n}", "found ==")
    for name, function, before, after, post in [
        ("live_key_replay", "enrollment_replay_exec_v1", "false", "true", "enrollment_replay_v1"),
        ("selected_vacancy", "enrollment_selected_vacant_exec_v1", "true", "false", "enrollment_selected_vacant_v1"),
        ("selected_duplicates", "enrollment_duplicate_slots_exec_v1", "false", "true", "enrollment_slots_distinct_v1"),
        ("retained_overlap", "enrollment_retained_clear_exec_v1", "true", "false", "enrollment_retained_clear_v1"),
    ]:
        add(name, function, f"    {before}\n}}", f"    {after}\n}}", post)
    fill = "    proof { assert(output@ =~= enrollment_plan_output_v1(*journal, entries@)); }"
    add("plan_coordinate_loss", "enrollment_fill_plan_exec_v1", fill,
        fill + "\n    if output.len() > 0 { output[0] = None; }", "enrollment_plan_output_v1")
    add("output_restoration", "enrollment_clear_output_exec_v1", "    let mut index = 0usize;",
        "    if output.len() > 0 { return; }\n    let mut index = 0usize;", "final(output)@ == Seq::new")
    tail = ("                    assert(previous[i].unwrap().slot <= previous[j].unwrap().slot);\n"
            "                }\n            }\n        }\n    }\n}")
    add("sorted_output_order", "enrollment_sort_slots_exec_v1", tail,
        tail[:-1] + "    if len > 1 { enrollment_swap_exec_v1(values, 0, len - 1); }\n}",
        "enrollment_sort_relation_v1")
    add("execution_success_result", "enrollment_journal_exec_v1", "    Ok(())\n}",
        "    Err(EnrollmentErrorV1::AllocationReplay)\n}", "enrollment_execution_relation_v1")
    for name, change in [
        ("producer_counts", "contents.counts.clear();"),
        ("producer_incarnation", "contents.next_incarnation = 0;"),
        ("stable_incarnation", "contents.stable.next_incarnation = 0;"),
    ]:
        add(name, "enrollment_issued_exec_v1", "    result\n}",
            f"    {change}\n    result\n}}", "enrollment_issued_relation_v1")
    return cases


def audit_source(inherited_checker, source, output):
    need(source.isascii() and source.startswith(PREFIX + INCLUDE), "exact initial enrollment include")
    need(source.count(INCLUDE) == 1, "single inherited include")
    for name, pin in DEPENDENCIES.items():
        inherited_checker.check_pin(output / name, PINS / pin)
    stripped = output / "audited-enrollment-body.rs"
    stripped.write_text("use vstd::prelude::*;\n" + source[len(PREFIX + INCLUDE):], encoding="ascii")
    inherited_checker.POLICY.scan(stripped)
    inherited_checker.audit_source((output / "context_producer_journal_issuance_v1.rs").read_text(), output)
    for name, pin in DEPENDENCIES.items():
        inherited_checker.check_pin(output / name, PINS / pin)


def postcondition_bounds(base, source, mutation):
    start, body, _ = base.function_bounds(source, mutation.function)
    cursor = start + source[start:body].index("ensures") + len("ensures")
    begin = cursor
    stack = []
    clauses = []
    pairs = {")": "(", "]": "[", "}": "{"}
    while cursor < body:
        char = source[cursor]
        if char in "([{":
            stack.append(char)
        elif char in ")]}":
            need(stack and stack.pop() == pairs[char], "balanced postcondition delimiters")
        elif char == "," and not stack:
            left, right = begin, cursor
            while left < right and source[left].isspace():
                left += 1
            while right > left and source[right - 1].isspace():
                right -= 1
            clauses.append((left, right))
            begin = cursor + 1
        cursor += 1
    need(not stack and not source[begin:body].strip(), "complete comma-terminated postconditions")
    matches = [(left, right) for left, right in clauses if mutation.postcondition in source[left:right]]
    need(len(matches) == 1, "unique complete target postcondition")
    return matches[0]


def check_result(base, status, stdout, stderr, source, path, mutation):
    need(status == (1 if mutation else 0), "normal expected solver exit")
    result = base.unique_json(stdout)["verification-results"]
    expected = {
        "encountered-error": mutation is not None,
        "encountered-vir-error": False,
        "success": mutation is None,
        "verified": COUNT - 1 if mutation else COUNT,
        "errors": 1 if mutation else 0,
        "is-verifying-entire-crate": True,
    }
    need(
        result == expected
        and all(type(result[key]) is type(value) for key, value in expected.items()),
        "exact whole-crate solver result",
    )
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        need(not diagnostics, "no positive diagnostics")
        return
    need(len(diagnostics) in (1, 2), "one error and optional compiler footer")
    if len(diagnostics) == 2:
        footer = diagnostics.pop()
        need(
            footer["message"] == "aborting due to 1 previous error"
            and footer["$message_type"] == "diagnostic"
            and footer["level"] == "error"
            and footer["code"] is None
            and not footer["spans"]
            and not footer["children"],
            "exact footer",
        )
    diagnostic = diagnostics[0]
    need(
        diagnostic["message"] == "postcondition not satisfied"
        and diagnostic["$message_type"] == "diagnostic"
        and diagnostic["level"] == "error"
        and diagnostic["code"] is None
        and not diagnostic["children"],
        "intended postcondition failure",
    )
    spans = diagnostic["spans"]
    need(len(spans) == 2, "exact postcondition and exit spans")
    for span in spans:
        base.check_span(span, source, path)
    primary = [span for span in spans if span["is_primary"] is True]
    exits = [span for span in spans if span["is_primary"] is False]
    need(len(primary) == len(exits) == 1, "one primary and one exit")
    _, body, end = base.function_bounds(source, mutation.function)
    post, exit_span = primary[0], exits[0]
    need(
        (post["byte_start"], post["byte_end"]) == postcondition_bounds(base, source, mutation)
        and post["label"] == "failed this postcondition",
        "exact target function and specified postcondition",
    )
    need(
        body <= exit_span["byte_start"] < exit_span["byte_end"] <= end
        and exit_span["label"] in ("at this exit", "at the end of the function body"),
        "exit inside mutated executable body",
    )


def campaign(args):
    need(__debug__, "assertions must be enabled")
    j4 = inherited()
    base = j4.BASE
    source_path = HERE / "context_version_journal_enrollment_v1.rs"
    verus = args.verus.resolve(strict=True)
    need(verus.name == "verus", "real Verus executable")
    pin_names = {
        source_path: "CONTEXT_VERSION_JOURNAL_ENROLLMENT_SHA256",
        Path(__file__): "JOURNAL_ENROLLMENT_CHECKER_SHA256",
        **{HERE / name: pin for name, pin in DEPENDENCIES.items()},
        Path(j4.__file__): "PRODUCER_JOURNAL_ISSUANCE_CHECKER_SHA256",
        Path(j4.PRODUCER.__file__): "PRODUCER_READ_INVARIANT_CHECKER_SHA256",
        Path(j4.INVARIANT.__file__): "READ_INVARIANT_CHECKER_SHA256",
        Path(j4.COMMIT.__file__): "READ_COMMIT_CHECKER_SHA256",
        Path(j4.PREFLIGHT.__file__): "READ_PREFLIGHT_CHECKER_SHA256",
        Path(base.__file__): "JOURNAL_ISSUANCE_CHECKER_SHA256",
        Path(j4.POLICY.__file__): "PROOF_SOURCE_CHECKER_SHA256",
        verus: "VERUS_SHA256",
        PINS / "VERUS_CLOSURE_MANIFEST": "VERUS_CLOSURE_MANIFEST_SHA256",
    }
    snapshots = j4.PRODUCER.pinned_snapshots(pin_names)
    closure = HERE.parents[2] / "examples/row_softmax_v1/verify-verus-closure.sh"
    snapshots[closure] = closure.read_bytes()
    need(
        hashlib.sha256(snapshots[closure]).hexdigest()
        == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
        "pinned Verus closure checker",
    )
    for path in (verus.parent / "rust_verify", verus.parent / "z3"):
        snapshots[path] = path.read_bytes()
    identities = {str(path): hashlib.sha256(data).hexdigest() for path, data in snapshots.items()}
    need({p: sha(Path(p)) for p in identities} == identities, "authenticated input snapshot")
    source = snapshots[source_path].decode("ascii")
    need(source.startswith(PREFIX + INCLUDE) and source.count(INCLUDE) == 1, "exact pinned include")
    cases = [("positive_before", None), *[(m.name, m) for m in mutations(base)], ("positive_after", None)]
    candidates = {name: base.mutate(source, mutation) if mutation else source for name, mutation in cases}
    output = args.output.resolve()
    output.mkdir()
    (output / "inputs-before.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent")}
    for key, default in (("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo")):
        environment[key] = os.environ.get(key, str(Path(environment["HOME"]) / default))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    closure_command = [
        "/bin/sh",
        str(closure),
        str(verus.parent),
        str(PINS / "VERUS_CLOSURE_MANIFEST"),
    ]
    status, _, _ = base.run_owned(
        closure_command, 120, output / "closure-before", environment
    )
    need(status == 0, "complete pinned Verus distribution before campaign")
    for name, mutation in cases:
        case = output / name
        case.mkdir()
        current = candidates[name]
        path = case / source_path.name
        path.write_text(current, encoding="ascii")
        for dependency in DEPENDENCIES:
            (case / dependency).write_bytes(snapshots[HERE / dependency])
        audit_source(j4, current, case)
        generated = {str(path): sha(path) for path in case.glob("*.rs")}
        (case / "sources.json").write_text(json.dumps(generated, indent=2) + "\n")
        command = [
            "/usr/bin/timeout",
            "--foreground",
            "--signal=TERM",
            "--kill-after=5",
            str(args.timeout),
            str(verus),
            "--crate-type",
            "lib",
            "--triggers-mode",
            "silent",
            "--no-cheating",
            "--output-json",
            "--error-format=json",
            "--num-threads",
            "4",
            str(path),
        ]
        need({p: sha(Path(p)) for p in identities} == identities, "authenticated inputs before solver")
        need({p: sha(Path(p)) for p in generated} == generated, "generated inputs before solver")
        status, stdout, stderr = base.run_owned(
            command, args.timeout + 10, case / "solver", environment
        )
        need({p: sha(Path(p)) for p in generated} == generated, "generated inputs after solver")
        need({p: sha(Path(p)) for p in identities} == identities, "authenticated inputs after solver")
        need(path.read_text() == current, "case source unchanged")
        for dependency, pin in DEPENDENCIES.items():
            j4.check_pin(case / dependency, PINS / pin)
        check_result(base, status, stdout, stderr, current, path, mutation)
        print(name + ": PASS", flush=True)
    status, _, _ = base.run_owned(
        closure_command, 120, output / "closure-after", environment
    )
    need(status == 0, "complete pinned Verus distribution after campaign")
    after = {path: sha(Path(path)) for path in identities}
    (output / "inputs-after.json").write_text(json.dumps(after, indent=2) + "\n")
    need(after == identities, "all campaign inputs unchanged")
    report = {
        "qualified": True,
        "scope": "complete logical enrollment with producer custody and issuance",
        "positive_obligations": COUNT,
        "inherited_obligations": 201,
        "negative_cases": len(cases) - 2,
        "full_logical_batch_enrollment_verified": True,
        "rust_sorting_search_refined": False,
        "native_or_performance_acceptance": False,
    }
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


def diagnostic_self_test(base, source):
    case = mutations(base)[0]
    changed = base.mutate(source, case)
    path = Path("/owned/enrollment.rs")
    start, end = postcondition_bounds(base, changed, case)
    function, body, finish = base.function_bounds(changed, case.function)

    def span(begin, stop, primary, label):
        first, last = changed.count("\n", 0, begin) + 1, changed.count("\n", 0, stop) + 1
        return {"file_name": str(path), "byte_start": begin, "byte_end": stop,
                "line_start": first, "line_end": last, "column_start": begin - changed.rfind("\n", 0, begin),
                "column_end": stop - changed.rfind("\n", 0, stop), "is_primary": primary,
                "label": label, "expansion": None,
                "text": [{"text": line} for line in changed.splitlines()[first - 1:last]]}

    report = {"verification-results": {"encountered-error": True, "encountered-vir-error": False,
              "success": False, "verified": COUNT - 1, "errors": 1, "is-verifying-entire-crate": True}}
    diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied", "code": None,
                  "level": "error", "children": [], "spans": [span(start, end, True, "failed this postcondition"),
                  span(body, finish, False, "at the end of the function body")]}
    footer = {"$message_type": "diagnostic", "message": "aborting due to 1 previous error",
              "level": "error", "code": None, "spans": [], "children": []}
    stdout, stderr = json.dumps(report), json.dumps(diagnostic)
    check_result(base, 1, stdout, stderr, changed, path, case)
    check_result(base, 1, stdout, stderr + "\n" + json.dumps(footer), changed, path, case)
    positive = copy.deepcopy(report)
    positive["verification-results"].update({"encountered-error": False, "success": True, "verified": COUNT, "errors": 0})
    check_result(base, 0, json.dumps(positive), "", source, path, None)
    rejected = 0

    def rejects(status=1, out=stdout, err=stderr):
        nonlocal rejected
        try:
            check_result(base, status, out, err, changed, path, case)
        except (ValueError, KeyError, TypeError):
            rejected += 1
        else:
            raise ValueError("enrollment parser accepted adverse diagnostic")

    for status in (0, 2, 101, 124, 137, -9, -15):
        rejects(status=status)
    for field, value in [("verified", COUNT), ("verified", float(COUNT - 1)), ("errors", 2), ("errors", True),
                         ("encountered-vir-error", True), ("is-verifying-entire-crate", False), ("success", 0)]:
        bad = copy.deepcopy(report)
        bad["verification-results"][field] = value
        rejects(out=json.dumps(bad))
    for field, value in [("$message_type", "other"), ("message", "precondition not satisfied"),
                         ("code", {"code": "E0308"}), ("level", "warning"),
                         ("children", [{"message": "solver timed out"}])]:
        bad = copy.deepcopy(diagnostic)
        bad[field] = value
        rejects(err=json.dumps(bad))
    for field, value in [("$message_type", "other"), ("message", "aborting due to 2 previous errors"),
                         ("level", "note"), ("code", {}), ("spans", [{}]), ("children", [{}])]:
        bad = copy.deepcopy(footer)
        bad[field] = value
        rejects(err=stderr + "\n" + json.dumps(bad))
    for begin, stop in ((start - 1, end), (start, end + 1), (start + 1, end), (start, end - 1), (function, body)):
        bad = copy.deepcopy(diagnostic)
        bad["spans"][0] = span(begin, stop, True, "failed this postcondition")
        rejects(err=json.dumps(bad))
    for field, value in [("file_name", "/owned/foreign.rs"), ("line_start", 1), ("column_start", 1),
                         ("label", "different postcondition"), ("text", []), ("expansion", {}), ("is_primary", 1)]:
        bad = copy.deepcopy(diagnostic)
        bad["spans"][0][field] = value
        rejects(err=json.dumps(bad))
    rejects(out=stdout + stdout)
    rejects(out=stdout.replace('"errors": 1', '"errors": 1, "errors": 1'))
    rejects(err=stderr + "\n" + stderr)
    rejects(err=stderr + "\nsolver timed out")
    rejects(err="")
    bad = copy.deepcopy(diagnostic)
    bad["spans"][1] = span(0, 2, False, "at this exit")
    rejects(err=json.dumps(bad))
    print(f"PASS: enrollment diagnostic self-test ({rejected} adverse results rejected)")


def self_test():
    inherited_checker = inherited()
    base = inherited_checker.BASE
    source = (HERE / "context_version_journal_enrollment_v1.rs").read_text()
    for case in mutations(base):
        changed = base.mutate(source, case)
        postcondition_bounds(base, changed, case)
    diagnostic_self_test(base, source)
    inherited_checker.self_test((HERE / "context_producer_journal_issuance_v1.rs").read_text())
    rejected = 0
    with tempfile.TemporaryDirectory(prefix="fe2o3-enrollment-audit-") as temporary:
        output = Path(temporary)
        for name in DEPENDENCIES:
            (output / name).write_bytes((HERE / name).read_bytes())
        audit_source(inherited_checker, source, output)
        for invalid in [source.replace(INCLUDE, 'include!("foreign.rs");\n'),
                        source.replace(INCLUDE, '#[cfg(false)]\n' + INCLUDE),
                        "\n" + source, "/*\n" + source + "\n*/", source + INCLUDE,
                        source + '\ninclude!("foreign.rs");\n', source + "\nmod foreign {}\n",
                        source + "\nverus! { proof fn bypass() { assume(false); } }\n",
                        source + "\n#[cfg(false)] fn hidden() {}\n"]:
            try:
                audit_source(inherited_checker, invalid, output)
            except (ValueError, inherited_checker.POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("enrollment auditor accepted adverse source")
        for name in DEPENDENCIES:
            path = output / name
            data = path.read_bytes()
            path.write_bytes(data + b"\n")
            try:
                audit_source(inherited_checker, source, output)
            except (ValueError, inherited_checker.POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("recursive dependency substitution accepted")
            finally:
                path.write_bytes(data)
        audit_source(inherited_checker, source, output)
    print(f"PASS: enrollment self-test ({len(mutations(base))} executable mutations, {rejected} adverse sources rejected)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--verus", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=int, default=180)
    args = parser.parse_args()
    need(1 <= args.timeout <= 300, "timeout must be 1 through 300 seconds")
    module = inherited().BASE
    for signum in module.SIGNALS:
        signal.signal(signum, module.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, module.SIGNALS)
    if args.self_test:
        need(args.verus is None and args.output is None, "self-test arguments")
        self_test()
    else:
        need(args.verus is not None and args.output is not None, "campaign arguments")
        campaign(args)


if __name__ == "__main__":
    main()
