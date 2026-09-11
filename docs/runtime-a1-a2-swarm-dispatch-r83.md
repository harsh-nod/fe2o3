# A1/A2 Swarm Dispatch After Local R83

Dispatch established by planning-only commit `32c1beff`, 2026-09-10, and refreshed
by three read-only workers on 2026-09-11 against signed R83 and the uncommitted
R84 DATA-SHELL implementation and regressions. The latest decomposition splits
native construction custody into two prerequisites before DATA-ADOPT. This is
the current assignment overlay for
[next-wave dispatch](runtime-a1-a2-next-wave.md) and the
[historical roadmap](runtime-a1-a2-swarm-plan.md). It supersedes their current
assignment rows, not their packet-specific evidence or historical contracts.
The immediate scope is remaining A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), observed open with issue
update `2026-09-10T10:50:51Z`. Later A3-A7 milestones remain separate.

## Checkpoint And Ownership

The signed implementation baseline is R83
`e07de3bfb87955fc885ef0c88788678ce8170aaa`, on both repositories' topic branch
`codex/r65-runtime-drain-versions`, not a main merge.
R83 implements private activation, nonflushing active custody, stream holds and
explicit drain/shutdown retirement. Its
[local evidence](evidence/local-r83-unpublished-lifecycle-2026-09-10/README.md)
records all seventeen frozen-source gates, 2,363 runtime tests per GNU/musl
target, fifteen new lifecycle tests and the resolved callback type-complexity
lint. The original planning-only checkpoint did not publish or qualify R83;
the later source/evidence packet supplies this local acceptance.

Production generated preparation installs no adoption hooks. Activation is a
private boundary, not a public API. Native DATA adoption, publication and typed
completion are still missing. R83's scripted lifecycle tests do not establish
actual charged-carrier integration, Linux execution or executable refinement.

DATA-SHELL now has uncommitted source changes: generated-only storage, opaque
source identity, one-shot inert packet transfer, ordinary/generated entries in
one backend allocation table, whole-roster Context registration and logical
credit retirement. Canonical disposal hardening, pure ingress checks and the
dedicated storage, Context, backend and charged-host regressions are now present.
These changes are not a released R84 packet: the complete final frozen-source
gate run and retained acceptance evidence remain pending. Two earlier full-suite
attempts stopped at lint failures; the corrected all-feature and production lint
profiles pass independently, not as a substitute for the complete final run.
R83 test counts do not qualify this modified source. This planning refresh does
not stage or publish those implementation files.

Three read-only workers completed this source audit. Their review turns are
complete; the follow-on queues are assignments, not unattended jobs.
Primary owns all edits, integration, conflict resolution, tests, proofs,
hardware scheduling and signed publication. There are three worker slots plus
Primary, not one worker per ticket. Shared Context/backend edits are serialized.

## First Parallel Packets

| Worker | First bounded assignment | Deliverable and exit gate |
| --- | --- | --- |
| Native: `r66_native_coexistence` | NATIVE-1 build-escrow design/review, then NATIVE-2 constructor custody | Root remaining original inputs, converted authorities, initialization descriptors and control prefixes across error/panic. Preserve successful results before closing retake. Distinguish pre-effect rejection, partial pre-queue effects and terminal post-create custody before DATA-ADOPT. |
| Admission: `r66_runtime_coexistence` | COMPLETE-ORACLE, independent of native adoption | Exact invocation/submission/generation and full-roster validation contract; failure and disposal-order fixtures covering malformed late output, decoder panic, observer loss and shutdown. No native completion authority from fixtures. |
| Resources: `r66_coexistence_model` | VER-1A, independent Context mutation journal | Bounded nonwrapping whole-allocation `Available/Pending/Unknown` transitions, atomic multi-destination admission, exact writer completion and a complete mutation-site inventory. Cross-run leases stay disabled. |
| Primary | Finish R84 frozen-source acceptance, then implement and integrate the bounded reviewed packets | Preserve R80 reservations and R83 custody. Own all edits and shared module changes. Do not install adoption callbacks before complete native custody exists. Retain separate CPU/proof/Linux/performance results and publish accepted packets to both topic remotes. |

Native and Admission first agree on the exact packet/hold/native-key boundary.
Admission can develop COMPLETE-ORACLE while Native specifies construction custody;
Resources does not depend on either. This avoids assigning both workers the same shared
implementation or recreating R83's existing phase/hold machinery.

