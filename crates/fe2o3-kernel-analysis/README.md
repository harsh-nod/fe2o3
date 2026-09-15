# Kernel Analysis

`fe2o3-kernel-analysis` provides conservative analyses of Kernel IR. Reports do
not grant source authenticity, formal verification, safe-launch or hardware
execution authority.

## Canonical Physical Occurrences

[`CanonicalKirPhysicalOccurrencesV1`](src/canonical_kir_physical_occurrences_v1.rs)
is derived from one admitted graph's `CanonicalKirInventoryV1` and a
`CheckedKernelIrContractCatalogV1` for that exact inventory. It borrows the
inventory and underlying catalog, not the checked-view header. Three vectors
retain function ranges, physical occurrences, and pointer-use rows. Existing
metered must-alias analysis runs at most once per report derivation when needed;
prior catalog validation may have performed its own separate alias analysis.

The report preserves actual allocation/access/synchronization effects, contract
markers, calls and opaque assembly, including unreachable blocks and helpers.
`function()` and `kernel_entry()` provide allocation-free views without expanding
calls. `operation_for()` and `use_for()` accept only rows belonging to that exact
report; structurally equal foreign rows are rejected.

Coordinates are explicit:

- Operation/effect/allocation coordinates are dense indices in the borrowed inventory.
- `Access.pointer_use` indexes the report's `pointer_uses()`; its `use_index()` then
  identifies the original inventory use.
- Contract storage/epoch and GEP offset coordinates directly index inventory uses.
- Kernel-entry views retain actual kernel/function ordinals, not source root ordinals.

Grounded must-alias results identify physical WorkgroupMemory allocations, never
block parameters. One GEP can retain an allocation-relative offset observation;
this is not pointer equality or a byte-bounds proof. Unknown entries, conflicting
origins, ungrounded cycles, casts, nested offsets and call results remain unresolved.
Stored/returned/call/assembly/unsupported pointer uses remain explicit escapes.
Other address spaces, including Generic, are outside this workgroup origin model,
not proven disjoint. Footprint tags do not establish initialization, bounds,
synchronization, complete lifecycle correspondence or race freedom.

## Resource Contract

Pass one shared verification resource ledger. `derive()` restores the incoming
storage floor on every Result path, preserving work, peak and first failures.
Reserve the returned receipt before another controlled allocation; drop the report
before releasing its receipt. The inventory and catalog remain caller-owned.

Retained storage is the report header plus the exact three vector payloads.
Temporary alias scratch is dropped before report transfer. The peak increment
above the caller's live floor is the maximum of alias scratch and alias retention
plus the report; a larger historical peak is preserved. Work is a structural
census/fill plus existing bounded alias work and metered catalog binary searches;
metadata length and sparse numeric IDs do not inflate this report's own profile.
These are logical payload bounds, not allocator RSS or standalone unwind guarantees.

## Tests

```sh
cargo test --locked -p fe2o3-kernel-analysis --lib canonical_kir_physical_occurrences_v1
cargo test --locked -p fe2o3-kernel-analysis --no-default-features --lib canonical_kir_physical_occurrences_v1
cargo test --locked -p fe2o3-kernel-analysis --doc CanonicalKirPhysicalOccurrencesV1
```

Direct tests cover actual KIR admission, exact/one-under work and storage, foreign
custody, grounded/cyclic joins, duplicate edges, distinct catalog keys, explicit
escapes, guarded/copy/atomic/barrier attributes, opaque assembly and entry views.
They do not claim that authored KIR fixtures are admitted Rust or safe executions.
