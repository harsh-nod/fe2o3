# A1/A2 Swarm Dispatch After R87 Allocation Custody

Dispatch established by planning-only commit `32c1beff`, 2026-09-10, and refreshed
by three read-only workers on 2026-09-11 against R87 pending allocation custody
on signed R86 whole-roster data conversion. The latest decomposition splits
the remaining native control work into session transitions, preparation ownership
and both bind settlements before NATIVE-2 and DATA-ADOPT. This is the current assignment overlay for
[next-wave dispatch](runtime-a1-a2-next-wave.md) and the
[historical roadmap](runtime-a1-a2-swarm-plan.md). It supersedes their current
assignment rows, not their packet-specific evidence or historical contracts.
The immediate scope is remaining A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), observed open with issue
update `2026-09-11T08:01:35Z` during this refresh. Later A3-A7 milestones remain separate.

## Checkpoint And Ownership

The current locally accepted implementation is R87's pre-record allocation
prerequisite, with [source evidence](evidence/local-r87-pending-allocation-2026-09-11/README.md).
Its baseline is signed R86 plus the signed planning-only commit
`d2e65f47d942af8307d3d54ffa6ee82bef5d2c30`.
Signed R86 is `1fa69f17e526e2b96ba69a22047f209c261c367e`, on both repositories'
topic branch `codex/r65-runtime-drain-versions`, not a main merge.
Its retained evidence records the source accepted at that commit, including
the then-current documentation. R87 adds allocation custody, not new
proof, hardware or performance acceptance.
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

DATA-SHELL implements generated-only storage, opaque
source identity, one-shot inert packet transfer, ordinary/generated entries in
one backend allocation table, whole-roster Context registration and logical
credit retirement. Canonical disposal hardening, pure ingress checks and the
dedicated storage, Context, backend and charged-host regressions are present.
All seventeen final frozen-source gates pass, with 5,616 source identities
unchanged and 2,388 runtime tests per GNU/musl target. Two earlier full-suite
attempts stopped at lint failures and remain separately retained; neither is
substituted for the accepted final run. R84 adds 25 runtime and two host CPU test
functions plus seven runtime compile-fail doctests. It does not install native
hooks or establish protected construction, executable refinement or GPU results.

R85 now passes all seventeen source gates and auxiliary checks, with 5,618
source identities unchanged, 258 GNU host tests and 141 musl host tests. Ten
new host tests exercise actual reserved readback custody, exact gate and roster
validation, settlement-before-readiness, panic and observer loss. The isolated
foreign-gate mutation fails as expected; restored source passes. The
[private host contract](runtime-charged-readback-completion-v1.md) is implemented,
not a remaining task. Runtime completion identity, native readback/disposition
and the engine reply remain open; no solver rerun or hardware result is claimed.

R86 implements the [whole-roster data conversion](runtime-dispatch-data-retention-v1.md)
portion of NATIVE-1. The original owner stays borrowed until every exact record,
domain, backing charge and initialization descriptor is checked; conversion then
uses bounded inline storage without callbacks or native effects. Thirteen new
CPU tests pass, and early-move/duplicate-check mutations are rejected. Its
[final acceptance](evidence/local-r86-dispatch-retention-2026-09-11/README.md)
passes all seventeen source gates and auxiliary checks, with 5,620 source
identities unchanged and 2,401 runtime tests per GNU/musl target. This supersedes
the proposed per-item recovering Host conversion,
not the complete NATIVE-1 construction owner or NATIVE-2 constructor work.

R87 implements [pending GTT allocation custody](runtime-pending-gtt-allocation-v1.md)
inside the existing allocator. Returned reservation, raw ALLOC output and mapping
are rooted before validation; errors/panics retain pending custody and quarantine
the optional Host debit. Fourteen new KFD tests pass, together with all seventeen
source gates: 5,622 source identities unchanged and 2,415 runtime tests per
GNU/musl target. Both expected-negative mutations reject. This does not retain
unreturned backend mappings, successful tokens across model projection, the full
preparation owner or outer closing-retake/validation state. Full NATIVE-1 remains
open, as do kernarg/executable backing and other control-memory budgets.

