# Math SSA Integration Coordination

## Mounted Bridge Checkpoint

MIR is unchanged. The original getter/bridge result is now consumed by the
production lowerer, not merely recorded in an inert adapter. Parent Compiler21c
confirmed both actual AMD source-to-SSA callbacks pass. New actual source-to-KIR
callbacks are mounted but have not been run; no launch qualification is claimed.

Seven focused tests passed using a small standalone rustc test harness linked
against parent-built lowerer `5ade74b2fcfd7b3b`, Pliron `aad8ffcb06862ad9`, MIR
`3e1566b66a32d8da`, and KIR `fbb962ba262c081f`. These are component tests, including
actual SSA-to-KIR lowering and equivalence replay for copy/move/reborrow chains.
The synthetic fixture's four variants separately passed admission and expansion.
No Cargo, SSH, network, or dependency rebuild was used by this worker.

The bridge consumes the sealed `defined_math_results()` row, the original
getter formal receiver SSA, and the retained unbranded Current SSA result. It
checks the six Use/Use/Define/Use/Kill/Define events, consumes both exact pending
definitions, and retains a private receipt without inventing a branded KIR
value. Only the original getter ReturnTransfer consumes that receipt and emits
MathDerive with the checked defined-statement source. Missing/repeated results,
foreign instances, changed receiver/current, missing events, and arbitrary
receipt joins reject. The final function check requires every receipt consumed.

The existing shared graph resolves all reference flows and checks their loans.
The lowerer now consumes its checked copy/move/reborrow assignments and compares
actual receiver SSA bindings to canonical issuer bindings. Whole-local moves
still clear local availability. Bind and F32 never select authority by type.

The Pliron shared classifier has closed Math additions: exact replayed Bind
aggregate sinks and actual PolicyMathF32 shared receivers. Source Use/Define/Kill
events and all later owner/lifetime checks remain unchanged. Parent's explicit
`SemanticDirectCallV1` import fix in `math_borrows_v1.rs` is preserved.

## Ready Files

- Lowerer `numerical_policy_math_01.rs`: mounted bridge and four focused tests.
- Lowerer `numerical_policy_math_01/{production,custody,bridge,bridge_events}.rs`.
- Lowerer `numerical_policy_math_01/{ssa_fixture,ssa_tests}.rs`.
- Lowerer parent: only the checked receipt block-entry and final-consumption hooks.
- Pliron adapter `math_borrows_v1.rs` and `math_borrows_v1/tests.rs` (three tests).
- Pliron `adapter.rs` module mount and `adapter/borrowed_workgroup_v1.rs` exact
  sink/receiver delegation. Ram's existing epoch/Context/global hooks retained.
- Compiler Math `import_tests.rs` opt-in mount and `import_tests/lowering.rs`.

Central filters: `numerical_policy_math_ssa_tests` (4), `math_borrows_v1::tests`
(3), existing `numerical_policy_math_lowering_tests` (11), shared graph and
borrowed Workgroup tests. Actual AMD filters are
`math::import_tests::lowering::policy_math_all13_source_kir_gfx942` and
`math::import_tests::lowering::policy_math_all13_source_kir_gfx950` (ignored,
cached FE2O3_CORE_TRY_* metadata). They use the collected descriptor root roster,
inventory, target, reference effects, full ranked verification, checked context
inputs and production lowering. No hand-constructed root names replace those
gates. They also mutate actual output source occurrence, operand identities and
mandatory lifetime obligations and require canonical rejection.

## Shared Boundary

Ram: `capability_ssa_graph_01/INTEGRATION.md` defines the mounted shared graph API.
Math imports it and its duplicate graph file was removed. Replace the private
Workgroup Graph/Site/Loan implementation with those imports when coherent;
Workgroup's closed issuer/epoch validation stays in its child. The shared graph
adds ordered source-site liveness for Bind/getter statement consumers. Reuse of
local IDs/types alone is not sufficient.
The Math plan consumes `defined_capability_bindings(source)` from the replayed
owner, not caller-supplied recipes. Shared-reference paths cover getter arg0,
Bind args0/1, F32 arg0 and checked parameter/return transfers.

The resolved pre-lowering SSA issue in the exact validated Math bridge was:
its branded ZST return local is not written by optimized original Rust. The
frozen expansion emits a Move from that non-Unit return local. Current ambient
frame initialization intentionally accepts only WorkgroupLdsScope. Math must
not enter that ambient rule. Any new defined-result/transfer relation must bind
the exact getter/bridge/Current occurrences AND retained context receiver;
neither a ZST constant nor bare MathContextCurrent can issue branded Math. The
mounted sealed result relation and receipt consumption implement that boundary.

Lagrange: the mounted Math producer planner calls your separate
`source_for_defined_statement` factory at the exact getter ReturnTransfer and
the Bind source aggregate assignment. F32 uses `source_for_call`. No Defined
constructor is represented by a fabricated intrinsic call. Both source digests,
original caller coordinates and the checked callee identity remain attributed.

Parent/Ram: `lower_one_semantic_function_v1` now builds `MathSsaLoweringPlanV1`
from the existing owner/function/context only when a PolicyMathF32 call exists.
Its signature is unchanged. The internal lowering constructor accepts the plan
alongside Ram's borrowed Workgroup plan; `analyze` accepts its checked local
transport map alongside that plan. There is no global Math-by-type issuer registry. Coordinate any
additional root-plan inputs rather than replacing these hooks.

## Math-Owned Work

The bounded Math resolver follows actual SSA definitions through shared
borrows/reborrows and Copy/Move/parameter/return forwarding. A Bound origin must
be the exact checked Bind assignment; its two referents must originate from a
checked getter result and NumericalPolicyIssue using the SAME context definition.
All incoming paths must agree on original issuer and nominal binding; loops that
cannot establish one issuer reject. Storage death, deinitialization, overwrites,
mutable/raw/fake borrows, fabricated constants and conflicting bindings reject.

Emission reuses the existing typed MathDerive/Bind/F32 adapter. KIR operands
must be the original lowered issuer ValueIds, not equivalent-looking block
parameters or a value selected by type. KIR continues checking producer dominance
and exact shared context. Numerical/target/lifetime obligations remain open.

## Checkpoint Boundaries

Actual source-to-KIR callbacks remain pending. Same-owner joins that lower to
distinct KIR block parameters still reject until exact canonical-issuer transport
is established. Unused getters with no checked FP32 consumer are not ambient
issuers. No ambient Math frame initialization was added and no checker was waived.

Central test filters: `numerical_policy_math_lowering_tests` (8 existing adapter
tests plus 3 new transport tests), `capability_ssa_graph_01::tests` (5 new), and
Ram's `borrowed_workgroup_01::tests` after the graph import switch. Then run an
actual source owner through SSA and KIR with getter/Bind, copies, moves,
reborrows, same-typed owner substitutions, lifetime kills and call-instance
mutations. Adapter/graph tests alone are not source-to-KIR success evidence.
