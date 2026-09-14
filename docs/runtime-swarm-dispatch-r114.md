# Runtime Swarm Dispatch After R114

R115 progression: [lower returning-control cleanup](runtime-returning-control-cleanup-v1.md)
is now locally accepted with [285 retained artifacts](evidence/local-r115-returning-control-cleanup-2026-09-14/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,727 tests with
five ignored; all 17 source gates, ten auxiliary checks, 15/37/7/9 restored
suites and 16 compiled negatives pass. This accepts only the lower returning
subset, not persistent/data cleanup, live retake, queue teardown, native,
formal or performance qualification. Native next takes the
[detached-persistent handoff](runtime-detached-persistent-control-cleanup-v1.md).

R114 locally accepts [pristine control cleanup and terminal parent transport](runtime-pristine-control-cleanup-v1.md).
The [254-artifact evidence packet](evidence/local-r114-pristine-control-cleanup-2026-09-13/README.md)
passes independent review. This supersedes the [candidate dispatch](runtime-swarm-dispatch-r114-candidate.md),
not the retained contracts and history in the [complete packet map](runtime-swarm-next-packets.md).
Issue [#182](https://github.com/harsh-nod/fe2o3/issues/182) remains open;
A1/A2 and full HIP/HSA parity are not complete.

The original work breakdown was refreshed against accepted source
`9eaf19141e8af6ade490feba3062c8b49d9b38ca` and the open issues on 2026-09-13.
The R115 progression above updates the current assignments; the original
R114 boundary and handoff details remain historical evidence.

## Historical R114 Boundary

GNU and musl each pass 2,712 tests with five ignored across 48 libtest harnesses
and the unchanged harnessless CSV benchmark. All 17 source gates, ten auxiliary
gates, frozen/restored 37/7/9 focused rosters and 15 compiled behavioral negatives
pass. Nine runner, 30 freeze and 80 qualification-contract tests pass. All 5,683
source identities are restored, and the closed collector and both independent
archive reviews pass.

The full GNU/musl runs are separately recorded pre-freeze prerequisites with
matching source endpoints, not executions inside the later fifteen-gate batch.
Preliminary failures, formatter changes and helper-review history are retained.
An earlier unwrapped formatter failure lacks saved inventories and is not
reconstructed as evidence. No live GPU, authenticated proof or performance
acceptance is added. Ordinary/returning cleanup and data cleanup remain open.

## Assigned Lanes

These three read-only workers completed their reviews. Their following queues
are assigned work, not unattended running jobs. Primary owns edits, integration,
serialized builds, proof/hardware scheduling and signed dual-remote publication.

| Owner | Immediate Packet | Following Queue |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N4-R2 detached persistent-control cleanup above locally accepted R115 | Persistent returned-data/ordinary bridges, data cleanup, applicable live/queue teardown, then N5 DATA-ADOPT |
| Admission: `submission_identity_handoff` | Qualify and integrate isolated C1; C2/C3 coverage can advance independently | Joint I2 ISSUE, C4 COMPLETE, C5 typed future/join, C6 generated GRAPH/DRAIN |
| Resources: `r102_evidence_review` | Integrate and qualify the isolated V3 settlement model/cost candidate | V4 proofs, V5/V6 production journal/hooks, V7 writers/recovery, V8 leases; M1-M4 retained-memory bounds |
| Primary | Integrate one reviewed packet at a time; own Q1/Q2/Q3 and release gate #277 | Coordinate A3-A7 and publish only the evidence actually obtained |

### Ready Now

The three worker slots run read-only reviews in parallel; Primary is the fourth
slot and owns edits under the current ownership policy. Native supplies the
cleanup design, Admission supplies identity/lifecycle oracles and cross-reviews
Native's tests, and Resources supplies settlement/accounting contracts.

| Lane | Independent Start | Does Not Yet Close |
| --- | --- | --- |
| Native | N4-R2 detached persistent-control root using the reviewed handoff | Persistent returned-data bridges, data disposal, live retake, queue teardown, N5 adoption |
| Admission | Integrate and qualify the isolated C1 candidate; C2-A/B and C3-A/B can be prepared independently | Actual generated ISSUE, COMPLETE, typed output or graph execution |
| Resources | Integrate and qualify the isolated V3 settlement candidate; start V4's accepted V1/V2 properties and M1-M3 design independently | Production journal receipts, all writers, recovery, reuse or total retained-memory bound |
| Primary | Specify Q1/Q2/Q3 evidence contracts and #277's missing feature-enabled release audit | New proof, hardware, release-policy or performance acceptance |

Each handoff names source ownership, prerequisites, exact positive/rejection
oracles, retained-resource behavior, and excluded claims. Shared module wiring
and builds are serialized. Review completion does not leave implementation,
solver or hardware jobs running unattended.

## Native Packets

1. **N4-R2 detached persistent controls:** Apply the
   [reviewed handoff](runtime-detached-persistent-control-cleanup-v1.md) to
   `release_detached_persistent_control_v1` in
   `crates/fe2o3-kfd/src/queue_dispatch_binding.rs`. Root the complete owner
   before generation/state validation, preserve the separate detached data
   owner, and share kernarg-first, **forward code order** cleanup. This unit
   result needs no returned-data allocation or extraction. R115 already
   accepts the lower `release_non_data_after_recycle` and
   `release_non_data_for_returning_destroy` paths.
2. **Persistent/ordinary bridges and data cleanup:** Preserve each API's exact
   returned-data-on-error behavior. Cover every supported host/device and
   initialized/uninitialized variant, retained charges, untouched suffix and
   completed prefix. No repeated disposal or duplicate refund.
3. **N4-L/N4-Q:** Retain original parents and taken lanes across model retake,
   commit and every teardown prefix. A failed disposal cannot expose a reusable
   allocation hole or queue slot. Each route consumes its applicable lower
   cleanup contract; not every teardown mode blocks the first N5 route.
4. **N5 DATA-ADOPT:** Join the required insertion/cleanup paths into actual
   nonpublishing generated resource adoption, with identity and Stop/drain
   custody. Do not replace the existing memory or queue engines.

R2's decisive tests cover first/middle/last controls, unmap/free/VA-release and
currentness failures, model-commit rejection, incomplete callbacks, one-shot
retry rejection, exact returned data and pre-effect validation/capacity failure.

### Accepted R115 Lower Contract

The returning root must own the complete original dispatch before generation
validation, then validate data/premise cardinality and reserve return capacity
before any disposal. Use a retained forward iterator, not repeated front removal.
Keep completed returned data inside the root until explicit extraction. Errors,
panics and an `Ok` callback without active-control Complete retain the whole
root; retry must not invoke cleanup again.

Preserve the two generation modes: returning destruction admits never-published
generation zero; after-recycle does not. Cancelled reservation history or an
exhausted next generation alone does not prohibit cleanup. Returned leases keep
their premises; existing `into_data()` preserves initialization flags without
reviving pre-dispatch content descriptors.

Tests reuse `pristine_abort::pristine_dispatch_fixture_v1(8)` for three distinct
controls and five data variants through the accepted model-aware lower adapter.
Drive reserve/publication/completion/recycle transitions instead of assigning
generation internals. Add a real preparation fixture for populated code identity
and packet metadata; those fields are empty in the pristine fixture. Compare
native call identities/order, model changes, disposed receipts, charges, full
owner metadata and return-storage pointers/capacities independently.

N4-L must later keep this root outside the actual live model loan and retain
Complete through retake and metadata commit. N4-QP returning destruction must
also retain data through subsequent completion-signal cleanup and callbacks.
The lower R115 root cannot by itself qualify either composition. Persistent
bridges separately preserve their existing returned-data-on-error contract.

## Admission Packets

| Packet | Implementation Boundary | Required Exit |
| --- | --- | --- |
| C1 | Isolated `context/tests/submission_identity_tests.rs`, narrow Context test wiring/cancel counter | Qualify 80 rejection cells, genuine cache/backend-ID reuse, destroyed streams and precedence without replacing validators. The nine focused and 724 runtime-library passes remain preliminary until integration and full immutable packet qualification. |
| C2-A | New `authorized_execution/tests/generated_identity.rs`; existing roster matchers | Every descriptor coordinate in both directions and against its source; valid controls pass and substituted identities reject. |
| C2-B | Same module; existing generated-storage transfer fixtures | Actual one-shot transfer, immutable source validation, then artifact/authority/currentness substitutions with exact buffer custody. |
| C3-A | New `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`; existing reservation/adoption fixtures | Two-owner isolation, Stop before disposal, A-success/B-failure/C-retained retirement, exact holds/credits and no retry or second reply. |
| C3-B | Same module; existing completion cell and identifiable wakers | Only the latest correct waker fires; panicking wake cannot lose custody or settle twice. Use a test-only borrowed accessor, not a cloned consumer. |

Paths in this table are relative to `crates/fe2o3-runtime/src`. C2/C3's new files
remain absent; their subpackets are independently reviewable but shared-file
edits need one integrating owner. Do not introduce a dummy native completion
adapter. Existing ordinary async, graph/drain and decoder machinery is extended.

## Resources Packets

Integrate and qualify the isolated candidate for the
[V3 settlement contract](runtime-context-version-settlement-v1.md), frozen on
2026-09-14: Success, NoEffect and sticky Unknown. Validate the complete touched
chain before mutation; preserve burned epochs and disjoint writers. Reuse the
seven vectors with O(k) touched work, no commit-time growth and no unrelated
arena scans. Require independent reference traces, full test auditors,
rejection snapshots, replay/corruption tests, MAX exhaustion and fixed-k costs.

V4 proof work can begin from accepted V1/V2. M1 aggregate-domain design, M2
native cost inventory and M3 host-image limits are also independent preparation.
V5/V6 join the production journal and mutation hooks; V7 covers all writers and
explicit Unknown recovery; V8 adds exact input leases. M4 closes total retained
memory only after concrete owner, journal, lease and cache accounting is joined.
Model evidence does not authenticate a production completion or NoEffect receipt.

### Resources Exits

| Packet | Bounded Deliverable And Exit |
| --- | --- |
| V3 | Add settlement and public/private Unknown to the existing journal model using the frozen full-reference inert evidence and error precedence. Success advances lineage; NoEffect preserves lineage and burned epochs; Unknown retains the complete chain. |
| V4 | Authenticate named issuance/membership/settlement properties with solver results, negative mutations and actual Rust/model correspondence. Stable V1/V2 work can start before V3 completes. |
| V5/V6 | Join the production Context journal and initial mutation hooks using authentic existing IDs and private move-only tickets. Begin precedes effects; settlement precedes callbacks. A backend error is not NoEffect authority. |
| V7 | Cover every mutation family, bounded ordered writers and explicit Unknown recovery. Host-write/copy-only coverage is insufficient. |
| V8 | Add exact input range/generation leases after V7 and exclusive graph reservation. Kernel reuse additionally needs admitted compiler effects. |
| M1 | Define shared-domain ceilings and terminal headroom. Repeated Context construction cannot reset limits or refund retained owners. |
| M2 | Inventory and admit compound native backing/control/slot costs before effects; retain exact failure-prefix charges without double charging. |
| M3 | Bound retained host images; join native executable residency/eviction with M2. Live operations prevent eviction and uncertain unload retains charges. |
| M4 | Join concrete owners, journals, leases and M1-M3 into a total retained-memory bound, including staging/COW, commands, replies/results, terminal quarantine and caches. State caller-owned exclusions. |

V3 must preserve all accepted journal/membership tests and add independent
reference traces, complete rejection snapshots, replay, empty/disjoint writers,
NoEffect epoch gaps, Unknown stickiness and final-epoch settlement. Validate the
whole touched chain before writing scratch, with enough return-stack headroom.
Commit canonical member returns before the writer slot. Audit global arena/free
partitions in tests; production settlement targets O(k + 1) touched work without
unrelated scans or changes to any of the seven storage pointers/capacities.
Counted cost tests do not establish an authenticated complexity proof.

## Joins And Qualification

- Required N3/N4 paths feed N5. N5 plus C1/C2 and preissue C3 feed I2 ISSUE;
  freeze its mutation-hook policy first. C4 COMPLETE enables typed output and
  generated GRAPH/DRAIN. C6 requires ISSUE/COMPLETE, not semantically C5.
- A first non-reusing ISSUE does not wait for V7/V8. Journal-enabled execution
  needs V5/V6; cross-run reuse needs V7/V8 and exclusive graph reservation.
  Kernel reuse additionally needs admitted compiler effects.
- Q1 authenticates named proofs and actual Rust/model correspondence. Q2 adds
  native depth, pressure, teardown, bounded-memory and device-overlap evidence.
  Q3 fixes matched KFD/HIP/HSA workloads and complete-output checks before tuning.
  CPU test duration is not a runtime speedup measurement.
- Rotate the same lanes through A3 local multi-GPU, A4 authenticated two-host
  control, A5 distributed transfers/collectives, A6 fault qualification and A7
  performance/production closure. Copy-only XGMI does not close those milestones.

Concrete Worker V3/capsule and compiler semantic-to-machine authority remain
external production gates. Fixtures cannot supply their backend or owned proof
artifacts. Primary separately owns [#277's release audit/CI gap](https://github.com/harsh-nod/fe2o3/issues/277).
MI300X work stays serialized in task-owned staging with cleanup; disruptive
faults need an isolated window on the shared machine. This packet launched no
SSH, GPU or solver jobs.

### Primary Qualification Queue

| Packet | First Deliverable | Required Acceptance |
| --- | --- | --- |
| Q1 | Property/implementation correspondence map, exact proof inputs and solver budgets | Authenticated positive and negative proof results for the named executable boundary; fixtures and inventories are not proofs |
| Q2 | Configured-resource example, runner and checker using existing budgets | Exact charges and pressure rejection, retained failure custody and complete teardown on the topic binary; extend to depth, out-of-order completion, reuse and device-timeline overlap |
| Q3 | Freeze matched KFD/HIP/HSA kernels, full-output oracles, copy/residency semantics, sizes, depth, warmup, repetitions and thresholds | Reproducible latency distributions, bandwidth, throughput and CPU/memory costs; report steady-state and end-to-end results separately |
| #277 | Add GNU release, `hardware-qualification`-enabled VecAdd benchmark build and strict ELF CI coverage; resolve the supported closure without weakening policy | Independently audit the actual binary, dependencies and toolchain, including prohibited-import negatives. A feature-disabled stub, `dlsym` whitelist or external-main smoke cannot qualify this branch. |

The separate externally reported teardown fix needs review and full-readback,
explicit-teardown reproduction on this topic branch. Native correctness, strict
ELF compliance, formal evidence and matched timing remain independent gates.
No new hardware run or performance result is part of this planning refresh.