Three read-only workers completed this source audit. Their review turns are
complete; the follow-on queues are assignments, not unattended jobs.
Primary owns all edits, integration, conflict resolution, tests, proofs,
hardware scheduling and signed publication. There are three worker slots plus
Primary, not one worker per ticket. Shared Context/backend edits are serialized.

## First Parallel Packets

| Worker | First bounded assignment | Deliverable and exit gate |
| --- | --- | --- |
| Native: `r66_native_coexistence` | CONTROL-1 session transition custody, CONTROL-2 preparation owner, CONTROL-3 both bind settlements; then NATIVE-2 | Preserve returned tokens across model projection and exact control stages, then root packet/plan/generation, data and successful dispatch outside fallible calls. Cover closing-retake and later validation error/panic before DATA-ADOPT. |
| Admission: `r66_runtime_coexistence` | CO-1/2/3 completion contract and fixtures, independent of native adoption | Exact invocation/submission/generation and full-roster validation contract; failure and disposal-order fixtures covering malformed late output, adapter panic, observer loss and shutdown. CO-4 native completion follows ISSUE; fixtures supply no native authority. |
| Resources: `r66_coexistence_model` | VER-1A, independent Context mutation journal | Bounded nonwrapping whole-allocation `Available/Pending/Unknown` transitions, atomic multi-destination admission, exact writer completion and a complete mutation-site inventory. Cross-run leases stay disabled. |
| Primary | Complete NATIVE-1 control/build ownership, then integrate NATIVE-2, runtime COMPLETE-ORACLE and VER-1A | Preserve accepted R84/R85/R86/R87, R80 reservations and R83 custody. Own all edits and shared module changes. Do not install adoption callbacks before complete native custody exists. Retain separate CPU/proof/Linux/performance results and publish accepted packets to both topic remotes. |

Native and Admission first agree on the exact packet/hold/native-key boundary.
Admission can develop COMPLETE-ORACLE while Native specifies construction custody;
Resources does not depend on either. This avoids assigning both workers the same shared
implementation or recreating R83's existing phase/hold machinery.

## First Deliverables

1. Native: R86 atomic data conversion and R87 pending allocation custody are
   implemented. First preserve successful control tokens across session model
   projection and consuming seal/map/retain transitions, then complete the preparation
   owner in `queue_dispatch_binding.rs` and its narrow private child module.
   Retain packet descriptions, plan, generation, converted data and exact
   premises, code prefixes/current code typestate, kernarg typestate and
   successful dispatch output outside `prepare_in_place`. The persistent bind
   callers in `queue_live/fixed_dispatch.rs` must root it before `catch_unwind`
   and the model-loan closure, then preserve it in terminal native custody on
   error/panic. Later code/kernarg failure must not use post-dispatch content
   reconstruction. Borrow executable envelopes synchronously; do not create a
   self-referential artifact owner or a second allocator/planner.
2. Admission: specify the runtime completion identity and terminal-outcome
   table against the existing reserved engine reply. Cover definite
   nonpublication, conclusive completion and uncertainty with one outcome per
   operation. Rejected, stale and duplicate candidates must not call the counted
   completion adapter or release custody; actual R85 integration follows ISSUE.
   Integrate with Native's exact key/hold
   contract before ISSUE; do not introduce a second reply or result decoder.
3. Resources: implement the isolated VER-1A Context journal and inventory every
   mutation path. Reuse the existing Context identity allocator and submission
   IDs; retain full logical destination rosters before backend translation.
   Preflight capacity and nonwrapping generations before an atomic
   multi-destination commit; test
   competing writers, stale completion and irreversible Unknown states.
   R65 graph-local history is not persistent version authority, and leases
   remain disabled until every VER-1B hook is integrated.

The workers own these bounded analysis/review lanes; Primary owns their patches
and acceptance. Native later takes fixture and native-budget harness reviews
while Admission integrates ISSUE/COMPLETE. Resources takes domains, backing,
residency and proof composition after the journal. These are queued handoffs,
not concurrent edits or unattended implementation jobs.

## Source-Grounded Work Orders

These are the three dependency-ready implementation packets. The named workers
have completed their read-only design reviews; Primary owns the following edits
and acceptance runs. Proposed files and types below do not exist yet.

