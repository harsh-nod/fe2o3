# Current Runtime Swarm Work Orders

Execution refresh: 2026-09-11. Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-11T08:01:35Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.
The committed, locally accepted runtime checkpoint is signed R95
`da90a0038c6ec4c697faf0fbe93d597e9fa36e1a`, observed on both topic remotes at
this refresh. R95 accepts auxiliary-constructor custody above signed planning
checkpoint `cb29dc6216cdf57c76d1f952264ceed9e257c40a`; preceding accepted runtime
source is R94 `363ce6b79938def9365016c34f020a00493c1f87`.
The [R95 record](evidence/local-r95-auxiliary-custody-2026-09-11/README.md) includes
seventeen final source gates, twelve auxiliary checks and eight compiled negative
mutations. Each GNU/musl runtime suite passes 2,516 tests with five ignored.
Opening currentness is now rooted, but composed tests do not establish the
complete shared-engine/platform matrix.

**R96 / NATIVE-2B.5A is an uncommitted candidate, not accepted evidence.** It
extracts production-used auxiliary preparation and CREATE/install phases and
adds same-engine fixtures after successful primary construction. The candidate
covers success and eight late-failure cells with original owners, plus a
production-glue guard. Its first full-source attempt stopped at a stale guard;
the repaired guard passed a focused rerun, but the subsequent format check
still requires a correction. Fresh final-source gates, final-source mutations,
evidence review and signed publication remain outstanding. This planning
refresh leaves that source patch unchanged and publishes no new acceptance.

The remaining native sequence is therefore **accept .5A -> complete .5B ->
2C -> DATA-ADOPT**. R96's duplicate-primary-ID case reaches lower CREATE
`Ambiguous` rejection with no committed ID or outputs. It does not exercise the
later retained auxiliary/SDMA roster rejection.

## Swarm Ownership

Three existing workers performed fresh independent, read-only source audits of
R95 plus the R96 candidate and returned these work orders. Their audit turns
are complete. The implementation queues below are assignments, not unattended
background jobs. Primary owns
edits, integration, conflict resolution, tests, proofs and publication. Shared
Context/backend/queue changes and builds are serialized.

| Lane and worker | First bounded task | Follow-on queue |
| --- | --- | --- |
| Native: `r66_native_coexistence` | Review .5A acceptance, then .5B-1 original-parent outer settlement | .5B-2 local platform composition / .5B-3 CREATE and installation matrix -> replacement/insertion -> generated data adoption -> native publication handoff |
| Admission: `r66_runtime_coexistence` | CO-1 allocation-free completion outcome contract and table tests | Exact identity -> reply/custody composition -> issue/completion integration -> typed future -> generated graph/drain |
| Resources: `r66_coexistence_model` | VER-1A.1 Context journal contract and complete mutation inventory | Model/proofs -> bounded journal -> mutation hooks -> cross-run leases; aggregate budgets and residency |
| Primary | Finish the existing R96 formatting/final-validation packet before new native source changes | Cross-lane integration, formal correspondence, hardware scheduling, matched benchmarks and signed pushes to both topic remotes |

### Immediate Handoffs

| Lane | First deliverable | Source boundary and cross-review |
| --- | --- | --- |
| Native | .5A frozen-source acceptance, then an exact .5B-1 opening/operation/reclaim error-and-panic matrix | KFD `queue_live/construction_auxiliary.rs`, its integration tests and shared primary fixtures. Resources checks exact charges; Admission checks terminal-outcome meaning. |
| Admission | CO-1 allocation-free observation/reply/disposal table and focused tests | Proposed runtime `async_engine/generated_operation/completion_contract.rs`, existing generated preparation/reply tests. Native reviews outcome authority; no native receipts are invented. |
| Resources | VER-1A.1 identity/capacity contract and a complete Context mutation/retirement inventory | Inspect existing `context.rs` and its children; proposed journal belongs in `context/versions.rs`. Admission reviews settlement-before-callback ordering; Native reviews logical-to-native identity preservation. |
| Primary | Integrate one reviewed packet at a time and record its exact acceptance scope | Shared Context/backend/queue edits, builds, proof runs, hardware and publication remain serialized. |

CO-1 and VER-1A.1 do not wait for .5B. The .5B test designs can be reviewed
independently, but their source edits share fixtures and must be integrated
serially. Neither `completion_contract.rs` nor `context/versions.rs` exists at
this checkpoint; their rows are assignments, not implementation claims.

## Native Queue

1. **R95 locally accepted.** Opening currentness now runs inside retained
   construction custody before the model loan. The shared borrowed preflight
   preserves pure `JournalCapacity` rejection. Error/panic tests retain the exact
   original parent without preparation, loan or retake; the auxiliary single-owner
   guard and both ledger poison assertions are present. Eight compiled mutations
   reject and exact source restoration passes. All seventeen final source gates
   and twelve auxiliary checks pass; broader native integration remains open.
2. **NATIVE-2B.5: accept .5A, then finish .5B below.** Run the actual
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
| .5A acceptance | Existing R96 candidate in `queue_live.rs`, `construction_auxiliary` and shared primary/preparation fixtures | Correct formatting, freeze source, rerun focused tests, all four final compiled negative mutations, all seventeen source gates and twelve auxiliary checks. Restore exact source, review evidence and sign/push. Do not rewrite the constructor or substitute preliminary runs. |
| .5B-1 outer settlement | After .5A; `queue_live/construction_auxiliary.rs`, its integration tests and narrow real loan/reclaim fixture forwards | Exercise the production outer ownership boundary after successful original primary construction. Cover opening rejection, every returned preparation/control prefix, operation/reclaim success/error/panic, first-panic preservation, terminal cleanup failure and whole-parent transport. No preparation or retake after opening failure; no CREATE after failed retake; pure capacity rejection remains pre-effect. |
| .5B-2 local platform composition | After .5B-1; primary `integration_platform.rs`, auxiliary platform cases and existing `queue_linux/primary_fixture.rs` helpers | Retain the primary runtime lease while adding auxiliary event/shadow/gate owners. Cover arm failures, exact prepublication cleanup and postpublication retention. Check identities and cleanup ordering. Local Linux mappings are not live KFD qualification. |
| .5B-3 CREATE and installation | After .5B-1; combine with .5B-2 for platform cells; auxiliary integration tests and narrow existing fixture injections | Cover no-effect, indeterminate, malformed and panicking CREATE; output/ID recovery; retained auxiliary/SDMA roster collisions; both late currentness failures; doorbell/gate failures; occupied and reusable slots. No failed installation, spent-generation reuse or lost original owner. Full-source and compiled mutation gates close only this named CPU/local-helper matrix. |

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

## Integration And Qualification

```text
accept .5A -> complete .5B -> 2C -> DATA-ADOPT --+
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
