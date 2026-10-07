#!/usr/bin/env python3
"""Source-bound executable mutations for the contents-only V4-J1 proof."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass


POSITIVE_COUNT = 69


@dataclass(frozen=True)
class Mutation:
    name: str
    function: str
    before: str
    after: str
    postcondition: str
    verified: int = 68


def mutations() -> tuple[Mutation, ...]:
    constructor = "constructor_contents_exec_v1"
    register = "register_writer_exec_v1"
    abort = "abort_reserved_exec_v1"
    relations = {
        constructor: "constructor_contents_relation_v1",
        register: "register_execution_relation_v1",
        abort: "abort_execution_relation_v1",
    }
    entries = [
        ("constructor_context", constructor, "context_generation: context", "context_generation: 0"),
        ("constructor_watermark", constructor, "registration_watermark: 0", "registration_watermark: 1"),
        ("constructor_count", constructor, "reserved_count: 0", "reserved_count: 1"),
        ("constructor_writer_bound", constructor, "allocation_capacity, writer_capacity,", "allocation_capacity, writer_capacity: allocation_capacity,"),
        ("constructor_writer_extent", constructor, "let writers = vacant_contents_exec_v1(writer_capacity);", "let writers = vacant_contents_exec_v1(allocation_capacity);"),
        ("constructor_allocation_extent", constructor, "let allocations = vacant_contents_exec_v1(allocation_capacity);", "let allocations = vacant_contents_exec_v1(writer_capacity);"),
        ("constructor_scratch_omitted", constructor, "let scratch = vacant_contents_exec_v1(allocation_capacity);", "let scratch = Vec::new();"),
        ("constructor_free_order", constructor, "let free = free_contents_exec_v1(writer_capacity);",
         "let mut free = free_contents_exec_v1(writer_capacity);\n    if free.len() > 1 {\n        let last = free.len() - 1;\n        let first_value = free[0];\n        let last_value = free[last];\n        free[0] = last_value;\n        free[last] = first_value;\n    }"),
        ("register_pop_omitted", register, "let _ = free.pop();", "let _ = free.len();"),
        ("register_wrong_slot", register, "Ok(WriterReferenceV1 { slot: plan.slot, key })", "Ok(WriterReferenceV1 { slot: 0, key })"),
        ("register_wrong_content", register, "writers[plan.slot] = Some(WriterEntryV1::Reserved(key));", "writers[plan.slot] = Some(WriterEntryV1::Pending { key, head: None, count: 0 });"),
        ("register_wrong_count", register, "*reserved_count = plan.reserved_count;", "*reserved_count = 0;"),
        ("register_next_watermark", register, "*watermark = key.local;", "*watermark = key.local + 1;"),
        ("abort_clear_omitted", abort, "writers[reference.slot] = None;", "let _ = writers.len();"),
        ("abort_push_omitted", abort, "free.push(reference.slot);", "let _ = free.len();"),
        ("abort_count_omitted", abort, "*reserved_count = plan.reserved_count;", "let _ = plan.reserved_count;"),
        ("abort_watermark_reset", abort, "*reserved_count = plan.reserved_count;", "*reserved_count = plan.reserved_count;\n    *watermark = 0;"),
        ("abort_wrong_error", abort, "Err(error) => return Err(error),", "Err(_error) => return Err(JournalErrorV1::InvalidState),"),
        ("abort_rejection_mutates", abort, "Err(error) => return Err(error),", "Err(error) => { *watermark = 0; return Err(error); },"),
    ]
    result = [Mutation(name, function, before, after, relations[function])
              for name, function, before, after in entries]
    result.extend([
        Mutation("register_scratch_frame", "register_contents_exec_v1",
                 "    register_writer_exec_v1(", "    let _ = journal.scratch.pop();\n    register_writer_exec_v1(",
                 "issuance_contents_frame_v1"),
        Mutation("abort_capacity_frame", "abort_contents_exec_v1",
                 "    abort_reserved_exec_v1(", "    journal.allocation_capacity = 0;\n    abort_reserved_exec_v1(",
                 "issuance_contents_frame_v1"),
    ])
    require(len(result) == 21 and len({item.name for item in result}) == 21, "mutation inventory")
    return tuple(result)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def unique_json(text: str):
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON key")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError(f"non-JSON numeric constant: {value}")

    return json.loads(text, object_pairs_hook=object_pairs, parse_constant=invalid_constant)


def function_bounds(source: str, function: str) -> tuple[int, int, int]:
    # The canonical source is digest-pinned. These anchors deliberately reject
    # layout drift rather than trying to accept arbitrary Rust syntax.
    anchor = f"\npub fn {function}("
    require(source.count(anchor) == 1, "unique executable function")
    start = source.index(anchor) + 1
    body = source.index("\n{\n", start) + 1
    end = source.index("\n}\n", body) + 2
    require("\npub " not in source[start:end], "function boundary")
    return start, body, end


def mutate(source: str, mutation: Mutation) -> str:
    require(source.isascii(), "ASCII source offsets required")
    _, body, end = function_bounds(source, mutation.function)
    contents = source[body:end]
    require(mutation.before != mutation.after and mutation.before, "nonempty mutation")
    require(contents.count(mutation.before) == 1, "unique body replacement")
    changed = contents.replace(mutation.before, mutation.after)
    require(changed.count(mutation.after) == 1, "unique inverse replacement")
    require(changed.replace(mutation.after, mutation.before) == contents, "reversible mutation")
    result = source[:body] + changed + source[end:]
    _, changed_body, changed_end = function_bounds(result, mutation.function)
    require(changed_body == body and result[:body] == source[:body], "unchanged contracts")
    require(result[changed_end:] == source[end:], "unchanged remaining proof")
    return result


def postcondition_bounds(source: str, mutation: Mutation) -> tuple[int, int]:
    start, body, _ = function_bounds(source, mutation.function)
    contract = source[start:body]
    needle = mutation.postcondition + "("
    require(contract.count(needle) == 1, "unique target postcondition")
    begin = start + contract.index(needle)
    cursor = begin + len(needle)
    depth = 1
    while depth:
        require(cursor < body, "postcondition closing parenthesis")
        depth += (source[cursor] == "(") - (source[cursor] == ")")
        cursor += 1
    return begin, cursor


def check_span(span: dict, source: str, path: Path) -> None:
    require(span["file_name"] == str(path), "diagnostic source path")
    start, end = span["byte_start"], span["byte_end"]
    require(type(start) is int and type(end) is int and 0 <= start < end <= len(source), "diagnostic byte range")
    for offset, which in [(start, "start"), (end, "end")]:
        line = source.count("\n", 0, offset) + 1
        column = offset - source.rfind("\n", 0, offset)
        require(type(span[f"line_{which}"]) is int and type(span[f"column_{which}"]) is int
                and span[f"line_{which}"] == line and span[f"column_{which}"] == column, "diagnostic coordinates")
    lines = source.splitlines()[span["line_start"] - 1:span["line_end"]]
    require([row["text"] for row in span["text"]] == lines, "diagnostic source excerpt")
    require(span["expansion"] is None, "no macro diagnostic substitution")


def check_result(status: int, stdout: str, stderr: str, source: str,
                 path: Path, mutation: Mutation | None) -> None:
    require(status == (1 if mutation else 0), "normal expected exit required")
    report = unique_json(stdout)
    expected = {
        "encountered-error": mutation is not None,
        "encountered-vir-error": False,
        "success": mutation is None,
        "verified": mutation.verified if mutation else POSITIVE_COUNT,
        "errors": 1 if mutation else 0,
        "is-verifying-entire-crate": True,
    }
    actual = report["verification-results"]
    require(actual == expected and all(type(actual[key]) is type(value) for key, value in expected.items()),
            "exact whole-crate summary")
    diagnostics = [unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        require(not diagnostics, "positive diagnostic output")
        return
    require(len(diagnostics) in (1, 2), "exact diagnostic count")
    if len(diagnostics) == 2:
        footer = diagnostics[1]
        require(footer["$message_type"] == "diagnostic"
                and footer["message"] == "aborting due to 1 previous error"
                and footer["level"] == "error" and not footer["spans"]
                and not footer["children"] and footer["code"] is None, "exact error footer")
    diagnostic = diagnostics[0]
    require(diagnostic["$message_type"] == "diagnostic"
            and diagnostic["level"] == "error"
            and diagnostic["message"] == "postcondition not satisfied"
            and diagnostic["code"] is None and not diagnostic["children"], "intended postcondition error")
    spans = diagnostic["spans"]
    require(len(spans) == 2, "exact postcondition/exit spans")
    for span in spans:
        check_span(span, source, path)
    primary = [span for span in spans if span["is_primary"] is True]
    secondary = [span for span in spans if span["is_primary"] is False]
    require(len(primary) == len(secondary) == 1, "one primary and one exit")
    post, exit_span = primary[0], secondary[0]
    require((post["byte_start"], post["byte_end"]) == postcondition_bounds(source, mutation)
            and post["label"] == "failed this postcondition", "exact intended postcondition")
    _, body, end = function_bounds(source, mutation.function)
    require(body <= exit_span["byte_start"] < exit_span["byte_end"] <= end, "exit inside target body")
    require(exit_span["label"] in ("at this exit", "at the end of the function body"), "expected exit label")


class CampaignInterrupted(RuntimeError):
    pass


SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)


def interrupted(signum, _frame):
    # InterruptedError is retried by selectors and can silently lose cancellation.
    raise CampaignInterrupted(f"campaign received signal {signum}")


def group_exists(group: int) -> bool:
    try:
        os.killpg(group, 0)
        return True
    except ProcessLookupError:
        return False


def run_owned(command: list[str], timeout: int, out: Path,
              environment: dict[str, str]) -> tuple[int, str, str]:
    out.mkdir()
    record = {"command": command, "started_ns": time.time_ns()}
    previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, SIGNALS)
    process = None
    stdout = stderr = ""
    retained_group = False
    try:
        # This CLI is single-threaded. The child must not inherit the spawn mask.
        process = subprocess.Popen(command, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True, text=True,
                                   preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask))
        record["process_group"] = process.pid
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException as error:
        record["exception"] = type(error).__name__
        raise
    finally:
        signal.pthread_sigmask(signal.SIG_BLOCK, SIGNALS)
        try:
            if process is not None:
                retained_group = group_exists(process.pid)
                if retained_group:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                # Recover cached output even if interruption followed child reaping.
                stdout, stderr = process.communicate()
                for _ in range(20):
                    if not group_exists(process.pid):
                        break
                    time.sleep(0.05)
                record["group_absent"] = not group_exists(process.pid)
            record.update(finished_ns=time.time_ns(), status=None if process is None else process.returncode)
            (out / "record.json").write_text(json.dumps(record, indent=2) + "\n")
            (out / "stdout.log").write_text(stdout)
            (out / "stderr.log").write_text(stderr)
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
    require(not retained_group and record["group_absent"], "command left process-group members")
    return process.returncode, stdout, stderr


def run_solver(verus: Path, timeout: int, source: Path, original: bytes, out: Path,
               mutation: Mutation | None, environment: dict[str, str]) -> None:
    require(source.read_bytes() == original, "generated source changed before solver run")
    command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", str(timeout),
               str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
               "--output-json", "--error-format=json", "--num-threads", "4", str(source)]
    status, stdout, stderr = run_owned(command, timeout + 10, out / "solver", environment)
    require(source.read_bytes() == original, "source changed during solver run")
    check_result(status, stdout, stderr, original.decode("ascii"), source, mutation)


def check_pin(path: Path, pin: Path) -> None:
    expected = pin.read_text().strip()
    require(len(expected) == 64 and digest(path.read_bytes()) == expected, f"source pin: {path.name}")


def campaign(source: Path, verus: Path, timeout: int, output: Path) -> None:
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    output.mkdir()
    script_dir = Path(__file__).resolve().parent
    root = script_dir.parents[2]
    pins = script_dir / "pins"
    require(verus.name == "verus", "named Verus executable required")
    check_pin(verus, pins / "VERUS_SHA256")
    check_pin(source, pins / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")
    check_pin(Path(__file__), pins / "JOURNAL_ISSUANCE_CHECKER_SHA256")
    checker = root / "examples/wave64_collectives_v1/check-proof-source.py"
    closure = root / "examples/row_softmax_v1/verify-verus-closure.sh"
    manifest = pins / "VERUS_CLOSURE_MANIFEST"
    check_pin(checker, pins / "PROOF_SOURCE_CHECKER_SHA256")
    check_pin(manifest, pins / "VERUS_CLOSURE_MANIFEST_SHA256")
    require(digest(closure.read_bytes()) == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c", "closure checker pin")
    inputs = [source, Path(__file__), checker, closure, manifest,
              verus, verus.parent / "rust_verify", verus.parent / "z3"]
    inputs.extend(pins / name for name in ["CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
                  "JOURNAL_ISSUANCE_CHECKER_SHA256", "PROOF_SOURCE_CHECKER_SHA256",
                  "VERUS_CLOSURE_MANIFEST_SHA256", "VERUS_SHA256"])
    identities = {str(path): digest(path.read_bytes()) for path in inputs}
    (output / "inputs.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/nonexistent")}
    for name in ("RUSTUP_HOME", "CARGO_HOME"):
        environment[name] = os.environ.get(name, str(Path(environment["HOME"]) / (".rustup" if name == "RUSTUP_HOME" else ".cargo")))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    original = source.read_text()
    cases = [("positive_before", None), *[(item.name, item) for item in mutations()], ("positive_after", None)]
    for phase in ["before", "after"]:
        status, _, _ = run_owned(["/bin/sh", str(closure), str(verus.parent), str(manifest)],
                                 120, output / f"closure-{phase}", environment)
        require(status == 0, "authenticated Verus distribution")
        if phase == "after":
            break
        for name, mutation in cases:
            out = output / name
            out.mkdir()
            candidate = out / "journal.rs"
            expected = (mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            (out / "source.sha256").write_text(digest(expected) + "\n")
            status, _, _ = run_owned(["/usr/bin/python3", "-I", str(checker), str(candidate)],
                                     30, out / "source-audit", environment)
            require(status == 0, "forbidden-source audit")
            run_solver(verus, timeout, candidate, expected, out, mutation, environment)
            require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "source/tool replacement")
            print(f"PASS: journal issuance {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "final source/tool identities")
    print(f"JOURNAL_ISSUANCE_OK obligations={POSITIVE_COUNT} executable_mutations={len(mutations())}", flush=True)


def self_test(source: str) -> None:
    cases = mutations()
    for case in cases:
        changed = mutate(source, case)
        postcondition_bounds(changed, case)
    # Synthetic structured diagnostics exercise parser rejection without invoking a solver.
    case = cases[8]
    changed = mutate(source, case)
    path = Path("/owned/journal.rs")
    start, end = postcondition_bounds(changed, case)
    _, body, finish = function_bounds(changed, case.function)

    def span(begin, finish, primary, label):
        first = changed.count("\n", 0, begin) + 1
        last = changed.count("\n", 0, finish) + 1
        return {"file_name": str(path), "byte_start": begin, "byte_end": finish,
                "line_start": first, "line_end": last,
                "column_start": begin - changed.rfind("\n", 0, begin),
                "column_end": finish - changed.rfind("\n", 0, finish),
                "is_primary": primary, "label": label, "expansion": None,
                "text": [{"text": line} for line in changed.splitlines()[first - 1:last]]}

    report = {"verification-results": {"encountered-error": True, "encountered-vir-error": False,
              "success": False, "verified": case.verified, "errors": 1, "is-verifying-entire-crate": True}}
    diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
                  "code": None, "level": "error", "children": [],
                  "spans": [span(start, end, True, "failed this postcondition"),
                            span(body, finish, False, "at the end of the function body")]}
    stdout, stderr = json.dumps(report), json.dumps(diagnostic)
    check_result(1, stdout, stderr, changed, path, case)
    positive = {"verification-results": {"encountered-error": False, "encountered-vir-error": False,
                "success": True, "verified": POSITIVE_COUNT, "errors": 0, "is-verifying-entire-crate": True}}
    check_result(0, json.dumps(positive), "", source, path, None)
    footer = {"$message_type": "diagnostic", "message": "aborting due to 1 previous error",
              "code": None, "level": "error", "spans": [], "children": []}
    check_result(1, stdout, stderr + "\n" + json.dumps(footer), changed, path, case)
    rejected = 0

    def rejects(status=1, out=stdout, err=stderr):
        nonlocal rejected
        try:
            check_result(status, out, err, changed, path, case)
        except (ValueError, KeyError, TypeError):
            rejected += 1
        else:
            raise ValueError("parser accepted adverse diagnostic")

    for status in [0, 2, 101, 124, 137, -9, -15]:
        rejects(status=status)
    for field, value in [("verified", 67), ("verified", 68.0), ("errors", 2), ("errors", True), ("success", True),
                         ("success", 0),
                         ("encountered-vir-error", True), ("is-verifying-entire-crate", False)]:
        bad = copy.deepcopy(report)
        bad["verification-results"][field] = value
        rejects(out=json.dumps(bad))
    for field, value in [("message", "precondition not satisfied"), ("code", {"code": "E0308"}),
                         ("level", "warning"), ("children", [{"message": "solver timed out"}])]:
        bad = copy.deepcopy(diagnostic)
        bad[field] = value
        rejects(err=json.dumps(bad))
    for field, value in [("file_name", "/foreign/journal.rs"), ("byte_start", start + 1),
                         ("byte_end", end - 1), ("line_start", 1), ("column_start", 1),
                         ("label", "different postcondition"), ("text", []), ("expansion", {})]:
        bad = copy.deepcopy(diagnostic)
        bad["spans"][0][field] = value
        rejects(err=json.dumps(bad))
    rejects(out=stdout + stdout)
    rejects(out=stdout.replace('"errors": 1', '"errors": 1, "errors": 1'))
    rejects(err=stderr + "\n" + stderr)
    rejects(err=stderr + "\nsolver timed out")
    rejects(err="")
    rejects(err=stderr.replace('"spans":', '"spans": [], "spans":'))
    bad = copy.deepcopy(diagnostic)
    bad["spans"][1] = span(0, 2, False, "at this exit")
    rejects(err=json.dumps(bad))
    bad_footer = dict(footer, **{"$message_type": "artifact"})
    rejects(err=stderr + "\n" + json.dumps(bad_footer))
    try:
        check_result(0, stdout, "", source, path, None)
    except ValueError:
        rejected += 1
    else:
        raise ValueError("negative summary accepted as positive")
    print(f"PASS: journal mutation self-test ({len(cases)} sources, {rejected} rejected diagnostics)")


def process_self_test() -> None:
    child_code = """