### Native: NATIVE-1-CONTROL

| Subpacket | Source boundary and dependency | Deliverable and exit gate |
| --- | --- | --- |
| CONTROL-1: session transitions | `shared_memory.rs`: `allocate_profile`, consuming seal/map methods, `commit_map_projection` and code/kernarg retention. Ready after R87. | Root returned tokens before evidence, projection and model replacement. Preserve exact token/record association through materialization and consuming transitions; uncertainty is retained custody, not reconstructed usable typestate. Test successful allocation followed by projection failure, partial seal/map, error/panic and configured/unconfigured profiles with exact records and charges. |
| CONTROL-2: preparation owner | `queue_dispatch_binding.rs` and proposed private `FixedDispatchPreparationCustodyV1`. Depends on CONTROL-1. | Existing planner/sequencer borrows an externally rooted bounded owner for packets, plan, generation, original data/premises, code prefix/current stage, kernarg and completed dispatch. Replace data-only failure recovery; test later-program and final-resolution failures without post-dispatch reconstruction of unpublished inputs. |
| CONTROL-3: bind settlement | Both persistent paths in `queue_live/fixed_dispatch.rs`, loan envelope in `queue_live.rs` and terminal custody in `persistent_compute.rs`. Depends on CONTROL-2. | Root preparation before catch/loan and completed dispatch/attachment before closing retake and later validation. Add terminal preparation custody. Test both cardinalities over operation success/error/panic crossed with retake success/error/panic, plus validation failure/panic. Preserve original panic, exact generations and every owner; uncertainty grants no binding or retry. |

The current model-loan custody wrapper retains a successful output across a
retake error but can lose that locally held output on retake panic. Store success
outside the closure; returning a richer error alone does not fix unwind.
Three-binding pre-detach rejection also must not return Retryable after a closing
retake has already terminalized the queue, even if attachment cancellation succeeds.
Test this classification against the corresponding single-binding path.

Every subpacket exercises production-used sequencers/settlement helpers with
fake-native records, exact custody/charges and zero publication. An early-drop
mutation must fail. Full source gates precede NATIVE-2 constructor integration;
CPU acceptance is not a Linux result or formal adapter refinement.

Native hands Admission an exact nonpublishing adoption identity and retirement
outcome, not a completion receipt. Reuse the existing allocator, planner and
R86 conversion; do not invent a second native runtime.

### Admission: COMPLETE-ORACLE

Start with a completion contract and focused fixtures under
`async_engine/tests/owned_tests/preparation_tests/`. Join the exact
preparation/source Arc, Context generation, runtime/backend submission, device,
stream/lane, publication occurrence and complete allocation-incarnation roster.
Reuse the reserved completion producer/consumer. Runtime fixtures use a counted,
data-only completion adapter, not host-private R85 decoding: host depends on
runtime, so a reverse dependency is forbidden. Actual R85 invocation belongs to
later COMPLETE integration; do not add another decoder, readback allocation or
completion cell.

| Subpacket | Dependencies | Deliverable and exit gate |
| --- | --- | --- |
| CO-1: outcome contract | Independent of native work. | Define Pending, rejected observation, exact success, conclusive failure, quiescent-without-result and uncertainty. Table tests distinguish reply disposition from authority to dispose the operation owner. |
| CO-2: identity oracle | CO-1; parallel-reviewable with CO-3. | Reuse roster matching and Context submission validation. Mutate every source, generation, submission, device/lane, publication and allocation coordinate independently, including byte-identical foreign storage, duplicates and replay. Rejection leaves owner and reserved reply unchanged. |
| CO-3: reply/custody fixtures | CO-1; existing preparation Harness and R80/R83 fixtures. | Test repeated rejection followed by success, late readback/currentness failure, partial retirement, adapter panic, observer loss, Stop/shutdown and waker replacement. Assert exact callback order, zero/one adapter calls and one reply without duplicate issue. |
| CO-4: native binding | NATIVE-1/2, DATA-ADOPT and ISSUE. | Integrate private `generated_operation/completion_contract.rs` with actual retained native ownership and exact publication/completion receipts. Readback, closing currentness and disposition checks precede actual R85 decoding. Scripted candidates cannot supply native authority. |

