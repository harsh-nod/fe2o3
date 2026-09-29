# Context Vecadd Source Handoff V1

Source-only increment based on `f2dd128e35d5345cf4b7f6fb3f71480ee483c961`,
2026-09-29. No builds, source-compilation tests, simulation or hardware runs were
performed for this patch. M1 remains incomplete; this fixture does not migrate
or qualify the manifest vecadd. M0 acceptance and the 0/47 strict-production
GPU baseline are unchanged.

## Fixture And Authored Checks

`crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/context_vecadd.rs`
is ordinary `#[kernel]` Rust with a logical `KernelContext<'_>` and explicit
required/max workgroup `[256, 1, 1]`. It includes the actual
`examples/vecadd/src/vecadd_body.rs` unchanged. The two `&[f32]` inputs and
`DisjointSlice<f32>` output remain the three physical arguments. No `Global`
alias, alternate executable body, new fixture feature or workload selector is
introduced. The existing `provider_context_protocol` scratch-source harness
selects this fixture only in tests.

This is a diagnostic consumer. `_ctx` covers logical context issuance and ABI
preservation only; the unchanged shared body still uses the existing
`thread::index_1d` API. It does not implement unified `ctx.invocation` derivation
or complete M1 memory-capability normalization. Existing typed slices and
`DisjointSlice` roles are observed, not promoted into new context-bound memory
authority.

Authored tests, all execution-dependent and ignored by default:

- `context_vecadd_normal_exports_require_checked_materialization` uses the
  normal simulation V5 exporter and LLVM extractor for gfx942 and gfx950. Each
  must reach `execution capabilities require checked canonical KIR materialization`
  and leave the requested output absent. An earlier unrelated rejection fails.
- `actual_context_vecadd_preserves_three_physical_arguments_then_refuses` checks
  genuine collector-bound source at the checked-root observer: three physical
  arguments, four helper arguments, no hidden arguments, zero-byte context
  layout, ignored logical-context ABI, unchanged
  ordered physical types/ABIs, SharedBorrow/SharedBorrow/ExclusiveOwner roles,
  actual issuance/helper calls and explicit launch. The normal materializer
  must then reach the exact refusal above.
- `actual_context_vecadd_observes_pending_memory_arithmetic_and_control` consumes
  the existing pending-source observer. It requires two f32 Global loads, one
  f32 add, one Global store, retained conditional control and at least two
  source-assertion attachments. It records exact graph-local operation/branch
  coordinates, operand/result IDs and any memory predicates beside source and
  V18 graph identities. It also records typed slice-parameter positions/IDs,
  pointer access roles (ReadOnly loads, ReadWrite store), alignment/volatility,
  slice data/length IDs, address base/offset IDs and less-than operands/results.
  These are symbolic addresses and extents, not concrete runtime addresses or
  numeric buffer lengths. A completed callback must still end at
  `PendingScopedObservationIncomplete`; no normal compiler continuation is
  accepted. Failure before pending construction remains a blocker, not a pass.

Each library parent requests both target profiles, optimization/MIR-optimization
`(0,0), (0,2), (3,0), (3,2)` and duplicate fresh-process observations: 16 source
sessions per parent, 32 total. These are authored expectations, not results.
The harness also requires the per-invocation compiler output directory to stay
empty. Recording branches and assertion attachments does not prove dominance,
assertion truth, exact source pointer provenance or source-to-graph value
equivalence. Counts alone do not identify which input bound an assertion covers.
The fixed observation bounds deliberately reject excessive inventories.

Primary-owned execution commands when the serialized build lane permits:

```sh
cargo test -p rustc-codegen-fe2o3 --lib actual_context_vecadd_ -- --ignored --nocapture
cargo test -p rustc-codegen-fe2o3 --test production_ranked_bounds_driver_v1 context_vecadd_normal_exports_require_checked_materialization -- --ignored --nocapture
```

