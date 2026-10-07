#!/usr/bin/env python3
"""Source binding and mutant construction only; executes no solver."""
from pathlib import Path
import runpy
import types

HERE = Path(__file__).resolve().parent
api = runpy.run_path(str(HERE / "check-queued-query.py"))
sources = api["snapshot"]()
api["audit"](sources)


def refuse(changed):
    try:
        api["audit"](changed)
    except (ValueError, KeyError):
        return
    raise AssertionError("hostile source join accepted")


def changed(path, before, after):
    assert sources[path].count(before) == 1, (path, before)
    result = dict(sources)
    result[path] = result[path].replace(before, after)
    return result


src, proof = api["SRC"], api["PROOF"]
parent = src / "context_queued_writers.rs"
declarations = src / "context_queued_writers/declarations.rs"
reads = src / "context_queued_writers/reads.rs"
read_declarations = src / "context_queued_writers/read_declarations.rs"
forward = src / "context_queued_writers/forward.rs"
dispose = src / "context_queued_writers/group_disposal.rs"

for path, before, after in [
    (parent, 'include!("context_queued_writers/declarations.rs");',
        '#[cfg(any())]\ninclude!("context_queued_writers/declarations.rs");'),
    (parent, "queued_root_body_v1!(self, writer)", "Err(Error::InvalidReference)"),
    (parent, "retained_writer_key_body!(left, right)", "left.local == right.local"),
    (declarations, "    inner: ContextProducerReadJournalV1,", "    inner: (),"),
    (declarations, "    next_read_incarnation: u64,", "    next_read_incarnation: u32,"),
    (declarations, "    read_producer_scratch: Vec<ContextWriterReferenceV1>,", "    read_producer_scratch: (),"),
    (read_declarations, "    version: Option<(u64, u64)>,", "    version: Option<u64>,"),
    (read_declarations, "    pub incarnation: u64,", "    pub incarnation: u32,"),
    (reads, 'include!("read_query_bodies.rs");',
        'mod inactive { include!("read_query_bodies.rs"); }'),
    (forward, "queued_active_lookup_body_v1!(self, reference)",
        "{ self.ensure_usable()?; queued_active_lookup_body_v1!(self, reference) }"),
    (dispose, "queued_ensure_usable_body_v1!(self)", "Ok(())"),
    (proof, "    ensures\n        *final(owner) == *old(owner),",
        "    requires false,\n    ensures\n        *final(owner) == *old(owner),"),
    (proof, "        *final(owner) == *old(owner),", "        true,"),
    (proof, 'include!("../src/context_queued_writers/read_declarations.rs");',
        '#[cfg(any())]\ninclude!("../src/context_queued_writers/read_declarations.rs");'),
]:
    refuse(changed(path, before, after))

shadow = dict(sources)
shadow[src / "lib.rs"] += """
impl crate::context_producer_reads::ContextProducerReadJournalV1 {
    pub fn lookup_allocation(&self, _: crate::context_version_journal::ContextAllocationReferenceV1)
        -> Result<crate::context_version_journal::ContextAllocationStateV1,
                  crate::context_version_journal::ContextVersionJournalErrorV1> {
        Err(crate::context_version_journal::ContextVersionJournalErrorV1::InvalidReference)
    }
}
"""
refuse(shadow)
extra = dict(sources)
extra[src / "unbound_query_override.rs"] = "// unexpected source roster member\n"
refuse(extra)
missing = dict(sources)
del missing[src / "lib.rs"]
refuse(missing)
inherited = api["V"] / "context_owner_inspection_bodies_v1.rs"
refuse(changed(inherited, "ensures *result == inspection_producer_projection_v1(*self),", "ensures true,"))

all_names = set()
for outer, expected in ((False, 25), (True, 11)):
    path = api["OUTER_BODY"] if outer else api["BODY"]
    mutants = api["mutations"](sources[path], outer)
    assert len(mutants) == expected
    assert len({body for body, _ in mutants.values()}) == expected
    for name, (body, focus) in mutants.items():
        assert name not in all_names
        all_names.add(name)
        assert body != sources[path] and focus.startswith("*")
        mutated = dict(sources)
        mutated[path] = body
        refuse(mutated)
        notes = api["selection_notes"](types.SimpleNamespace(LOGICAL_ERRORS=set()), focus, outer)
        assert len(notes.SELECTION_NOTES) == 2
        assert all("selected functions" in note for note in notes.SELECTION_NOTES)
assert len(all_names) == 36

if api["EXPECTED_VERIFIED"] is None:
    try:
        api["campaign"]()
    except ValueError as error:
        assert "unmeasured" in str(error)
    else:
        raise AssertionError("unmeasured positive accepted")
else:
    assert type(api["EXPECTED_VERIFIED"]) is int and api["EXPECTED_VERIFIED"] > 0
    assert api["campaign"]().QUERY_MODULE_ARGS == ["--verify-only-module", "reads"]
    assert api["campaign"](True).QUERY_MODULE_ARGS == ["--verify-root"]

print("PASS: queued-query source calibration (4 groups; does not execute the 36 logical mutants)")
