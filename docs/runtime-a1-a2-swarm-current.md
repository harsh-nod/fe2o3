# Current Runtime Swarm Work Orders

Execution refresh: 2026-09-11. Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-11T08:01:35Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.
R97 locally accepts **NATIVE-2B.5B-1**, production-used outer settlement and the
named CPU/fake-native prefix matrix, above signed planning checkpoint
`82c8cd854bc6cb8b300a4f5a6b9a2467827dcc6f` and signed R96
`369f99835cfb2af5df9fda45cc828d462ef6b156`.
The [R97 record](evidence/local-r97-auxiliary-outer-settlement-2026-09-11/README.md)
contains seventeen successful source gates, twelve auxiliary checks and four
compiled behavioral negatives. GNU/musl each pass 2,531 tests with five ignored;
all 5,651 non-documentation source identities are unchanged and exactly restored.
No new solver, live KFD or performance result is claimed.

R96 locally accepts
**NATIVE-2B.5A**, production-used auxiliary preparation and CREATE/install phase
composition, above signed planning checkpoint
`10902ca32a853448f79b59cfcf22072e3cdd9325`. The preceding accepted runtime source
is signed R95 `da90a0038c6ec4c697faf0fbe93d597e9fa36e1a`.
The [R96 record](evidence/local-r96-auxiliary-shared-engine-2026-09-11/README.md)
contains seventeen final source gates, twelve auxiliary checks and four compiled
negative mutations. GNU/musl each pass 2,519 runtime tests with five ignored;
5,648 source identities remain unchanged and restored. The failed first
full-source attempt and formatting check remain historical, not acceptance.

At accepted R96, the shared-engine fixtures cover success and eight late-failure
cells with original owners, plus a production-glue guard. They use a fixture outer scope
and scripted platform leaves, not complete concrete Linux outer settlement.
The remaining native sequence is **.5B-2/.5B-3 -> 2C -> DATA-ADOPT**.
R96's duplicate-primary-ID case reaches lower CREATE
`Ambiguous` rejection with no committed ID or outputs. It does not exercise the
later retained auxiliary/SDMA roster rejection.

### Accepted R97 Scope

The .5B-1 implementation has a private original-parent adapter, an owning
outer scope and one production-used auxiliary driver. The shared-engine fixture
calls that driver instead of copying its orchestration. The separate early-prefix
oracle and failure matrix now exist: original-parent transport and poisoned
ledgers, exact memory/account/token ownership, real loan/reclaim rejection,
preparation/control faults and cleanup-panic precedence are exercised without
weakening R96's late oracle.

Fourteen integrated functions, twelve new over R96, cover 369 shared auxiliary
driver runs: 366 failures and three successes. One separate primary-only capacity
fixture calls the real borrowed preflight twice. The control sweep pins 36
boundary occurrences and 72 error/panic cells; native allocation/map/seal/write
and projection failures retain exact returned or in-session owners. Both frozen
and restored construction suites pass 53 tests. All four final mutations compile
and fail their intended behavioral test; the retake mutation detects lost error
precedence, not an observed extra CREATE.

The next Native task is **.5B-2 local platform composition**, followed by
**.5B-3 CREATE/installation coverage**. R97's scripted platform leaves do not
qualify the concrete Linux platform composition. Callback failures are tested
before allocating/returning owners, not for callback-internal unreturned custody.
Append-only pending-slot tests do not qualify native released-slot reuse.

## Swarm Ownership

At the user's renewed swarm request, three existing workers independently
reviewed the current source, the R97 candidate and the remaining roadmap. They
returned the bounded work orders below. Their read-only review turns
are complete. The implementation queues below are assignments, not unattended
background jobs. Primary owns
edits, integration, conflict resolution, tests, proofs and publication. Shared
Context/backend/queue changes and builds are serialized.

