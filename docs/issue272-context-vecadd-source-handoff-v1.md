# Context Vecadd Source Handoff V1

Source-only increment based on `f2dd128e35d5345cf4b7f6fb3f71480ee483c961`,
2026-09-29. Authoring performed no builds or execution; the primary's subsequent
validation is recorded below. M1 remains incomplete; this fixture does not migrate
or qualify the manifest vecadd. M0 acceptance and the 0/47 strict-production
GPU baseline are unchanged.

## Fixture And Checks

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

The following tests are execution-dependent and ignored by default. Their
completed guarded runs are recorded below:

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
sessions per parent, 32 total. All 32 passed in `r46-context-vecadd-source`.
The harness also requires the per-invocation compiler output directory to stay
empty. Recording branches and assertion attachments does not prove dominance,
assertion truth, exact source pointer provenance or source-to-graph value
equivalence. Counts alone do not identify which input bound an assertion covers.
The fixed observation bounds deliberately reject excessive inventories.

Primary-owned execution commands, run serially with the pinned toolchain:

```sh
cargo test -p rustc-codegen-fe2o3 --lib actual_context_vecadd_ -- --ignored --nocapture
cargo test -p rustc-codegen-fe2o3 --test production_ranked_bounds_driver_v1 context_vecadd_normal_exports_require_checked_materialization -- --ignored --nocapture
cargo test -p rustc-codegen-fe2o3 --test production_ranked_bounds_driver_v1 kernel_context_source_protocol_rejects_without_export_authority -- --ignored --nocapture
```

Rebuild the authentic device dependencies when running these commands: the
provider's documentation-only correction changes its source closure fingerprint.
Do not reuse provider receipts from the unmodified source tree.

## Primary Validation

The first guarded run, `r39-context-vecadd-source`, built the compiler library
but failed both parent tests. All 32 actual source sessions stopped at
`FE2O3-CAP-AUTH001`: the logical context provider was not authentic. Neither
requested observation was reached. Source and tool hashes stayed unchanged;
no protected proof or GPU execution occurred.

The documentation change in `fe2o3-device/src/context.rs` had not refreshed the
two exact reviewed package hashes. The existing
`safe_execution_source_closure_matches_the_reviewed_pin` regression reproduced
the mismatch in guarded run `r40-provider-pin-repro`. Independent raw-byte
hashing also reproduced both previous pins from the unmodified base and the
two new canonical/vendor hashes from the changed source.

The correction refreshes only those two accepted identities, retains comment
bytes in authentication, adds context-source mutation controls and preserves
the underlying provider refusal reason in the context diagnostic. All 54 provider
tests then passed in `r41-provider-pin-controls`. The next actual-source run,
`r42-context-vecadd-source`, passed provider authentication but all 32 sessions
stopped at the generated wrapper's identity-reference reborrows. Neither failure
was accepted as satisfying a downstream test.

The flow checker now admits only whole-reference `&*arg` / `&mut *arg` transport
of a live original physical argument before the helper call. The destination
and base MIR types must normalize in the actual root instance to the original
physical reference type, including mutability. Projected fields, raw pointers,
mutability changes, special loans, dead locals and context/result/unit origins
remain rejected. Original argument ordinals and helper order are preserved.
This does not change the separate execution-capability borrow rules.

The first build of that repair, `r43-context-entry-reborrow-controls`, exposed an
unsupported pinned-rustc API and ran no tests. The repair uses the root instance's
fallible MIR instantiation/normalization API. `r44` was interrupted and has no
terminal report or test credit. Its process was confirmed absent before `r45`
started. Completed validation on 2026-09-29/30 UTC:

| Run | Result | Scope |
| --- | --- | --- |
| r45-context-entry-reborrow-controls | 8 passed | Pure origin, borrow-kind and existing flow controls. |
| r46-context-vecadd-source | 2 parent tests, 32 source sessions passed | Actual unchanged shared body, both targets, four optimization pairs, duplicate fresh processes. |
| r47-context-flow-provider | 62 passed | 54 provider tests plus all 8 flow controls; not 62 additional unique tests beyond those groups. |
| r48-context-driver-build | Build passed | Normal exporter/extractor and integration driver; no tests executed. |
| r49-context-source-exports | Partial; command timed out with status 124 | Normal vecadd test passed all four gfx942/gfx950 simulation/LLVM refusal requests; the following source-protocol test did not finish. |
| r50-context-source-protocol | Passed independently | All 30 protocol cases and 14 additional LLVM requests; exact diagnostic stages and absent output files checked. |