## DATA-SHELL Acceptance Checklist

SH-1 through SH-4 have implementations and regressions in the working tree;
their three read-only reviews found no remaining blocking issue. SH-5's complete
final gates and retained acceptance are still pending. The checklist states the
acceptance obligations, not new assignments to recreate that work. This is an
inert metadata/storage packet, not a new native execution profile.

| Item | Owned implementation boundary | Required result |
| --- | --- | --- |
| SH-1: canonical disposal | `kfd_backend/generated_shells.rs`, `kfd_backend/allocation_table.rs` | Before removing control or any record, require canonical ordinals, unique logical IDs, consecutive backend IDs, exact adoption owner/descriptors and no extra table member for that owner. Corrupt the retained plan, not only a copied argument, in rejection tests; all custody and credits must remain intact. |
| SH-2: pure ingress checks | `context/generated_preparation.rs`, `context/generated_shells.rs` | Reject a held-stream/device mismatch or already-installed generated key before invoking `source_mut()` or native currentness. Count callbacks in tests; preserve the existing closing currentness validation for eligible work. |
| SH-3: storage and control tests | `persistent_projection.rs`, `generated_source.rs`, host generated storage/invocation tests | Original buffer pointers, full ordinals and source identity survive conversion; byte-identical replacement storage rejects. Cover stale authority, changed HSACO, occupied transfer destination and replay. No public control extraction, dummy payload or encoded-byte shadow. |
| SH-4: Context and backend tests | `context/generated_preparation/tests.rs`, proposed shell/table test modules | Configured/unconfigured success; whole-roster byte/record pressure; Context/backend ID exhaustion; ordinary read/write/free/copy/compute rejection; held-stream release and exact retirement. Rejections preserve counters, control, maps and credits; unrelated ordinary allocations remain usable. |
| SH-5: acceptance and evidence | Primary gate/evidence integration | Recheck compilation after the latest edits, add focused regressions, then freeze source and run the existing full GNU/musl, host/fixture/doc, lint, policy and checker gates. Record actual charged-storage coverage and remaining synthetic-fixture boundaries. Keep native hooks absent in this packet. |

The earlier disposal finding concerned malformed retained state, not an observed
GPU failure. The current implementation rejects duplicate/corrupt members and
extra same-owner entries before removing any record or control. Late Context and
backend mismatch regressions also preserve custody and credits. Those fixes still
need acceptance on the final frozen source.

## Generated Execution Queue

Paths below are relative to `crates/fe2o3-runtime/src/` unless otherwise stated.
New modules are proposed, not existing implementation claims.