| Candidate or outcome | Required fixture result |
| --- | --- |
| Pending, rejected observation, foreign/stale candidate or replay | No decode, disposal, republish or reply duplication; retain the actual operation owner. |
| Exact successful completion | Scripted readback/currentness/disposition checks precede one counted-adapter call and one existing reply completion; actual R85/native integration is CO-4. |
| Conclusive failure or quiescence without result | No typed output; release only after conclusive native retirement, then one failure reply. |
| Publication uncertainty, currentness loss or partial retirement | No readiness or refund of possibly live custody; an error reply is not disposal authority. |
| Decoder-adapter panic, dropped observer, Stop or shutdown | Preserve fixture ownership and one-reply semantics before and after adapter transfer; actual charged/native composition is CO-4. |

Mutate each identity/roster coordinate independently, including byte-identical
foreign storage and late output mismatch. After packet transfer, use immutable
`RuntimeGfx942GeneratedSourceV1` validation: `source_mut().validate()` requires
control to remain present and is not the completion path. Scripted outcomes are
not native receipts. ISSUE remains gated on complete native adoption.

### Resources: VER-1A

Introduce the isolated `context/versions.rs` journal, focused tests and matching
bounded model/proof work. Use `Context::next_id` for synchronous writer identity
and the exact `RuntimeSubmissionIdV1` for asynchronous writes. Preallocate scratch
and preflight the complete deduplicated destination set before any state change;
finish also validates the full writer roster before committing any member.
Unknown stays unavailable until allocation retirement. Available records describe
mutation lineage, not proof of initialized or correct contents.

| Subpacket | Dependencies | Deliverable and exit gate |
| --- | --- | --- |
| VER-1A.1: contract/inventory | Independent of native construction and completion. | Freeze exact Context/allocation/device/writer identity, finite capacity and whole-allocation Available/Pending/Unknown semantics. Name every mutation hook below. |
| VER-1A.2: executable transitions | .1. | Isolated runtime-model transitions consumed by the journal, with matching Verus statements/negatives. Begin and finish preflight the entire roster before mutation; first/middle/last failure changes no entry. Only exact successful writer settlement restores availability. |
| VER-1A.3: bounded journal | .2; proposed `context/versions.rs` and child tests. | Preallocate metadata/scratch, prevent live-entry eviction and epoch wrap, retain exact membership and move-only writer tickets. Failed disposal retains entries; Pending/Unknown yield no reusable authority. |
| VER-1A.4: initial hooks | .3; Primary owns shared Context and graph edits. | Wire host writes and ordinary/graph copies into the actual journal. Retain logical destinations before backend translation, invalidate before effects and settle before callbacks. Comprehensive hook closure stays VER-1B. |
| VER-1A.5: acceptance | .1 through .4. | Focused/full source gates and authenticated property proofs; omitted-member, wrong-writer and wrapping mutations must fail. Report CPU and proved properties separately; leases remain disabled. |

| Existing boundary | Required inventory/hook contract |
| --- | --- |
| Allocation, generated shell registration and disposal | Transactional exact-identity membership; failed disposal retains entries. |
| `write_allocation` | Begin after range validation, before backend effects; settle error/panic conservatively. |
| Prepared ordinary/graph copy and `peer_copy` | Preserve logical destination/device before backend translation; reuse the actual submission identity. |
| Prepared ordinary/snapshot/graph/atomic/collective launch | Retain a complete logical mutation roster. Caller-declared access alone cannot justify a precise kernel write set. |
| `transition_submission_status`, quiescence, cancel and cleanup | Settle before callbacks; Pending is not another mutation and quiescence is not successful content production. |
| Terminal/protocol/panic/currentness loss and future generated issue/completion | Invalidate pending/reusable state; host preparation/decoding does not supply a native mutation hook. |

Test competing writers, overlapping destinations, foreign/retired identities,
capacity and first/middle/last epoch exhaustion, stale/wrong-writer completion,
late roster mismatch, backend error/panic, cancellation and allocation reuse.
Pure admission rejection must leave all entries unchanged and call no backend.
VER-1B must cover every inventory hook before VER-2 enables cross-run leases.
R65 graph-local proofs do not establish this whole-set transaction.