The protocol suite retains all 21 previous cases and adds nine reference
transport/substitution cases. Its valid handwritten declarations intentionally
reach the missing typed-binding refusal; they are not successful exports. Both
real vecadd parents reach their required observers and terminal refusals. The
normal vecadd export test reaches the checked-materialization refusal, not an
earlier authentication failure. No run executes a proof or GPU.

Runs r46-r50 share the unchanged Git-visible source snapshot
`b4dcbbccb85c6e9e4a8a97c430a46dd3585cb9520e69ae4edfddca3ad16c1b9a`.
The runner also verifies unchanged compiler/tool hashes before and after each
run. Tests use locked/offline nightly `2026-04-03`, one Cargo job, serial tests,
disabled GPU visibility, a 12 GiB process-memory ceiling and a 1,200-second
deadline. The timed-out r49 protocol scratch was removed only after its
processes were confirmed absent. r46/r50 removed their own private scratch.

Local reports and logs are in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory. Key log SHA-256:

| Run | Log SHA-256 |
| --- | --- |
| r46 | `9b8b2efa320234bc5b318b37c437cf8200fca51804881515ca4434edd85fd8a8` |
| r47 | `a4f6cea2068ad9bd3317e4d9cfb65d51359ba393a57d854ef6c54ebf8294cfe6` |
| r49 | `8ce017f55c76b3e76bbb2159f4dc4e982aae3bdaf89134c2af50b40c6ce7ded6` |
| r50 | `1c197ce03224e3c0032c414057026dc8178148e692af75cb6bf45b7f0be76058` |

These results establish source admission and diagnostic observations, not an
executable context-bearing production continuation. M1 remains incomplete and
the strict protected production-to-safe-GPU matrix remains 0/47.

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

Earlier #271 coordination on 2026-09-29:
[owner handoff 5894692188](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5894692188).
It reports an unpublished optimized execution recipe census at
`012ccbf23b304ebf924f48e9f1bdd7fa73e678d6` handling ContextIssue, WorkgroupDerive
and ScopeEnd under retained original/optimized source owners. This is an owner
report, not independently inspected or qualified code in this patch.

The subsequent owner candidate
`a566403b7125cbef41a4a2230cdd74dece3c796f` on
`wip/issue271-v1798-mixed-production` was fetched and its relevant boundaries
independently inspected, not merged or executed here. It contains execution
recipe support, so another independent recipe producer is not required. Two
concrete consumer gaps remain in that candidate:

- `with_original_source_conditional_mixed_pure_cse_v26` selects the nominal
  pointer-sized V35 import profile, while context-bearing import selects exact
  MIR V29. The V35 validator rejects schemas newer than V15; non-V35 schemas
  reject nominal pointer-sized types. The owner must provide a compatible,
  preselected contract in the existing source-owned custody policy, without
  retrying a weaker profile or relabeling a schema.
- `store_source_expression_v23` in
  `production_source_private_expression_v22.rs` accepts only fixed-width integer
  stores. The original-to-optimized expression correspondence must support the
  actual f32 loads/add/store, including operand origins and floating-point
  semantics. Removing the scalar guard alone would not establish that relation.

Reuse the owner's source-owned pipeline and target-lowering consumers. Obtain
its reviewed integration revision and tests before integration; do not import
the WIP wholesale. Compilation of a candidate or diagnostic observation does
not establish its finalization, proof, simulator or GPU readiness.

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

## Shared Context And Nominal Types Candidate

The private M1 candidate adds an explicit semantic-MIR V40 composition for
ordinary operations, RustCall locals, execution-role records and nominal
`usize`/`isize`. V29 cannot encode nominal pointer-sized types; V35 deliberately
excludes execution roles and RustCall locals. Selecting the larger version number
or erasing nominality would not resolve that incompatibility.

V40 reuses the existing validators and encoding fields with closed feature
membership. The legacy admission defaults, V29/V35 bytes and specialized sibling
grammars remain unchanged. Decoding these inert records does not authenticate a
Rust producer or create a capability. Twelve focused tests cover the four
context/nominal combinations, mixed roots/helpers, RustCall, representation and
role mutations, exact resource boundaries and incompatible-family refusal.
The isolated full MIR-model library run `r69-isolated-context-nominal-mir` passed
all 382 tests, including these twelve, at `0f113f634ad4b438d588a6e967902780126adc66`.
It used pinned nightly `2026-04-03`, locked/offline dependencies and a fresh
worktree-specific target directory. Source/tool hashes were unchanged; log
SHA-256 is `6eb57bd85c902c1abe60980c383d7cbb062eedae0595e4630b6733f678002e3e`.
These are inert-model tests, not ordinary-Rust or executable-path validation.

