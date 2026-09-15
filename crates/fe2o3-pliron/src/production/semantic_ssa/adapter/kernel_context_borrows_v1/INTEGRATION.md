# Closed KernelContext Reference Flows

Pauli owns this child, its tests and the requested context mount in Ram's
existing shared borrow classifier. The six detached component tests passed with
`rustc --test` against cached `libfe2o3_mir_model-3e1566b66a32d8da.rlib`.
No Cargo has run for this addition, and no integrated helper-flow pass is claimed.

## Root Cause

The two lowerer19c source-carrier failures expand the original
`helper(&KernelContext)` calls into reference Copy parameter assignments. The
legacy adapter rejects consumed `value_alias` candidates and reference forks.
The newer fork-aware classifier already handles those assignments, but its
closed reference/terminal roster contains only Workgroup/subgroup operations.
Do not weaken the legacy rule for every intrinsic or remove the helper tests.

## Mounted Hooks

`adapter.rs` mounts this child alongside the existing borrow classifier.
`borrowed_workgroup_v1.rs` imports `KernelContextBorrowV1` and passes types only
for the authenticated execution-view entry point. The existing direct path and
its tests continue to pass `None`. These hooks are mounted, not a pending handoff.

1. The internal `sites` function takes optional type declarations. Existing
   direct/type-less entry points pass `None`; execution passes
   `Some(semantic.types())`. Existing tests using `sites` directly pass `None`.
2. Its callable-roster loop additionally uses
   `KernelContextBorrowV1::for_callable(types, callable)` when types are present;
   for `Some(fact)`, insert `fact.reference_pair()` with the existing
   conflicting-pair rejection.
3. At an actual terminal, it derives the fact from the callable selected by the
   actual callee ID. Accept argument 0 when
   `fact.accepts(call, argument, candidates[index].source_type)` holds, in
   addition to the existing Workgroup acceptance condition. Charge this second
   signature traversal against the existing work budget.

All reference forwarding, component escape poisoning and duplicate-definition
handling remain in the existing classifier. This child has no graph solver,
ambient initialization, root construction or issuer factory. It accepts only
exact policy-issuance and existing global-bind terminals, and derives the owned
type from the immutable thin reference type edge and the admitted source ABI.
Defined Math getter/Bind sinks still need the separate replay-checked occurrence
relation, not this terminal API.

## Central Regressions

Run the two existing expanded-helper positives unchanged, all five source
carrier tests, the borrowed adapter tests and `kernel_context_borrows_v1::tests`.
Then test expanded helper copies, reference moves, reborrows and repeated calls;
unknown calls/escapes, mutable/raw references, changed receiver/output/source
identity, storage death and missing actual context definitions must reject.
Transparency can never substitute for SSA definitions or the lowerer's actual
root-authenticated KernelContextIssue. Component inputs in source-carrier tests
are not production ranked-root qualification.

The standalone tests cover callable/type facts only, not reference loan
liveness. In particular, passing the original owner through a reference cannot
make owner StorageDead/overwrite invisible. Preserve or add the actual SSA and
lowerer lifetime regressions when mounting this classification extension.