| Packet and lead | Dependency / module boundary | Acceptance before advancing |
| --- | --- | --- |
| DATA-SHELL acceptance: Primary; three worker reviews complete | Existing uncommitted R84 implementation on signed R83. Storage/source modules, `context/generated_shells.rs`, `kfd_backend/allocation_table.rs` and `kfd_backend/generated_shells.rs`; host invocation. | Finish SH-5 on the final source. Preserve implemented SH-1 through SH-4, full unused/read-only ordinals, exact IDs and bounded records without exposing packet/decoder authority. No native effects. |
| NATIVE-1: Native | Independent construction contract now; implementation after the R84 source freeze. `crates/fe2o3-kfd/src/queue_dispatch_binding.rs`, proposed isolated build-escrow module and narrow `shared_memory.rs` exact-record conversion. | One rooted bounded owner retains unvisited original inputs, converted authorities/initialization premises, packet, code and kernarg prefixes, and exact generation provenance. Existing planner/allocator remains the only path. Inject failure/panic after each ordinal/control step; conversion rejection preserves its input. |
| NATIVE-2: Native; Admission review | NATIVE-1. `crates/fe2o3-kfd/src/queue_live.rs` and `queue_live/fixed_dispatch.rs`; narrow existing abort integration. | Retain primary/auxiliary bootstrap state and successful insertion outputs before closing retake. Separate unchanged pre-effect rejection, partial pre-queue effects and terminal CREATE_QUEUE-attempted failures. Return a usable lane only after complete success; never synthesize a pristine abort continuation from an incomplete constructor. |
| NATIVE-3 / DATA-ADOPT: Native + Admission; Resources reviews charges | Accepted DATA-SHELL, NATIVE-1/2, R80 reservations, R82 abort and R83 lifecycle. Proposed `kfd_backend/generated_adoption.rs`; narrow Context, `compute_state.rs`, `compute_dispatch.rs` and existing adoption hooks. | Switch the rooted shell into non-discardable native custody before effects. Borrow original initialization bytes, bind without publication, retain the exact session/lane/prefix, then retire through R82 abort and exact data disposal. Exercise initial/auxiliary/reused lanes, allocation/map/bind/retake error/panic, empty-prefix Stop and drain. Linux bind/abort/reuse remains a separate gate. |
| COMPLETE-ORACLE: Admission; Resources review | Independent now. Proposed completion contract plus focused host charged-storage and runtime preparation fixtures. | Freeze one-reply semantics, exact completion identity, full-roster failures, original-storage disposal and charge lifetime before ISSUE. Reuse R73/R80 tests rather than duplicating them. |
| ISSUE: Admission + Native | Composed DATA-ADOPT and COMPLETE-ORACLE. Generated driver, Context registration, `compute_state.rs` and `compute_dispatch.rs`. | A private linear permit binds exact resources/submission and survives actual flush/retry. Reject substitution/replay; distinguish definite nonpublication from unknown publication. No broad backend authorization flag or per-invocation backend replacement. |
| COMPLETE: Admission; Resources + Native review | ISSUE. Generated driver, host invocation/results/readback and backend readback hooks. | Fill existing R80 destinations, validate every buffer, destroy original encoded storage before committing externally droppable typed outputs, then resolve once. Test stale/partial results, currentness loss, decoder panic, retained results and credits after shutdown. Never reserve readback or the completion reply twice. |
| API: Admission; Primary integration | ISSUE + COMPLETE. Runtime handle, generated host interfaces and narrow macro integration. | One executor-neutral typed future; blocking launch joins the same engine. Test completion/poll races, latest-waker replacement, reentrancy, reply/queue pressure, equivalent outcomes and ownership compile failures. No blocking executor hidden in a command callback. |
| GRAPH/DRAIN: Admission | API; cross-run reuse also needs VER-1B/VER-2. `async_engine/graph`, drain/capture and owned shutdown. | Repeated generated graphs, exact dependencies/results, accepted-prefix drain, dropped observers and failure retention. Preserve graph/standalone exclusion. Production hardware acceptance additionally needs exact protected compiler evidence. |

NATIVE-1/2 are concrete prerequisites, not a new allocator or queue implementation.
The existing consuming dispatch wrappers discard failure custody; initial and
auxiliary constructors do not expose a complete retained build state; the generic
live-memory wrapper drops a successful operation result on closing-retake failure.
Existing fail-stop retention does not supply a recoverable typed-prefix contract.
Keep adoption callbacks absent until native ownership, runtime drain and terminal
Drop all consume the new contract. Native and Resources must serialize later
edits to `shared_memory.rs`; the completion oracle and isolated journal are
independent of those files.

COMPLETE-ORACLE's first deliverable is a completion contract plus focused
fixtures in proposed
`async_engine/tests/owned_tests/preparation_tests/completion_contract_tests.rs`
and the existing host charged invocation/readback tests. Freeze exact
invocation/submission/generation, the full original ordinal/access/length roster,
read-only preservation and disposal-before-readiness. Include duplicate/foreign
completion, malformed last output, readback failure, decoder panic, observer
loss and shutdown. This work can proceed before native adoption; fixtures do
not create native completion authority.

## Resources And Proof Queue

