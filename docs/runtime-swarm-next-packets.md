# Runtime Swarm: Remaining Packets

Reviewed through R113's locally accepted executable-model campaign,
above accepted R112 `a2feef229758381b66f962ad8be2b87843eb33a3`.
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

R112 locally accepts [N3-L3-D](runtime-uninitialized-device-insertion-custody-v1.md),
with [retained evidence](evidence/local-r112-uninitialized-device-insertion-2026-09-13/README.md).
All seventeen source gates, ten auxiliary checks and nine frozen/restored suites
pass. GNU and musl each pass 2,683 tests with five ignored across 48 harnesses.
Sixteen compiled negatives fail at exact behavioral oracles, with all 5,679
source identities restored after each. The closed collector and independent
archive review pass. The 263 raw artifacts preserve two preliminary failures
and an excluded passing allocator child whose wrapper rejected a UTC rewind;
a separately pinned continuation supplies its accepted rerun and later suites.
R112 adds no native/formal/performance acceptance.

R113 locally accepts V2 [membership and whole-roster Begin](runtime-context-version-membership-v1.md),
with [retained evidence](evidence/local-r113-context-version-membership-2026-09-13/README.md).
Seventeen source gates and ten auxiliary checks pass. GNU/musl each pass 2,695
tests with five ignored across 48 libtest harnesses; the existing harnessless
CSV benchmark is accounted separately. Seventeen compiled negatives reject at
their exact behavioral oracles; restored journal/membership suites pass 23/12.
All 5,680 source identities and 344 raw artifact hashes match. The closed
collector and independent archive review pass. Nine runner, nine freeze and
52 corrected collector-contract tests pass; rejected preliminary attempts remain
preserved. This is executable-model acceptance only: settlement, production
Context hooks, authenticated proofs, native execution and performance remain open.

## Swarm Ownership

Three read-only review agents completed independent source-grounded handoffs.
Their implementation queues below are assigned work, not unattended jobs.
Primary owns edits, shared wiring, integration, serialized builds, proof and
hardware runs, and signed publication to both repositories. Workers cross-review
bounded source/test changes; there are three worker slots plus Primary.

The [R111 uninitialized-coherent packet](runtime-uninitialized-coherent-insertion-custody-v1.md)
is now locally accepted after full/focused/auxiliary checks, fourteen compiled
negatives, exact restoration and independent archive review. Refreshed read-only
handoffs established C1 and V2 as independent packets. With V2 locally accepted
as R113, Native takes N4-R1, Admission qualifies C1 and Resources freezes the
reviewed V3 settlement draft. Primary integrates above accepted R113.

| Worker | First Deliverable | Review Boundary |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N4-R1 pristine control cleanup | Retain the active control before disposal callbacks; preserve exact mapped/unmapped/disposed custody and native progress |
| Admission: `submission_identity_handoff` | C1 submission-identity test matrix | New `fe2o3-runtime/src/context/tests/submission_identity_tests.rs`; Primary owns integration of the candidate's Context wiring and mock cancel-entry counter |
| Resources: `r102_evidence_review` | Freeze V3 settlement contract, then settlement/cost implementation handoff | Existing `fe2o3-runtime-model/src/context_version_journal.rs`; retain accepted V1/V2 regressions and exact membership invariants |
| Primary | Integrate one reviewed packet at a time; Q1/Q2/Q3 contracts | Shared Context/backend/queue modules, immutable validation campaigns, evidence review and dual-remote publication |

Crate paths in the tables are relative to `crates/`. Existing ordinary typed
async launches, graph/drain, validators, budgets and decoder implementations
must be extended, not replaced.

## Execution Waves

The user-requested swarm has three worker lanes and one integrating Primary.
These are bounded assignments, not a promise that every downstream packet runs
concurrently. Workers review source and return implementation/test handoffs;
Primary owns edits under the current ownership policy. A completed review is
not a completed implementation packet.

