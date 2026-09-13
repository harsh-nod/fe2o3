# Runtime Swarm: Remaining Packets

Reviewed through R111's locally accepted source/test campaign,
above accepted R110 `356523e6ea8c61c70c6812762aeaab33506de6e2`.
[Issue #182](https://github.com/harsh-nod/fe2o3/issues/182) remains open; its GitHub
API `updatedAt` is `2026-09-12T10:51:18Z`. A1/A2 are not complete.

R107 accepted device-initialization custody at its named CPU/test boundary.
R108 accepted the bounded writer-issuance model, not a production Context
journal. R109 accepts initialized-device live insertion at its named
CPU/shared-sequencer and concrete-facade boundaries, with
[retained evidence](evidence/local-r109-live-device-insertion-2026-09-12/README.md).
R110 additionally accepts initialized coherent insertion at those named CPU
boundaries, with [retained evidence](evidence/local-r110-live-coherent-insertion-2026-09-12/README.md).
R111 additionally accepts uninitialized coherent insertion through shared
settlement and the two direct-session APIs' preflight/missing-engine boundaries,
with [retained evidence](evidence/local-r111-uninitialized-coherent-insertion-2026-09-12/README.md).
These packets add no native, authenticated formal or performance qualification.
The [full work orders](runtime-a1-a2-swarm-current.md) retain detailed contracts
and historical evidence; this document is the short current assignment map.

## Swarm Ownership

Three read-only review agents completed independent source-grounded handoffs.
Their implementation queues below are assigned work, not unattended jobs.
Primary owns edits, shared wiring, integration, serialized builds, proof and
hardware runs, and signed publication to both repositories. Workers cross-review
bounded source/test changes; there are three worker slots plus Primary.

The [R111 uninitialized-coherent packet](runtime-uninitialized-coherent-insertion-custody-v1.md)
is now locally accepted after full/focused/auxiliary checks, fourteen compiled
negatives, exact restoration and independent archive review. Refreshed read-only
handoffs confirm that C1 and V2 remain independently ready; Native next takes L3-D.

| Worker | First Deliverable | Review Boundary |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N3-L3-D uninitialized device allocation/map custody; independent N4-R cleanup | Reuse `fe2o3-kfd/src/queue_live/data_insertion.rs` settlement and existing borrowed lower cores; preserve uninitialized identity and actual incomplete owners |
| Admission: `submission_identity_handoff` | C1 submission-identity test matrix | New `fe2o3-runtime/src/context/tests/submission_identity_tests.rs`; Primary owns Context wiring and the missing mock cancel-entry counter |
| Resources: `r102_evidence_review` | V2 allocation membership and Begin | Existing `fe2o3-runtime-model/src/context_version_journal.rs` and separate membership tests; retain V1 regressions |
| Primary | Integrate one reviewed packet at a time; Q1/Q2/Q3 contracts | Shared Context/backend/queue modules, immutable validation campaigns, evidence review and dual-remote publication |

Crate paths in the tables are relative to `crates/`. Existing ordinary typed
async launches, graph/drain, validators, budgets and decoder implementations
must be extended, not replaced.

## Native Queue

| Packet | Remaining Work | Dependency And Exit |
| --- | --- | --- |
| N3-L1 / R109 | Locally accepted: initialized-device append, explicit insertion and remembered-hole replacement | Fourteen new test functions and eleven compiled negatives; original-engine scripted composition and concrete missing-engine facade coverage are separate. Native success and formal refinement remain unqualified. |
| N3-L2 / R110 | Locally accepted: initialized coherent insertion/replacement | Nineteen new functions (eighteen dynamic and one routing guard), ten compiled negatives and unchanged R109 regressions. Complete survives retake/commit; earlier lower failure custody stays intact. No native success or formal refinement qualification. |
| N3-L3-C / R111 | Locally accepted: uninitialized coherent insertion | Nineteen new functions, fourteen compiled negatives and unchanged initialized regressions. Shared settlement preserves explicit insertion and required-hole replacement, with no copy or initialized-content authority. Two direct APIs, no new facade APIs; native/formal qualification remains open. |
| N3-L3-D | Uninitialized device allocation/map custody and insertion | Retain actual None/Unmapped/Mapped stages, per-call native attempt and map progress using borrowed lower cores. Preserve DEVICE_LOCAL backing and hole-or-append policy. C-first is scheduling, not a semantic dependency. |
| N4-R | Lower control release/unmap/returning cleanup | Independently ready. Root the owner before validation; first/middle/last error or panic retains untouched owners. Confirmed disposal and failed model projection must not produce retry or duplicate refund. |
| N4-L | Live detach and data/control release | Needs applicable N4-R contracts. Retain returned owners through retake; failed settlement cannot commit a reusable hole or reconstruct disposed authority. |
| N4-QA / N4-QP | Auxiliary destruction, then full/returning destruction | Needs applicable lower/live cleanup contracts. Keep the taken lane and parent through every prefix; cover attached/detached return modes. Slot reuse requires confirmed full disposal; QA-first is scheduling. |
| N5 | Nonpublishing generated DATA-ADOPT | Needs required N3/N4 custody paths. Bind original bytes to exact lane/resources without publication; cover initial, auxiliary and reused lanes, partial failure, Stop/drain and exact abort/disposal. |

R109 repaired the displaced session attributes and module-path wiring and added
the constructed-engine composition tests. Its preliminary compile failure,
incorrect ordinary-error poison oracle and rejected first mutation checker are
preserved with the accepted evidence, not erased or counted as passing runs.
Later-slot insertion tests may relocate a real auxiliary owner behind a vacancy;
the current two-compute-lane limit does not permit claiming a second constructed
auxiliary. CPU fixtures are not live Linux/KFD qualification.

## Admission Queue

| Packet | Remaining Work | Dependency And Exit |
| --- | --- | --- |
| C1 | Submission identity across eight existing ingresses | Ready now: five coordinates give forty pending plus forty retained-success rejection cells with valid controls. Genuine cached completion, released backend-ID reuse and destroyed-stream semantics; rejection preserves supplied handles, owners and callbacks before any backend entry. |
| C2 | Generated descriptor identity | Independently ready. Cover each roster coordinate in both match directions, source identity after transfer, and later artifact/currentness substitution. Descriptions do not grant native authority. |
| C3 | Reply and retained-owner lifecycle gaps | Independently ready on existing lifecycle fixtures. Two-owner isolation, Stop before disposal, A-success/B-failure/C-retained retirement, latest-waker and panicking-wake behavior; repeated progress cannot retry failed retirement. |
| I2, joint Native | Actual generated ISSUE | N5 + C1/C2/preissue-C3; specify the Resources mutation hook first. Bind one submission/permit to actual lane, queue, publication and allocation incarnations. Never retry uncertain publication. First non-reusing ISSUE does not require V7/V8. |
| C4 | Generated COMPLETE | Needs I2. Exact completion, full readback, closing currentness and native disposition precede decode/readiness; reject stale, foreign, partial and repeated observations. |
| C5 | Generated typed-output future and blocking join | Needs C4. Share one async path; test bounded admission, wake races, observer loss, cancellation, reentrancy and owner-local non-Send contracts. |
| C6 | Generated GRAPH/DRAIN | Needs actual ISSUE/COMPLETE; C5-first is API scheduling. Repeated graphs, exact dependencies, cancelled unissued nodes and accepted-prefix drain. Cross-run reuse additionally requires V7/V8. |

C1/C2/C3 test files remain absent. Generated preparation still installs no
production adoption hooks; these are integration gaps, not a missing-runtime
rewrite. Protected production cells also need the external Worker/compiler
authority described below.

## Resources Queue

| Packet | Remaining Work | Dependency And Exit |
| --- | --- | --- |
| V2 | Preallocated allocation/member arenas, scratch and whole-roster Begin | Independent of Native/C1. Exact references, backlinks, cardinalities, acyclicity and free partitions; validate the full canonical roster before mutation. First/middle/last rejection leaves state and scratch unchanged. Freeze empty-roster and canonical ordering contracts before coding. |
| V3 | Settlement and cost model | After V2: retained-roster success, exact NoEffect and sticky Unknown; no epoch rollback or release on dropped references. Count O(k) touched work independent of unrelated A/W; no commit-time growth. |
| V4 | Authenticated journal proofs | Start stable V1 properties now; extend through V2/V3. Actual solver results and property-specific negatives, followed by production correspondence. Inventory is not proof execution. |
| V5 | Production Context journal | Stable V1-V3 contracts; verified acceptance also needs V4/Q1. Private move-only tickets, authentic existing IDs and construction-only opt-in; no second ID allocator. |
| V6 | Initial mutation hooks | Integrate V5 with original logical destinations before translation. Install Pending before effects and settle before callbacks. |
| V7 | Complete mutation coverage, ordered writers and recovery | Cover every writer family, bounded ordered writers and explicit Unknown recovery; host-write/copy-only coverage is insufficient. |
| V8 | Exact cross-run input leases | Complete V7 plus exclusive graph reservation. Exact range/generation leases; copy-graph reuse first, kernel reuse additionally needs admitted compiler effects. |
| M1 | Aggregate resource domains and terminal headroom | Design independently ready; reuse existing accounts. Repeated Context creation and simultaneous quarantine cannot reset limits or refund retained owners. |
| M2 | Native backing, control and occupied-slot admission | Cost inventory independently ready; integrate with Native. Complete compound pre-effect admission, exact failure-prefix charges and no duplicate backing charge. |
| M3 | Host-image and native executable/cache residency | Host-image ceiling independently ready; native residency follows M2. Live operations prevent eviction; uncertain unload retains charges. |
| M4 | Total retained-memory bound | Integrate concrete owners, journals and leases. Include staging/COW, commands, arenas, replies/results, terminal slots and quarantine; distinguish caller-owned exclusions and avoid double charging. |

V2 preserves R108's Reserved-only count, exact older-reservation lookup and
nonwrapping identities. Canonicalization is separate O(n log n) preparation;
Begin targets O(k) work on k canonical destinations. Full invariant scans belong
in test auditors, not the production transition.

## Next-Packet Handoffs

- Accepted N3-L3-C / R111 reuses lower coherent allocation/map custody and keeps
  the uninitialized mapped result outside the loan through commit. N3-L3-D must
  retain the actual unmapped device lease before borrowed mapping; the existing
  consuming public map is not a custody substitute. Reuse preallocated terminal
  storage, not per-call boxing or a large inline engine root. Preserve existing
  flags, error precedence, ledger policy and R109/R110 regressions. The three
  existing uninitialized entrypoints are direct-session APIs; adding selected-lane
  facade methods would be a separate public API extension.
- L3-D's sole existing device route accepts size/alignment, not an input lease.
  Use actual None/Unmapped/Mapped custody, one-shot/failed state, per-call native
  attempt and map progress, without source/content fields. Generalize the
  preallocated terminal element to distinguish initialized and uninitialized
  roots; occupancy must recognize both variants. Cross-kind occupied-slot tests
  must preserve earlier custody and unchanged storage capacity. Device operations
  preserve the existing coherent model content rather than projecting new
  coherent allocations.
- L3-D's negative-test roster must distinguish owner stage from native progress:
  reserve failure can retain an admitted charge with no lease, while a successful
  native map followed by failed closing currentness retains the actual Unmapped
  owner and ambiguous mapping progress. Do not infer per-call admission from a
  sticky engine flag. Parameterize the initialized fixture's PUBLIC-backing
  oracle for DEVICE_LOCAL, and test both mixed-kind terminal-slot directions
  directly so quarantine rejection cannot hide an occupancy bug. Include lower
  configured/unconfigured backing-unwind behavior and one-shot roots after both
  success and admitted failure.
- Admission C1 substitutes Context brand, logical ID, backend ID, stream and
  device across poll, wait, query, callback registration, release, event, cancel
  and drain. Use real completion/event paths and real backend-ID reuse; record
  poll/wait/cancel entry counters. Keep existing destroyed-stream, terminal,
  deadline and graph-reservation precedence. Consuming release returns the
  exact supplied invalid handle.
- C1's first patch is the new `context/tests/submission_identity_tests.rs`, its
  test-module declaration and a mock `cancel_call_count` increment before any
  backend lookup. Keep production validators unchanged. Complete retained-success
  controls through actual event wait with an empty supplied-handle cache; obtain
  stale caches by actual wait/release and backend-ID reuse by a replacement launch.
  Only poll/wait/event require a live stream. Wait validates identity before
  constructing its deadline; drain checks an expired deadline first. Pure query
  remains available in terminal or graph-reserved Contexts.
- C1 uses a fresh fixture for each of its eighty cells, with another live-stream
  submission, a real foreign Context and observable callback probes. Snapshot the
  exact supplied handle, target/neighbor records, callback storage and payload
  addresses, backend entry counters and resource state. Rejected registration
  drops only its new callback once, without invoking it. A stale cached handle
  comes from a genuinely waited submission followed by release and a new launch
  using the mock's one-shot backend-ID override, not hand-seeded maps or cache.
  Real stream destruction leaves QuiescentWithoutResult: only poll/wait/event
  require that stream to remain live. Preserve separate valid controls for all
  eight APIs in both states, including consuming-release token return.
- Resources V2 uses an independent map/set reference and a test-only full arena
  auditor. Validate references, device/range, capacity and epoch arithmetic for
  the entire canonical roster before changing state or scratch. Distinguish
  Reserved count from occupied writers. Empty-Pending and final-epoch contracts
  must be explicit; model references do not grant production authority or prove
  that the supplied roster contains every kernel write.
- V2's first patch includes enrollment and whole-roster Begin, not arenas-only
  scaffolding. Keep all eleven V1 regression names. Narrow the existing EOF-wide
  no-loop source guard to the O(1) issuance operations and indexed helpers; give
  Begin its own O(k), no-unrelated-arena-scan checks. Expand complete snapshots
  and the auditor to every allocation/member/scratch arena, free stack, count and
  storage pointer/capacity. Proposed semantics remain empty Pending retaining W,
  MAX-1 to MAX as the final valid epoch increment, and full-allocation-key ordering
  with exact device/extent validation; freeze them before implementation.

## Qualification And Integration Order

1. Advance N3-L3-D, C1 and V2 independently; N4-R, C2/C3 and M1/M2/M3 design work
   need not wait for those packets. Primary serializes shared edits and builds.
2. Join required N3/N4 into N5; join N5 and C1/C2/C3 into I2, then C4/C5/C6.
   V7/V8 gate cross-run reuse, not the first non-reusing generated launch.
3. Q1 runs incrementally: authenticate property-specific proofs and the actual
   Rust/model ownership and commit correspondence, with explicit external contracts.
4. Q2 supplies the missing configured native budget example/runner/checker and
   actual depth, out-of-order, pressure, reuse, teardown and overlap evidence.
   Investigate retained R103 default-concurrency musl watchdog failures under
   recorded load without weakening deadlines or substituting filtered reruns.
5. Q3 freezes matched KFD/HIP/HSA workloads, full-output oracles, copy/residency
   semantics, sizes, depth, warmup and thresholds before tuning. Report raw
   repetitions, latency, bandwidth, throughput, CPU/memory and tails. Physical
   overlap requires device timelines, not merely host queue depth.

Every implementation packet requires focused and applicable full/auxiliary
checks, decisive compiled negatives, exact source restoration and independent
evidence review. Preserve failed attempts and immutable accepted archives.
Keep CPU/source, authenticated formal, native and performance status separate.
MI300X scheduling stays Primary-owned with task-owned staging/processes and
cleanup; disruptive fault tests require an isolated window. R109-R111 ran local
source/test campaigns, not solver, SSH or GPU jobs.

## Later Rotations And External Owners

| Milestone | Lead And Remaining Exit |
| --- | --- |
| A3 | Native + Resources/Admission: unified local multi-GPU compute, topology, shards/replicas, admitted peer/staged transfer and group drain. Copy-only XGMI does not close it. |
| A4 | Admission + Primary: authenticated two-host membership/epochs, exact receipts and terminal classification without duplicate publication. |
| A5 | Native + Resources: bounded distributed data movement and separately qualified broadcast, reduce-scatter, all-gather and all-reduce. |
| A6 | Primary + Admission: device, participant, network, transfer and collective fault campaigns; no unsafe replay, early release or false completion. |
| A7 | Primary + Native: matched single-device/multi-GPU/two-host performance, recovery costs and direct-KFD dependency/symbol closure. No blanket speedup claim. |

The same three worker slots rotate into later milestones; these are not extra
concurrent agents. Worker V3/capsule authority (#209, #130/#131/#132), compiler
semantic-to-machine contracts (#134 and successors, #214), target admission
(#274), mixed SIMT/tile consumers (#275), protected kernels, offline installation
(#252), deployment (#253) and debugger containment (#269) retain their owning
teams and issues. See [external handoffs](runtime-a1-a2-swarm-current.md#later-milestones).
The current Worker V3 host module supplies neither a concrete production
refinement backend nor owned proof artifacts. Fixtures cannot replace that gate.
Closing A1/A2 alone does not close issue #182 or establish full HIP/HSA parity.