| Packet and lead | Dependency / module boundary | Acceptance before advancing |
| --- | --- | --- |
| VER-1A: Resources | Independent now. Proposed `context/versions.rs`, isolated model/proof/tests; Primary owns shared exports. | Bounded capacity and version exhaustion, whole-allocation invalidation, multiple destinations, exact writer completion and unknown outcomes. Existing graph-local versions are history, not cross-run authority. |
| VER-1B, then VER-2: Resources + Primary | VER-1A. Context host-write/copy/peer/launch/atomic/collective, prepared/generated submission, completion/release/currentness hooks; proposed `async_engine/graph/input_leases.rs`. | Every mutation path invalidates or rejects before issuing private cross-run leases. Test operations outside graphs, overlapping writers, reuse, foreign Contexts, replay and unknown publication. Application-defined arguments alone cannot justify precise kernel write sets: require admitted effects, conservatively invalidate or reject reusable-input admission. |
| MEM-DOM-1A/B: Resources | Independent domain/headroom design; integrate `fe2o3-resource-accounting` and Context/session construction. | One bounded root, exact children, charged account arenas and reserved terminal headroom. Repeated Contexts and simultaneous quarantine cannot reset ceilings; parent exhaustion is failure-atomic. |
| MEM-N1B, then MEM-3: Resources + Native | Existing R70/R72 primitives; global claims also require domains. Native shared-memory accounting, queue/dispatch and slot/arena hooks. | First kernarg/executable backing, then AQL/userptr/remaining control profiles. Distinguish physical backing, aliases and VA; no duplicate debit. Cover every construction/disposal failure and progress-safe resource acquisition. |
| MEM-4A/B: Resources | Host-image ceiling can start independently; native residency needs backing/control integration. Proposed backend residency module. | Bound retained host images and materialized code caches; exact identity/live leases constrain eviction. Uncertain unload retains charges. Executable GTT is not VRAM. |
| PRF + MEM-5: Primary; Resources review | Incremental with every packet, not deferred until the end. Models, adapter checks, proof inventory and retained evidence. | First target R83 one-shot lifecycle/R82 cleanup correspondence. Ultimately account for registry, commands/captures, replies/results, arenas, journals and aggregate quarantine, with executable-transition refinement or explicit remaining boundaries. |

VER-1A's first deliverable is the independently bounded journal and complete
mutation-site inventory. Whole-allocation states must transition atomically
across multiple destinations, bind an exact writer and never wrap generations.
Test competing writers, stale completion and unknown outcomes. Keep cross-run
input leases disabled until every VER-1B mutation path is covered; graph-local
version reports are historical observations, not reusable input authority.

The retained authenticated R73 proof checkpoint reports 62 positive sources,
1,374 obligations and 686 negative mutations. It does not prove later adapter
changes. Inventory checks are not solver reruns. See the
[R73 evidence](evidence/local-r73-charged-results-2026-09-10/README.md).

## Independent Qualification Queue

Native rotates into these packets after its current handoff; Primary alone
schedules hardware. They need no new protected production compiler handoff
unless explicitly stated.

| Packet | Implementation versus qualification | Exit gate |
| --- | --- | --- |
| SCALE-1A-FIXTURE | Independent new bounded fixture sources/manifests/oracles/checker tests under `benchmarks/runtime_gfx942`; then sequential Linux correctness. | Freeze ABI/effects, geometry, source/object/toolchain and complete output/padding oracles. Mutated coordinates reject. New artifacts need explicit qualification profiles without widening existing R26 admission. Short/long labels are not measured durations. |
| MEM-QUAL-HARNESS | New example and signed runner/checker exercising existing N1/N2 and both cache policies in both startup orders. | Bootstrap consumption, padded byte/record pressure, zero caching, retained reuse and exact disposal; complete data and native identities. Existing logical-budget examples do not qualify native budgets. |
| R76/R78/R82 qualification | Native primitives are implemented; dedicated retained-scope, rebound-initialization and same-queue abort/rebind harness cells remain to be added. | Genuine retained device before/after lazy bootstrap; R78 rebound with distinct complete HostVisible triplets, not cached reuse; R82 pristine abort then rebind. Require full outputs, exact queue/generation and conclusive cleanup. Positive generated construction separately needs compiler evidence. |
| OVL-QUAL-2 / DRN-2A | Existing coexistence and copy-drain campaigns, not duplicate implementations. | Frozen signed source, complete outputs/canaries, retained native identity, bounded staging and independently confirmed owned-process/file cleanup. These captures alone do not establish physical overlap. |
| SCALE-CAP -> SCALE-2 | New bounded native profile/admission before larger depth qualification. | Native backing/control/slot limits before increasing capacity. Distinguish queued host records from actually published/retained native work and measure out-of-order completion. |
| DRN-2B / generated drain | Fixture compute drain follows sequential fixture correctness; production graph/drain follows generated integration and exact compiler evidence. | Outstanding compute, dropped observers, complete outputs and cleanup. Fixture results do not fill production-generated cells. |
| SCALE-3 | Consistency protocol/checker exists; matched KFD/HSA/HIP producers and accepted correctness-first campaigns remain open. | Predeclare artifacts, geometry, bytes, completion/reuse semantics, timing categories, thresholds and run rotation. Full-output checks precede CPU/memory/latency/bandwidth/tail measurements. Independent device timelines are required for physical overlap. HIP/HSA are comparison producers, not production backends. |

