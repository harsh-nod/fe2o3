# Mounted Borrowed KIR Checkpoint

All 80 shared edits across 20 files were mounted with `apply_patch` after exact
anchor validation and `git apply --check`. The parent approved the batch.
Parent Lowerer21: compilation passed, 230 tests passed and five existing
loaded-value/write-contract cases failed; all nine borrowed graph tests and both
Context source-carrier tests passed. Parent KIR21b: all 136 library tests passed,
including six borrowed tests and six parent-owned source-location tests.
Compiler21 and the actual Workgroup full-import/lowering callback are pending.
The catalog extension described below is complete but has not been Cargo-tested.
Parent previously reported Pliron19b 203/203, including eight borrowed tests.
No Cargo, SSH, network, compiler build or test execution was performed here.

## Durable Files

- `integration.edits.json`: historical snapshot of the 80 mounted shared edits.
- `integration.patch`: mounted patch snapshot, regenerated against execution-source
  uniqueness child, verifier field and insertion hook. All 80 anchors remain
  unique in the pre-mount baseline. Do not reapply this snapshot.
- `check_staged.mjs`: pre-mount anchor validation and unified patch generation.
  Default checks neither compile nor tests. Optional `--parse` invokes rustfmt
  on in-memory overlays; this attempt timed out without a syntax result. Direct
  rustfmt syntax checks of both catalog files passed after the mount.
- Lowerer children `transport.rs`, `dispatch.rs`: mounted.
- KIR children `execution_capability_v1/borrowed_subgroup.rs`, its `tests.rs`,
  and `verify/borrowed_subgroup.rs`: mounted and covered by parent KIR21b.
- Real AMD regression child:
  `rustc-codegen-fe2o3/src/collector/production_importer_v1/subgroup_partition_v1/workgroup_source_v1/canonical_transport_v1/lowering_tests.rs`.
  Mounted through the existing real AMD import-test hook; compiler21 pending.

The patch references these existing new children; it is not a standalone bundle
for another worktree. The actual callback remains mandatory before claiming
successful imported Workgroup lowering through AMD and simulator consumers.

## Representation And Dispatch

Operation tag25 / contract revision3:
`SubgroupDeriveBorrowed { workgroup_reference, workgroup, subgroup, width }`.
All identities are `ExecutionTypeIdentityV1`; width is `u32`, currently exact64.
Source ABI remains `[workgroup_reference] -> subgroup`. Logical operand is the
already lowered, exact owned Workgroup SSA `ValueId`, never a type-selected value.
New role tag15 / type revision3:
`BorrowedSubgroup { workgroup_reference, workgroup, width }`.
Verifier operand contract is `WorkgroupSharedBorrow { workgroup }`.
Target, dynamic-workgroup and lifetime obligations remain exact.

Lagrange codec5 occurrence fields are unchanged. Expanded borrowed calls encode
with revision5, retaining the original source function/block/operation, whole
expansion identity, root view identity, caller instance and expanded block.
Legacy V1/V2/V4 encoding branches and tags retain their existing semantics.
Math operation26 and MIR V22 tag80 are untouched. Pascal should preserve KIR
operation25 and role15 while integrating the separate WGIndex conversion.

The shared patch constructs `BorrowedWorkgroupPlanV1` from the replayed owner and
authenticated root before SSA transport planning, then mounts the child dispatch
only after Lagrange's `source_for_call` admission and exact source ABI checks.
Epoch projection returns the existing owned SSA binding only for the replayed
defined getter occurrence. Source semantic reference types remain separate from
logical transport types. Original source bodies and their Use/Define/Kill events
are not rewritten by production lowering.

Private, nonserializable receipts retain live owned storage loans through
subgroup and partition derive/reduce/broadcast. Recorded legacy derives remain
distinct from absent receipts; an absent borrowed receipt rejects. Exact KIR
issuer equality `subgroup_issuer.operands == [epoch_workgroup]` remains enforced.
Other borrowed consumers without a checked lifetime adapter still reject.
The backend and simulator validate the canonical graph before logical token
erasure and retain partition operand/provenance/epoch checks. This does not grant
machine, artifact, load or launch authority; final proof gaps are not waived.