Rebuild the authentic device dependencies when running these commands: the
provider's documentation-only correction changes its source closure fingerprint.
Do not reuse provider receipts from the unmodified source tree.

## Current Source Entrypoints

Paths below are relative to `crates/`; these are implementation boundaries at
the base revision, not evidence that this fixture has executed them.

- `rustc-codegen-fe2o3/src/collector/production_importer_v1.rs`:
  `construct_production_semantic_mir_v1` selects exact MIR V29 for retained
  execution roles and authenticates semantic construction.
- `rustc-codegen-fe2o3/src/production_pipeline.rs`:
  `materialize_with_context_observer_v29` uses `materialize_prepared_v29` and
  then `ProductionPreRankedKirOwnerV1::try_materialize_with_budget`.
- `rustc-codegen-fe2o3/src/production_context_handoff_v29.rs`:
  test-only `observe_context_handoff_v29` follows import/middle-end/SSA and that
  same ordinary materializer. The checked-root callback grants no continuation.
- `rustc-codegen-fe2o3/src/production_pending_context_observer_v29.rs`:
  test-only `observe_pending_scoped_source_v29` uses the shared prepared-source
  path and `ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget`
  in `fe2o3-lower-mir-kernel/src/production_pending_scoped_source_v29.rs`.
  Its observation ends with `PendingScopedObservationIncomplete`.
- `fe2o3-lower-mir-kernel/src/production_semantic_kir_v1.rs`:
  `lower_module_with_call_budget_inner_v1` rejects MIR V29 before ordinary
  lowering. `rustc-codegen-fe2o3/src/production_ranked_projection_v1.rs`:
  `reject_retired_production_intrinsics_v1` independently rejects Execution.
- `fe2o3-lower-mir-kernel/src/production_execution_discharge_v29.rs`:
  `ProductionExecutionDischargeV29::try_discharge` accepts V15 and returns V12;
  it is not the checked V18 production successor needed here.

## V18 Dependency And Ownership

Latest #271 coordination read on 2026-09-29:
[owner handoff 5894692188](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5894692188).
It reports an unpublished optimized execution recipe census at
`012ccbf23b304ebf924f48e9f1bdd7fa73e678d6` handling ContextIssue, WorkgroupDerive
and ScopeEnd under retained original/optimized source owners. This is an owner
report, not independently inspected or qualified code in this patch. Obtain the
owner's reviewed dependency/integration revision before implementing another
execution recipe producer. Do not import that WIP wholesale.

The same handoff identifies #275's source-owned V18 scalar-candidate observer as
diagnostic only. Its tile-operation replacement correspondence and GridLeader
formal-domain work remain with #275/#271; context-only vecadd adds neither.
The shared `production_source_attachment_inventory_v18.rs` integration belongs
to that owner, including terminal/source-coordinate extensions.

The concrete M1 consumer dependency is a checked, source-owning V18 continuation
that preserves the complete storage-layout table and original collector,
source/SSA, launch, call-instance, occurrence and assertion bindings. Compose
the owner's execution recipe with ordinary value/control/memory correspondence,
assertion discharge or retained failure behavior, and operation-coordinate
transport through lifecycle erasure. Expose the resulting exact owned graph to
the existing pre-ranked/ranked/formal/target consumers. Unchecked pending replay
and equal physical output are insufficient. The standalone V15-to-V12 lifecycle
eraser cannot accept this V18 owner by dropping its layout table.

At the base revision `materialize_with_context_observer_v29` still invokes the
ordinary `ProductionPreRankedKirOwnerV1` constructor; the lowerer rejects MIR29.
Ranked projection independently rejects Execution intrinsics, and legacy
root/helper selection and correspondence attachment need the checked successor.
Keep these refusals until the shared continuation supplies their missing facts.
Do not label expanded helpers RawEmpty or add a vecadd-specific production route.

Popper/Carson's larger-root resource windows, native process/proof transport,
finalizer activation and protected safe-host launch are separate owners. This
patch changes none of those implementations and claims no milestone completion.