| Lane and worker | First bounded task | Follow-on queue |
| --- | --- | --- |
| Native: `r66_native_coexistence` | .5B-2 local platform composition after accepted .5B-1 | .5B-3 CREATE and installation matrix -> replacement/insertion -> generated data adoption -> native publication handoff |
| Admission: `r66_runtime_coexistence` | CO-1 allocation-free completion classifier consumed by ordinary operation progress | Exact identity -> reply/custody composition -> issue/completion integration -> typed future -> generated graph/drain |
| Resources: `r66_coexistence_model` | VER-1A.1 Context journal contract and complete mutation inventory | Model/proofs -> bounded journal -> mutation hooks -> cross-run leases; aggregate budgets and residency |
| Primary | Integrate the next reviewed .5B-2, CO-1 or VER-1A.1 packet without conflicting shared edits | Cross-lane integration, formal correspondence, hardware scheduling, matched benchmarks and signed pushes to both topic remotes |

### Immediate Handoffs

| Lane | First deliverable | Source boundary and cross-review |
| --- | --- | --- |
| Native | .5B-2 fixture setup before successful primary construction, then the unchanged shared auxiliary driver | KFD primary `integration_platform.rs`, auxiliary integration tests and `queue_linux/primary_fixture.rs`. Resources checks charges; Admission checks terminal-outcome meaning. |
| Admission | CO-1 allocation-free observation classifier, real production consumer and focused tests | Proposed runtime `async_engine/generated_operation/completion_contract.rs`; Primary wires ordinary `async_engine/operation.rs::Operation::advance`. Native reviews outcome authority; no native receipts are invented. |
| Resources | Proposed `docs/runtime-context-version-journal-v1.md`: identity/capacity contract and complete Context mutation/retirement inventory | Inspect existing `context.rs` and its children; proposed journal belongs in `context/versions.rs`. Admission reviews settlement-before-callback ordering; Native reviews logical-to-native identity preservation. |
| Primary | Integrate one reviewed packet at a time and record its exact acceptance scope | Shared Context/backend/queue edits, builds, proof runs, hardware and publication remain serialized. |

CO-1 and VER-1A.1 do not wait for .5B. The .5B test designs can be reviewed
independently, but their source edits share fixtures and must be integrated
serially. Neither `completion_contract.rs` nor `context/versions.rs` exists at
this checkpoint; their rows are assignments, not implementation claims.

The first execution wave is **.5B-2 + CO-1 + VER-1A.1**. CO-2 and CO-3 follow
the frozen CO-1 interface; Resources then takes the executable model before the
journal implementation. The Native .5B-2/.5B-3 designs can be reviewed now, but
their integration now follows accepted .5B-1 and shares the primary trace, platform
and memory fixtures. Do not defer all Admission/Resources work until Native
finishes. Shared module wiring, Context mutation hooks, backend issue and fixture
edits still pass through Primary one packet at a time.

### Ready And Dependent Work

| Wave | Native | Admission | Resources |
| --- | --- | --- | --- |
| Ready now | .5B-2 local helper composition; review .5B-3 matrix | Implement production-used CO-1 | Write VER-1A.1 contract and complete mutation inventory |
| After each lane's first gate | .5B-3 complete CREATE/install matrix | CO-2 identity and CO-3 lifecycle composition against frozen CO-1 | VER-1A.2 executable model/proofs, then .3 bounded journal |
| Integration | 2C replacement/insertion, then nonpublishing DATA-ADOPT | ISSUE with Native, then CO-4/COMPLETE and typed API | .4/.5 initial mutation hooks, then complete VER-1B coverage |
| A1/A2 closure | Native depth, memory pressure and overlap qualification | Generated GRAPH/DRAIN and end-to-end typed execution | VER-2 cross-run leases, aggregate accounting and residency closure |

CO-2/3 do not require native adoption. First non-reusing ISSUE does not require
cross-run leases, but its mutation hook must be specified with Resources.
Cross-run reuse does require complete VER-1B and VER-2. Formal correspondence is
reviewed with each packet; it is not deferred to the final hardware wave.

## Native Queue