## Pauli Coordination

The patch removes the duplicate Workgroup Graph/Site/Loan implementation and
imports the already-mounted common graph. It uses `use_value`, `definition`,
`incoming`, `reaches`, `charge`, and `loan_live` directly. Loan fields are
`borrow`, `owner_local`, `owner_value`; the consumer is an exact terminator site.
No common API widening is needed for this bounded implementation.

Primary review budget correction: the borrowed plan now receives the unchanged
`max_operations`, matching the existing Math plan and SSA analysis budget
threading. The proposed `128x` multiplier and separate cap were removed; neither
was an established budget. Exhaustion uses the shared graph's `AnalysisWork`
resource error. The graph boundary test checks its exact supplied limit.

All current constructor callers are included: three `SemanticControlFlowSsaPlanV1::analyze`
calls (production constructor, parent in-file unit test, transport test at986)
and three `new_interprocedural` calls (production, test wrapper, frame helper
at355). Existing Math arguments, including the parent's recent `None` repair,
are preserved; the new borrowed-plan argument is appended separately.

PLIRON Workgroup transparency admits only exact borrowed execution terminals,
closed forwarding/forks and replay-recorded epoch getters. It does not authorize
generic defined `&KernelContext` helper arguments. The reported Lagrange helper
failure is at that separate boundary. Preserve source replay and closed-use
validation when adding the generic Context transfer; do not mark references
transparent simply because their pointee is compiler-issued.

## Tests And Remaining Boundaries

Six mounted KIR tests cover the new codec/role, unchanged legacy operations,
same-typed different SSA owners, lifetime/signature/role loss, changed or missing
expanded source custody, and dominating stale-epoch transitions. Existing nine
graph tests are ported to Pauli's graph without deleting their assertions.

The mounted real AMD callback extension retains the current macro/session
registration and source. It derives typed descriptors from the live collection,
replays semantic/SSA owners, runs ranked and reference checks, transfers actual
authenticated contexts, and invokes production ranked-roster KIR lowering.
It checks borrowed source ABI/role/occurrence, backend gfx950 lowering, simulator
projection source coordinates, and simulation of all64 invocations. Negatives
mutate this actual output's issuer, epoch, occurrence and lifetime obligation.
A negative-only source mutation kills the exact observed borrowed owner's
storage before both partition calls; no positive authority is synthesized.
Compiler21 is compiling the callback; no actual-run result is available yet.

Known bounded refusals remain: multiple definitions per local in one block,
cyclic loans, unsupported projections/escapes, and distinct KIR block parameters
for an otherwise equal owner. No equality or lifetime gate is removed for these.

The parent-assigned catalog gap is now repaired in
`fe2o3-kernel-ir/tests/execution_capability_catalog_v13.rs` and its new
`execution_capability_catalog_v13/extended.rs` child. The original 23 cases remain
in order with their revision1/tag/u16-tail assertions. The expanded exact roster
contains 27 families and 43 cases: policy issuance, all three partition variants,
borrowed subgroup, Math derivation/binding and all 13 FP32 consumers. Each new case
uses explicit issuer chains and the existing semantic, SSA-shape, capability,
effect and canonical round-trip tests. Revision2/3/4 tags and role revisions are
asserted independently. No wildcard, ignore or assertion weakening was added.

Parent-owned KIR source uniqueness is now mounted in `verify.rs` and
`verify/execution_source.rs` with six tests now passed in parent KIR21b. Legacy keys remain
`(source.operation, source.block)`; expanded keys also retain caller instance and
expanded block, under one root/expansion/view and original/expanded mode per
function. This batch preserves that child, field and insertion hook unchanged.
Source authentication remains the producer's replay obligation; inert occurrence
records do not grant it. No codec5 field changes or fabricated roster are added.

Parent test selectors:
`borrowed_subgroup::tests`, `borrowed_workgroup_01::tests`, Pauli shared graph and
Math resolver suites, `--test execution_capability_catalog_v13`, and the existing exact ignored
`...canonical_transport_v1::import_tests::workgroup_full_import_gfx950_v20`
using the parent's cached real AMD metadata environment. No new ignore is added.