import json, os, signal, subprocess, sys, time
from pathlib import Path
child = subprocess.Popen(['/usr/bin/sleep', '30'])
print('owned stdout', flush=True)
print('owned stderr', file=sys.stderr, flush=True)
Path(sys.argv[1]).write_text(json.dumps({'child': os.getpid(), 'grandchild': child.pid,
    'blocked': list(signal.pthread_sigmask(signal.SIG_BLOCK, []))}))
if sys.argv[2] == 'wait':
    time.sleep(0.05)
    os.kill(os.getppid(), signal.SIGTERM)
child.wait()
"""
    with tempfile.TemporaryDirectory(prefix="fe2o3-journal-process-") as temporary:
        root = Path(temporary)
        environment = {"PATH": "/usr/bin:/bin"}
        real_popen = subprocess.Popen
        for mode, expected in [("spawn", CampaignInterrupted), ("wait", CampaignInterrupted),
                               ("timeout", subprocess.TimeoutExpired)]:
            ready = root / f"{mode}.json"

            def spawn_then_interrupt(*args, **kwargs):
                process = real_popen(*args, **kwargs)
                deadline = time.monotonic() + 5
                while not ready.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                if not ready.exists():
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
                    raise ValueError("child readiness timed out")
                os.kill(os.getpid(), signal.SIGTERM)
                return process

            if mode == "spawn":
                subprocess.Popen = spawn_then_interrupt
            try:
                run_owned([sys.executable, "-u", "-c", child_code, str(ready), mode],
                          1 if mode == "timeout" else 10, root / mode, environment)
            except expected:
                pass
            else:
                raise ValueError(f"missing {mode} interruption")
            finally:
                subprocess.Popen = real_popen
            child = unique_json(ready.read_text())
            require(not set(child["blocked"]).intersection(SIGNALS), "child inherited blocked signals")
            record = unique_json((root / mode / "record.json").read_text())
            require(record["exception"] == expected.__name__ and record["status"] != 0,
                    "original interruption recorded")
            require(record["group_absent"] and not group_exists(record["process_group"]), "owned group cleanup")
            require((root / mode / "stdout.log").read_text() == "owned stdout\n"
                    and (root / mode / "stderr.log").read_text() == "owned stderr\n", "interrupted raw output")
        try:
            run_owned([str(root / "missing-executable")], 1, root / "spawn_failure", environment)
        except FileNotFoundError:
            pass
        else:
            raise ValueError("missing spawn failure")
        def spawn_then_interrupt_after_reaping(*args, **kwargs):
            process = real_popen(*args, **kwargs)
            communicate = process.communicate
            first = True

            def reap_then_signal(*args, **kwargs):
                nonlocal first
                result = communicate(*args, **kwargs)
                if first:
                    first = False
                    os.kill(os.getpid(), signal.SIGTERM)
                return result

            process.communicate = reap_then_signal
            return process

        subprocess.Popen = spawn_then_interrupt_after_reaping
        try:
            run_owned([sys.executable, "-c", "print('reaped output')"], 10, root / "reaped", environment)
        except CampaignInterrupted:
            pass
        else:
            raise ValueError("missing post-reap interruption")
        finally:
            subprocess.Popen = real_popen
        require((root / "reaped" / "stdout.log").read_text() == "reaped output\n", "post-reap raw output")
        status, stdout, stderr = run_owned([sys.executable, "-c", "print('complete')"],
                                           10, root / "normal", environment)
        require((status, stdout, stderr) == (0, "complete\n", ""), "normal command result")
    print("PASS: journal owned-process self-test (spawn/wait/post-reap interruption, timeout, spawn failure, success)")


def main() -> None:
    for signum in SIGNALS:
        signal.signal(signum, interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, SIGNALS)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("source", type=Path)
    parser.add_argument("verus", type=Path, nargs="?")
    parser.add_argument("timeout", type=int, nargs="?")
    parser.add_argument("output", type=Path, nargs="?")
    args = parser.parse_args()
    if args.self_test:
        require(args.verus is None and args.timeout is None and args.output is None, "self-test arguments")
        self_test(args.source.read_text())
        process_self_test()
    else:
        require(args.verus is not None and args.timeout is not None and args.output is not None, "campaign arguments")
        campaign(args.source, args.verus, args.timeout, args.output)


if __name__ == "__main__":
    main()