1. **R95 locally accepted.** Opening currentness now runs inside retained
   construction custody before the model loan. The shared borrowed preflight
   preserves pure `JournalCapacity` rejection. Error/panic tests retain the exact
   original parent without preparation, loan or retake; the auxiliary single-owner
   guard and both ledger poison assertions are present. Eight compiled mutations
   reject and exact source restoration passes. All seventeen final source gates
   and twelve auxiliary checks pass; broader native integration remains open.
2. **NATIVE-2B.5: R96 accepts .5A; R97 accepts .5B-1.** Finish .5B-2/.5B-3 below.
   Run the actual
   production-used sequence after a completed primary constructor, borrowing
   the same engine, foundation, memory/accounts and original platform owners.
   Compare exact primary-plus-auxiliary owners, not just counters. A second
   scripted constructor is not acceptance.
3. **NATIVE-2C: replacement/insertion custody.** Retain consumed session/data
   before planning and returned owners through closing observation. Cover
   occupied slots, exhausted generations, old/new identities and recycled
   versus pristine-abort provenance.
4. **DATA-ADOPT.** Install generated adoption only after 2B/2C acceptance. Enter
   non-discardable native custody before effects; bind original bytes without
   publication. Cover initial, auxiliary and reused lanes, partial failure,
   Stop/drain and exact abort/disposal. Reuse existing R80 reservations and R83
   lifecycle rather than adding duplicate allocation or completion ownership.
5. **ISSUE handoff.** With Admission, bind one linear permit to actual resources
   and one logical submission through deferred flush/retry. Definite
   nonpublication and uncertain publication remain distinct.

### Auxiliary Acceptance Packets

Paths below are relative to `crates/fe2o3-kfd/src/`. Primary owns edits and
execution; Native owns each design/review handoff.

| Packet | Dependency and affected files | Exit gate |
| --- | --- | --- |
| .5A accepted R96 | `queue_live.rs`, `queue_live/construction_auxiliary` and shared primary/preparation fixtures | Frozen focused suite, all four final compiled negative mutations, all seventeen source gates and twelve auxiliary checks pass. Exact source is restored; final evidence is independently reviewed. Full outer/platform acceptance remains .5B. |
| .5B-1 accepted R97 | After .5A; `queue_live/construction_auxiliary.rs`, its integration tests and narrow real loan/reclaim fixture forwards | Shared production outer driver after successful primary construction; named early-prefix/operation/reclaim/cleanup matrix, full-parent transport and pure borrowed capacity rejection. Frozen/restored 53-test suites, all 17 source gates, 12 auxiliary checks and four behavioral mutations pass. Concrete local platform and complete CREATE/install coverage remain below. |
| .5B-2 local platform composition | After .5B-1; primary `integration_platform.rs`, auxiliary platform cases and existing `queue_linux/primary_fixture.rs` helpers | Retain the primary runtime lease while adding auxiliary event/shadow/gate owners. Cover arm failures, exact prepublication cleanup and postpublication retention. Check identities and cleanup ordering. Local Linux mappings are not live KFD qualification. |
| .5B-3 CREATE and installation | After .5B-1; combine with .5B-2 for platform cells; auxiliary integration tests and narrow existing fixture injections | Cover no-effect, indeterminate, malformed and panicking CREATE; output/ID recovery; retained auxiliary/SDMA roster collisions; both late currentness failures; doorbell/gate failures; occupied and reusable slots. No failed installation, spent-generation reuse or lost original owner. Full-source and compiled mutation gates close only this named CPU/local-helper matrix. |

### R97 Completed Work Units

