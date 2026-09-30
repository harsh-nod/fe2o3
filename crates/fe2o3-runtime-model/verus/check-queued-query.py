#!/usr/bin/env python3
"""Actual outer/queued read queries over complete arbitrary owners.

Artifact-specific source binding includes the complete implementation crate to
exclude unbound inherent methods shadowing the proven inner Deref routes.
No constructor reachability, global list validity, credit or native claim.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
CRATE = Path("crates/fe2o3-runtime-model")
V = CRATE / "verus"
SRC = CRATE / "src"
PROOF = V / "context_queued_query_execution_v1.rs"
BODY = SRC / "context_queued_writers/read_query_bodies.rs"
OUTER_BODY = SRC / "context_queued_writers/query_bodies.rs"
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "0ee2e8d821a9cc0572fdbad358eb0dc4770470ef3585febb630d08d461c09545"
INHERITED_TREE_SHA = "246c85109c6b8edb1b952c70708ef4259837098aee3c612f045343f6f64fc189"
PROOF_SHA = "fbe03df86e334dab6f08a3cb0e6ea219d5848ebf3f91e19678688795d5d62609"
# Measured by the full, unfiltered development discovery on draft 2.
EXPECTED_VERIFIED = 161
FILES = [CRATE / name for name in (
    "src/context_producer_reads/declarations.rs", "src/context_producer_reads/query_bodies.rs",
    "src/context_queued_writers/declarations.rs", "src/context_queued_writers/query_bodies.rs",
    "src/context_queued_writers/read_declarations.rs", "src/context_queued_writers/read_query_bodies.rs",
    "src/context_read_leases/acquire_bodies.rs", "src/context_read_leases/acquire_declarations.rs",
    "src/context_read_leases/declarations.rs", "src/context_read_leases/guard_bodies.rs",
    "src/context_read_leases/release_bodies.rs", "src/context_read_leases/release_declarations.rs",
    "src/context_version_journal/begin_bodies.rs", "src/context_version_journal/declarations.rs",
    "src/context_version_journal/inspection_bodies.rs", "src/context_version_journal/lookup_bodies.rs",
    "src/context_version_journal/retained_bodies.rs", "src/context_version_journal/writer_lookup_bodies.rs",
    "verus/context_journal_begin_bodies_v1.rs", "verus/context_journal_begin_decisions_v1.rs",
    "verus/context_journal_begin_execution_v1.rs", "verus/context_journal_begin_witnesses_v1.rs",
    "verus/context_owner_inspection_bodies_v1.rs", "verus/context_producer_query_bodies_v1.rs",
    "verus/context_producer_query_decisions_v1.rs", "verus/context_producer_query_execution_v1.rs",
    "verus/context_queued_query_execution_v1.rs", "verus/context_reader_guards_bodies_v1.rs",
    "verus/context_reader_guards_decisions_v1.rs", "verus/context_reader_guards_execution_v1.rs",
    "verus/context_stable_acquire_bodies_v1.rs", "verus/context_stable_acquire_decisions_v1.rs",
    "verus/context_stable_acquire_execution_v1.rs", "verus/context_stable_release_bodies_v1.rs",
    "verus/context_stable_release_decisions_v1.rs", "verus/context_stable_release_execution_v1.rs",
)]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def tree_hash(sources):
    image = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(image, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = set(FILES) | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        file = ROOT / path
        need(file.is_file() and not file.is_symlink() and file.resolve() == file, "ordinary exact source path")
        result[path] = file.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "complete implementation source roster and bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "reviewed exact arbitrary-input query contracts and full owner frame")
    inherited = {path: sources[path] for path in FILES if path != PROOF and path.is_relative_to(V)}
    need(tree_hash(inherited) == INHERITED_TREE_SHA, "unchanged actual inner execution/schema/Deref contracts")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        need(path in sources and path in FILES, "closed proof input membership")
        reached.add(path)
        for include in re.findall(r'^\s*include!\("([^"\n]+)"\);', sources[path], re.M):
            child = (ROOT / path.parent / include).resolve().relative_to(ROOT)
            pending.append(child)
    need(reached == set(FILES), "exact complete shared-body and actual-inner include closure")
    need("requires" not in sources[PROOF] and "external_body" not in sources[PROOF]
         and "assume(" not in sources[PROOF], "no new precondition or trusted query adapter")


def change(body, macro, before, after):
    anchor = "macro_rules! " + macro + " {"
    need(body.count(anchor) == 1, "unique changed shared macro")
    start = body.index(anchor)
    end = body.find("\nmacro_rules! ", start + len(anchor))
    end = len(body) if end < 0 else end
    selected = body[start:end]
    need(selected.count(before) == 1 and before != after, "one meaningful shared-body mutation")
    return body[:start] + selected.replace(before, after) + body[end:]


def mutations(body, outer=False):
    if outer:
        rows = [
            ("writer-slot", "queued_writer_same_body_v1", "queued_writer_same_v1", "$left.slot == $right.slot &&", "$left.slot == $right.slot ||"),
            ("writer-key", "queued_writer_same_body_v1", "queued_writer_same_v1", "queued_writer_key_same_v1($left.key, $right.key)", "$left.key.local == $right.key.local"),
            ("terminal-error", "queued_ensure_usable_body_v1", "ensure_usable", "Err(Error::InvalidState)", "Err(Error::InvalidReference)"),
            ("terminal-skip", "queued_ensure_usable_body_v1", "ensure_usable", "if $owner.disposal_terminal", "if false && $owner.disposal_terminal"),
            ("root-inner-lookup", "queued_root_body_v1", "root", "$owner.inner.lookup_writer($writer)?;", "let _ = $writer;"),
            ("root-identity", "queued_root_body_v1", "root", "if !queued_writer_same_v1", "if false && !queued_writer_same_v1"),
            ("destination-generation", "queued_destination_body_v1", "destination", "state.device.context_generation !=", "false && state.device.context_generation !="),
            ("destination-local", "queued_destination_body_v1", "destination", "|| state.device.local !=", "|| false && state.device.local !="),
            ("destination-extent", "queued_destination_body_v1", "destination", "if state.byte_extent !=", "if false && state.byte_extent !="),
            ("active-lookup-terminal", "queued_active_lookup_body_v1", "lookup_producer_read", "$owner.inner.lookup_producer_read($reference)", "{ $owner.ensure_usable()?; $owner.inner.lookup_producer_read($reference) }"),
            ("active-status-terminal", "queued_active_status_body_v1", "producer_read_status", "$owner.ensure_usable()?;", "let _ = $owner.disposal_terminal;"),
        ]
    else:
        rows = [
            ("reference-slot", "queued_read_reference_same_body_v1", "queued_read_reference_same_v1", "$left.slot == $right.slot", "($left.slot == $right.slot || true)"),
            ("reference-incarnation", "queued_read_reference_same_body_v1", "queued_read_reference_same_v1", "$left.incarnation == $right.incarnation", "($left.incarnation == $right.incarnation || true)"),
            ("reference-consumer", "queued_read_reference_same_body_v1", "queued_read_reference_same_v1", "queued_writer_key_same_v1($left.consumer, $right.consumer)", "$left.consumer.local == $right.consumer.local"),
            ("entry-empty-error", "queued_read_entry_body_v1", "read_entry", "None => Err(Error::InvalidReference)", "None => Err(Error::InvalidState)"),
            ("resolved-links", "queued_read_links_body_v1", "validate_read_links", "$entry.previous.is_none() && $entry.next.is_none()", "($entry.previous.is_none() && $entry.next.is_none()) || true"),
            ("resolved-root", "queued_read_links_body_v1", "validate_read_links", "if !matches!($entry.status", "let _ = $owner.root($entry.request.producer)?;\n        if !matches!($entry.status"),
            ("root-zero-count", "queued_read_links_body_v1", "validate_read_links", "root.read_count == 0 ||", "false && root.read_count == 0 ||"),
            ("root-excess-count", "queued_read_links_body_v1", "validate_read_links", "|| root.read_count >", "|| false && root.read_count >"),
            ("previous-producer", "queued_read_links_body_v1", "validate_read_links", "if !queued_writer_same_v1(previous.request.producer", "if false && !queued_writer_same_v1(previous.request.producer"),
            ("previous-status", "queued_read_links_body_v1", "validate_read_links", "|| !matches!(previous.status", "|| false && !matches!(previous.status"),
            ("previous-reciprocal", "queued_read_links_body_v1", "validate_read_links", "|| match previous.next", "|| false && match previous.next"),
            ("root-read-head", "queued_read_links_body_v1", "validate_read_links", "else if match root.read_head", "else if false && match root.read_head"),
            ("next-producer", "queued_read_links_body_v1", "validate_read_links", "if !queued_writer_same_v1(next.request.producer", "if false && !queued_writer_same_v1(next.request.producer"),
            ("next-status", "queued_read_links_body_v1", "validate_read_links", "|| !matches!(next.status", "|| false && !matches!(next.status"),
            ("next-reciprocal", "queued_read_links_body_v1", "validate_read_links", "|| match next.previous", "|| false && match next.previous"),
            ("inspect-terminal", "queued_read_inspect_body_v1", "inspect_queued_read", "$owner.ensure_usable()?;", "let _ = $owner.disposal_terminal;"),
            ("inspect-reference", "queued_read_inspect_body_v1", "inspect_queued_read", "if !queued_read_reference_same_v1", "if false && !queued_read_reference_same_v1"),
            ("count-missing-error", "queued_read_inspect_body_v1", "inspect_queued_read", "None => return Err(Error::InvalidReference)", "None => return Err(Error::InvalidState)"),
            ("count-zero", "queued_read_inspect_body_v1", "inspect_queued_read", "if count == 0", "if false && count == 0"),
            ("inspect-links", "queued_read_inspect_body_v1", "inspect_queued_read", "$owner.validate_read_links(entry)?;", "let _ = entry;"),
            ("success-epoch", "queued_read_inspect_body_v1", "inspect_queued_read", "epoch != state.attempt_epoch ||", "false && epoch != state.attempt_epoch ||"),
            ("success-lineage", "queued_read_inspect_body_v1", "inspect_queued_read", "|| lineage != state.content_lineage", "|| false && lineage != state.content_lineage"),
            ("success-pending-writer", "queued_read_inspect_body_v1", "inspect_queued_read", "state.pending_writer.is_some()", "(false && state.pending_writer.is_some())"),
            ("success-current-version", "queued_read_inspect_body_v1", "inspect_queued_read", "|| state.attempt_epoch != state.content_lineage", "|| false && state.attempt_epoch != state.content_lineage"),
            ("non-success-version", "queued_read_inspect_body_v1", "inspect_queued_read", "else if entry.version.is_some()", "else if false && entry.version.is_some()"),
        ]
    result = {name: (change(body, macro, before, after), "*" + (method if method.startswith("queued_")
        else "ContextQueuedWriterJournalV1::" + method))
              for name, macro, method, before, after in rows}
    need(len(result) == len(rows), "unique logical control names")
    return result


def selection_notes(leaf, focus, outer=False):
    selected = focus.removeprefix("*")
    need(focus.startswith("*") and selected in {value[1].removeprefix("*")
         for value in mutations((ROOT / (OUTER_BODY if outer else BODY)).read_text(), outer).values()}, "exact query selector")
    module = "root module" if outer else "module reads"
    path = "context_queued_query_execution_v1::"
    if selected == "queued_read_reference_same_v1":
        path += "reads::"
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying " + module + " (selected functions)",
        "verifying " + module + ", function " + path + selected + " (selected functions)",
    })


def campaign(outer=False):
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "full positive proof count remains unmeasured")
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated inherited controller")
    source = raw.decode()
    before = '["--verify-function", focus, "--verify-root"]'
    after = '["--verify-function", focus, *QUERY_MODULE_ARGS]'
    need(source.count(before) == 1, "exact module-selection adaptation only")
    module = types.ModuleType("queued_query_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(source.replace(before, after), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, OUTER_BODY if outer else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.QUERY_MODULE_ARGS = ["--verify-root"] if outer else ["--verify-only-module", "reads"]
    module.mutations = lambda body: mutations(body, outer)
    module.selection_notes = lambda leaf, focus: selection_notes(leaf, focus, outer)
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-queued-query.py"))
    outer = "--outer" in sys.argv
    if outer:
        sys.argv.remove("--outer")
    campaign(outer).main()
