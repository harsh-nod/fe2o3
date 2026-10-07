#!/usr/bin/env python3
"""Source-bound additive planner-input root; source checks are not proof runs."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
V = BASE.relative_to(ROOT)
PROOF = V / "context_producer_journal_composition_v1.rs"
EXTENSION = V / "producer_planner_input_composition_v1.rs"
GUARD_SHA = 'dbd5d05bf94fe3e1929a18f8cf9d90869dc11bff9574c2de62be784e00e9b576'
COMPLETION = tuple(V / ("context_completion_reconciliation_" + name + "_v1.rs")
                   for name in ("graph", "validation", "effects"))
BINDINGS = (
    Path("crates/fe2o3-runtime/src/context/versions/producer_readers.rs"),
    Path("crates/fe2o3-runtime/src/context/versions.rs"),
    Path("crates/fe2o3-runtime/src/context/completion_reconciliation_body.rs"),
    Path("crates/fe2o3-runtime-model/src/context_queued_writers/forward.rs"),
)
INSERTION = '    include!("producer_planner_input_composition_v1.rs");\n'


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         "ordinary canonical source: " + str(path))
    return path.read_bytes()


def load_guard():
    path = BASE / "check-producer-journal-composition.py"
    need(sha(ordinary(path)) == GUARD_SHA, "exact existing composition source guard")
    spec = importlib.util.spec_from_file_location("planner_input_predecessor", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def augmented(text):
    guard = load_guard()
    need(sha(text.encode()) == guard.PROOF_SHA, "unchanged qualified concrete root")
    need(text.endswith("    }\n}\n") and INSERTION not in text,
         "one closed private concrete module and no pre-existing extension")
    return text[:-2] + INSERTION + "}\n"


def sources():
    guard = load_guard()
    closure = set(guard.files()) | set(COMPLETION) | {EXTENSION}
    need(len(set(guard.files())) == 47, "unchanged 47-input concrete closure")
    result = {path: ordinary(ROOT / path).decode() for path in closure | set(BINDINGS)}
    need(sha(result[PROOF].encode()) == guard.PROOF_SHA, "exact predecessor root")
    guard.semantics(result)
    guard.live_binding(result)
    need(not re.search(r"\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external",
                       result[EXTENSION]), "no new assumptions or opaque executable conversions")
    # These adapters are source-bound, not part of the executable finite model.
    producer = result[BINDINGS[0]]
    need(producer.count("let result = self.validate_producer_read_v1(id);\n"
                        "        self.journal_result_v1(result)") == 1,
         "runtime directed status uses its actual producer result")
    need(producer.count("        .reconcile()\n        .map(Some)") == 1,
         "present runtime producer root wraps actual reconciliation")
    need(result[BINDINGS[1]].count("result.map_err(|_| {\n"
         "            self.quarantine_submission_writers_v1();\n"
         "            RuntimeValidationErrorV1::InvalidBackendDescription\n"
         "        })") == 1, "unchanged runtime journal error/quarantine adapter")
    need("match $context.directed_input_status_v1($id)?" in result[BINDINGS[2]],
         "actual shared planner consumes directed input status")
    return result, closure


def manifest(values, closure):
    return {"schema": "fe2o3-producer-planner-input-v1", "entire_concrete_root": True,
            "projection_only": True, "whole_context_refinement": False,
            "sources": {str(path): sha(text.encode()) for path, text in sorted(values.items())},
            "proof_closure": sorted(str(path) for path in closure),
            "source_only_bindings": [str(path) for path in BINDINGS],
            "generated_root_sha256": sha(augmented(values[PROOF]).encode())}


def stage(destination, values, closure):
    need(destination.is_absolute() and destination.resolve() == destination
         and not destination.exists() and destination.parent.is_dir(), "fresh owned stage")
    destination.mkdir()
    for path in sorted(closure):
        output = destination / path
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(augmented(values[path]) if path == PROOF else values[path])
    return destination / PROOF


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    values, closure = sources()
    current = manifest(values, closure)
    if args.manifest:
        need(json.loads(ordinary(args.manifest)) == current, "exact frozen source manifest")
    if args.stage:
        stage(args.stage, values, closure)
    print(json.dumps(current, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