| Wave | Native | Admission | Resources | Primary And Exit |
| --- | --- | --- | --- | --- |
| 0: independent starts after R113 | N4-R1 pristine active-control cleanup handoff | Qualify C1 identity matrix: 80 rejection cells plus valid controls | Freeze V3 settlement contract | Qualify one reviewed packet at a time above accepted R113 |
| 1: cleanup, lifecycle and settlement | N4-R2 ordinary/returning controls, then data disposal; applicable N4-L and N4-Q paths | C2 generated descriptor identity and C3 retained-owner lifecycle coverage | V3 settlement; V4 proof work and M1/M2/M3 contracts can start incrementally | Serialize shared-file edits and builds; require exact ownership, failure-atomicity and negative-test evidence per packet |
| 2: generated execution | N5 DATA-ADOPT, then joint I2 actual ISSUE | I2, C4 COMPLETE, C5 typed output and C6 GRAPH/DRAIN | Approve mutation-hook policy; integrate V5/V6 for journal-enabled paths | Join actual production ownership paths; fixtures do not supply external Worker/compiler authority |
| 3: reuse and resource closure | Integrate compound backing/control/slot admission and native residency | Exercise reused generated graphs and bounded retained replies | V7 complete writers/recovery, V8 input leases; integrate M1-M4 total retained-memory limits | Cross-run reuse requires complete mutation coverage and exclusive graph reservation; kernel reuse also needs admitted effects |
| Qualification, incremental throughout | Native depth, disposal, pressure and physical-overlap observations | Wake/cancel/drain and complete-output oracles | Version/accounting correspondence and retained-resource bounds | Q1 authenticated proofs, Q2 native evidence, Q3 matched HIP/HSA measurements remain separate acceptance gates |

C1/C2/C3 extend coverage around existing validators and lifecycle machinery.
C1's isolated candidate passes nine focused tests, all 724 runtime-library
tests and strict all-feature/all-target Clippy; it is not integrated or accepted.
C2/C3's proposed test files remain absent. Ordinary async, graph and
drain APIs already exist. I2/C4/C5/C6 compose the missing generated production path rather
than replace those APIs. A first non-reusing generated launch does not wait for
V7/V8, but a journal-enabled launch must have its production journal and hooks.

M1 aggregate-domain design, M2 cost inventory and M3 host-image limits are
independent preparation work, rotated through the Resources slot. Native cache
residency and total retained-memory closure depend on actual backing/control
ownership and journal/lease integration. V4/Q1 can start from accepted V1 now;
neither an inventory check nor model tests count as authenticated solver runs.

After A1/A2, rotate these same lanes through A3 local multi-GPU, A4 two-host
execution, A5 transfers/collectives, A6 failure qualification and A7 matched
performance. Their detailed owners remain in the later-milestone table below.

## Native Queue