No SSH, GPU workload, solver or runtime test was started for the original
planning-only refresh; subsequent R83 runtime tests are recorded separately
above. Future shared-MI300X campaigns require an idle admitted GPU, bounded
private staging, one owned campaign at a time and cleanup of only owned resources.
Disruptive fault tests require an agreed isolated window.
No SSH, GPU workload, solver or new runtime test was started for this planning
refresh either. The previous R84 formatter check completed successfully.
Documentation/link checks are not DATA-SHELL acceptance.

## Integration Batches

| Batch | Parallel worker work | Primary integration and exit |
| --- | --- | --- |
| 1: ready now | Native: NATIVE-1 build-escrow contract, followed by NATIVE-2 constructor contract. Admission: COMPLETE-ORACLE contract/fixture design. Resources: VER-1A journal/inventory. | Finish and accept DATA-SHELL; implement independently reviewed native/oracle/journal changes as separate bounded packets. No native publication or cross-run leases. |
| 2: native custody | Native: accepted NATIVE-1/2, then NATIVE-3 DATA-ADOPT. Admission: exact ISSUE/completion interface review. Resources: VER-1B inventory/hooks and domain/headroom work. | Serialize Context/backend and native shared-memory edits; accept full partial-prefix/lane custody and R82 abort before connecting ISSUE. Keep proof and Linux gates separate. |
| 3: execution and reuse | Admission: ISSUE, then COMPLETE, then API. Native: fixtures and native qualification harnesses. Resources: close all mutation hooks before VER-2; backing/control/residency work. | Gate each execution transition, retain R73/R80 charge ownership and one completion cell. No API-only claim of graph, native-depth or budget closure. |
| 4: A1/A2 qualification | Admission: generated GRAPH/DRAIN. Native: depth/overlap/copy/performance campaigns. Resources: resource/proof composition and aggregate bounds. | Signed source, independent evidence review, complete outputs, actual native-depth and repeated graph/version/resource checks, matched baselines and owned-resource cleanup. |

Each worker takes one bounded assignment at a time. Primary owns code changes,
tests, proof runs and shared-machine scheduling. Independent harness design can
start earlier when a worker becomes free; dependencies above constrain
integration and acceptance, not unrelated analysis. The current worker audit
turns have finished, so queued implementation packets are not background jobs.

## Ordering And Closure

```text
signed R83 -> final R84 DATA-SHELL acceptance -------------------> DATA-ADOPT
NATIVE-1 build escrow -> NATIVE-2 constructor custody -----------> DATA-ADOPT
R80 reservations + R82 abort + R83 lifecycle --------------------> DATA-ADOPT
DATA-ADOPT -> ISSUE -> COMPLETE -> API -> GRAPH/DRAIN
COMPLETE-ORACLE -> ISSUE
VER-1A -> all VER-1B mutation hooks -> VER-2 cross-run input leases
domains + backing/control/residency -> MEM-5 aggregate closure
each implemented packet -> CPU + adapter/proof + relevant Linux acceptance
```

A1/A2 closure also requires native depth, repeated graph/version/resource
qualification and matched measurements, not only completion of the generated
API. Keep `Proved`, `Checked`, `Validated`, `Contracted` and `Unsupported` results
property-specific. Protected compiler evidence is an external handoff for
production-generated acceptance, not a reason to stop independent local work.

After A1/A2, the same slots rotate through the remaining
[issue milestones](https://github.com/harsh-nod/fe2o3/issues/182):

| Milestone | Lead and boundary |
| --- | --- |
| A3: local multi-GPU | Native, with Resources/Admission: placement, shards/replicas, compute/peer ownership and group failure/drain. Existing two-device copy support is insufficient. |
| A4: distributed control | Admission: authenticated membership, artifact/plan exchange, publication receipts and terminal run records. |
| A5: distributed data/collectives | Native + Resources: bounded transfers, versions/credits and separately qualified collective plans. |
| A6: fault qualification | Admission + Primary: participant/network/device failures without unsafe replay, early release or false completion. |
| A7: production performance | Native + Primary: predeclared single-device, multi-GPU and two-host performance gates with direct-KFD dependency/symbol audits. |

This dispatch does not close A1/A2, #182, full HIP/HSA parity or any performance
target. Local R83 acceptance covers its private lifecycle only, not the queued
native adoption, completion, proof, hardware or performance work.