Allocation is provisional under the
[#271 coordination scope](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5902088576).
The live importer and checked source-owned continuation still need to consume
this contract with their original source, ABI and execution custody intact.
This schema prerequisite does not complete M1 or advance the 0/47 matrix.

### Isolated Consumer Candidate Checks

The separate source-consumer candidate at
`13b5c31a8a142a9f44bd4e8ac2158a39ebf4889f` composes the exact V40 importer,
Policy9/10 selections, context census, f32 correspondence and strict context
reborrow changes on the reviewed #271 dependency snapshot. This is not yet
integrated into the primary checkpoint or the complete public96 descendant set.

The first lowerer run, r74, passed one of five tests and rejected the other four
during fixture admission: the newly appended nominal types were not reachable
from a root. The fixture now declares root-local temporaries of those types,
preserving the original function identity, ABI, blocks and kernel export.
Production type-closure validation is unchanged.

`r76-isolated-context-and-f32` passed all 19 selected tests: five V40 census
controls and fourteen f32/integer source-correspondence controls. The census
checks cover ordinary/mixed roots, nominal roles, source/launch substitution,
missing or altered context inputs, and exact/one-short resource accounting.
They are inert census coverage, not authenticated Rust source execution.

The source snapshot SHA-256 is
`f871fb9d5406c87b67fb6995851bc0108c0e06b43fe721bacb18233f5417f950`;
the log SHA-256 is
`cb11d22b33c87256519205800b81a41e006dbc69c4e7ccc0a23f90be491c264f`.
Before/after source and tool snapshots were identical in the dedicated worktree
target. Provider/backend and actual-Rust parent tests remain separate gates.
No protected proof, GPU execution or new milestone completion is claimed.

The subsequent `r77-isolated-v40-backend-controls` built the actual rustc backend
at the same candidate revision and passed all 53 selected provider and observer
controls, including the composed canonical/vendor pins and the false-edge guard
control. The pinned AMD LLVM `opt` was `22.0.0git`, SHA-256
`13cb4c99d1810b4db40bca5db0759ca94c8efd3c437bb8f7fe1a94f5bda66203`.
The log SHA-256 is
`7868ca0f1471f456db254485d4a587da0298dd305271038b2ed08fa09fbf9f6a`.

`r78-actual-context-nominal-pure-cse` then ran the real Rust Policy10
context-plus-nominal vecadd parent. All sixteen source sessions failed at
`FE2O3-CAP-AUTH001`, reporting that pre-optimization context-producer MIR was
already unavailable. No intended positive continuation was observed. The test
requested successive transactions in one rustc session; context capture must
precede queries that consume that MIR and its authenticated records are
move-only. The correction must isolate test transactions, not relax that guard
or clone authenticated context evidence. Both target dependency trees and the
parent scratch directory were removed by the harness after failure.

The r78 log SHA-256 is
`7f7b0faa9be8b166c4c032c0441dd437b88d685ea77d3bc873e4315ac15cf145`.
Both runs retained the same source/tool snapshots, with no protected proof,
compiled kernel artifacts or GPU execution. The actual-source gate remains
failed; the 53 control passes do not substitute for it.

`r79-actual-ordinary-v40-pure-cse` separately ran the ordinary, no-context
Policy10 parent at the same candidate revision. All sixteen source sessions
failed during macro validation: `the typed vecadd V2 profile does not support
max_grid`. The public macro still selected the legacy exact-signature validator
when no logical context was present, although typed expansion already used the
generic signature/launch model. This is a production validation inconsistency,
not the context-capture defect or a reason to change the kernel's launch contract.
The correction must make both entry forms use the same generic validator.

The r79 log SHA-256 is
`2e1a3e8de89f3081d67f673fd02f08b804a21ab214fd9dde00d88cb9c8159a4b`.
Source/tool snapshots remained unchanged. The harness checked empty compiler
output directories and removed its dependency trees and parent scratch on
failure. No positive continuation, protected proof or GPU run is credited.

### Generic Macro Admission Correction

The three-file correction `54585b104` uses the existing generic signature and
launch model after removing any logical context from the physical signature.
It no longer selects the legacy three-argument vecadd validator for context-free
entries. The four historical exact-profile helper functions are test-only;
there is no new production selector or change to compiler/evidence authority.
Independent review found no blocking issue.

The source-consumer candidate's `r81-generic-macro-admission` passed all 76 macro
library tests. The new regression covers both context forms, two names, 64/256
thread blocks and finite grid bounds. Invalid block size, inconsistent dimensions,
non-rank-one grid, missing required dimensions and unsupported occupancy still
reject through the generic model. The preceding r80 run passed 75 tests and
failed one new negative fixture at parser setup; adding its required `max`
declaration restored the intended occupancy rejection test without changing
production parsing.

The passing source snapshot SHA-256 is
`3841adc5b0ad4e259b7315aa93db675f14f3f8ee98d5ea272b6adccc4d3b66c9`;
the log SHA-256 is
`f8e6023fa3b0dbb0456ebf5e331fa6764a2a926d074307724dfad600ed58e317`.
This result predates primary-worktree revalidation and the actual-source rerun.
The logical `_ctx` fixture still covers issuance/ABI only, not context-derived
indexing or complete typed global-memory capabilities.

Primary-worktree revalidation `r83-primary-generic-macro-admission` also passed
all 76 macro tests at `d7de27f0bdcff84413680d3b70da5bfc7375d079`, with unchanged
source/tool snapshots. Log SHA-256:
`53c0b9277d77d609142be9d2b053505a8d3fc74afebc45e4be87f509132df929`.

### Fresh-Session Source Tests

Candidate `4cc0a29a78aa3a019ff94b4d1713ea65b5ec5742` splits positive, foreign V29,
equal-content foreign V40, incomplete ABI and consumer-refusal checks into
separate fresh compiler children. Each captures genuine context producers once.
Foreign owners are constructed only from public inert data, never cloned
authenticated captures. Negatives require the intended consumer and exact outer
error; they cannot turn an earlier pipeline failure into a pass. Existing
graph-observation checks were extracted without behavioral changes.

`r82-fresh-mode-backend-controls` passed all 54 selected provider and harness
controls. Source snapshot SHA-256:
`22d45974bd926a1cd837e0f029535c3aff0812ffd0bb0cfb0987ee6f59ba5287`.
Log SHA-256:
`5545f4dc5484f5043387ad0a5b30f773483f67f905849d584754c34cb4c6740d`.

The actual Policy10 context-plus-nominal parent, `r84`, then ran all 80 fresh
sessions: five modes, two targets, four optimization pairs and two repeats.
Every session failed before its consumer with `WireVersionCannotRepresent {
requested: V29, required: V35 }`. Function/declaration commitments and the scope
census still fixed V29 despite the importer's explicit V40 selection. This
failure is not successful negative coverage. The source/tool snapshots remained
unchanged and the harness removed its temporary dependency trees and scratch.
Log SHA-256:
`a406a56cdf75318cd400b7a1b45860eb6a74dada7f035dca9b3d5c9f89e03776`.

The narrow schema-preservation correction is coordinated in
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5903143076).
It must bind the selected schema before capture and require exact agreement at
sealing, not erase nominal types or relax old-schema membership. Actual source
continuation, proof, simulator and hardware acceptance remain outstanding.

