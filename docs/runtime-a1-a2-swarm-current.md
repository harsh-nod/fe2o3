# Current Runtime Swarm Work Orders

Execution refresh: 2026-09-11. Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-11T08:01:35Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.
Signed R96 `369f99835cfb2af5df9fda45cc828d462ef6b156` locally accepts
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
The remaining native sequence is **complete .5B -> 2C -> DATA-ADOPT**.
R96's duplicate-primary-ID case reaches lower CREATE
`Ambiguous` rejection with no committed ID or outputs. It does not exercise the
later retained auxiliary/SDMA roster rejection.

### In-Progress R97

The uncommitted .5B-1 candidate has a private original-parent adapter, an owning
outer scope and one production-used auxiliary driver. The shared-engine fixture
calls that driver instead of copying its orchestration. The separate early-prefix
oracle and failure matrix now exist: original-parent transport and poisoned
ledgers, exact memory/account/token ownership, real loan/reclaim rejection,
preparation/control faults and cleanup-panic precedence are exercised without
weakening R96's late oracle.

The latest completed development run of `cargo +nightly-2026-04-03 test --locked
--offline -p fe2o3-kfd --all-features --lib auxiliary_cases` passed 13 tests on
unchanged source during that run. Subsequent edits pin the 36-boundary/72-cell
control sweep, assert seal progress, add callback-before-allocation error/panic
cases and strengthen the production-glue guard. Those final edits are formatted
but have not completed their final test run. The local development log is
`/home/harsh/.codex-tmp/r97-native-prefix-second.log`; it is not a published R97
acceptance record.

The next Native task is **R97-3 acceptance**, not reimplementation of the oracle
or matrix. Final lint, frozen-source tests, compiled negative mutations, exact
restoration, full applicable gates and reviewed evidence remain. R96 stays the
accepted baseline; this planning refresh does not publish unfinished source or
claim a new full-suite, formal, hardware or performance result.

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
| Native: `r66_native_coexistence` | R97-3 acceptance of the implemented .5B-1 candidate | .5B-2 local platform composition / .5B-3 CREATE and installation matrix -> replacement/insertion -> generated data adoption -> native publication handoff |
| Admission: `r66_runtime_coexistence` | CO-1 allocation-free completion classifier consumed by ordinary operation progress | Exact identity -> reply/custody composition -> issue/completion integration -> typed future -> generated graph/drain |
| Resources: `r66_coexistence_model` | VER-1A.1 Context journal contract and complete mutation inventory | Model/proofs -> bounded journal -> mutation hooks -> cross-run leases; aggregate budgets and residency |
| Primary | Integrate the next reviewed .5B-1, CO-1 or VER-1A.1 packet without conflicting shared edits | Cross-lane integration, formal correspondence, hardware scheduling, matched benchmarks and signed pushes to both topic remotes |

### Immediate Handoffs

| Lane | First deliverable | Source boundary and cross-review |
| --- | --- | --- |
| Native | R97-3 frozen-source acceptance: final focused/full gates, compiled mutations, exact restoration and reviewed evidence | KFD `queue_live/construction_auxiliary.rs`, its integration tests and shared primary fixtures. Resources checks exact charges; Admission checks terminal-outcome meaning. |
| Admission | CO-1 allocation-free observation classifier, real production consumer and focused tests | Proposed runtime `async_engine/generated_operation/completion_contract.rs`; Primary wires ordinary `async_engine/operation.rs::Operation::advance`. Native reviews outcome authority; no native receipts are invented. |
| Resources | Proposed `docs/runtime-context-version-journal-v1.md`: identity/capacity contract and complete Context mutation/retirement inventory | Inspect existing `context.rs` and its children; proposed journal belongs in `context/versions.rs`. Admission reviews settlement-before-callback ordering; Native reviews logical-to-native identity preservation. |
| Primary | Integrate one reviewed packet at a time and record its exact acceptance scope | Shared Context/backend/queue edits, builds, proof runs, hardware and publication remain serialized. |

CO-1 and VER-1A.1 do not wait for .5B. The .5B test designs can be reviewed
independently, but their source edits share fixtures and must be integrated
serially. Neither `completion_contract.rs` nor `context/versions.rs` exists at
this checkpoint; their rows are assignments, not implementation claims.