| Unit | Deliverable and exit assertion |
| --- | --- |
| R97-1: early-prefix oracle, locally accepted | Separate stage-aware oracle preserves R96's stricter late `assert_pair`. Join original primary owners with auxiliary data, preparation/control prefixes, real terminal tokens and exact account/native records, without double counting markers. Observe real foundation location rather than assuming reclaim succeeded. |
| R97-1: terminal transport, locally accepted | On admitted terminal failure require an empty live parent slot, occupied terminal-parent slot and actual poisoned ledgers. On success require the inverse; pure pre-effect rejection leaves the original parent unchanged. Snapshot equality alone misses poison and slot-transfer omissions. |
| R97-2: ingress/opening, locally accepted | Pure capacity rejection has no opening, loan, preparation, retake or CREATE. Opening and loan errors/panics preserve the original parent; no retake without a returned loan. Capacity pressure uses model-only history and the real borrowed preflight, not the full public Linux entrypoint. |
| R97-2: returned prefixes, locally accepted | Exercise the named preparation/control error-and-panic matrix after original primary success. Auxiliary code-memory ordinals are session-global 3-5; preparation-stage ordinals remain local 0-2. Preserve the trace and exact original bytes, owners and charges. Callback failure before allocating or returning owners does not qualify callback-internal unreturned ownership. |
| R97-2: operation/reclaim, locally accepted | Cross operation success/error/panic with reclaim success/pre-error/pre-panic/post-error/post-panic. Exercise actual loan/reclaim and separately genuine certificate rejection. Failed reclaim permits no CREATE. |
| R97-2: cleanup, locally accepted | Store the complete terminal parent before cleanup; preserve the first panic even when cleanup panics. |
| R97-3: local acceptance complete | Final lint, frozen/restored tests, four compiled behavioral negatives, all 17 source gates and 12 auxiliary checks pass with 5,651 exact source identities. Retained evidence includes preliminary failed attempts; independent source/evidence review covers only the named .5B-1 scope. |

### Native First Handoff

Install one fixture-local gate before original primary construction and retain
the same local resources through auxiliary construction. Make runtime admission
fallible before minting a fixture owner, so a real gate rejection cannot create
a phantom owner. Reuse actual local registration/phase, shadow initialization,
protection, restore, publication and cleanup helpers; do not create a second
constructor or a fake production runtime descriptor whose Drop touches the
process-global gate.

The first matrix covers success, arm/event/install rejection, shadow-init and
restore error/panic, cleanup panic before/after disposal, late doorbell/gate
failure and cross-event substitution. Assert original primary retention,
auxiliary-only unpublished cleanup and both published payloads retained after
late failure. Inspect owned local mappings before fixture disposal. Synthetic
events and independent VM reservations are not real KFD BO aliasing, native
doorbell execution, runtime-enable ioctls or confirmed native teardown.

The subsequent 2C packet owns existing replacement in
`queue_live.rs::recreate_compute_aql_queue_with_fixed_dispatch` and binding in
`queue_live/fixed_dispatch.rs`, not a second constructor. DATA-ADOPT then owns
the proposed runtime `kfd_backend/generated_adoption.rs` and narrow existing
shell/Context/compute hooks. Local slot-reuse tests do not establish a native
destroy/recreate cycle.

Full 2B acceptance still does not establish callback-internal unreturned-owner
custody, concurrent bootstrap, live device behavior, new formal refinement or
performance. Keep those boundaries explicit rather than absorbing them into a
green fixture count.

## Admission Queue

1. **CO-1: outcomes, independently ready.** Separate observations, reply
   disposition and permission to dispose owners. Table-test pending, rejected,
   success-candidate, failure/cancellation, quiescence without result, uncertainty
   and already-settled states. Success observation alone permits no decode or
   disposal; rejected observation permits re-observation, not reissue.
2. **CO-2: exact identity.** Independently mutate source/preparation, Context
   generation, submissions, device/stream/lane, queue/publication occurrence and
   every allocation incarnation. Reject substitution/replay without consuming
   the retained owner or reserved reply.
3. **CO-3: reply and custody composition.** After CO-1, review alongside CO-2.
   Cover repeated rejection, late failure, partial retirement, adapter panic,
   observer loss, Stop/shutdown and waker replacement. Assert callback ordering,
   zero/one adapter calls and one existing reply. Use a data-only adapter;
   runtime must not depend on host-private decoding.
