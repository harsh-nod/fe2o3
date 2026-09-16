# Runtime Swarm Dispatch After R114

Current Native checkpoint: [R125 live prepared persistent cancellation](runtime-live-persistent-cancel-v1.md),
with [576 raw artifacts](evidence/local-r125-live-persistent-cancel-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,901 tests with
five ignored; all 25 source/auxiliary leaves, 18 compiled behavioral negatives
across 18 maps, restored 47/744 suites and 207 checker calibrations pass.
All 5,709 source identities match and all 126 recorded owned groups are absent.
This accepts cancellation custody, inherited-generation handling and the boxed
fixed ledger at the CPU/test boundary. Native, formal, aggregate-memory and
HIP/HSA performance acceptance remain open; A1/A2 and issue #182 are incomplete.

Next Native packet: retained ordinary primary-queue Release custody (R126).
The root must retain the parent plus exact event/payload/runtime/doorbell,
resource, dispatch and signal cleanup prefixes before fallible native effects.
Preserve all-four-unmap, then all-four-release, then shadow completion; keep
process-gate custody until dispatch and signals are also conclusively released.
The [R126 development](runtime-primary-queue-release-v1.md) now includes borrowed
foundation restoration, four-resource cleanup, retained Linux teardown and the
ordinary-primary runtime owner. Sixteen constructed-parent tests exercise the
shared driver, including dispatch-data and model-boundary failures. A packetless
native probe confirms completed public-root Drop on one MI300X. Remaining
parent/runtime coverage and fresh qualification stay open. R125 is still
accepted. Then connect N5
DATA-ADOPT, actual I2 ISSUE, C4/C5 completion/readback/typed replies and C6
Stop/drain/graphs. Other queue modes and cold-output profiles remain required.

Preceding Native checkpoint: [R124 ordinary recycled detach is locally accepted](runtime-live-recycled-detach-v1.md),
with [476 raw artifacts](evidence/local-r124-live-recycled-detach-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,882 tests with
five ignored; all 25 source/auxiliary leaves, their exact transcript checker,
19 compiled behavioral negatives across 19 maps, and the final 158-test restored
regression pass. All 5,706 source identities match. The explicit two-boot
qualification retains 175 historical helper cases separately from 197 fresh
calibrations, pins 16 prior-boot closure records without rescanning old PGIDs,
and observes all 91 current-boot groups absent, including the closed collector.
The interrupted first musl run remains unqualified. This accepts ordinary live
recycled-detach ownership settlement at the CPU/test boundary only. Native
execution, formal implementation correspondence, aggregate-memory and
performance acceptance remain open; A1/A2 and issue #182 are not complete.

Preceding Native checkpoint: [R123 live retained-control release is locally accepted](runtime-live-retained-control-release-v1.md),
with [336 raw artifacts](evidence/local-r123-live-retained-control-release-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,865 tests with
five ignored; seventeen source gates, ten auxiliary checks, restored 128/17
suites, ten compiled negatives across ten maps and 134 checker calibrations
pass. All 5,703 source identities matched and all 76 recorded owned process
groups were observed absent at acceptance. This accepts live retained
persistent-control release at the CPU/test boundary.

R124's identified prepared persistent-cancellation gap is now addressed by
R125 at the CPU/test boundary. Returned data and removed owners stay rooted
through cleanup, retake, native restoration and allocation-ledger cancellation.

Preceding Native checkpoint: [R122 live detached-data release is locally accepted](runtime-live-data-release-v1.md),
with [402 raw artifacts](evidence/local-r122-live-data-release-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,846 tests with
five ignored; seventeen source gates, ten auxiliary checks, restored 35/10
suites, twelve compiled negatives across twelve maps and 89 parser calibrations
pass. All 5,700 source identities match; all 93 recorded owned process groups
are absent. This accepts live data release and runtime outer-roster retention
only at the CPU/test boundary. Other live/queue cleanup, native, formal,
aggregate-memory and performance acceptance remain open. The outer traversal
is linear; repeated ledger lookup/removal can still be quadratic.

Preceding Native checkpoint: [R121 ordinary and typed-data cleanup is locally accepted](runtime-ordinary-data-cleanup-v1.md),
with [665 raw artifacts](evidence/local-r121-ordinary-data-cleanup-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,818 tests with
five ignored; seventeen source gates, ten auxiliary checks, restored 59/17
suites, 26 compiled negatives across 25 maps and thirty parser calibrations pass.
All 5,696 source identities match. This is lower-cleanup CPU/test acceptance,
not live composition, native, formal, aggregate-memory or performance acceptance.
R122 subsequently integrates live data release and runtime outer-roster retention.

Current Resources checkpoint: [R116/V3 settlement is locally accepted](runtime-context-version-settlement-v1.md#integrated-candidate)
with [378 retained artifacts](evidence/local-r116-context-version-settlement-2026-09-14/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,746 tests with
five ignored; all 17 source gates, ten auxiliary checks, 19/42/12/2 restored
suites and 29 compiled negatives pass. Nine runner, 31 freeze and 122
qualification-contract tests pass; all 5,688 source identities are restored.
Earlier 2,744-test full runs retain their original source cohort. This accepts
executable-model settlement only, not production Context, authenticated proofs,
native execution or performance. Resources next takes V4-J1 issuance proofs.

Resources development update: the external V13 issuance candidate
`r120-journal-issuance-v13.rs`, SHA-256
`1bc85ba1280dc0bfd5f0ac99a726bd0d21ad1b57fe71dbf837247ed3487f47cd`,
passed its first unchanged positive run with 56 verified obligations and zero
errors. `r120-development-v13-positive-01` took 10.929 seconds, closed normally,
left no live owned process-group members and retained unchanged source/named-tool
hashes. This is development sequence-refinement evidence, not authenticated
formal acceptance or production Rust correspondence. Five isolated executable
registration mutations subsequently reach only their intended unchanged
postcondition, each with 55 verified obligations and one error: omitted free-slot
pop, wrong slot, wrong content, wrong reserved count and next-value watermark.
The corrected V2 checker rejects competing diagnostics and checks exact source
replacements, source/tool stability, normal exit and the declaration/return
locations. Its return-line predicate passes ten calibration cases. The first
omitted-pop attempt remains failed harness history because V1 rejected Verus's
extra diagnostic gutter marker; it is not reused as a passing probe.
The fresh post-negative positive run, `r120-development-v13-positive-02`,
again reports 56 verified and zero errors on unchanged V13 and named tools;
it closes normally after 10.058 seconds with no live owned process-group members.
These development probes do not establish full proof qualification.
Full toolchain authentication, constructor/abort refinement,
physical storage/ownership and unwind correspondence remain open. The accepted
Resources checkpoint remains R116/V3.

An external V14 draft now extends V13 with executable abort sequence refinement:
`r120-journal-issuance-v14.rs`, SHA-256
`33dcc4a54f626e4fbbc1ce74f016125aca8f99c9ace503bb3541c85d8ff016de`.
Read-only review confirms the intended preflight and update ordering, including
unchanged watermark/history and malformed-state rejection. The first positive
solver run, `r120-development-v14-positive-01`, now passes with 58 verified and
zero errors in 14.904 seconds. Source and named tool hashes remain unchanged;
the run closes normally with no live owned process-group members. Following the
host reboot, a new independent calibration record passes all 82 diagnostic/source
checker cases. Six independent executor-only abort mutations then each produce
57 verified and exactly one intended postcondition error at line 550: omitted
slot clear, omitted free push, omitted count update, reset watermark, wrong error,
and a watermark write on rejection. Four failures bind the exact body-end source
at line 565; two bind the exact early-return arm at line 556. All six harnesses
close normally with unchanged source/named-tool identities and no live owned
groups on their current boot. Their same-boot chain starts at the fresh
calibration, not at the old-boot positive record. The post-negative unchanged
positive rerun, `r120-development-v14-positive-02`, again passes 58 verified and
zero errors in 19.622 seconds, with unchanged source/named tools, normal closure
and no live owned process-group members. These remain development
sequence-refinement results.
V14's observed-capacity argument and projection framing do not establish
physical storage, constructor, production-source or unwind refinement.

An external V15 candidate adds constructor contents refinement, using generic
vacant-slot and descending-free-list loops, all five production scalar fields
and seven vectors, and a projection that reads every actual field. Its exact
initial-state equality is independent of storage admission; the issuance
invariant remains conditional on storage labels. The candidate has SHA-256
`366c03768a9f662e511c5a5efaa6290cfdaba9b0534e04032af4ac45267825c8`.
Static review found no field, guard-priority or ownership mismatch. The positive
development run, `r120-development-v15-positive-01`, passes exactly 65 verified
and zero errors in 17.408 seconds. Independent read-only review confirms unchanged
source/named-tool identities, the same-boot V14-positive-02 predecessor and normal
closure with no live owned group members. Record SHA-256:
`92b2eb430d66b8d905c6736dabb0171e5c362bc51dca793b2549556b06255c51`.
The evidence-pinned candidate retains its historical "unexecuted" header; these
results supersede that annotation without altering executed bytes. No V15
negative campaign has run. Preflight-before-initialization order is source-reviewed,
not an observable property of the Result-only postcondition. Allocation failure,
physical storage, production resize/iterator binding and unwind remain outside
this contents-only development proof. R116/V3 remains the accepted Resources checkpoint.

Preceding Native checkpoint: [R117 detached persistent controls are locally accepted](runtime-detached-persistent-control-cleanup-v1.md)
with [372 retained artifacts](evidence/local-r117-detached-persistent-control-cleanup-2026-09-14/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,762 tests with
five ignored; all 17 source gates, ten auxiliary checks, 16/15/37/7/9 restored
suites and 32 compiled negatives pass. Nine runner, 31 freeze and 117
qualification-contract tests pass; all 5,689 source identities are restored.
The isolated candidate's two failed attempts remain separately recorded.
This accepts only scripted lower detached-control cleanup, not persistent-data
bridges, live/queue composition, native, formal, memory-bound or performance
qualification.

Preceding Native checkpoint:
[R119 persistent returned-data cleanup is locally accepted](runtime-persistent-returned-data-cleanup-v1.md)
above published R118B, with
[464 raw artifacts](evidence/local-r119-persistent-returned-data-cleanup-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,795 tests with
five ignored. All 17 source gates, ten auxiliary checks and six frozen/restored
suites (15/16/15/37/7/9) pass. All 37 compiled production negatives reach their
named behavioral failures across 30 distinct maps; all 5,693 source identities
are restored after each. Core/history/runner/freeze/lifecycle/qualification
contracts pass 48/46/9/26/22/227 checks, and the closed collector matches the
archive. Isolated history stays separate; the original integrated musl timeout
remains rejected and only its complete retry supplies acceptance. This adds
scripted lower cleanup acceptance, not data disposal, live/queue composition,
N5, native, formal, aggregate-memory or performance qualification.

Current accepted Admission checkpoint:
[R118B C1/C2/C3 qualification](runtime-admission-lifecycle-qualification-r118.md),
with [1,432 retained raw artifacts](evidence/local-r118b-admission-lifecycle-2026-09-15/README.md)
and two passing independent archive reviews. Fresh GNU/musl each pass 2,780
tests with five ignored. All seventeen source gates, ten auxiliary checks and
six frozen/restored suites (9/4/5/1/15/14) pass. The 78 compiled negatives cover
74 distinct source maps, with 75 production executions, one combined-defense
execution and two helper-calibration executions distinguished explicitly.

All 5,692 source identities are restored, and the closed collector matches the
archive. Core/prior-history/corrected-history/runner/freeze/lifecycle/qualification
contracts pass 48/45/27/9/57/22/167 checks. Stopped R118, including its rejected
destructor-abort case, remains unaccepted history. The corrected full campaign's
case 73 has a normal named failure; no earlier negative or preliminary regression
is reused as a passing result. This checkpoint changes tests only and adds no
native, formal, total-memory or performance acceptance. Admission next joins
N5 adoption with generated ISSUE/COMPLETE; those production paths remain open.

R115 progression: [lower returning-control cleanup](runtime-returning-control-cleanup-v1.md)
is now locally accepted with [285 retained artifacts](evidence/local-r115-returning-control-cleanup-2026-09-14/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,727 tests with
five ignored; all 17 source gates, ten auxiliary checks, 15/37/7/9 restored
suites and 16 compiled negatives pass. This accepts only the lower returning
subset, not persistent/data cleanup, live retake, queue teardown, native,
formal or performance qualification. R117 subsequently adds the lower
detached-persistent boundary above.

R114 locally accepts [pristine control cleanup and terminal parent transport](runtime-pristine-control-cleanup-v1.md).
The [254-artifact evidence packet](evidence/local-r114-pristine-control-cleanup-2026-09-13/README.md)
passes independent review. This supersedes the [candidate dispatch](runtime-swarm-dispatch-r114-candidate.md),
not the retained contracts and history in the [complete packet map](runtime-swarm-next-packets.md).
Issue [#182](https://github.com/harsh-nod/fe2o3/issues/182) remains open;
A1/A2 and full HIP/HSA parity are not complete.

The original work breakdown was refreshed against accepted source
`9eaf19141e8af6ade490feba3062c8b49d9b38ca` and the open issues on 2026-09-13.
The R115/R116/R117/R118B/R119/R121/R122/R123 progressions above update the current assignments; the original
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
| Native: `native_replacement_handoff` | Integrate ordinary recycled detach and remaining N4-L live/control routes above R123 | Applicable queue teardown, then N5 DATA-ADOPT |
| Admission: `submission_identity_handoff` | Prepare joint I2/C4 after locally accepted R118B C1/C2/C3; production integration needs N5 | Actual ISSUE, COMPLETE, C5 typed future/join, C6 generated GRAPH/DRAIN |
| Resources: `r102_evidence_review` | V4-J1 issuance proofs above accepted executable V1/V2/V3 models | Membership/settlement proofs, V5/V6 production journal/hooks, V7 writers/recovery, V8 leases; M1-M4 retained-memory bounds |
| Primary | Integrate one reviewed packet at a time; own Q1/Q2/Q3 and release gate #277 | Coordinate A3-A7 and publish only the evidence actually obtained |

### Ready Now

The three worker slots run read-only reviews in parallel; Primary is the fourth
slot and owns edits under the current ownership policy. Native supplies the
cleanup design, Admission supplies identity/lifecycle oracles and cross-reviews
Native's tests, and Resources supplies settlement/accounting contracts.

| Lane | Independent Start | Does Not Yet Close |
| --- | --- | --- |
| Native | Compose remaining live detach across model retake/commit; R123 accepts retained persistent-control release above R122 data release | Remaining live/control routes, queue teardown, N5 adoption and native qualification |
| Admission | Retain accepted R118B identity/lifecycle regressions; prepare the joint I2/C4 handoff | Actual generated ISSUE, COMPLETE, typed output or graph execution |
| Resources | Implement V4-J1's reviewed issuance-proof handoff; M1-M3 design remains independently ready | Production journal receipts, all writers, recovery, reuse or total retained-memory bound |
| Primary | Specify Q1/Q2/Q3 evidence contracts and #277's missing feature-enabled release audit | New proof, hardware, release-policy or performance acceptance |

Each handoff names source ownership, prerequisites, exact positive/rejection
oracles, retained-resource behavior, and excluded claims. Shared module wiring
and builds are serialized. Review completion does not leave implementation,
solver or hardware jobs running unattended.

## Native Packets

1. **N4-R2 detached persistent controls, accepted R117:** The
   [qualified contract](runtime-detached-persistent-control-cleanup-v1.md) for
   `release_detached_persistent_control_v1` in
   `crates/fe2o3-kfd/src/queue_dispatch_binding.rs` roots the complete owner
   before generation/state validation, preserves the separate detached data
   owner, and shares kernarg-first, **forward code order** cleanup. This unit
   result needs no returned-data allocation or extraction. R115 already
   accepts the lower `release_non_data_after_recycle` and
   `release_non_data_for_returning_destroy` paths.
2. **Persistent returned data, accepted R119; ordinary/data cleanup, accepted R121:** The
   [qualified persistent contract](runtime-persistent-returned-data-cleanup-v1.md)
   preserves exact returned-data-on-error behavior and one-shot extraction.
   [R121](runtime-ordinary-data-cleanup-v1.md) combines ordinary full-owner release
   with typed disposal while preserving ordinary validation and forward order
   without return allocation. All five data representations, retained charges,
   active receipt, untouched suffix and completed prefix pass local tests.
   Native device accounting remains distinct from coherent-host model keys.
   These lower routines now require live and queue composition.
3. **N4-L/N4-Q:** R122 accepts live detached-data release and runtime outer-roster
   retention; R123 adds live retained persistent-control release at the CPU/test
   boundary. Remaining routes must retain original
   parents and taken lanes across model retake,
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

The C1/C2/C3 test contracts below are locally qualified as R118B. Their isolated
preliminary cohorts and stopped R118 remain separate historical evidence.

| Packet | Implementation Boundary | Required Exit |
| --- | --- | --- |
| C1 | `context/tests/submission_identity_tests.rs`, narrow Context test wiring/cancel counter | R118B qualifies 80 rejection cells, genuine cache/backend-ID reuse, destroyed streams and precedence without replacing validators. The isolated nine focused and 724 runtime-library passes retain their preliminary source cohort. |
| C2-A | New `authorized_execution/tests/generated_identity.rs`; existing roster matchers | Every descriptor coordinate in both directions and against its source; valid controls pass and substituted identities reject. |
| C2-B | Same module; existing generated-storage transfer fixtures | Actual one-shot transfer, immutable source validation, then artifact/authority/currentness substitutions with exact buffer custody. |
| C3-A | New `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`; existing reservation/adoption fixtures | Two-owner isolation, Stop before disposal, A-success/B-failure/C-retained retirement, exact holds/credits and no retry or second reply. |
| C3-B | Same module; existing completion cell and identifiable wakers | Only the latest correct waker fires; panicking wake cannot lose custody or settle twice. Use a test-only borrowed accessor, not a cloned consumer. |

Paths in this table are relative to `crates/fe2o3-runtime/src`. C1/C2/C3 modules
are integrated and locally qualified in R118B; isolated histories retain their own source
maps. Shared-file edits need one integrating owner.
Do not introduce a dummy native completion
adapter. Existing ordinary async, graph/drain and decoder machinery is extended.

## Resources Packets

R116 locally accepts the executable
[V3 settlement contract](runtime-context-version-settlement-v1.md), frozen on
2026-09-14: Success, NoEffect and sticky Unknown. Preserve complete touched-chain
validation before mutation, burned epochs and disjoint writers. Reuse the
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
| V3 / R116 | Locally accepted executable settlement and public/private Unknown model against the frozen full-reference inert evidence and error precedence. Success advances lineage; NoEffect preserves lineage and burned epochs; Unknown retains the complete chain. |
| V4 | Start the reviewed V4-J1 issuance handoff, then authenticate membership/settlement properties with solver results, negative mutations and actual Rust/model correspondence. No proof execution is added by R116. |
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