## DATA-SHELL Acceptance Checklist

SH-1 through SH-5 are locally accepted in R84. Three read-only reviews found no
remaining blocking issue, and the full final gates and retained evidence pass.
The checklist records accepted CPU obligations, not new assignments to recreate
that work. This is an inert metadata/storage packet, not a new native execution
profile or executable-refinement proof.

| Item | Owned implementation boundary | Required result |
| --- | --- | --- |
| SH-1: canonical disposal | `kfd_backend/generated_shells.rs`, `kfd_backend/allocation_table.rs` | Before removing control or any record, require canonical ordinals, unique logical IDs, consecutive backend IDs, exact adoption owner/descriptors and no extra table member for that owner. Corrupt the retained plan, not only a copied argument, in rejection tests; all custody and credits must remain intact. |
| SH-2: pure ingress checks | `context/generated_preparation.rs`, `context/generated_shells.rs` | Reject a held-stream/device mismatch or already-installed generated key before invoking `source_mut()` or native currentness. Count callbacks in tests; preserve the existing closing currentness validation for eligible work. |
| SH-3: storage and control tests | `persistent_projection.rs`, `generated_source.rs`, host generated storage/invocation tests | Original buffer pointers, full ordinals and source identity survive conversion; byte-identical replacement storage rejects. Cover stale authority, changed HSACO, occupied transfer destination and replay. No public control extraction, dummy payload or encoded-byte shadow. |
| SH-4: Context and backend tests | `context/generated_preparation/tests.rs`, proposed shell/table test modules | Configured/unconfigured success; whole-roster byte/record pressure; Context/backend ID exhaustion; ordinary read/write/free/copy/compute rejection; held-stream release and exact retirement. Rejections preserve counters, control, maps and credits; unrelated ordinary allocations remain usable. |
| SH-5: acceptance and evidence | Primary gate/evidence integration | All seventeen final GNU/musl, host/fixture/doc, lint, policy and checker gates pass on unchanged source. Focused shell/storage, lifecycle/async, dependency and proof-inventory checks pass. Actual charged-host coverage and synthetic-fixture boundaries are recorded; native hooks remain absent. |

The earlier disposal finding concerned malformed retained state, not an observed
GPU failure. The current implementation rejects duplicate/corrupt members and
extra same-owner entries before removing any record or control. Late Context and
backend mismatch regressions also preserve custody and credits. These fixes pass
the accepted final frozen-source suite.

## Generated Execution Queue

Paths below are relative to `crates/fe2o3-runtime/src/` unless otherwise stated.
New modules are proposed, not existing implementation claims.

