#!/usr/bin/env python3
"""Source-generation calibration only; this does not execute logical mutants."""
import hashlib
import json
from pathlib import Path
import runpy

r = runpy.run_path(str(Path(__file__).with_name("check-producer-input-preflight.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_text() for path in r["FILES"]}
sources = {path: (root / path).read_text() for path in r["PINS"]}


def rejects(operation):
    try:
        operation()
    except ValueError:
        return
    raise ValueError("hostile producer-input preflight source accepted")


r["audit"](inputs)
r["source_gate"](sources, inputs[r["PROOF"]])
for path in inputs:
    for suffix in ("\nassume(false);", "\n#[verifier::external_body]", "\nmod foreign;",
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");', '\nenv!("FOREIGN");'):
        rejects(lambda: r["audit"]({**inputs, path: inputs[path] + suffix}))
    rejects(lambda: r["audit"]({p: value for p, value in inputs.items() if p != path}))
include = 'include!("../../fe2o3-runtime/src/context/versions/producer_input_preflight_body.rs");'
for replacement in ("/* " + include + " */", 'r##"' + include + '"##',
                    "#[cfg(any())]" + include, "fn unrelated() { " + include + " }", include + include):
    rejects(lambda: r["audit"]({**inputs, r["PROOF"]: inputs[r["PROOF"]].replace(include, replacement)}))
proof = inputs[r["PROOF"]]
for old, new in (
    ("requires vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),", ""),
    ("struct RuntimeSubmissionIdV1 { context_generation: u64, local: u64 }", "struct RuntimeSubmissionIdV1 { local: u64 }"),
    ("producer_input_preflight_body!(verus_exec_expr, self, id)", "unimplemented!()"),
    ("requires vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),",
     "requires vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(), false,"),
    ("use std::collections::HashMap;", "use crate::stand_in::HashMap;"),
    ("use std::collections::HashMap;", "#[cfg(any())]\nuse std::collections::HashMap;"),
):
    need(proof.count(old) == 1, "proof adapter attack site")
    rejects(lambda: r["audit"]({**inputs, r["PROOF"]: proof.replace(old, new)}))
for suffix in ("\nuse crate::stand_in::Other as HashMap;", "\ntype HashMap<K, V> = Selected<K, V>;",
               "\nuse crate::stand_in as std;"):
    rejects(lambda: r["audit"]({**inputs, r["PROOF"]: proof + suffix}))
for path in sources:
    rejects(lambda: r["source_gate"]({**sources, path: sources[path] + "\n"}, proof))
    rejects(lambda: r["source_gate"]({p: value for p, value in sources.items() if p != path}, proof))
for old, new in (("context_generation: u64, local: u64, kind: ContextWriterKindV1",
                  "context_generation: u64, local: u64"),
                 ("slot: usize, incarnation: u64, consumer: ContextWriterKeyV1",
                  "slot: usize, consumer: ContextWriterKeyV1"),
                 ("queued_references: Vec<ContextQueuedProducerReadReferenceV1>",
                  "queued_references: Vec<ContextProducerReadReferenceV1>"),
                 ("custody: C", "custody: u64")):
    need(old in proof, "schema mutation site")
    changed = proof.replace(old, new)
    rejects(lambda: r["source_gate"](sources, changed))
for old, new in (("enum ContextWriterKindV1 { Synchronous, Submission }", "enum ContextWriterKindV1 { Submission }"),
                 ("enum ProducerReadDomainV1 { DirectedPeer, Launch }", "enum ProducerReadDomainV1 { Launch }"),
                 ("Active(A), Queued(Q)", "Active(A), Queued(Q), Foreign"),
                 ("Generated { stream: RuntimeStreamIdV1, hold: u64, shell_key: u64 }", "Generated { hold: u64, shell_key: u64 }"),
                 ("struct RuntimeStreamIdV1 { context_generation: u64, local: u64 }", "struct RuntimeStreamIdV1 { local: u64 }")):
    need(proof.count(old) == 1, "complete enum/identity attack site")
    rejects(lambda: r["source_gate"](sources, proof.replace(old, new)))

cases = r["mutations"](inputs[r["BODY"]])
need(len(cases) == 22, "exact unexecuted logical mutation roster")
for changed, focus in cases.values():
    need(changed != inputs[r["BODY"]] and focus in {"*producer_input_root_v1", "*first_reference"}, "actual shared-body mutation")
    rejects(lambda: r["source_gate"]({**sources, r["BODY"]: changed}, proof))
rejects(lambda: r["mutations"](inputs[r["BODY"]] * 2))

campaign = r["campaign"]()
classifier = campaign.inherited()
leaf = classifier.inherited()
verifier = {"version": "calibration-only"}
path = "/snapshot/context_producer_input_preflight_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for focus in ("*producer_input_root_v1", "*first_reference"):
    notes = r["selection_notes"](leaf, focus)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(message) for message in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected logical diagnostic")
        need(not check([dict(note, message=message + " foreign"), error]), "unknown selection fails")
    for message in ("type annotations needed", "Resource limit (rlimit) exceeded", "unsupported feature", "internal error"):
        need(not check([dict(error, message=message)]), "tool failure is not a logical counterexample")
    need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign failure rejected")
print("PASS: producer-input preflight source calibration (4 groups; does not execute the 22 logical mutants)")
