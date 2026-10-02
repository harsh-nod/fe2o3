#!/usr/bin/env python3
"""Bounded shared cold-predicate proof, not native state or adapter refinement."""

import argparse
import hashlib
import importlib.util
from pathlib import Path
import re
import signal
import sys
import types


ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
K = Path("crates/fe2o3-kfd/src")
SUPPORT = V / "compute_xgmi_packet_plan_check.py"
SUPPORT_HASH = "61074f6cdbfde1df24bff3d1e56e04398f2850dd00518c233d847f13a29e0db4"
support_path = ROOT / SUPPORT
if (support_path.is_symlink() or not support_path.is_file()
        or hashlib.sha256(support_path.read_bytes()).hexdigest() != SUPPORT_HASH):
    raise ValueError("pinned packet-plan controller support")
spec = importlib.util.spec_from_file_location("cold_endpoint_support", support_path)
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
need, digest, save, strict_json = support.need, support.digest, support.save, support.strict_json
VERIFIER = support.VERIFIER
BODY = K / "queue_live/compute_xgmi_cold_body.rs"
RUST = K / "queue_live/compute_xgmi.rs"
OBSERVER = K / "queue_submit.rs"
ESTABLISHED = K / "queue_live.rs"
PROOF = V / "compute_xgmi_cold_endpoint_v1.rs"
TEST = V / "compute_xgmi_cold_endpoint_test.py"
FOCUS = "compute_xgmi_cold_endpoint_is_quiescent_v1"
MACRO = "compute_xgmi_cold_endpoint_body_v1"
FIELDS = (
    ("completion_releasable", "bool"), ("submission_pristine", "bool"),
    ("dispatch_attached", "bool"), ("unpublished_clear", "bool"),
    ("detached_data_count", "usize"), ("detached_generation_present", "bool"),
    ("detached_identity_count", "usize"), ("detached_insertion_present", "bool"),
    ("next_persistent_generation", "u64"),
)
SCOPE = ("Shared cold Boolean predicate and established-result preservation only. "
         "No proof of queue-observer authenticity, ownership, Linux/KFD state, model restoration, "
         "DMA, runtime, concurrency, hardware, whole-adapter refinement or performance.")


def facts_schema(source):
    matches = re.findall(r"struct ComputeXgmiColdEndpointFactsV1\s*\{([^}]+)\}", source)
    need(len(matches) == 1, "one facts schema")
    fields = re.sub(r"pub\(super\)\s*", "", matches[0])
    actual = re.findall(r"\s*([a-z_]+)\s*:\s*(bool|usize|u64)\s*,", fields)
    remainder = re.sub(r"\s*[a-z_]+\s*:\s*(bool|usize|u64)\s*,", "", fields)
    need(not remainder.strip() and tuple(actual) == FIELDS, "exact facts schema")


def validate_sources(sources):
    for path, expected in {SUPPORT: SUPPORT_HASH, **support.PINS}.items():
        need(digest(sources[str(path)]) == expected, "pinned support: " + str(path))
    rust, body, proof = (sources[str(path)].decode("ascii") for path in (RUST, BODY, PROOF))
    facts_schema(rust)
    facts_schema(proof)
    need(re.findall(r'include!\("([^"]+)"\);', rust) == [BODY.name], "actual Rust include")
    need(re.findall(r'include!\("([^"]+)"\);', proof)
         == ["../../fe2o3-kfd/src/queue_live/" + BODY.name], "actual proof include")
    need("include!" not in body, "closed predicate body")
    need(not re.search(r'\b(?:assume|admit)\s*\(|external_body|external_fn_specification|\baxiom\b',
                       body + proof), "no proof trust escape")
    compact = re.sub(r"\s+", "", rust)
    need(compact.count("pub(super)constfn" + FOCUS
                       + "(facts:ComputeXgmiColdEndpointFactsV1,)->bool{" + MACRO + "!(facts)}") == 1,
         "actual production body forwarding")
    start = compact.index("fnprimary_lane_is_quiescent_for_compute_xgmi_v1(&self)->bool{")
    end = compact.index("pub(super)fnrequire_no_xgmi_attachment_v1", start)
    primary = compact[start:end]
    need(primary.endswith("established||cold}"), "unchanged established OR cold result")
    need(primary.count("letestablished=auxiliary_compute_lane_quiescence_from_facts_v1(") == 1
         and primary.count("letcold=self.key==self.compute_lane_session&&" + FOCUS
                           + "(ComputeXgmiColdEndpointFactsV1{") == 1,
         "actual established and cold inputs")
    for projection in (
        "letcompletion_releasable=self.completion_owner.ensure_releasable().is_ok();",
        "ComputeXgmiColdEndpointFactsV1{completion_releasable,",
        "submission_pristine:self.submission.as_ref().is_some_and(NativeAqlSubmissionOwnerV1::is_pristine_v1),",
        "dispatch_attached:self.dispatch.is_some(),",
        "unpublished_clear:self.unpublished_dispatch.is_clear(),",
        "detached_data_count:self.detached_data_count,",
        "detached_generation_present:self.detached_dispatch_generation.is_some(),",
        "detached_identity_count:self.detached_data_identities.len(),",
        "detached_insertion_present:self.detached_next_insertion_index.is_some(),",
        "next_persistent_generation:self.next_persistent_compute_generation,",
    ):
        need(primary.count(projection) == 1, "actual cold-facts projection")
    need(compact.count("||!self.primary_lane_is_quiescent_for_compute_xgmi_v1()") == 1,
         "actual endpoint applies combined predicate")
    observer = re.sub(r"\s+", "", sources[str(OBSERVER)].decode("ascii"))
    need("pub(super)fnis_pristine_v1(&self)->bool{self.phase==SubmissionPhaseV1::Ready"
         "&&self.ring.write()==0&&self.ring.last_read()==0}" in observer,
         "actual pristine observation remains source-bound, not proved")
    proof_compact = re.sub(r"\s+", "", proof)
    need("letcold=primary_session&&" + FOCUS + "(facts);established||cold}" in proof_compact,
         "proof uses actual Boolean extension shape")