The first execution wave is **R97-3 + CO-1 + VER-1A.1**. CO-2 and CO-3 follow
the frozen CO-1 interface; Resources then takes the executable model before the
journal implementation. The Native .5B-2/.5B-3 designs can be reviewed now, but
their integration follows accepted .5B-1 and shares the primary trace, platform
and memory fixtures. Do not defer all Admission/Resources work until Native
finishes. Shared module wiring, Context mutation hooks, backend issue and fixture
edits still pass through Primary one packet at a time.

### Ready And Dependent Work

| Wave | Native | Admission | Resources |
| --- | --- | --- | --- |
| Ready now | Accept R97's existing candidate; review .5B-2/.5B-3 designs | Implement production-used CO-1 | Write VER-1A.1 contract and complete mutation inventory |
| After each lane's first gate | .5B-2 and .5B-3, serialized shared fixtures | CO-2 identity and CO-3 lifecycle composition against frozen CO-1 | VER-1A.2 executable model/proofs, then .3 bounded journal |
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
2. **NATIVE-2B.5: R96 accepts .5A; finish .5B below.** Run the actual
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
| .5B-1 outer settlement | After .5A; `queue_live/construction_auxiliary.rs`, its integration tests and narrow real loan/reclaim fixture forwards | Exercise the production outer ownership boundary after successful original primary construction. Cover opening rejection, every returned preparation/control prefix, operation/reclaim success/error/panic, first-panic preservation, terminal cleanup failure and whole-parent transport. No preparation or retake after opening failure; no CREATE after failed retake; pure capacity rejection remains pre-effect. |
| .5B-2 local platform composition | After .5B-1; primary `integration_platform.rs`, auxiliary platform cases and existing `queue_linux/primary_fixture.rs` helpers | Retain the primary runtime lease while adding auxiliary event/shadow/gate owners. Cover arm failures, exact prepublication cleanup and postpublication retention. Check identities and cleanup ordering. Local Linux mappings are not live KFD qualification. |
| .5B-3 CREATE and installation | After .5B-1; combine with .5B-2 for platform cells; auxiliary integration tests and narrow existing fixture injections | Cover no-effect, indeterminate, malformed and panicking CREATE; output/ID recovery; retained auxiliary/SDMA roster collisions; both late currentness failures; doorbell/gate failures; occupied and reusable slots. No failed installation, spent-generation reuse or lost original owner. Full-source and compiled mutation gates close only this named CPU/local-helper matrix. |

### R97 Work Units

| Unit | Deliverable and exit assertion |
| --- | --- |
| R97-1: early-prefix oracle, candidate implemented | Separate stage-aware oracle preserves R96's stricter late `assert_pair`. Join original primary owners with auxiliary data, preparation/control prefixes, real terminal tokens and exact account/native records, without double counting markers. Observe real foundation location rather than assuming reclaim succeeded. |
| R97-1: terminal transport, candidate implemented | On admitted terminal failure require an empty live parent slot, occupied terminal-parent slot and actual poisoned ledgers. On success require the inverse; pure pre-effect rejection leaves the original parent unchanged. Snapshot equality alone misses poison and slot-transfer omissions. |
| R97-2: ingress/opening, candidate implemented | Pure capacity rejection has no opening, loan, preparation, retake or CREATE. Opening and loan errors/panics preserve the original parent; no retake without a returned loan. Capacity pressure uses model-only history and the real borrowed preflight, not the full public Linux entrypoint. |
| R97-2: returned prefixes, candidate implemented | Exercise the named preparation/control error-and-panic matrix after original primary success. Auxiliary code-memory ordinals are session-global 3-5; preparation-stage ordinals remain local 0-2. Preserve the trace and exact original bytes, owners and charges. Callback failure before allocating or returning owners does not qualify callback-internal unreturned ownership. |
| R97-2: operation/reclaim, candidate implemented | Cross operation success/error/panic with reclaim success/pre-error/pre-panic/post-error/post-panic. Exercise actual loan/reclaim and separately genuine certificate rejection. Failed reclaim permits no CREATE. |
| R97-2: cleanup, candidate implemented | Store the complete terminal parent before cleanup; preserve the first panic even when cleanup panics. |
| R97-3: remaining acceptance | Run final lint and focused tests; compile negative mutations of opening, retake/result gating and full-parent retention, requiring behavioral test failure rather than compilation failure. Restore exact source, run all applicable source/auxiliary gates, retain failure attempts and final evidence, independently review, sign and push both remotes. Only then mark .5B-1 locally accepted. |

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

## Integration And Qualification

```text
R96 .5A -> complete .5B -> 2C -> DATA-ADOPT -----+
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
