# Current Runtime Swarm Work Orders

Planning refresh: 2026-09-11. Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-11T08:01:35Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.
Accepted runtime source remains signed R94
`363ce6b79938def9365016c34f020a00493c1f87`, followed by planning checkpoint
`d464dc442c903e6915fe6ed11dcdfff8fee5db94`.
The working-tree R95 auxiliary-constructor implementation is **unaccepted** and
is not included in this planning packet. Its composed tests do not establish
the complete shared-engine/platform matrix or resolve the finding below.

## Swarm Ownership

Three existing workers performed independent, read-only source audits and
returned these work orders. Their audit turns are complete. The implementation
queues below are assignments, not unattended background jobs. Primary owns
edits, integration, conflict resolution, tests, proofs and publication. Shared
Context/backend/queue changes and builds are serialized.

| Lane and worker | First bounded task | Follow-on queue |
| --- | --- | --- |
| Native: `r66_native_coexistence` | Review R95 opening-currentness repair and local acceptance; specify NATIVE-2B.5's same-engine fixture | Integrated auxiliary matrix -> replacement/insertion -> generated data adoption -> native publication handoff |
| Admission: `r66_runtime_coexistence` | CO-1 allocation-free completion outcome contract and table tests | Exact identity -> reply/custody composition -> issue/completion integration -> typed future -> generated graph/drain |
| Resources: `r66_coexistence_model` | VER-1A.1 Context journal contract and complete mutation inventory | Model/proofs -> bounded journal -> mutation hooks -> cross-run leases; aggregate budgets and residency |
| Primary | Resolve and accept one bounded source packet at a time, with cross-review | Cross-lane integration, formal correspondence, hardware scheduling, matched benchmarks and signed pushes to both topic remotes |

## Native Queue

1. **R95 acceptance blocker.** The public auxiliary constructor currently calls
   `check_currentness` before entering retained construction custody. Move the
   actual observation inside that custody before the model loan. Preserve the
   existing pure `JournalCapacity` rejection, or explicitly document and test a
   changed terminal policy. Error/panic tests must retain the exact original
   parent and execute no preparation, loan or retake. Extend the single-owner
   guard to the auxiliary module and assert the retained parent and both
   ledgers are poisoned. Rerun source gates and compiled negative mutations.
2. **NATIVE-2B.5: integrated auxiliary acceptance.** Run the actual production-used
   sequence after a completed primary constructor, borrowing the same engine,
   foundation, memory/accounts and original platform owners. Cover every
   returned prefix, opening/retake, CREATE, ID collisions, doorbells, gate
   finalization and cleanup failure/panic. Compare exact primary-plus-auxiliary
   owners, not just counters. A second scripted constructor is not acceptance.
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
2. **VER-1A.2/.3: model and journal.** Implement a production-consumed model with
   property proofs, then preallocated Context metadata and move-only tickets.
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
5. **Memory closure.** MEM-DOM-1 establishes aggregate domains and reserved
   terminal headroom. N1B/MEM-3/MEM-4 then cover remaining backing, controls,
   occupied slots and executable residency. An isolated host-image ceiling is
   independently ready. MEM-5 must include commands, captures, replies/results,
   registries, arenas, journals and quarantine, without double-charging aliases.

## Integration And Qualification

```text
R95 repair/acceptance -> 2B.5 -> 2C -> DATA-ADOPT
CO-1 -> CO-2/CO-3 ----------------> ISSUE -> COMPLETE -> API -> GRAPH/DRAIN
VER-1A -> complete VER-1B -> VER-2 -------------------------> input reuse
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