def source_snapshot():
    paths = [BODY, RUST, OBSERVER, ESTABLISHED, PROOF, TEST,
             Path(__file__).relative_to(ROOT), SUPPORT, *support.PINS]
    sources = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source file")
        sources[str(path)] = selected.read_bytes()
    validate_sources(sources)
    return sources


def mutations(body):
    cases = {}
    for name, before in (
        ("completion", "$facts.completion_releasable"),
        ("submission", "$facts.submission_pristine"),
        ("dispatch", "!$facts.dispatch_attached"),
        ("unpublished", "$facts.unpublished_clear"),
        ("data", "$facts.detached_data_count == 0"),
        ("generation", "!$facts.detached_generation_present"),
        ("identities", "$facts.detached_identity_count == 0"),
        ("insertion", "!$facts.detached_insertion_present"),
        ("persistent-generation", "$facts.next_persistent_generation == 1"),
    ):
        need(body.count(before) == 1, "unique mutation site " + name)
        cases["omit-" + name] = body.replace(before, "true")
    need(len(set(cases.values())) == 9, "nine distinct logical negatives")
    return cases


def classify(status, stdout, stderr, proof, negative=False):
    try:
        data = strict_json(stdout)
        diagnostics = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get("verus") == VERIFIER, "exact verifier identity")
        result = data["verification-results"]
        need(result.get("encountered-vir-error") is False, "no translation error")
        errors = [row for row in diagnostics if row.get("level") == "error"]
        need(all(row.get("level") in {"error", "note"} for row in diagnostics), "no hidden warnings")
        if not negative:
            return (status == 0 and result.get("encountered-error") is False
                    and result.get("errors") == 0 and result.get("verified") == 2
                    and result.get("success") is True
                    and result.get("is-verifying-entire-crate") is True and not diagnostics)
        need(status == 1 and result.get("encountered-error") is True
             and result.get("errors") == 1 and result.get("verified") == 0
             and result.get("is-verifying-entire-crate") is False, "one selected logical failure")
        logical = [row for row in errors if row.get("message") == "postcondition not satisfied"]
        abort = [row for row in errors if row.get("message") == "aborting due to 1 previous error"
                 and row.get("spans") == []]
        need(len(logical) == len(abort) == 1 and len(errors) == 2,
             "one postcondition failure and compiler summary, not tool error")
        notes = {
            "verifying root module (selected functions)",
            "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
        }
        need(all(row.get("message") in notes for row in diagnostics if row.get("level") == "note"),
             "exact selected-function notes")
        lines = proof.read_text().splitlines()
        first = next(i + 1 for i, line in enumerate(lines) if line.startswith("fn " + FOCUS + "("))
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(MACRO + "!("))
        body = (proof.parent / "../../fe2o3-kfd/src/queue_live/compute_xgmi_cold_body.rs").resolve()
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + MACRO + " {"))
        spans = logical[0].get("spans", [])
        contract = any(span.get("is_primary") is True
                       and Path(span.get("file_name", "")).resolve() == proof
                       and first < span.get("line_start", 0) < call for span in spans)
        expansion = any(
            Path(span.get("file_name", "")).resolve() == body
            and span.get("expansion", {}).get("macro_decl_name") == MACRO + "!"
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
    owner = types.ModuleType("cold_endpoint_process_owner")
    owner.__file__ = str(ROOT / support.OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(support.OWNER)], owner.__file__, "exec"), owner.__dict__)
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
    closure_command = ["/bin/sh", str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)]

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

    def proof_run(name, body, negative=False):
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
                   *(["--verify-function", "*" + FOCUS, "--verify-root"] if negative else []), str(source)]
        run(name, command, lambda status, stdout, stderr: classify(status, stdout, stderr, source, negative))

    error = None
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout
            and re.findall(r"^Ran (\d+) tests in [0-9.]+s$", stderr, re.M) == ["6"]
            and stderr.endswith("\nOK\n"))
        run("release-before", closure_command, closure)
        original = before[str(BODY)].decode("ascii")
        proof_run("positive-before", original)
        if not args.positive_only:
            for name, body in mutations(original).items():
                proof_run("negative-" + name, body, True)
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
    expected = 4 if args.positive_only else 14
    result = dict(accepted=error is None and len(rows) == expected and all(row["accepted"] for row in rows),
                  full_campaign=not args.positive_only, scope=SCOPE, stages=rows, error=error,
                  source_unchanged=source_snapshot() == before, whole_adapter_verified=False)
    save(out / "result.json", result)
    print(support.json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
