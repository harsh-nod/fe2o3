#!/usr/bin/env python3
"""Bounded shared-body arithmetic proofs, not native DMA or adapter refinement."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sys
import types


ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
K = Path("crates/fe2o3-kfd/src")
BODY = K / "sdma/compute_xgmi_plan_body.rs"
RUST = K / "sdma/compute_xgmi_plan.rs"
PROOF = V / "compute_xgmi_packet_plan_v1.rs"
TEST = V / "compute_xgmi_packet_plan_test.py"
CONSTANTS = {
    K / "sdma.rs": "pub const GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1: u32 = 0x003f_ffe0;",
    Path("crates/fe2o3-runtime-model/src/r74_ordered_peer_copy.rs"):
        "pub const MAX_ORDERED_PEER_COPY_SEGMENTS_V1: usize = 4096;",
}
OWNER = V / "check-journal-issuance.py"
CLOSURE = Path("examples/row_softmax_v1/verify-verus-closure.sh")
MANIFEST = V / "pins/VERUS_CLOSURE_MANIFEST"
PINS = {
    OWNER: "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480",
    CLOSURE: "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
    MANIFEST: "f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01",
}
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f",
            "platform": {"os": "linux", "arch": "x86_64"},
            "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
            "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}
SCOPE = "Shared count/subrange arithmetic and contiguous full coverage only; no DMA, mapping, currentness, ownership, runtime, concurrency, hardware or performance refinement."


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    with path.open("x") as output:
        output.write(json.dumps(value, sort_keys=True, indent=2) + "\n")


def strict_json(text):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    def invalid(value):
        raise ValueError("nonfinite JSON: " + value)
    return json.loads(text, object_pairs_hook=pairs, parse_constant=invalid)


def source_snapshot():
    paths = [BODY, RUST, PROOF, TEST, Path(__file__).relative_to(ROOT), *PINS, *CONSTANTS]
    result = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source file")
        result[str(path)] = selected.read_bytes()
    for path, expected in PINS.items():
        need(digest(result[str(path)]) == expected, "pinned support: " + str(path))
    rust = result[str(RUST)].decode("ascii")
    body = result[str(BODY)].decode("ascii")
    proof = result[str(PROOF)].decode("ascii")
    need(re.findall(r'include!\("([^"]+)"\);', rust) == [BODY.name], "actual Rust include")
    need(re.findall(r'include!\("([^"]+)"\);', proof)
         == ["../../fe2o3-kfd/src/sdma/" + BODY.name], "actual proof include")
    need(not re.search(r'\b(?:assume|admit)\s*\(|external_body|external_fn_specification|\baxiom\b', body + proof),
         "no proof trust escape")
    need("include!" not in body, "closed arithmetic body")
    compact = re.sub(r"\s+", "", rust.split("#[cfg(test)]", 1)[0])
    for call in (
        "compute_xgmi_packet_count_body_v1!(total_bytes,GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1asu64,MAX_ORDERED_PEER_COPY_SEGMENTS_V1asu64)?",
        "compute_xgmi_packet_at_body_v1!(total,count,index,GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1asu64)",
    ):
        need(compact.count(call) == 1, "actual production body forwarding")
    need("pubstructGfx942ComputeXgmiPacketPlanV1{total_bytes:u64,count:usize,}" in compact,
         "immutable private plan state")
    for path, declaration in CONSTANTS.items():
        need(result[str(path)].decode("ascii").count(declaration) == 1, "existing constant identity")
    need("const PACKET_BYTES: u64 = 0x003f_ffe0;" in proof
         and "const MAX_PACKETS: u64 = 4096;" in proof, "matching proof constants")
    return result


def mutations(body):
    cases = {}
    for name, before, after, family in (
        ("deny-maximum", "$total > packet_bytes * max_packets", "$total >= packet_bytes * max_packets", "count"),
        ("floor-count", "(($total - 1) / packet_bytes + 1)", "($total / packet_bytes)", "count"),
        ("fixed-count", "(($total - 1) / packet_bytes + 1) as usize", "1_usize", "count"),
        ("deny-first", "$index >= $count", "$index == 0 || $index >= $count", "at"),
        ("repeat-first-offset", "$index as u64 * packet_bytes", "0_u64", "at"),
        ("overlap-offset", "$index as u64 * packet_bytes", "($index as u64 * packet_bytes) / 2", "at"),
        ("empty-tail", "                remaining\n", "                0_u64\n", "at"),
        ("oversized-tail", "                remaining\n", "                packet_bytes\n", "at"),
    ):
        need(body.count(before) == 1, "unique mutation site " + name)
        cases[name] = (body.replace(before, after), "compute_xgmi_packet_" + family + "_v1")
    need(len(set(cases.values())) == 8, "eight distinct logical negatives")
    return cases


def classify(status, stdout, stderr, proof, focus=None):
    try:
        data = strict_json(stdout)
        diagnostics = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get("verus") == VERIFIER, "exact verifier identity")
        result = data["verification-results"]
        need(result.get("encountered-vir-error") is False, "no translation error")
        errors = [row for row in diagnostics if row.get("level") == "error"]
        need(all(row.get("level") in {"error", "note"} for row in diagnostics), "no hidden warnings")
        if focus is None:
            return (status == 0 and result.get("encountered-error") is False
                    and result.get("errors") == 0 and result.get("verified") == 5
                    and result.get("success") is True
                    and result.get("is-verifying-entire-crate") is True and not diagnostics)
        need(status == 1 and result.get("encountered-error") is True
             and result.get("errors") == 1 and result.get("verified") == 0
             and result.get("is-verifying-entire-crate") is False, "one selected logical failure")
        logical = [row for row in errors if row.get("message") == "postcondition not satisfied"]
        abort = [row for row in errors if row.get("message") == "aborting due to 1 previous error"
                 and row.get("spans") == []]
        need(len(logical) == len(abort) == 1 and len(errors) == 2,
             "one postcondition failure and compiler summary, not parse/overflow/tool error")
        allowed_notes = {
            "verifying root module (selected functions)",
            "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
        }
        need(all(row.get("message") in allowed_notes for row in diagnostics if row.get("level") == "note"),
             "exact selected-function notes")
        lines = proof.read_text().splitlines()
        first = next(index + 1 for index, line in enumerate(lines) if line.startswith("fn " + focus + "("))
        last = next((index + 1 for index, line in enumerate(lines[first:], first)
                     if line.startswith("fn ")), len(lines) + 1)
        macro = focus.removesuffix("_v1") + "_body_v1"
        body = (proof.parent / "../../fe2o3-kfd/src/sdma/compute_xgmi_plan_body.rs").resolve()
        definition = next(index + 1 for index, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + macro + " {"))
        call = next(index + 1 for index, line in enumerate(lines) if line.strip().startswith(macro + "!("))
        spans = logical[0].get("spans", [])
        contract = any(span.get("is_primary") is True
                       and Path(span.get("file_name", "")).resolve() == proof
                       and first < span.get("line_start", 0) < min(last, call)
                       for span in spans)
        expansion = any(
            Path(span.get("file_name", "")).resolve() == body
            and span.get("expansion", {}).get("macro_decl_name") == macro + "!"
            and Path(span["expansion"]["span"]["file_name"]).resolve() == proof
            and span["expansion"]["span"]["line_start"] == call
            and Path(span["expansion"]["def_site_span"]["file_name"]).resolve() == body
            and span["expansion"]["def_site_span"]["line_start"] == definition
            for span in spans if isinstance(span.get("expansion"), dict))
        return contract and expansion
    except (ValueError, KeyError, TypeError, StopIteration, OSError):
        return False


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--positive-only", action="store_true", help="development only, not full acceptance")
    args = parser.parse_args()
    out, verus = args.output, args.verus
    need(out.is_absolute() and out.resolve() == out and not out.exists()
         and not out.is_relative_to(ROOT), "fresh output outside repository")
    need(verus.is_absolute() and verus.resolve() == verus and verus.is_file(), "canonical verifier")
    before = source_snapshot()
    owner = types.ModuleType("packet_plan_process_owner")
    owner.__file__ = str(ROOT / OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(OWNER)], owner.__file__, "exec"), owner.__dict__)
    out.mkdir(parents=True)
    (out / "tmp").mkdir()
    for path, data in before.items():
        target = out / "inputs" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    save(out / "source-before.json", {path: digest(data) for path, data in before.items()})
    env = {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin:/home/harsh/.cargo/bin",
           "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "RUSTUP_HOME": "/home/harsh/.rustup",
           "VERUS_Z3_PATH": str(verus.parent / "z3"), "TMPDIR": str(out / "tmp")}
    save(out / "environment.json", env)
    rows = []
    handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    for number in handlers:
        signal.signal(number, owner.interrupted)
    closure_command = ["/bin/sh", str(ROOT / CLOSURE), str(verus.parent), str(ROOT / MANIFEST)]
    def run(name, command, accept):
        need(source_snapshot() == before, "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / "record.json").read_text())
        accepted = bool(receipt.get("group_absent") is True and accept(status, stdout, stderr))
        rows.append({"name": name, "status": status, "accepted": accepted})
        print(name + ": " + ("PASS" if accepted else "FAIL"), flush=True)
        need(source_snapshot() == before, "source continuity after " + name)
        need(accepted, "rejected stage " + name)
    def closure(status, stdout, stderr):
        return status == 0 and stdout == "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n" and not stderr
    def proof_run(name, body, focus=None):
        staged = out / (name + "-source")
        for path in (BODY, PROOF):
            target = staged / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body.encode("ascii") if path == BODY else before[str(path)])
        source = staged / PROOF
        command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120",
                   str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                   "--output-json", "--error-format=json", "--no-report-long-running",
                   "--num-threads", "1", "--multiple-errors", "1",
                   *([] if focus is None else ["--verify-function", "*" + focus, "--verify-root"]), str(source)]
        run(name, command, lambda status, stdout, stderr: classify(status, stdout, stderr, source, focus))
    error = None
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout
            and re.findall(r"^Ran (\d+) tests in [0-9.]+s$", stderr, re.M) == ["5"]
            and stderr.endswith("\nOK\n"))
        run("release-before", closure_command, closure)
        original = before[str(BODY)].decode("ascii")
        proof_run("positive-before", original)
        if not args.positive_only:
            for name, (body, focus) in mutations(original).items():
                proof_run("negative-" + name, body, focus)
            proof_run("positive-after", original)
    except BaseException as failure:
        error = type(failure).__name__ + ": " + str(failure)
    finally:
        try:
            run("release-after", closure_command, closure)
        except BaseException as failure:
            error = (error or "") + "; closing: " + type(failure).__name__ + ": " + str(failure)
        for number, handler in handlers.items():
            signal.signal(number, handler)
    expected = 4 if args.positive_only else 13
    result = dict(accepted=error is None and len(rows) == expected and all(row["accepted"] for row in rows),
                  full_campaign=not args.positive_only, scope=SCOPE, stages=rows, error=error,
                  source_unchanged=source_snapshot() == before, whole_adapter_verified=False)
    save(out / "result.json", result)
    print(json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
