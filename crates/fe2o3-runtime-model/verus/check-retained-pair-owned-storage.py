#!/usr/bin/env python3
"""Conditional normal-return refinement of shared owned-storage declarations/bodies.

The proof starts after run_operation returns. It does not establish callback,
automatic Drop/unwind, native currentness, allocation, compiler or ISA behavior.
Generic quarantine has only a normally-returning uninterpreted relation; terminal
is a fresh unconstrained query. The requires-false empty adapter models an
unreachable occupied-owner path, not process abort. Pinned vstd Option and borrow
specifications are trusted. This root must not be advertised as --no-cheating.
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
OWNER = SRC / "sdma/retained_pair/owned.rs"
DECLARATIONS = SRC / "sdma/retained_pair/owned/declarations.rs"
BODY = SRC / "sdma/retained_pair/owned/bodies.rs"
PROOF = V / "retained_pair_owned_storage_v1.rs"
FILES = [PROOF, DECLARATIONS, BODY]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "eb7c6a72e8bd3ec02f1811b8919f5488416e73254bb9d3d3c561d9858f79fe58"
PROOF_SHA = "bc6aa63e6cc401ea68d2ef8bcadb127922ec4f64ba6ffdc48bab25b58830856a"
# Measured by the root's full, unfiltered owned-storage proof discovery.
EXPECTED_VERIFIED = 8
MUTANT_COUNT = 21


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
             "ordinary exact owned-storage source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF}, "complete KFD Rust roster plus owned proof")
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "reviewed complete KFD Rust roster and bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact occupied-state contracts, opaque payloads and adapter trust")
    for name in ("declarations", "bodies"):
        included = 'include!("../../fe2o3-kfd/src/sdma/retained_pair/owned/' + name + '.rs");'
        need(sources[PROOF].count(included) == 1, "actual shared " + name + " proof include")
        need(sources[OWNER].count('include!("owned/' + name + '.rs");') == 1,
             "actual production " + name + " include")
    need(len(re.findall(r"\binclude!\(", sources[PROOF])) == 2
         and not re.search(r"\binclude!\(", sources[BODY] + sources[DECLARATIONS]),
         "exact three-file executable proof closure")
    need(sources[PROOF].count("use vstd::prelude::verus as retained_pair_owned_declarations_v1;") == 1,
         "actual declarations use the verifier macro, not a duplicated schema")
    need(sources[PROOF].count("#[verifier::external_body]") == 1
         and sources[PROOF].count("fn empty_owned_context<T>() -> T\n    requires false,") == 1,
         "only impossible-empty external adapter, never assumed callable")
    need(sources[PROOF].count("uninterp spec fn returning_owned_quarantine_effect") == 1
         and sources[PROOF].count("ensures returning_owned_quarantine_effect(*old(self), *final(self));") == 1
         and sources[PROOF].count("fn terminal(&self) -> bool;") == 1,
         "conditional quarantine and unconstrained fresh terminal query")
    need(not re.search(r"\b(?:assume|admit)\s*\(", sources[PROOF]), "no added assumed ownership facts")


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
        rows.append((name, "retained_pair_owned_" + macro + "_body", "*" + focus, before, after))

    add("parts-impossible-empty", "parts", "Parts::into_parts",
        "($parts.queue, $parts.source, $parts.destination)",
        "{ let _discarded = $parts; empty_owned_context() }")
    add("new-empty-holder", "new", "Owned::new", "context: Some($context),",
        "context: { let _discarded = $context; None },")
    for name, focus in (("context", "Owned::context"), ("context_mut", "Owned::context_mut"),
                        ("take", "Owned::take")):
        add(name.replace("_", "-") + "-impossible-empty", name, focus,
            "Some(context) => context,", "Some(_context) => empty_owned_context(),")
    add("take-discards-first-owner", "take", "Owned::take", "match $owner.context.take()",
        "let _discarded = $owner.context.take();\n        match $owner.context.take()")

    admit = "admit_post"
    focus = "Owned::admit_post"
    add("admit-refusal-not-entry", admit, focus, "entry_refusal: true,", "entry_refusal: false,")
    add("admit-refusal-promoted", admit, focus,
        "Err(error) => Err(Failure {\n                owner: $owner,\n                error,\n                entry_refusal: true,\n            }),",
        "Err(error) => { let _discarded = error; Ok($owner) },")
    add("admit-refusal-drops-holder", admit, focus, "owner: $owner,",
        "owner: { let _discarded = $owner; Owned { context: None } },")

    finish = "finish_post"
    focus = "Owned::finish_post"
    add("finish-refusal-is-entry", finish, focus, "entry_refusal: false,", "entry_refusal: true,")
    add("finish-refusal-no-quarantine", finish, focus, "$owner.context_mut().quarantine();",
        "let _ = $owner.context();")
    # The relation need not compose across two effects; this is not a call-count theorem.
    add("finish-refusal-double-quarantine", finish, focus, "$owner.context_mut().quarantine();",
        "$owner.context_mut().quarantine();\n                $owner.context_mut().quarantine();")
    add("finish-refusal-promoted", finish, focus,
        "Err(error) => {\n                $owner.context_mut().quarantine();\n                Err(Failure {\n                    owner: $owner,\n                    error,\n                    entry_refusal: false,\n                })\n            }",
        "Err(error) => { let _discarded = error; Ok($owner.take()) }")
    add("finish-refusal-drops-holder", finish, focus, "owner: $owner,",
        "owner: { let _discarded = $owner; Owned { context: None } },")
    add("finish-success-quarantines", finish, focus, "Ok(()) => Ok($owner.take()),",
        "Ok(()) => { $owner.context_mut().quarantine(); Ok($owner.take()) },")

    recover = "recover"
    focus = "Failure::recover_unadmitted"
    add("recover-inverts-entry-guard", recover, focus, "!$failure.entry_refusal", "$failure.entry_refusal")
    add("recover-ignores-entry-guard", recover, focus, "!$failure.entry_refusal || ", "")
    add("recover-refusal-changes-entry", recover, focus, "return Err($failure);",
        "$failure.entry_refusal = !$failure.entry_refusal;\n            return Err($failure);")
    add("recover-refusal-drops-holder", recover, focus, "return Err($failure);",
        "$failure.owner.context = None;\n            return Err($failure);")
    add("recover-success-quarantines", recover, focus, "let context = $failure.owner.take();",
        "$failure.owner.context_mut().quarantine();\n        let context = $failure.owner.take();")
    add("recover-refusal-quarantines", recover, focus, "return Err($failure);",
        "$failure.owner.context_mut().quarantine();\n            return Err($failure);")

    # Dropping the terminal-call guard is not rejected by the deliberately weak
    # recovery postcondition. Heterogeneous part swaps are Rust errors, not logical controls.
    result = {name: (change(body, macro, before, after), selected)
              for name, macro, selected, before, after in rows}
    need(len(result) == len(rows) == MUTANT_COUNT and len(set(result.values())) == MUTANT_COUNT,
         "distinct owned-storage logical control roster")
    return result


def selection_notes(leaf, focus):
    selectors = {row[1] for row in mutations((ROOT / BODY).read_text()).values()}
    need(focus in selectors, "exact owned-storage selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function retained_pair_owned_storage_v1::" + focus[1:] + " (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated owned-process campaign")
    source = raw.decode("utf-8")
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
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "measured positive count required")
    module = types.ModuleType("retained_owned_storage_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-retained-pair-owned-storage.py"))
    campaign().main()
