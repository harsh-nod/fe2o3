# Inert Final Write Request Checkpoint

No pipeline connection, proof execution, final admission, or Cargo changes.

## Installed Hooks

- `fe2o3-kernel-analysis/src/canonical_ranked_view_v1.rs` exports
  `CanonicalRankedViewPreflightV1` and `preflight_canonical_ranked_view_v1`.
  Its existing prepare API shares a private planner with preflight and still
  calls `require_write_contracts`. All materialization/revalidation gates remain.
- `canonical_ranked_view_v1/tests.rs` includes `tests/preflight.rs`.
- `rustc-codegen-fe2o3/src/production_ranked_projection_v1.rs` exposes the crate-local
  `final_write_contract_requests_v1` child. No functional phase API was changed.

## Parent Call Site

Call `prepare_final_write_contract_requests_v1(&roster, &source_owner,
&protected_reference_bindings, &final_canonical, final_epoch)` while the exact
roster still has source-phase proof custody. The function borrows that custody;
its result owns only inert facts, allowing the parent to move the roster into
the existing prepared phase afterward. This function does not accept a final
module receipt and cannot relabel one as a per-effect source receipt.

Preflight retains exact final canonical bytes/epoch/function, actual store
pointer/index/predicate/RHS mappings, and bounded existing typed expressions.
The request child revalidates the source owner, proof phase, evidence, and live
source-contract export. Root/output joins are bijections. Scalar renaming uses
the actual SOURCE entry parameter resolved by the source owner's correspondence,
after exact entry signature equality; final SSA IDs are never source lookup IDs.
Ignored source arguments do not become physical parameter ordinals. Scalarized
arguments that cannot satisfy this initial injective mapping reject.

CPU expressions come only from the independent source reference root, never
from final GPU expressions. Source effects, view declarations, ownership,
numerical contracts, source proof requests, and the complete coordinate/domain
DAG are retained as their existing types. Source allocation/noalias identifiers
are not assumed equal to canonical parameter-derived view origins.

## Remaining Boundary

Every currently supported canonical f32 slice view has a dynamic ABI length
and no proved static extent specialization. Consequently a full successful
write-request result is NOT currently reachable: it rejects
`UnsupportedDynamicExtent` (or an earlier unsupported source condition).
Even with a static shape, predicated/partial/frame writes reject explicitly.
This checkpoint neither fixes `MissingWriteContracts` by itself nor claims fill
is admitted. Static launch geometry alone cannot establish output length,
ownership, noalias, coordinate coverage, or preservation of unwritten elements.

The parent must supply an independently authenticated final extent/view and
effect correspondence before extending this subset. At future attachment,
revalidate the same source context/epoch/live export and final canonical/epoch;
`require_exact` on these inert facts is a subject comparison, not live graph
validation. Establish final effect/ownership proofs, retain the original CPU
root, bind actual final graph values, attach contracts, then seal and run the
common mandatory scheduler. No boolean gate waiver or source/final receipt
scope substitution is provided here.

## Focused Central Tests

- Analysis filter: `canonical_ranked_view_v1::tests::preflight` (4 tests).
- Compiler filter: `final_write_contract_requests_v1::tests` (13 tests).
- Re-run the existing canonical ranked view suite to retain all write gates.

The request tests reuse the real neutral ranked fixture to reject absent
functional proof custody. A separate real source-owner fixture checks an ignored
source argument and final SSA renumbering. Neither fabricates proof receipts.
Positive tests cover inert preflight, parameter conversion, exact output joins,
and typed operations only, not successful final write admission.

Local verification is limited to rustfmt parsing/checks and whitespace review;
Cargo and execution tests remain parent-owned.
