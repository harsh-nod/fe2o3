# Bounded-Unroll Canonical-Wire Goldens

Five literal canonical V12 input graphs run through the actual bounded owning
unroll pass and independent pair checker. Exact before/after graphs, all eleven
limits, selected fact/count, and complete block/definition/operation/terminator/
edge/edge-argument lineage are compared to literal expectations. Zero-trip
selection still retains its executed header and explicitly omits body lineage.
The three-trip case preserves each private Store/Load and checked update.
Symbolic and over-ceiling loops remain exact no-ops; a second pass is a no-op.

The ordinary parent executes the exact ignored child twice, requiring one
successful test per process and equal bounded transcripts including canonical
input/output bytes. Missing child markers fail. Malformed wire, all six hostile
row families, foreign input, changed limits and an independently admitted wrong
output payload are typed refusals, with floor/ledger/sibling cleanup checked.

The shared helper is the unchanged donor decoder/renderer, not a new parser.
Expected files are independently authored, never blessed by the producer or
regenerated during tests. Strings, byte copies and hostile candidates belong to
the harness, outside verifier scratch accounting; returned owner/pair receipts
are reserved for controlled use and backing drops before refund. Existing
strict resource tests remain separate, with no new storage-phase oracle here.

Root qualification commands (not an execution claim):

```sh
cargo test --locked --offline --no-default-features -p fe2o3-kernel-opt --test bounded_loop_unroll_wire_goldens -- --test-threads=1 --show-output
cargo test --locked --offline --no-default-features -p fe2o3-kernel-opt --test refined_forwarding_wire_goldens -- --test-threads=1 --show-output
```

This is canonical-wire golden coverage, not registered-dialect Pliron textual
lit, ordinary Rust source admission, SIM/native/hardware execution, transported
history authority or default activation. Those gates remain separate. These
fixtures contain no source-derived fact inferred from Rust spelling.