| Packet | Remaining Work | Dependency And Exit |
| --- | --- | --- |
| N3-L1 / R109 | Locally accepted: initialized-device append, explicit insertion and remembered-hole replacement | Fourteen new test functions and eleven compiled negatives; original-engine scripted composition and concrete missing-engine facade coverage are separate. Native success and formal refinement remain unqualified. |
| N3-L2 / R110 | Locally accepted: initialized coherent insertion/replacement | Nineteen new functions (eighteen dynamic and one routing guard), ten compiled negatives and unchanged R109 regressions. Complete survives retake/commit; earlier lower failure custody stays intact. No native success or formal refinement qualification. |
| N3-L3-C / R111 | Locally accepted: uninitialized coherent insertion | Nineteen new functions, fourteen compiled negatives and unchanged initialized regressions. Shared settlement preserves explicit insertion and required-hole replacement, with no copy or initialized-content authority. Two direct APIs, no new facade APIs; native/formal qualification remains open. |
| N3-L3-D / R112 | Locally accepted: uninitialized device insertion | Actual None/Unmapped/Mapped custody, per-call native attempt and map progress use borrowed lower cores. DEVICE_LOCAL and hole-or-append policy remain intact. Twenty-six new functions, sixteen compiled negatives and full/focused/auxiliary/restoration/archive checks pass; native/formal/performance qualification remains open. |
| N4-R1 | Pristine active-control cleanup | Design ready. Extend `queue_dispatch_binding/pristine_abort.rs` and borrowed lower cleanup cores. First/middle/last control failure retains actual mapped/unmapped/disposed custody and untouched data/continuation; disposal cannot be retried. |
| N4-R2 | Ordinary and returning control cleanup | Reuse R1's borrowed cleanup contract. Root `DispatchResourceOwnerV1` before validation and reserve return capacity before disposal. Preserve forward code order, exact returned data and disposed receipts after failed model projection. |
| N4-R: data extension | Lower data cleanup and mixed-roster release | Reuse the common cleanup contract; mixed rosters join R2. Cover all host/device and initialized/uninitialized variants. Exact records/charges and untouched owners survive every failed destructive prefix; no repeated free or duplicate refund. |
| N4-L | Live detach and data/control release | Each route needs its applicable lower cleanup contract. Retain input, returned owners and disposed receipts outside the model loan through retake/commit. Failed settlement cannot commit a reusable hole or reconstruct disposed authority. |
| N4-QA | Auxiliary destruction | Needs applicable lower/live cleanup contracts. Keep the taken lane and parent through every queue/event/doorbell/resource/signal teardown prefix. Failed destruction cannot expose a reusable slot. |
| N4-QP | Full and returning destruction | Needs applicable lower/live cleanup contracts; QA-first is scheduling. Cover Release, ReturnAttached, ReturnDetached, callbacks and optional SDMA owners. Return data and earlier disposal receipts survive later failures without repeated cleanup. |
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
| C1 | Submission identity across eight existing ingresses | Ready now: five coordinates give forty pending plus forty retained-success rejection cells with valid controls. Genuine cached completion, released backend-ID reuse and destroyed-stream semantics; rejection preserves supplied handles, owners and pre-existing callbacks before any backend entry. |
| C2 | Generated descriptor identity | Independently ready. Cover each roster coordinate in both match directions, source identity after transfer, and later artifact/currentness substitution. Descriptions do not grant native authority. |
| C3 | Reply and retained-owner lifecycle gaps | Independently ready on existing lifecycle fixtures. Two-owner isolation, Stop before disposal, A-success/B-failure/C-retained retirement, latest-waker and panicking-wake behavior; repeated progress cannot retry failed retirement. |
| I2, joint Native | Actual generated ISSUE | N5 + C1/C2/preissue-C3; specify the Resources mutation hook first. Bind one submission/permit to actual lane, queue, publication and allocation incarnations. Never retry uncertain publication. First non-reusing ISSUE does not require V7/V8. |
| C4 | Generated COMPLETE | Needs I2. Exact completion, full readback, closing currentness and native disposition precede decode/readiness; reject stale, foreign, partial and repeated observations. |
| C5 | Generated typed-output future and blocking join | Needs C4. Share one async path; test bounded admission, wake races, observer loss, cancellation, reentrancy and owner-local non-Send contracts. |
| C6 | Generated GRAPH/DRAIN | Needs actual ISSUE/COMPLETE; C5-first is API scheduling. Repeated graphs, exact dependencies, cancelled unissued nodes and accepted-prefix drain. Cross-run reuse additionally requires V7/V8. |

C1's isolated candidate adds the mock cancel-entry counter and nine test
functions: the eighty-cell initial matrix, valid controls, actual cached
completion, release and backend-ID reuse, destroyed streams, deadlines,
backend-terminal state and genuine graph reservations. The nine focused tests,
all 724 runtime-library tests and strict all-feature/all-target Clippy pass
after correcting a test assertion's
`wait_event` return enum. The failed compile and corrected source maps remain
retained; full immutable qualification and integration are still pending.
Snapshots include retained records, callback storage and
probes, mock backend fields and aggregate allocation-credit usage; they do not
inspect private credit-account identity/storage. C2/C3 test files remain absent.
Generated preparation still installs no production adoption hooks; these are
integration gaps, not a missing-runtime rewrite. Protected production cells also
need the external Worker/compiler authority described below.

## Resources Queue

| Packet | Remaining Work | Dependency And Exit |
| --- | --- | --- |
| V2 / R113 | Locally accepted: preallocated allocation/member arenas, scratch and whole-roster Begin | Twelve new tests include independent map/set traces, full arena auditing, exact rejection snapshots and fixed-k work checks. Full/focused/auxiliary checks, seventeen compiled negatives, exact restoration and independent archive review pass. No production Context consumer or formal proof is added. |
| V3 | Settlement and cost model | [Design draft](runtime-context-version-settlement-v1.md), after V2: retained-roster success, exact NoEffect and sticky Unknown; no epoch rollback or release on dropped references. Count O(k) touched work independent of unrelated A/W; no commit-time growth. |
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

V3's reviewed design proposes borrowed, inert model evidence for exact Pending
success/NoEffect. Success advances each retained allocation's lineage to its
admitted epoch; NoEffect preserves lineage and burned epochs. Both return only
the exact retained members and writer slot. Unknown retains the chain and all
backlinks; repeated Unknown is idempotent only after touched-chain validation.
Neither ordinary settlement nor Begin may overwrite Unknown. These decisions
are not implemented, frozen qualification or authenticated receipt authority.