| Packet and lead | Dependency / module boundary | Acceptance before advancing |
| --- | --- | --- |
| DATA-SHELL: locally accepted R84 | Storage/source modules, `context/generated_shells.rs`, `kfd_backend/allocation_table.rs` and `kfd_backend/generated_shells.rs`; host invocation. | SH-1 through SH-5 pass. Full unused/read-only ordinals, exact IDs and bounded records remain private without packet/decoder authority exposure. No native effects. |
| NATIVE-1: Native | R86 data conversion and R87 pending allocation custody implemented; CONTROL-1/2/3 remain open. `crates/fe2o3-kfd/src/shared_memory.rs`, `queue_dispatch_binding.rs`, proposed build-owner module and persistent bind terminal custody. | Reuse the existing allocator/conversion. Preserve successful tokens across model projection, then data/premises, packet/plan/generation, control stages and completed output outside all fallible construction/settlement. Inject every error/panic with exact descriptors and charges; no post-dispatch recovery for unpublished data. |
| NATIVE-2: Native; Admission review | NATIVE-1. `crates/fe2o3-kfd/src/queue_live.rs` and `queue_live/fixed_dispatch.rs`; narrow existing abort integration. | Retain primary/auxiliary bootstrap state and successful insertion outputs before closing retake. Separate unchanged pre-effect rejection, partial pre-queue effects and terminal CREATE_QUEUE-attempted failures. Return a usable lane only after complete success; never synthesize a pristine abort continuation from an incomplete constructor. |
| NATIVE-3 / DATA-ADOPT: Native + Admission; Resources reviews charges | Accepted DATA-SHELL, NATIVE-1/2, R80 reservations, R82 abort and R83 lifecycle. Proposed `kfd_backend/generated_adoption.rs`; narrow Context, `compute_state.rs`, `compute_dispatch.rs` and existing adoption hooks. | Switch the rooted shell into non-discardable native custody before effects. Borrow original initialization bytes, bind without publication, retain the exact session/lane/prefix, then retire through R82 abort and exact data disposal. Exercise initial/auxiliary/reused lanes, allocation/map/bind/retake error/panic, empty-prefix Stop and drain. Linux bind/abort/reuse remains a separate gate. |
| CO-1/2/3 COMPLETE-ORACLE: Admission; Resources review | Independent now. Runtime completion contract and preparation fixtures above the accepted R85 host boundary. | Freeze exact prepared/adoption/submission/device/queue/generation identity, one reserved reply and terminal outcomes. Foreign/replayed/late or uncertain completion cannot invoke the counted adapter, remove retained custody or authorize retry. Actual R85/native binding is CO-4 after ISSUE; scripted candidates are not native receipts. |
| COMPLETE-HOST-SUBSTRATE: locally accepted R85 | Host `generated_runtime_arguments/readback.rs`, shared charged decoder transaction and ten new tests. | Actual R80 readback retains its existing debit through decoding. Exact gate/full roster/read-only checks and both disposal settlements precede commit. Final source and auxiliary gates pass; native completion and adapter refinement remain open. |
| ISSUE: Admission + Native | Composed DATA-ADOPT and COMPLETE-ORACLE. Generated driver, Context registration, `compute_state.rs` and `compute_dispatch.rs`. | A private linear permit binds exact resources/submission and survives actual flush/retry. Reject substitution/replay; distinguish definite nonpublication from unknown publication. No broad backend authorization flag or per-invocation backend replacement. |
| CO-4 / COMPLETE: Admission; Resources + Native review | ISSUE and accepted R85. Generated driver, host invocation and backend readback hooks. | Admit exact conclusive completion, fill existing R80 destinations and check closing currentness/native disposition before calling R85. Resolve the reserved engine cell once. Test stale/partial results, currentness loss, decoder panic, retained results and credits after shutdown. Do not rebuild the host decoder or reserve readback/reply twice. |
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

COMPLETE-ORACLE's first deliverables are the CO-1 contract and CO-2/3 fixtures
under `async_engine/tests/owned_tests/preparation_tests/`; the production bridge
follows at CO-4. Freeze exact
invocation/submission/generation and one-reply semantics. Cover duplicate/foreign
completion, native readback failure, observer loss and shutdown; assert in the
fixtures that rejected completion never invokes the counted adapter. R85 already validates the
host roster/read-only bytes and disposal-before-readiness. This work can proceed
before native adoption; fixtures do not create native completion authority.

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
The preceding planning refresh started no SSH, GPU workload, solver or new
runtime test. R84 subsequently passed its complete local source suite as recorded
above; no SSH, GPU or solver run was added. Documentation/link checks are not
substituted for that source acceptance.

## Integration Batches

| Batch | Parallel worker work | Primary integration and exit |
| --- | --- | --- |
| 1: ready now | Native: CONTROL-1 -> CONTROL-2 -> CONTROL-3, then NATIVE-2. Admission: CO-1, then CO-2/3. Resources: VER-1A.1 through .5. | Build on accepted R84/R85/R86/R87; implement independently reviewed native/runtime/journal changes as separate bounded packets. No native publication or cross-run leases. |
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
R86 conversion + R87 allocation -> CONTROL-1/2/3 -> NATIVE-2 -----> DATA-ADOPT
R80 reservations + R82 abort + R83 lifecycle --------------------> DATA-ADOPT
DATA-ADOPT -> ISSUE -> CO-4 / COMPLETE -> API -> GRAPH/DRAIN
CO-1/2/3 completion contract and fixtures -> ISSUE
R85 private host substrate + ISSUE -> runtime COMPLETE
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
target. Local R83/R84/R85/R86/R87 acceptance covers private lifecycle, inert allocation
shells, host readback decoding, atomic native data conversion and pending
allocation custody, not complete
native construction, adoption, runtime completion, proof, hardware or performance.
