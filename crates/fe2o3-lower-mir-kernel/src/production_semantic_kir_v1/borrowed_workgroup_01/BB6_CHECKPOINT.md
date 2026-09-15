# Actual Workgroup BB6 Checkpoint

Max is paused. No Cargo, dependency rebuild, source snapshot7 edit, DSO rebuild,
authority waiver, or success claim. Parent reports Pliron236 passing, including
four Context tests; actual 23c clears retained local8 then rejects execution
argument materialization at expanded fn0/bb6.

## New Source Evidence

A standalone rustc-driver probe of the unchanged `canonical_transport_v1/source.rs`
using the exact cached gfx950 core/device metadata and production macro/session
binding succeeded. Private probe, source, and complete bounded output:
`/tmp/fe2o3-borrowed-bb6-probe/{probe.rs,probe,source-mir.log}`.
It runs `-Zno-codegen`, not Cargo, and writes only private scratch outputs.

The actual generated physical wrapper has:

```text
bb0: _2 = KernelContext::__compiler_issue() -> bb1
bb1: _3 = logical_body(const KernelContext { PhantomData fields }, copy _1) -> bb2
```

The logical body borrows its Context parameter and passes it to `with_workgroup`.
The real cached `with_workgroup` MIR passes its mutable reference directly to
`__compiler_workgroup_capability_current`. Thus the physical issuer remains, but
its source SSA edge into the logical helper has been erased to a ZST constant.
This is source evidence for a constructor-to-argument transport gap, not evidence
that a same-typed Context value can be substituted. The next rebuilt callback
will confirm its exact expanded bb6 origin.

## Current API Gap

- `collector.rs::AuthenticatedKernelContextSourceV1` retains the unique issuance
  commitment, marker and root, but no issuer-destination -> helper-call argument
  relation. `ObservedKernelContextIssuanceV1` retains only block and terminal ID.
- `production_importer_v1.rs::CollectedKernelContextV1` and
  `AuthenticatedProductionKernelContextRootV1` carry those commitments onward;
  they do not retain the logical-helper identity or an erased argument receipt.
- `ProductionKernelContextLoweringInputV1` is explicitly inert. Its constructor
  cannot confer this missing source relation.
- Lowerer's `lower_operand` handles a ZeroSized constant structurally unless it
  is the already separate checked GridLeader case. `KernelContext.values()` is
  already exactly one value; changing that is not the fix.

## Required Coordination Before Production Repair

The producer must retain and authenticate the exact source edge before ZST
erasure: physical root, logical helper, argument ordinal, source context type,
actual issuance occurrence/destination and actual helper call occurrence. For a
local macro-generated root, a checked HIR local-binding use tied to its exact
trusted initializer and resolved helper call can supply source evidence; a
type/layout/name/span-only join cannot. Alternatively retain the actual
pre-erasure MIR edge through existing source-capture custody.

Pauli/common mapper must replay that checked edge into the synthetic call-entry
assignment, retaining the original source body and the source argument as a
reference/constant observation. Lowering must select the original issuer's
already lowered SSA binding, with dominance and kill checks, rather than
manufacturing a Context token at bb6. No new execution terminal tag is required;
a new retained record/carrier needs explicit schema and owner coordination.

Required negatives: wrong helper/argument, foreign root or issuer, changed
context type, absent or duplicated source edge, dead issuer, and a same-typed
ZST constant without the checked constructor edge. The real AMD callback and
its original owner/epoch/lifetime mutations remain mandatory.

## Mounted Diagnostic Only

`canonical_transport_v1/lowering_failure.rs` captures the replayed operation,
source occurrence, operands and bounded local-definition chain before the owner
is consumed. `lowering_tests.rs` prints only the matching failure and still
panics. No production check was changed. The new child passes standalone rustc
metadata type-check against the cached compiler MIR/Pliron/lowerer libraries;
rustfmt syntax and git whitespace checks pass. Parent must rebuild/rerun the
existing `workgroup_full_import_gfx950_v20` callback to obtain expanded evidence.
