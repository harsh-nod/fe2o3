#!/usr/bin/env python3
"""Run the full pinned producer/live composition campaign, never selected proofs."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
V = BASE.relative_to(ROOT)
PINS = BASE / "pins"
TIMEOUT = 120
COUNTS = {"leaf": 42, "conditional": 64, "concrete": 218}
CASE_COUNTS = {"leaf": 38, "conditional": 21, "concrete": 49}
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f",
            "platform": {"os": "linux", "arch": "x86_64"},
            "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
            "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}
CAMPAIGN_FILES = (
    "check-queued-query.py", "check-producer-input-fold.py", "check-producer-input-validate.py",
    "check-producer-input-composition.py", "check-producer-journal-observers.py",
    "producer-input-diagnostics-v1.py", "producer-journal-composition-diagnostics-v1.py",
    "producer-input-source-extraction-v2.py", "producer-live-source-extraction-v1.py",
    "producer-live-predecessor-v1.json", "producer-journal-composition-diagnostic-fixtures-v1.json",
    "producer-live-diagnostic-fixtures-v1.json",
    "test-producer-input-validate.py", "test-producer-input-composition.py",
    "test-producer-journal-composition.py", "test-producer-journal-composition-mutations.py",
    "test-producer-journal-composition-diagnostics.py", "test-producer-live-composition.py",
    "test-producer-live-composition-diagnostics.py",
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         "ordinary canonical input: " + str(path))
    return path.read_bytes()


def pinned(path, name):
    expected = ordinary(PINS / name).decode().strip()
    need(len(expected) == 64 and digest(ordinary(path)) == expected,
         "exact pinned input: " + str(path))


def load(name):
    path = BASE / name
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def resource_gate(output):
    memory = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
    available = int(memory["MemAvailable"].split()[0]) * 1024
    need(available >= 16 * 1024**3, "unchanged 16 GiB available-memory admission gate")
    used = sum(path.stat().st_size for path in output.rglob("*") if path.is_file())
    need(used < 1024**3, "campaign scratch remains below 1 GiB")
    return {"available_memory_bytes": available, "scratch_bytes": used}


def reporting_options(case):
    # Historical controls were captured with one-error enumeration enabled.
    if case is not None and case["boundary_label"] != "actual-live-validation-result-equality":
        return ["--multiple-errors", "1"]
    return []


def full_positive(diagnostics, parser, status, stdout, stderr, family):
    need(type(status) is int and status == 0 and stderr == "" and stdout.endswith("\n"),
         "normal complete positive without diagnostics")
    result = diagnostics.parse(parser, stdout)
    need(type(result) is dict and set(result) == {"verus", "verification-results", "func-details"}
         and result["verus"] == VERIFIER, "exact positive report and pinned tool identity")
    expected = {"encountered-error": False, "encountered-vir-error": False, "success": True,
                "verified": COUNTS[family], "errors": 0, "is-verifying-entire-crate": True}
    actual = result["verification-results"]
    need(type(actual) is dict and set(actual) == set(expected)
         and all(type(actual[k]) is type(v) and actual[k] == v for k, v in expected.items()),
         "exact unfiltered positive root count")
    need(type(result["func-details"]) is dict and 0 < len(result["func-details"]) <= 1024
         and all(type(name) is str and value == {"obligation_proof_notes": [], "failed_proof_notes": []}
                 for name, value in result["func-details"].items()), "complete finite positive function schema")
    return expected


def campaign(verus, output):
    need(output.resolve() == output and output.parent.is_dir(), "canonical fresh output under an existing owner")
    output.mkdir()
    need(verus.resolve() == verus and verus.name == "verus", "canonical pinned verifier")
    bindings = {
        Path(__file__).resolve(): "PRODUCER_LIVE_COMPOSITION_RUNNER_SHA256",
        BASE / "producer-live-validation-mutations-v1.py": "PRODUCER_LIVE_VALIDATION_MUTATIONS_SHA256",
        BASE / "producer-live-composition-diagnostics-v1.py": "PRODUCER_LIVE_COMPOSITION_DIAGNOSTICS_SHA256",
        BASE / "producer-journal-composition-mutations-v1.py": "PRODUCER_LIVE_INHERITED_MUTATIONS_SHA256",
        BASE / "check-producer-journal-composition.py": "PRODUCER_LIVE_COMPOSITION_GUARD_SHA256",
        BASE / "check-journal-issuance.py": "JOURNAL_ISSUANCE_CHECKER_SHA256",
        PINS / "VERUS_CLOSURE_MANIFEST": "VERUS_CLOSURE_MANIFEST_SHA256",
        PINS / "PRODUCER_LIVE_CAMPAIGN_INPUTS.json": "PRODUCER_LIVE_CAMPAIGN_INPUTS_SHA256",
        verus: "VERUS_SHA256",
    }
    for path, name in bindings.items():
        pinned(path, name)
    ownership = load("check-journal-issuance.py")
    guard = load("check-producer-journal-composition.py")
    inherited = load("producer-journal-composition-mutations-v1.py")
    live = load("producer-live-validation-mutations-v1.py")
    diagnostics = load("producer-live-composition-diagnostics-v1.py")
    base_diagnostics = diagnostics.load_base()
    parser_path = BASE / "producer-input-diagnostics-v1.py"
    parser = base_diagnostics.parser(parser_path)
    support = base_diagnostics.parse(parser, ordinary(PINS / "PRODUCER_LIVE_CAMPAIGN_INPUTS.json").decode())
    need(type(support) is dict and set(support) == set(CAMPAIGN_FILES), "exact complete campaign support roster")
    for name, value in support.items():
        need(type(value) is str and len(value) == 64 and digest(ordinary(BASE / name)) == value,
             "exact campaign support input: " + name)
    cases, sources = inherited.checked()
    cases.update({"concrete/" + key: value for key, value in live.construct(sources, guard.files()).items()})
    need(len(cases) == 108 and {family: sum(row["family"] == family for row in cases.values())
                              for family in CASE_COUNTS} == CASE_COUNTS, "exact complete 108-control campaign")
    inventory = inherited.inventory(cases)
    encoded = inherited.canonical(inventory).encode()
    need(digest(encoded) == ordinary(PINS / "PRODUCER_LIVE_COMBINED_ROSTER_SHA256").decode().strip(),
         "immutable combined mutation roster")
    closure_script = ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"
    need(digest(ordinary(closure_script)) == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
         "unchanged complete Verus closure checker")
    identities = {str(path): digest(ordinary(path)) for path in
                  {*bindings, *(PINS / name for name in bindings.values()),
                   PINS / "PRODUCER_LIVE_COMBINED_ROSTER_SHA256", closure_script, parser_path,
                   verus.parent / "rust_verify", verus.parent / "z3", *(BASE / name for name in CAMPAIGN_FILES),
                   Path(sys.executable).resolve()}}
    source_hashes = {str(path): digest(text.encode()) for path, text in sources.items()}
    write_json(output / "inputs.json", {"tools_and_checks": identities, "sources": source_hashes})
    write_json(output / "roster.json", inventory)
    native_snapshot = output / "bound-source"
    for path, text in sources.items():
        retained = native_snapshot / path
        retained.parent.mkdir(parents=True, exist_ok=True)
        retained.write_text(text)
    environment = {"HOME": os.environ.get("HOME", "/nonexistent"), "TMPDIR": str(output),
                   "VERUS_Z3_PATH": str(verus.parent / "z3")}
    environment["CARGO_HOME"] = os.environ.get("CARGO_HOME", environment["HOME"] + "/.cargo")
    environment["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", environment["HOME"] + "/.rustup")
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    closure_command = ["sh", str(closure_script), str(verus.parent), str(PINS / "VERUS_CLOSURE_MANIFEST")]
    records = []
    result = {"complete": False, "qualified_scope": "source-bound producer journal/live validator/fold composition",
              "credit_lock_refinement": False, "whole_context_refinement": False,
              "hardware_execution": False, "cases": records}

    def unchanged():
        need(all(digest(ordinary(Path(p))) == value for p, value in identities.items()),
             "all tools/checks unchanged during campaign")
        current = guard.snapshot()
        guard.audit(current)
        need(current == sources, "complete native/model/proof inventory unchanged")

    def proof_stage(label, family, root, closure, case=None):
        unchanged()
        admitted = resource_gate(output)
        stage = output / label
        stage.mkdir()
        staged = stage / "source"
        selected = {path: sources[path] for path in closure}
        if case is not None:
            selected[case["path"]] = case["text"]
        for path, text in selected.items():
            destination = staged / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(text)
        hashes = {str(p): digest(t.encode()) for p, t in selected.items()}
        write_json(stage / "inputs.json", {"source_sha256": hashes, "resources": admitted})
        command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", str(TIMEOUT),
                   str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                   "--output-json", "--error-format=json", "--num-threads", "4"]
        command += reporting_options(case) + [str(staged / root)]
        status, stdout, stderr = ownership.run_owned(command, TIMEOUT + 10, stage / "solver", environment)
        need(all(digest(ordinary(staged / Path(p))) == h for p, h in hashes.items()), "staged source unchanged")
        observed = (full_positive(base_diagnostics, parser, status, stdout, stderr, family)
                    if case is None else diagnostics.negative(parser_path, VERIFIER, selected, set(selected),
                                                             staged, case, status, stdout, stderr))
        unchanged()
        write_json(stage / "classification.json", observed)
        records.append({"stage": label, "family": family, "positive": case is None,
                        "result": observed, "source_unchanged": True})
        print(label, "PASS", flush=True)
        write_json(output / "result.json", result)

    leaf = guard.helper("check-producer-input-validate.py")
    conditional = guard.helper("check-producer-input-composition.py")
    roots = {"leaf": (leaf.PROOF, leaf.FILES), "conditional": (conditional.PROOF, conditional.FILES),
             "concrete": (guard.PROOF, guard.files())}
    try:
        status, _, _ = ownership.run_owned(closure_command, 120, output / "closure-before", environment)
        need(status == 0, "opening complete verifier closure")
        for name in (
            "test-producer-input-validate.py", "test-producer-input-composition.py",
            "test-producer-journal-composition.py", "test-producer-journal-composition-mutations.py",
            "test-producer-journal-composition-diagnostics.py", "test-producer-live-composition.py",
            "test-producer-live-composition-diagnostics.py",
        ):
            resource_gate(output)
            status, _, _ = ownership.run_owned([sys.executable, "-I", "-B", str(BASE / name)],
                                               120, output / ("controls-" + name.removesuffix(".py")), environment)
            need(status == 0, "source/diagnostic controls: " + name)
        for family, (root, closure) in roots.items():
            proof_stage("positive-before-" + family, family, root, closure)
        for index, (key, case) in enumerate(cases.items()):
            proof_stage(f"negative-{index:03d}-" + key.replace("/", "-"), case["family"],
                        case["root"], case["closure"], case)
        for family, (root, closure) in roots.items():
            proof_stage("positive-after-" + family, family, root, closure)
        status, _, _ = ownership.run_owned(closure_command, 120, output / "closure-after", environment)
        need(status == 0, "closing complete verifier closure")
        unchanged()
        need(len(records) == 114, "all six positive brackets and 108 negatives completed")
        result.update(complete=True, positive_brackets=6, logical_controls=108, finished_ns=time.time_ns())
    except BaseException as error:
        result.update(exception=type(error).__name__, message=str(error), finished_ns=time.time_ns())
        raise
    finally:
        write_json(output / "result.json", result)


if __name__ == "__main__":
    arguments = argparse.ArgumentParser(description=__doc__)
    arguments.add_argument("--verus", type=Path, required=True)
    arguments.add_argument("--output", type=Path, required=True)
    args = arguments.parse_args()
    campaign(args.verus.absolute(), args.output.absolute())