The correction is integrated in the separate source candidate at
`d83cca77dc6f90cc6b740837e0ccb7a707025fe4`. Its exact schema is retained from
capture through function/declaration commitments and scope census; the frozen
V29 encoding and default admission remain unchanged. Independent review found
no blocking issue. `r86-context-schema-custody-controls` passed all 68 selected
backend tests, including cross-schema sealing, body mutation, nominal declaration
preservation, and exact census-schema binding. Source/tool snapshots were stable;
log SHA-256:
`0683ad8914b3e4d659a2ccf637122849a46b9fc2d7ca06e9e3263c42e9e2ac76`.
This does not replace the failed r84 actual-source matrix or qualify the primary
worktree's different dependency set. The actual-source rerun is a separate gate.

`r87-actual-context-nominal-schema-custody` then completed the same 80 fresh
source sessions at that revision. The schema refusal was eliminated, but every
session stopped before its consumer with `source reference intrinsic effect is
not represented`. Source/tool snapshots were stable and temporary dependency
trees were removed. Log SHA-256:
`03d4a1e2ec65efb6b4da0eebb78e7963b11c39075d4c966e5558f5a5f9799620`.
The existing #271 owner's index-witness/reference-effect repair is the next
dependency; it is not duplicated or bypassed here. The
[handoff](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5903321856)
requests an immutable validated dependency before integration. No intended
positive or negative consumer, proof, simulator or GPU result is credited.

The separate candidate `b072d24b731ed0766712b1ee7f5d0c0d90c51158` additionally
requires reached callbacks and exact ABI errors in the Policy9 negatives. Its
foreign-account negative now matches the original storage limit/reservation,
so underfunding cannot substitute for account-identity rejection. Both accounts'
work/live/peak storage are checked unchanged around that query. Independent
review found no remaining issue. `r90-policy9-rejection-boundary-controls`
compiled those tests and passed the same 68 selected backend controls, with
stable source/tool snapshots; log SHA-256:
`24a584572a5e1cd95424726edcc65adb0a1e5e78e041105f3d7f9f86d486b2e2`.
The hardened ordinary-source negatives are not executed by that selection;
they remain dependent on the actual-source gate rather than earning pass credit.