4. **ISSUE -> CO-4/COMPLETE.** Requires DATA-ADOPT and CO-1/2/3. Exact native
   completion, complete readback, closing currentness and native disposition
   precede decode/readiness. Connect the existing R85 decoder; do not reserve
   the R80 reply/readback roster again.
5. **API -> GRAPH/DRAIN.** One executor-neutral typed future, with blocking as
   a join over that same path. Test poll/wake races, capacity, reentrancy,
   cancellation and owner-local non-Send contracts. Then qualify repeated
   generated graphs, dependencies, accepted-prefix drain and dropped observers.
   Cross-run input reuse additionally requires the complete version journal.

### CO-1 First Handoff

Add the private classifier at the proposed
`async_engine/generated_operation/completion_contract.rs`. Its initial real
production consumer is ordinary `async_engine/operation.rs::Operation::advance`
after submission, not generated preparation, whose adoption hooks remain absent.
Borrow `&Result<RuntimeCompletionStatusV1, RuntimeErrorV1<E>>` so non-`Clone`
backend errors remain owned by their existing path. Preserve one poll/query
sequence, rejection counters, raw reply timing and Context custody.

Classification is descriptive: no class grants typed decode, disposal, retry or
native authority. Use existing `Reply::complete` / `r61_reply_may_resolve_v1` for
the already-settled gate instead of adding another settlement state machine.
Generated preparation, the R80 roster and its reserved reply are downstream
consumers, not resources for CO-1 to recreate. Do not add another public
completion API, native receipt, readback allocation, decoder or host dependency.

| Observation family | Required classification/test |
| --- | --- |
| Pending or rejected | Reply remains pending; retain ownership. Re-observation is allowed, reissue is not. |
| Succeeded | Candidate only, with no decode, readiness or disposal authority. |
| Failed or cancelled | No typed output; failure reporting and confirmed native retirement are separate. |
| Quiescent without result | No successful content or decode permission. |
| Terminal or uncertain | Error reporting cannot release potentially live ownership or authorize retry. |
| Already settled or replayed | No second reply, adapter call or state reopening. |

The first focused tests cover each row, non-consuming/non-`Clone` error handling,
sticky settlement and allocation-free classification over prebuilt inputs using
the existing test-only allocator counter. Reuse ordinary rejected-poll,
cancellation and DRN-3A regressions to check production behavior. No adapter is
called by this classifier. Later
CO-3 composes the existing reservation/adoption Harness rather than duplicating
its Stop, observer-loss and waker machinery. Production adoption hooks remain
absent; R85 decoding is implemented but not native-connected.

## Resources Queue

1. **VER-1A.1: contract/inventory, independently ready.** Freeze exact
   Context/allocation/device/writer identities, finite capacity, nonwrapping
   `Available/Pending/Unknown` versions, whole-destination rosters and retirement
   rules. Graph-local history is not persistent Context authority.
2. **VER-1A.2 -> .3: model, then journal.** Implement a production-consumed model
   with property proofs, then preallocated Context metadata and move-only tickets.
   Whole-roster admission/settlement is atomic. Wrong writers, omitted members,
   replay, overflow and first/middle/last failures cannot partially mutate the
   journal; dropped tickets cannot restore availability.
3. **VER-1A.4/.5 -> VER-1B: hooks and acceptance.** Integrate host-write and
   ordinary/graph-copy hooks, preserving logical destinations before backend
   translation. Invalidate before effects and settle before callbacks. Extend
   one mutation family at a time through peer copies, every launch family,
   generated issue, currentness, cancellation, cleanup and retirement.
4. **VER-2: input leases.** Enable only after complete mutation coverage. Reject
   outside-graph writes, overlaps, stale/foreign/replayed identities and unknown
   publication. Caller-declared kernel access is insufficient authority.
