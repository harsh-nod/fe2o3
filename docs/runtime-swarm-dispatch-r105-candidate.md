# Runtime Swarm Dispatch: R105 Candidate

Planning refresh: 2026-09-12, against accepted R104
`b69a6f21c2beb0aad870d3f4b8cdc2eb183f456f` and the uncommitted R105 candidate.
[Issue #182](https://github.com/harsh-nod/fe2o3/issues/182) remains open; the
GitHub API still reports `updatedAt` `2026-09-12T10:51:18Z`.
This dispatch supersedes immediate assignment/status rows in the
[current board](runtime-a1-a2-swarm-current.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their accepted evidence.

R104 ordinary live-rebind custody is locally accepted. R105/N2 pristine
rebind custody is implemented but **not accepted**. Its frozen seventeen-gate
source campaign completed during this dispatch; GNU/musl each passed 2,573
tests with five ignored, and all 5,664 source identities remained unchanged.
Frozen/restored focused gates, auxiliary gates, nine compiled negatives,
evidence review and runtime publication remain pending. These are observations
of the uncommitted candidate, not archived acceptance. This planning record
does not accept R105 or add formal, hardware or performance qualification.

## Ownership And First Assignments

The user-requested three-worker review is complete. All workers were read-only;
none edited source, ran builds/proofs, used SSH or published changes. The rows
below assign subsequent implementation packets, not unattended background jobs.
Primary owns edits, shared module wiring, integration, serialized validation,
hardware scheduling and signed publication to both repositories.

| Lane / worker | First implementation packet | Decisive exit |
| --- | --- | --- |
| Native / `native_replacement_handoff` | N3-C: coherent initialization custody, after the current source freeze is released | Original CPU/mapped tokens survive every copy/map error or panic; borrowed source cannot escape. Reuse existing transition custody. |
| Admission / `submission_identity_handoff` | C1 / CO-2A: Context submission-identity tests | Five coordinates across eight ingresses; genuine cached completion, backend-ID reuse, destroyed streams and rejection precedence. No backend entry or unrelated-owner mutation on rejection. |
| Resources / `r102_evidence_review` | V1 / VER-1A.2a: executable writer-issuance model | Bounded Reserved slots, exact existing-ID lookup and explicit pre-effect abort. Capacity, replay, stale-slot and overflow failures leave state unchanged. Model-only. |
| Primary | N2-V: finish R105 acceptance before runtime publication | Full and focused gates, nine compiled negatives, exact source restoration, reviewed evidence and explicit scope. Preserve every failed attempt. |

N3-C, C1 and V1 are logically independent. Shared files and builds remain
serialized; parallel review does not authorize simultaneous mutation of the
frozen worktree. C1/C2/C3 and V1 need not wait for native DATA-ADOPT.

## Native Packets

Paths in this table are under `crates/fe2o3-kfd/src/` unless qualified.
The mechanisms already exist; these packets close their remaining custody and
composition gaps rather than introduce another queue, allocator or loan engine.

| Packet | File boundary | Dependency and acceptance |
| --- | --- | --- |
| N3-C: coherent initialization | `shared_memory/coherent_initialization.rs`, borrowed-initialization tests | Root CPU and mapped owners in place across copy/map failure. Reuse R88 allocation/map transitions; do not store borrowed input beyond the synchronous call. |
| N3-D: device initialization | `shared_memory.rs` device initializer and authenticated-source helpers | Independent lower packet. Retain original source and lease through validation, CPU map/write/full verification/unmap and GPU map. Preserve PUBLIC allocation and repeated-byte semantics. |
| N3-L: live insertion/replacement | `queue_live/fixed_dispatch.rs`, lane facade | Consume N3-C/D and accepted live settlement. Reserve identity Vec capacity before loan/effects; retain completed output through retake; commit count/ordinal afterward. Cover explicit insertion, remembered-hole replacement and append. |
| N4-R: lower release | `queue_dispatch_binding.rs::release*`, `shared_memory.rs::release_fixed_dispatch_data` | Lower review can start independently. First/middle/last cleanup failures retain untouched controls/data and exact records. Never restore confirmed disposed authority. |
| N4-L: live detach/release | Recycled detach and detached/retained-control release in `queue_live/fixed_dispatch.rs` | Requires N4-R and live settlement. Failed retake after disposal cannot create retry rights or a reusable ordinal. |
| N4-Q: returning destroy | Auxiliary/full destroy and completion in `queue_live.rs` | Requires release custody. Retain original parent, taken lane and every teardown prefix. Reuse a slot only after confirmed full disposal; exercise actual destroy/recreate. |
| N5: generated DATA-ADOPT | Runtime generated preparation/shell owners; proposed `kfd_backend/generated_adoption.rs` | Requires accepted N2-N4 custody. Install existing adoption hooks; bind original bytes without publication. Cover initial, auxiliary and reused lanes, partial failure, Stop/drain and exact abort. |
| I2: generated ISSUE | Runtime generated-operation and backend owners, existing classified submit | Joint Native/Admission packet after N5 and C1/C2/C3. Bind one logical submission and permit to actual lane/queue/publication/allocation identities. Never retry uncertain publication. |

## Admission Packets

Paths are under `crates/fe2o3-runtime/src/` unless qualified. Existing Context
validators, ordinary typed futures, graph/drain paths, preparation/reservation,
completion classifier and host decoder are reused.

| Packet | File boundary | Dependency and acceptance |
| --- | --- | --- |
| C1: Context identity | New `context/tests/submission_identity_tests.rs` | Existing validator, not new production logic. Context brand, logical ID, backend ID, stream and device versus poll/wait/query/event/callback/cancel/drain/release. Preserve genuine cache, graph, terminal and deadline semantics. Primary wires module and missing mock cancel-entry counter. |
| C2: descriptor identity | New `authorized_execution/tests/generated_identity.rs` | Independently ready. Every descriptor coordinate, both matching directions, immutable source after one-shot control transfer and later artifact/currentness substitution. Descriptive checks are not native receipts. |
| C3: preissue reply/custody | New `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs` | Independently ready. Two-owner reply isolation, Stop-before-disposal, partial retirement, latest-waker and panicking-wake behavior. Reuse the R80 reply; do not invent a successful native completion adapter. |
| C4: CO-4/COMPLETE | Generated-operation completion and existing host decoder integration | Requires I2. Exact completion, complete readback, closing currentness and native disposition precede decode/readiness. Reject partial, foreign, stale and replayed results. Reuse R80 storage and R85 decoder. |
| C5: generated typed-output API | Generated-operation API; `fe2o3-host/src/generated_runtime_invocation.rs` | Requires C4. Executor-neutral future and blocking join over the same path; capacity, wake races, reentrancy, cancellation and owner-local non-Send contracts. |
| C6: generated GRAPH/DRAIN | Existing async graph/drain modules | Requires generated issue/completion. Repeated graphs, exact dependencies, cancelled unissued nodes, accepted-prefix drain, observer loss and complete outputs. Cross-run reuse additionally requires V7/V8. |

## Resources Packets

The [journal contract](runtime-context-version-journal-v1.md) is accepted as a
contract/inventory only. Its model, production journal and cross-run leases are
absent. Existing R65 versions are graph-local, not substitutes for this work.

| Packet | File boundary | Dependency and acceptance |
| --- | --- | --- |
| V1: issuance | New `fe2o3-runtime-model/src/context_version_journal.rs` and tests | Existing Context IDs, construction-only opt-in, zero initial watermark, explicit independent A/W capacities. Register 41 then 44 and still look up 41; reject unregistered 42 and aborted-key replay. No Begin, second allocator or runtime activation API. |
| V2: membership | Same model, separate membership tests | After V1. Exact writer/member/free-slot partition and acyclicity; whole-roster validation and unchanged scratch/state on rejection. Preallocate plans. |
| V3: settlement/cost | Same model, settlement and counted-access tests | After V2. Exact success/NoEffect/Unknown settlement of retained membership; never roll back burned attempt epochs. Fixed-k work does not grow with unrelated capacity; no production arena scans. |
| V4: authenticated proofs | New `verus/context_version_journal_v1.rs`, named negative sources | Prove stable shared transitions with exact source/tool/runner/transcript identities and observed obligations. Pure-plan proof does not establish the Context commit refinement. |
| V5: production journal | New runtime `context/versions.rs` and tests | Consume the shared model. Preallocate metadata and commit exclusively without allocation; dropped tickets cannot lose membership or free capacity. |
| V6: initial hooks | Shared Context, generated-shell, prepare/submit and observation paths | After V5. Allocation/disposal, host write and copy hooks; Pending precedes effects, ambiguity retains custody and settlement precedes callbacks. |
| V7: complete coverage/recovery | Context, graph/generated mutation and terminal paths | Peer/ordinary/snapshot/atomic/collective/generated issue, deferred flush, observation/cancel/drain/cleanup. Add bounded ordered writers and explicit Unknown recovery without restricting existing ordered work. |
| V8: cross-run leases | New runtime `async_engine/graph/input_leases.rs` | Requires complete V7 and exclusive graph reservation. Exact range/generation leases reject intervening writes and stale/foreign/replayed authority. Start with repeated copy graphs; kernel reuse needs admitted compiler effects. |

Memory work reuses the existing resource accounts, batch admission, native
budgets and pool/cache policies:

| Packet | Lead and acceptance |
| --- | --- |
| M1a/M1b: aggregate domains | Resources contract then Primary/Native integration. Independently ready design; bootstrap, child arenas and simultaneous quarantine share an enduring root ceiling. |
| M2a/M2b: native backing, controls and slots | Resources costs plus Native profiles. Non-userptr backing first, then AQL/userptr; compound pre-effect admission and progress headroom without double charging. |
| M3a/M3b: host images and native residency | Resources. Host-image ceiling is independently ready; native code/control residency follows M2. Live leases prevent eviction; uncertain unload retains charges. |
| M4: total retained memory | All lanes after concrete owners exist. Include staging/COW, commands, registry, journal/leases, captures, replies/results, terminal and quarantine storage. Caller-declared payload sizes alone are insufficient. |

## Integration And Qualification

```text
N2 acceptance + N3/N4 -> N5 DATA-ADOPT ---------+
C1/C2/C3 ------------------------------------+-> I2 ISSUE -> C4 COMPLETE -> C5 API -> C6 GRAPH/DRAIN
V1 -> V2 -> V3 -> V4 -> V5 -> V6 -> V7 -> V8 -------------------------------> cross-run reuse

Concrete Worker V3 refinement backend + owned proof artifacts
  + admitted compiler/machine/target contracts
  + generated runtime integration -----------------------------> protected production execution
```

Fixture-ready ISSUE/COMPLETE is distinct from protected production execution.
The host's existing
[Worker V3 adapter](../crates/fe2o3-host/src/worker_v3_verification_admission.rs)
ships no concrete semantic-to-machine refinement backend or owned proof
artifact. That is a required production join, not an optional later feature.
Runtime tests must not fabricate its authority. First non-reusing ISSUE needs
the mutation hook specified with Resources, not completed cross-run leases;
reuse requires complete V7/V8.

| Gate | Primary-coordinated work |
| --- | --- |
| Q0: R105 local acceptance | Complete frozen/focused/auxiliary/mutation/restoration campaigns. Before collection, enforce restoration-after-mutation timestamps and exact auxiliary test-name multisets; both were identified by independent review. Preserve failures and raw bytes. |
| Q1: executable correspondence | Incremental property-level adapter proofs and negative mutations, not only abstract models or proof-inventory checks. |
| Q2: native qualification | Original engine/account/platform composition, actual later lane slots, native depth, memory pressure, both cache startup orders, device timelines and isolated fault/drain campaigns. Existing runners need qualification, not duplicate implementation. |
| Q3: matched performance | Signed KFD/HIP/HSA producers, complete-output checks, pinned workloads and precommitted thresholds; report latency, throughput, bandwidth, CPU, memory and tails. SCALE-3's consistency checker is not measured performance. |

Maintain separate **CPU/source**, **authenticated formal**, **live Linux/KFD**
and **matched performance** columns. R103's unaccepted default-concurrency musl
watchdog failures remain unresolved; a four-thread pass is not their diagnosis.
Only Primary schedules MI300X, checks shared-machine availability and removes
task-owned resources. Disruptive faults require an isolated window. This review
launched no hardware work and establishes neither full parity nor a speedup.

## Later Rotations And Cross-Team Handoffs

| Work | Lead and required outcome |
| --- | --- |
| A3: unified local multi-GPU | Native with Admission/Resources: topology, sharding/replicas, peer/staged transfers, group drain and partial-failure isolation. Existing exact-two-device copy-only XGMI is not unified compute. |
| A4: distributed control | Admission/Primary: authenticated membership epochs, exact receipts, bounded leases and two-host execution without duplicate publication. |
| A5: distributed data/collectives | Native/Resources: bounded reference transfers and separately qualified broadcast, reduce-scatter, all-gather and all-reduce. |
| A6: failure qualification | Primary/Admission: device, participant, network, transfer and collective faults without unsafe replay, premature release or false complete outputs. |
| A7: release/performance | Primary/Native: single-device, multi-GPU and two-host gates, observability, recovery cost and production dependency/symbol closure. |
| Worker/capsule authority | Owning teams: #209 and #130/#131/#132, including the production join above. |
| Device language and machine semantics | Compiler owners: broad Rust/G2/G4, protected kernel families, #214 scalar GEMM and authenticated atomic/collective profiles. Runtime owns exact contract consumption and per-family qualification. |
| Additional target/tile support | #274 gfx950 admission and #275 with #134/#271/#272. gfx942 evidence cannot qualify gfx950. |
| Deployment/debugging | Owning teams: offline installation #252, disposable deployment #253 and debugger descendant containment #269. |

These later rows are coordination queues using the same worker slots, not
additional running agents. A1/A2 closure alone does not complete issue #182 or
the independent compiler, Worker, target, deployment and debugger work.
