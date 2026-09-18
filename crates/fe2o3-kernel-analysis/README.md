# Kernel Analysis

`fe2o3-kernel-analysis` provides conservative analyses of Kernel IR. Reports do
not grant source authenticity, formal verification, safe-launch or hardware
execution authority.

## Canonical MemorySSA

`CanonicalKirMemorySsaV1` borrows an exact immutable inventory and builds one
conservative all-memory version graph in linear structural work. Each defined
function has a LiveOnEntry node; every block has an unpruned phi with all incoming
edge occurrences, and the entry block also retains its synthetic entry input.
Block exits are built before predecessor links, so backedges, duplicate edges,
irreducible regions and disconnected components require no iterative guess.

Ordinary reads use the current version. Writes, allocations, volatile effects,
atomics, synchronization, calls, assembly, execution operations and compiler
ordering conservatively define a new version. A Def is a possible clobber or
ordering barrier, not proof that a physical write occurs. This report proves
neither aliasing nor initialization, reachability, absence of traps, race freedom
or legal instruction motion. It does not authorize optimization rewrites.

The report charges construction work, actual vector capacities and its header;
queries charge fixed work. Its immutable borrow supplies graph custody, not a
hash-based cache key. Node IDs are local numeric locators, not transferable
identity. `with_canonical_analysis_scope_v1` in `fe2o3-pliron` lazily retains the
report beside sparse analysis on the same ledger. This adds an M4 analysis
prerequisite, not LICM, load forwarding or a new production pass policy.

## Checked Store Forwarding

`CanonicalKirStoreForwardingPlanV1` uses an exact inventory and MemorySSA report
to find ordinary private-memory integer loads with an earlier store to the same
pointer SSA value in the same block. The access attributes and integer widths
must match. An intervening write, allocation, call, ordering effect, volatile
access, convergence operation or potentially trapping arithmetic stops the rule.
Only a closed set of total scalar operations and matching loads may intervene.
The store supplies initialization; a repeated read alone does not.

The rewrite keeps the store and all operation coordinates, replacing the load
with `BitOr(stored_value, stored_value)` of the same fixed-width integer type.
`check_canonical_kir_store_forwarding_v1` independently checks the store, interval,
types and complete before/after module without trusting the MemorySSA report or
the proposed rewrite rows. Work for candidate mutation is paid before any edit.
Block-local rewrites work within arbitrary CFGs, including loops, but no
cross-block, alias-based, floating-point or global-memory forwarding is claimed.

`fe2o3-kernel-opt::optimize_checked_store_forwarding_v1` composes the analysis,
budgeted V12 candidate copy, rewrite, fresh canonical admission and independent
check. Its result retains the input borrow, actual output and checked rows; it
cannot stand in for a Policy3 owner or final source/private-address admission.
This service is not appended to the frozen production pass schedule.

The plan reserves at most one row per operation and scans linearly. Candidate
copying, application and replay have separately metered work/storage receipts.
Result exits restore the incoming floor, retaining work, peak and denial data;
these are logical payload bounds, not whole-process or allocator RSS limits.

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
cargo test --locked -p fe2o3-kernel-analysis --lib memory_ssa
cargo test --locked -p fe2o3-kernel-analysis --no-default-features --lib memory_ssa
cargo test --locked -p fe2o3-pliron --lib memory_ssa
cargo test --locked -p fe2o3-kernel-analysis -p fe2o3-kernel-opt --lib store_forwarding
cargo test --locked -p fe2o3-kernel-analysis --lib canonical_kir_physical_occurrences_v1
cargo test --locked -p fe2o3-kernel-analysis --no-default-features --lib canonical_kir_physical_occurrences_v1
cargo test --locked -p fe2o3-kernel-analysis --doc CanonicalKirPhysicalOccurrencesV1
```

Direct tests cover actual KIR admission, exact/one-under work and storage, foreign
custody, grounded/cyclic joins, duplicate edges, distinct catalog keys, explicit
escapes, guarded/copy/atomic/barrier attributes, opaque assembly and entry views.
They do not claim that authored KIR fixtures are admitted Rust or safe executions.