Validate reference/phase, evidence, the complete retained chain, then release
headroom and vacant scratch before mutation. The existing member/scratch fields
are sufficient. Require matching epochs and prior lineage with
`prior_lineage < attempt_epoch`, not `attempt_epoch == prior_lineage + 1`:
earlier NoEffect attempts legitimately leave gaps. Global free-stack uniqueness
and partitions remain initialization/preservation invariants, audited in tests
and eventually proved in V4; O(k) touched checks cannot establish them against
arbitrary unrelated private-state corruption. Production sealing is a V5 gate.

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
- N4-R1 roots the active control in the existing pristine-abort owner before a
  borrowed cleanup callback. Its one-shot progression is Mapped, Unmapped,
  NativeDisposedReceipt, then Complete; confirmed disposal must never be rebuilt
  into releasable authority after a later projection/currentness failure. Reuse
  the existing live `terminal_abort` transport, not a new settlement layer or
  per-control heap allocation. Preserve kernarg-first/reverse-code disposal
  order and the current unmap/release revision and projection ordering. Tests
  must inspect the removed active control as well as untouched data and
  continuation. Populate real model state through existing allocation/seal/map
  transitions so the fixture exercises the production cleanup projections;
  raw-engine-only construction cannot qualify those paths.
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
- Accepted R113/V2 includes enrollment and whole-roster Begin, not arenas-only
  scaffolding. Keep all eleven V1 regression names. Narrow the existing EOF-wide
  no-loop source guard to the O(1) issuance operations and indexed helpers; give
  Begin its own O(k), no-unrelated-arena-scan checks. Expand complete snapshots
  and the auditor to every allocation/member/scratch arena, free stack, count and
  storage pointer/capacity. The frozen membership contract specifies empty
  Pending retaining W, MAX-1 to MAX as the final valid epoch increment, and
  full-allocation-key ordering with exact device/extent validation. The isolated
  membership module adds twelve tests and 29,282 bounded differential traces.
  The accepted immutable campaign covers the final source; earlier isolated
  journal/full-model/lint checks remain historical, without production acceptance.
- C2 uses `authorized_execution/tests/generated_identity.rs` with existing source
  projection/authority and generated-storage fixtures. Test every descriptor
  coordinate in both directions, then genuine one-shot control transfer and
  later artifact/authority/currentness substitutions. Use the immutable source
  view after transfer so an earlier mutable-view rejection cannot mask the oracle.
- C3 uses `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`
  and existing reservation/adoption fixtures. Cover two-owner completion isolation,
  Stop-before-disposal, A-retired/B-failed/C-retained retirement, no cleanup retry,
  latest-waker replacement and panicking-wake containment. Count reply credit
  separately from payload custody. No dummy native success adapter is introduced.

## Qualification And Integration Order

1. Advance N4-R1, C1 and V3 settlement independently above accepted R113;
   C2/C3 and M1/M2/M3 design work need not wait for those packets. Primary
   serializes shared edits and builds, preserving the isolated preliminary V2 worktree.
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
R113's revised runner cleans up its own process group and records an unclosed
child without inventing a terminal outcome; its freeze helper rescans the exact
inventory and HEAD before publication. Both helpers have dedicated regression
tests and independent source review. The collector accounts for the existing
harnessless benchmark rather than silently dropping summary-free targets;
its original preparation rejection and corrected 52-test roster are retained.
MI300X scheduling stays Primary-owned with task-owned staging/processes and
cleanup; disruptive fault tests require an isolated window. R109-R113 ran local
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
The separate [#277 release gate](https://github.com/harsh-nod/fe2o3/issues/277)
tracks the threaded GNU benchmark's strict ELF-audit rejection and missing
release-benchmark CI coverage. Its `2026-09-13T03:42:32Z` update reports an
auxiliary-teardown fix in main commit `ae26717922b1fb7ad62fdd5ad70814d83eb01177`
and a passing existing-fixture functional smoke with complete readback and
explicit teardown. The strict audit still rejects `dlsym`. Those external
results do not qualify this frozen topic branch, resolve N4-R1's active-control
custody gap, or establish a causal link between the earlier failures. Primary
owns the audit/CI work; Native assesses the teardown fix at its own integration
boundary without importing unsupported acceptance claims.