5. **Memory closure.** Split MEM-DOM-1 into its independently ready domain
   contract and subsequent shared-accounting/Context integration. Test repeated
   Context creation, parent exhaustion, foreign children and simultaneous
   quarantine with reserved terminal headroom. N1B -> MEM-3 then cover remaining
   kernarg/executable backing, controls and occupied slots. MEM-4A's isolated
   host-image ceiling is independently ready; MEM-4B native residency follows
   backing/control integration. MEM-5 must include commands, captures,
   replies/results, registries, arenas, journals and quarantine, without
   double-charging aliases. Concurrent-bootstrap pre-effect reservation is a
   separate missing contract, not permission to relax the creation gate.

Preserve logical destination rosters before `prepare_context_launch_v1`,
`prepare_context_copy_v1` and `peer_copy` translate identities. Journal
settlement must precede callbacks in `transition_submission_status`. Existing
single-account credits are not aggregate domains. Complete version hooks block
cross-run input leases, not the first non-reusing generated ISSUE; freeze that
submission's mutation hook now without inventing a second identity allocator.

### VER-1A.1 First Handoff

Freeze a separate journal/profile capacity and reject allocation admission before
creating an untrackable allocation. Context's 1,048,576-entry bounds and the
credit engine's 65,536-record bound are different quantities, not a journal
capacity choice. Bounded metadata alone does not establish aggregate charging;
bootstrap/account-arena charging remains MEM-DOM/MEM-5 work.

The contract must name the following actual mutation surfaces under
`crates/fe2o3-runtime/src/` before journal implementation:

| Family | Inventory boundary |
| --- | --- |
| Allocation lifecycle | `context.rs::allocate/release_allocation/cleanup`; `context/generated_shells.rs::install_generated_shells_v1/retire_generated_shells_v1` |
| Host writes | `write_allocation`: range validation before journal admission, journal admission before backend effects |
| Ordinary/graph copies | Shared `prepare_context_copy_v1` and `submit_prepared_copy_v1`; preserve original logical destination/device |
| Peer copies | `peer_copy`: preserve logical identity before backend translation |
| Kernel writers | Shared `prepare_context_launch_v1` and `submit_prepared_launch_v1`, including snapshot/graph/atomic/collective families and future generated ISSUE |
| Settlement/retirement | `transition_submission_status`, `completion_backend_result`, `mark_stream_quiescent`, cancellation/drain, protocol sealing, terminal/panic/currentness and cleanup |

Use existing Context IDs for synchronous writers and exact runtime submission
IDs for asynchronous writers. The Context retains complete pending rosters;
dropped tickets cannot restore availability. Available describes mutation
lineage, not initialized or correct contents. Complete VER-1B coverage remains a
prerequisite for cross-run leases, not for the first non-reusing generated ISSUE.

Separate a monotonic attempt epoch from content lineage. Exact successful writer
settlement commits the new lineage. A separately named `NoEffect` settlement
requires exact, attempt-bound definite-no-write evidence and restores the prior
lineage without rolling back the attempt epoch or Context identity. Rejected
poll/query observations for an already issued writer are not `NoEffect`
evidence. Cancellation, quiescence without result and unknown publication cannot
restore reusable authority. Test wrong/omitted roster members, epoch wrap,
NoEffect epoch rollback, rejected polling as NoEffect and lineage zero being
misinterpreted as initialized contents.

### Independent Resource Packets

| Packet | Ready boundary and dependency | Exit gate |
| --- | --- | --- |
| MEM-DOM-1A -> 1B | Domain/terminal-headroom contract now; shared accounting and Context/session integration afterward | One exact root/child hierarchy, charged bootstrap/arenas, parent exhaustion, repeated Contexts and simultaneous quarantine without premature refunds. Concurrent-bootstrap reservation remains a separate contract. |
| MEM-N1B-1 -> N1B-2 -> MEM-3 | Native backing, then AQL/USERPTR/control/occupied-slot integration with Native | Whole compound admission before effects; exact retained allocation/map/error/panic prefixes. Reuse R70 admission and existing ledgers. |
| MEM-4A -> 4B | Isolated host-image ceiling now; native residency after backing/control integration | Repeated loads, live leases, rejected eviction and ambiguous unload. Executable GTT is not VRAM. |
| PRF + MEM-5 | Incremental adapter correspondence and total resource inventory | Include commands, captures, results, journals, arenas, quarantined roots and callback/panic-payload exclusions; authenticate named properties separately from tests and hardware. |

R65 graph-local history, R67/R70 single-account credits, optional N1/N2 backing
budgets, both cache policies and R73/R80/R85 charged storage already exist.
Extend and qualify them; do not rebuild them as a second accounting system.
They do not establish persistent Context version authority, aggregate ceilings
or newer adapter refinement.

## Integration And Qualification

```text
R97 .5B-1 -> .5B-2/.5B-3 -> 2C -> DATA-ADOPT ---+
CO-1 -> CO-2/CO-3 -----------------------------+-> ISSUE -> COMPLETE -> API -> GRAPH/DRAIN
VER-1A -> complete VER-1B -> VER-2 ------------------------------------------> cross-run input reuse
```

Independently ready work can fill a free worker slot: SCALE-1A correctness
fixtures, a native N1/N2/cache budget harness, R76/R78/R82 qualification cells,
aggregate-domain design, and incremental adapter correspondence. Existing
overlap/drain runners and ordinary cache policies need qualification, not
duplicate implementation. Backing/control/slot admission precedes larger native
depth. Host queue depth alone does not establish native in-flight depth.

Each accepted source packet needs focused tests, applicable full-source gates,
compiled negative mutations, exact source identities and independent review.
Authenticated formal refinement, live Linux/KFD behavior and performance are
separate acceptance columns. Protected generated execution also requires exact
compiler/machine evidence; CPU fixtures cannot manufacture that prerequisite.

Record each packet in four separate columns: **CPU/source acceptance, formal
correspondence, live Linux/KFD qualification, matched performance**. Require
exact source/toolchain identities and retained failure attempts. A proof
inventory audit is not a solver run, and a successful fixture is not a proved
production adapter. Review correspondence incrementally, starting with the
existing R83 lifecycle and R82 cleanup boundaries, instead of deferring proofs
until after API integration.

Primary schedules MI300X work serially, checks shared-machine availability,
uses task-owned processes/staging and cleans up only those owned resources.
Disruptive fault tests require an isolated window. Matched HIP/HSA/KFD producers
must validate complete outputs before measuring latency, throughput, bandwidth,
CPU use, memory bounds and tail latency. Physical overlap needs device timelines.
No full parity or orders-of-magnitude improvement is accepted by this plan.

## Later Milestones

These remain outside A1/A2 closure and reuse the same three worker slots.

| Milestone | Lead and exit scope |
| --- | --- |
| A3: local multi-GPU | Native with Admission/Resources: topology, sharding/replicas, peer/staged transfer and group drain; all admitted GPUs, exact versions and partial-failure isolation |
| A4: distributed control | Admission/Primary after distributed semantics contracts: authenticated sessions, membership epochs, receipts and drain across two hosts without duplicate publication |
| A5: data/collectives | Native transport plus Resources versions/credits: bounded reference transfers and separately qualified broadcast, reduce-scatter, all-gather and all-reduce |
| A6: failure qualification | Admission/Primary: participant, network, device, transfer and collective faults; no unsafe replay, early disposal or false complete outputs |
| A7: production performance | Native/Primary: precommitted single-device, multi-GPU and two-host gates; matched baselines, recovery costs and direct-KFD dependency/symbol audits |

Broad device-language support and production atomics/collectives also depend on
their compiler semantic-to-machine contracts. Closing A1/A2 alone does not close
those ownership boundaries or issue #182.

Protected scalar GEMM still depends on the separately open
[#214 machine/IEEE refinement](https://github.com/harsh-nod/fe2o3/issues/214),
checked through the GitHub API at this refresh (`updatedAt`
`2026-09-01T18:56:15Z`). Keep Worker/capsule, deployment and debugger release
work with their owning teams; this three-lane A1/A2 assignment does not close
all issues carrying the runtime label.
