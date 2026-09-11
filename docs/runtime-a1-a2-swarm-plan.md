# Remaining A1/A2 Work: Swarm Plan

Baseline: signed `96212b87bb67eef0dc4e8f6e0137ebf3c35e2d37` (R72 plus the
dispatch audit, pushed to both remotes), reviewed 2026-09-10. The locally
validated R73 follow-on and its exact source/evidence identities are below.
This decomposes the remaining local
A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), not the later multi-GPU
and distributed milestones. The issue's A1/A2 exit criteria were rechecked on
2026-09-10. Three agents independently reconciled their lanes against the current
code through MEM-2A; the subsequent DRN-1A implementation is recorded below.
The checkpoint history distinguishes local
implementation from hardware acceptance; queued tickets are not completed work
or unattended background jobs.

The [next-wave dispatch](runtime-a1-a2-next-wave.md) is the concise current
ticket split from the three-agent read-only audit and locally validated R73.
It records generated-admission/result ownership, independent native fixtures,
the missing native-budget qualification harness and completion gates. That
planning audit adds no runtime implementation or hardware acceptance; the
history below retains the earlier packet boundaries.

The [R65 contract](runtime-async-drain-versions-v1.md) is the current baseline:
cooperative drain, reply-count admission and graph-local version lineage are
implemented. Complete native budgets, persistent cross-run authority, general
generated async launches, active-work drain qualification and measured
compute/copy overlap remain open. Existing pure-guard proofs do not prove the
whole executable reference executor.

The
[native compute/SDMA coexistence contract](runtime-compute-sdma-coexistence-v1.md)
describes OVL-1/2's implemented checker and runtime
integration with bounded-scan proofs and CPU tests. Their live native admission
and hardware acceptance remain open.

## Source Checkpoint

The first swarm wave and its three follow-up packets have passed the local
gates recorded in the [R67 evidence](evidence/local-r67-owned-credits-2026-09-10/README.md).
That record also retains failed environmental attempts and the optional legacy
musl compiler limitation. Older R66 evidence does not qualify the new source;
signed live hardware acceptance remains separate.

Signed implementation `bd8aa3de` is pushed to both remotes. Its first
[MI300X R66 campaign](evidence/mi300x-r66-coexistence-2026-09-10/README.md)
was rejected because exact retained native-roster observation was unavailable.
The complete owned-process/stage cleanup check passed; this does not fill any
accepted hardware cell. Native-roster diagnosis now precedes a new campaign.

The following swarm packet implemented OVL-DIAG-1 and MEM-BASE. Native and runtime
owners added immutable typed rejection stages; cross-review found no change to
acceptance predicates, digest bytes, publication or retirement authority. The
qualifier now identifies the exact cell and observation phase. The primary's
runner retains the bounded actual binary after successful build and before
qualification, including later rejected campaigns. Earlier build failures may
still have no binary. Local integration gates and a new signed hardware
campaign remain separate from this source checkpoint.

The [final diagnostic/extraction local gates](evidence/local-r66diag-mem-base-2026-09-10/README.md)
pass 2,034 runtime tests on each of GNU and musl, with five existing ignores.
Host/fixture/doctest, lint, 134 runner/checker tests, 32 standalone lockfiles and
the 43-package production dependency audit also pass. Proof inputs are unchanged;
the negative inventory was rechecked, not the full solver run. Live acceptance
remains open.

Signed diagnostic/extraction implementation `d5ada879` is pushed to both
remotes. Its [new MI300X campaign](evidence/mi300x-r66diag-d5ada879-2026-09-10/README.md)
built successfully and retained the actual independently audited binary, but
the shared GPU became busy before launch. The qualifier never started. All
owned processes/groups and staging files were independently confirmed absent;
foreign GPU work was left untouched. Native-roster diagnosis therefore still
requires an idle shared-GPU window; this is not a new native acceptance result.

The resources owner moved the existing credit engine into
`fe2o3-resource-accounting`; the runtime keeps its device-branded wrapper. That
checkpoint implemented only the extraction. Parent budgets, batch/split
transactions and aggregate quarantine are not implied. The core's
moved tests are included in CPU and release-test rosters, and dependency policy
prevents an upward runtime dependency.

MEM-2A then implemented optional, immutable session-local N2 backing admission.
KFD derives padded bytes from its actual private layout and
retains the charge with the native record until complete backing/VA disposal.
The [native accounting contract](runtime-native-resource-accounting-v1.md)
separates this from GTT, session bootstrap, queues, pools, parent budgets and
runtime-wide forwarding. Independent review found panic-continuation gaps in
mapping, CPU access/initialization and paired XGMI transitions; configured-path
quarantine handling and fault tests are part of the integration gate. R68 adds
only a production-used cost projection and property-specific proofs, not a
proof of native disposal or the whole executor. The
[final MEM-2A local gates](evidence/local-r68-native-backing-2026-09-10/README.md)
pass 2,065 runtime tests on each of GNU and musl, with five existing ignores.
The full authenticated proof run passes 57 positive sources, 1,334 obligations
and 645 expected negatives. Host/fixture/doctest, lint, runner/checker,
dependency and lockfile gates also pass. Failed build/lint and disk-exhaustion
attempts are retained separately. A read-only shared-GPU query still found GPU1
busy; no new remote stage or qualifier was started. Signed hardware acceptance
and measured performance remain open.

DRN-1A now integrates all four capture slices: private Context registration,
native coherent read-into, byte-credit ownership through result disposal, and
the existing owner's conclusive drain boundary. It also adds fixed error
classification and focused capture failures. Cross-review corrected an
already-poisoned queue being classified as an ordinary rejection. The
[capture contract](runtime-host-drain-capture-v1.md) and
[R69 local record](evidence/local-r69-host-capture-2026-09-10/README.md) distinguish
CPU/proof gates from still-open signed outstanding-work qualification. R69
adds a range predicate, not whole-drain or native refinement.

The final R69 local gates pass 2,106 runtime tests on each of GNU and musl,
with five existing ignores. All sixteen local gates and the 43-package
production metadata audit pass. The full authenticated Verus run passes 58
positive sources, 1,338 obligations and 650 expected negatives with pre/post
source, inventory, exact transcript and release-closure checks. No SSH or GPU
work was started for this packet; hardware and performance acceptance remain
open. The latest three-agent dispatch audit changes the next assignments below,
not those acceptance boundaries.

The following R70 swarm completed MEM-2A-FWD, MEM-TXN-1 and DRN-3B. Its
[final local record](evidence/local-r70-batch-forwarding-2026-09-10/README.md)
passes 2,143 runtime tests on each of GNU and musl, with five existing ignores.
All sixteen local gates and the 43-package production audit pass. The full
authenticated Verus run passes 59 positive sources, 1,347 obligations and 659
distinct expected negatives, including nine new batch obligations and mutations.
The record retains failed attempts and distinguishes CPU/source-wiring tests
from native acceptance. A read-only MI300X query found GPU1 busy; no remote
stage or workload was started. Pool integration, account-domain closure,
signed drain/coexistence qualification, generated authority, whole-executor
refinement and matched performance remain open.

R71 then implemented the device-only MEM-2B cache ceiling and DRN-2A copy-first
qualifier, checker and signed runner. Its
[final local record](evidence/local-r71-device-pool-drain-2026-09-10/README.md)
passes 2,180 runtime tests on each of GNU and musl, with five existing ignores
across 48 harnesses. All sixteen local gates, 151 checker/runner tests and the
43-package production audit pass. The authenticated Verus run passes 60 positive
sources, 1,362 obligations and 669 distinct expected negatives; R71 adds 15
policy obligations and ten mutations, not native or whole-executor refinement.
Cross-review corrected active-owner versus terminal-table observation and
publication-history accounting. The record retains failed attempts and the
deterministic repair of an older reply-credit test race. One read-only MI300X
query showed zero instantaneous utilization but resident foreign allocation;
no remote stage or workload was started. Signed pool/drain/coexistence hardware,
generated authority, host/control/code and aggregate budgets, persistent
versions, capacity, whole-executor refinement and matched performance remain open.

R72 implements MEM-N1A ordinary coherent non-userptr GTT backing admission,
native lifetime/panic hooks and immutable forwarding in both startup orders.
The [host-backing contract](runtime-host-visible-backing-v1.md) and
[local evidence](evidence/local-r72-host-backing-2026-09-10/README.md) record
2,219 passing runtime tests on each of GNU and musl, five existing ignores,
all sixteen local gates and the 43-package production audit. The authenticated
Verus run passes 61 positive sources, 1,367 obligations and 678 distinct expected
negatives. R72 adds five cost-projection obligations and nine mutations, not
native disposal or whole-executor refinement. The 39 added tests distinguish
private fake-backend/model ownership from Linux queue/pool behavior. Review added
real closing-currentness panic, exact completion-arena size and explicit dropped
token cases. The record retains an interrupted proof attempt and an in-progress
compile failure. One read-only MI300X check found the selected GPU busy; no
remote stage or workload was started. Broader GTT profiles, host-pool ceilings,
aggregate domains, control/code budgets, versions, generated admission, capacity,
hardware acceptance and matched performance remain open.

R73 implements GEN-2R charged typed-result storage and packing, with nonblocking
slot polling. Its [local evidence](evidence/local-r73-charged-results-2026-09-10/README.md)
passes all seventeen current-source gates: 2,222 runtime tests on each GNU/musl
target, 227 GNU and 110 musl host tests, and generated fixtures. The 31 focused
owned-argument tests include 20 charged-path cases and 11 legacy cases. Review
added actual mutex poisoning, lower packing rejection and a bounded contention
regression that first failed against the blocking observer, then passed after
the `try_lock` fix. The authenticated proof run passes 62 positive sources,
1,374 obligations and 686 distinct negative mutations. Seven new obligations
cover costs/shapes, not mutex/Box adapters or native/whole-executor refinement.
No SSH or hardware work was started. GEN-2A nonexecuting invocation custody and
GEN-2B Context/native authority remain open; storage ownership cannot authorize
publication or establish completion, and polling has no Future/wakeup contract.

| Slice | Implemented locally | Boundary still open |
| --- | --- | --- |
| OVL-QUAL-1 | Eight-cell R26 coexistence example, immutable native-custody observations, independent checker, signed-source runner and negative tests | Signed live capture, native extraction refinement, physical overlap and production generated-kernel authority |
| MEM-1 | Nineteen-dimensional credit primitive and opt-in Context requested-byte/allocation-record admission; retain uncertain charges and reject false cleanup completion | Actual native residency/slot cost extraction, global budget closure and whole-account/executor refinement |
| GEN-1 | Owned typed arguments/results, complete-invocation preflight, authenticated packing-plan reuse and compile-fail ownership checks | Invocation-bound async authority, Context freshness, global credit lifetime and compiler machine evidence |
| GEN-2R | Charged result-peak roster, original typed-seed reuse, private complete-roster decode/commit and nonblocking slot extraction | GEN-2A/B invocation/operation custody and wake/retry policy, whole-adapter refinement, native authority and aggregate budgets |
| DRN-3A/B | Earlier accepted-submit/observation/cancellation regressions plus twelve DRN-3B graph, operation/waiter, capture and Stop scenarios | Active-work hardware drain and whole-executor refinement |
| SCALE-3-PROTO | Bounded matched-plan/checker protocol; fourteen tests and independent cross-review pass | Signed producers, genuine correctness/timing captures, performance and physical overlap |
| MEM-BASE / MEM-5 inventory | Shared credit-engine extraction and device-branded wrapper; concrete allocation-site inventory | Parent budgets, compound native admission and complete aggregate byte-budget closure |
| MEM-2A/FWD | Optional session-local N2 padded-backing/record admission; immutable forwarding before both native startup orders; configured panic quarantine | Signed hardware qualification, pool-path coverage and parent/global closure |
| MEM-2B | Optional device-only cached-free byte/record ceiling, exact padded cost and existing reuse/disposal paths | Live facade pressure in both startup orders, host pools, native refinement and parent/global closure |
| MEM-N1A | Optional ordinary coherent host-backing/record admission, configured panic retention and forwarding through both startup orders | Linux bootstrap/pool pressure, broader GTT profiles, native disposal refinement and parent/global closure |
| MEM-TXN-1 | Single-account complete-roster reservation with independently owned member credits and corruption poisoning | Compound native cost/ownership integration, parent admission and whole-account refinement |
| DRN-1A | Integrated private source validation, native coherent read-into, owned capture credits and cutoff/failure handling | DRN-2 signed outstanding-work capture, native/whole-executor refinement and aggregate MEM-5 closure |
| DRN-2A | Eight-cell copy-first qualifier, immutable actual-owner observations, independent output/membership checker and signed runner | Signed outstanding-work hardware capture; native/executor refinement; generated production cells |

The previous R67 source record has 2,021 passing all-feature/all-target runtime
tests on each of GNU and musl, with five
existing ignores across 46 harnesses. GNU five-crate doctests pass 75; musl
runtime doctests pass 64 and direct-KFD host doctests pass ten. Default host tests
pass 90; the all-feature GNU host gate passes 207 with four existing ignores
after correcting a stale test-process runtime-directory environment. The macro
fixture harness passes seven tests and runner/checker suites pass 132. Lint,
formatting and the 42-package production dependency audit pass. These are
local test results, not hardware or production generated-authority evidence.

The full authenticated Verus run also passed: 56 positive sources, 1,330
obligations and 640 expected-negative rejections, with the exact transcript and
pre/post source, inventory and pinned release-closure checks accepted. R67 adds
14 vector/record-decision obligations and eight named mutations. These totals
aggregate property-specific results; mutex/arena ownership, native cost
extraction, Context adapters and whole-executor refinement remain open. The
remaining live release work includes signed source freeze, actual qualifier ELF
and census audits, and separately scheduled hardware acceptance.

Cross-review found no blocking issue in the owned data boundary or native
qualifier. This is a code-review result, not a proof of the adapters. Requested
allocation bytes are not physical resident bytes. An inert decoded result is
not an authenticated native completion. Simultaneously retained submissions are
not proof of simultaneous GPU execution.

## Ownership

| Lane | Assigned agent | Implementation backlog |
| --- | --- | --- |
| Native execution and qualification | `r66_native_coexistence` | OVL-QUAL-1/2, SCALE-1/2/3 and SCALE-CAP: qualify disjoint compute/SDMA custody, admitted mixed-duration profiles, native capacity and matched performance |
| Resources and versions | `r66_coexistence_model` | MEM-1 release gate, MEM-2 through MEM-5, VER-1/2: physical accounting, pools/residency, persistent mutation journal and cross-run leases |
| Admission and drain | `r66_runtime_coexistence` | GEN-1/2, DRN-1/2/3: owned generated launches, authenticated async admission, host-only capture and active/failure drain qualification |
| Integration and proof composition | Primary | PRF-1/2: shared Context hooks, executable refinement, authenticated proof roster, cross-review, hardware scheduling and publication |

These are three concurrent read-only worker slots, not one simultaneous worker
per ticket. Each lane takes its analysis/review queue sequentially. Primary owns
all edits, integration, tests and conflict resolution; workers provide bounded
contracts and cross-review. Shared `context.rs`, reservation/completion hooks,
model exports, proof rosters and `kfd_backend.rs`/`queue_live.rs` changes remain
serialized. Coordinate generated macro changes with the generated-host owner.
Only Primary schedules MI300X jobs or pushes integrated changes to both remotes.
Historical implementation-ownership tables below describe earlier packets, not
new permission for workers to edit the shared tree.

## Next Assignments

R72 was released as signed `474b40a7` on both remotes with ordinary host-GTT
admission; R73 adds charged result storage and passes its local/proof gates.
Hardware acceptance remains open. These assignments
are analysis/review queues, not unattended implementation jobs; all edits and
hardware remain primary-owned.

| Lane | Next bounded packet | Dependency and exit gate |
| --- | --- | --- |
| Native | SCALE-1A-FIXTURE, then MEM-QUAL-HARNESS and ordinary host-cache hooks | Independent bounded artifacts and complete-output oracles; new harness must exercise actual N1/N2/cache limits before signed qualification. Capacity and timing follow correctness |
| Resources | Host-cache policy, then N1B and MEM-DOM-1 | GEN-2R data storage is implemented locally. R72 suffices for ordinary host-cache policy; root/device/Context and bootstrap/terminal headroom remain separate |
| Admission | GEN-2A owner-local nonexecuting invocation and private permit/decoder | Bind artifact, packing, device, geometry, ABI/effects and publication; Context allocation generations/admission and publication belong to GEN-2B. Positive execution requires exact compiler/machine evidence |
| Primary | Implement reviewed packets and shared GEN-2/cache/account integration; signed OVL/DRN/MEM campaigns | MEM campaigns first need the new pressure harness. Hardware requires idle selected GPU, real binary/census and complete cleanup. Do not promote fixtures to production authority |

### R71 Implementation Ownership

| Agent | Integrated packet | Boundary still requiring acceptance |
| --- | --- | --- |
| `r66_native_coexistence` | Device cache configuration, native padded-cost projection, reuse/disposal hooks and R66 lifecycle-observation correction | Real Linux pool pressure in both startup orders and signed coexistence observations |
| `r66_coexistence_model` | Device cache policy, R71 bounded-roster model, property proofs and fault/adapter tests | Native extraction/disposal refinement, host pools, parent/global budgets |
| `r66_runtime_coexistence` | Eight-cell copy-first drain example, independent byte/membership checker and synthetic negatives | Signed outstanding-work hardware capture and later GEN-2 generated cells |
| Primary | Runtime forwarding, immutable copy observer, publication-history hooks, runner, regression gates and proof registration | Whole-executor refinement, hardware acceptance and matched performance |

Cross-review found and corrected a real observation mismatch: pending native work
lives in active-owner indexes, not the terminal submission table. It also moved
history recording out of generic index restoration into successful native
publication transitions. Pending polls/waits are not new publications. A scripted
copy regression exercises the actual async-copy path; it is not native evidence.

### R70 Implementation Ownership

There are three worker slots plus the primary, not one worker per roadmap ID.
Resources handed Native the immutable configuration/pool ownership contract.
After that interface agreement, the following packets were implemented in
parallel without overlapping file edits. The primary integrated shared hooks.

| Agent | First deliverable | Owned files and handoff | Required focused gate |
| --- | --- | --- | --- |
| `r66_native_coexistence` | MEM-2A-FWD constructor forwarding in both startup orders | KFD `shared_memory.rs` and `queue_live.rs`; primary integrates runtime `kfd_backend.rs` and `compute_dispatch.rs` | Configure before N2 allocation/materialization and queue certification; unchanged default, late/replacement/foreign rejection, actual transfer/loan/retake and partial-constructor custody tests |
| `r66_coexistence_model` | MEM-TXN-1 complete bounded roster reservation | `fe2o3-resource-accounting/src/lib.rs`, `batch.rs`, isolated model/proof sources; primary registers exports/pins | Ordinary late-member/vector/record/generation exhaustion changes no account state; detected corruption poisons; independently disposed members refund exactly once; authenticated positive and negative obligations plus adapter tests |
| `r66_runtime_coexistence` | DRN-3B active-prefix/capture composition fixture | `async_engine/tests/owned_tests/drain_capture_tests.rs`, narrowly `drain_tests.rs` and `owned_tests.rs`; production fixes handed to primary | Active graph/operation/waiter identities, graph reservation release, exact reply and retained credits, no duplicate issue, and controlled Stop ordering; preserve the eight existing DRN-1A tests |
| Primary | Integration and evidence | Shared constructors, Context/owner hooks, model exports, proof roster, local gates and release records | Cross-review each packet, complete current-source gates, signed commits and both remote refs; hardware acceptance separately recorded |

MEM-TXN-1 is one-account compound admission, not parent/global admission or
arbitrary splitting of an already-issued debit. DRN-3B adds composition to the
basic rejection/panic/exhaustion tests. Stop cannot preempt a
synchronous capture callback: before pickup it prevents capture; a completed
capture may legitimately win before a concurrent Stop is processed, but partial
bytes must never become a result.

### Remaining Queue By Lane

| Lane | Ordered follow-on packets | Gates that cannot be skipped |
| --- | --- | --- |
| Native | SCALE-1A independent fixtures; MEM-QUAL-HARNESS; host-cache and N1B/MEM-3/4 hooks; SCALE-CAP; SCALE-2; SCALE-3 signed producers and measurements | Ordinary host-cache limits can use R72 N1A now; capacity needs real backing/control/slot budgets and admitted workloads; thousands queued is not thousands native-retained; device timeline required for physical overlap |
| Resources | Host-cache policy; MEM-N1B; MEM-DOM-1; MEM-3A/B; MEM-4A/B; VER-1A/B then VER-2; MEM-5 closure | Cached residency stays charged; compound native creation adopts exact MEM-TXN-1 members without double charging; parent/bootstrap/quarantine bounds precede aggregate claims |
| Admission | GEN-2A nonexecuting preparation with GEN-2R; GEN-2B Context/native bridge; signed DRN-2A support; generated graph/drain qualification | Copy qualification can precede GEN-2; positive generated execution requires exact compiler/machine evidence; cross-run reuse needs VER-1/2; blocking must join the same async path |
| Primary | OVL-QUAL-2 signed campaign; DRN-2 signed campaign; incremental PRF-1; PRF-2 exit audit and publication | Idle selected GPU and independent cleanup; complete output/identity audits; keep implementation, proof, CPU, fixture hardware, production hardware and performance statuses separate |

SCALE-1A isolated fixtures can proceed independently of GEN-2 and broader memory
profiles. VER-1 can move ahead when the resource slot is free. The GEN-2
compiler handoff can be prepared without changing frozen runtime source. These
are scheduling alternatives, not extra simultaneous workers. Missing idle
hardware or compiler evidence does not block unrelated CPU implementation.

### R71 Packet Contracts

The three agents implemented the following boundaries against R70. These are
retained packet contracts, not the next assignments or hardware acceptance.

**MEM-2B, resources and native:** an optional immutable device-cache
byte/record ceiling to the existing SDMA free pool. Resources owns an isolated
`fe2o3-kfd/src/sdma/pool_policy.rs` policy and model/tests; Native owns minimal
layout projection and checkout/recycle/trim integration in `sdma.rs` and
`queue_live.rs`. Primary owns runtime configuration and shared forwarding.
Derive padded cache cost from each device lease's native layout. Existing mixed
host/device pool observations report usable extents, not padded N2 residency.
Keep the N2 record as the sole resident debit; do not add another backing ledger.

Freeze an explicit cache-versus-dispose decision. If the incoming idle device
buffer cannot be cached, use its existing physical-release path, preserving
uncertain custody on failure. A generic recovered cache-full rejection would
currently become terminal in runtime transient recycling. Keep the existing
bounded best-fit scan, defaults and explicit trim; add no background eviction
or automatic retry. Test padding, byte/count boundaries, both startup orders,
foreign/late configuration, generation reuse, logical resize/release, exact
account identity, failed trim/currentness/panic and refund only after complete
physical disposal. Host pools require MEM-N1; parent/bootstrap/aggregate claims
still require MEM-DOM-1 and MEM-5.

**DRN-2A, admission:** a copy-only outstanding-work capture example
at `fe2o3-runtime/examples/gfx942-runtime-drain-capture.rs`, plus independent
`check_drain_capture.py` and `test_check_drain_capture.py` in
`benchmarks/runtime_gfx942`. Native and Primary supply a narrow immutable
`kfd_backend/qualification_drain_capture.rs` observation adapter; Primary owns
the signed runner, runner tests, shared exports and qualification contract.
Do not widen R66's compute-gated observer or treat logical Pending as native
publication. Bind queued/published operation identities to actual retained SDMA
receipts without granting compute authority.

Freeze eight cells: queued/native-retained cutoff, one/two streams and
retained/dropped operation or graph observers. Retain the capture future in
every cell for complete output and extracted-result-credit checks; dropping that
future would require a separately named cleanup-only cell. Register the source
and reserve its destination before cutoff, pre-admit verification downloads,
and permit accepted-prefix publication after cutoff. Keep graph and
standalone-operation schedules separate where exclusivity requires it.
Independently check all output/guard bytes,
odd offsets/tails, unique identities/publications, generations, exact phase/cell
rosters and bounded canonical evidence; reject malformed data, duplicate keys,
omitted phases and changed outputs. Require conclusive drain, capture-credit
retention through extraction/shutdown, refund after result disposal and exact
native cleanup. CPU/checker/runner work needs neither GEN-2 nor a GPU window;
signed capture does. Generated-kernel cells require GEN-2. Native retention is
not proof of physical activity or overlap at cutoff.

### DRN-1A Implementation Ownership

The completed implementation was split as follows. These rows describe the
integrated packet, not additional outstanding implementation assignments.

| Lane | First bounded ticket | Deliverable | Then |
| --- | --- | --- | --- |
| Native | DRN-1A native capture | Fixed-error coherent read-into, exact late native identity/currentness validation and no capture-induced GPU work | Native budget forwarding/pool hooks after resource review; SCALE-1; review OVL-QUAL-2 when hardware is available |
| Resources | DRN-1A charged destination/result | Isolated owned-storage module; charge actual slice bytes before cutoff and retain the debit after reply extraction and owner shutdown | MEM-2A-FWD and MEM-2B; MEM-N1/TXN/DOM prerequisites; MEM-3/4/5 and VER-1/2 |
| Admission | DRN-1A capture state | Bounded registration, admission cutoff, private quiescence gate and exactly-once capture reply | DRN-3B; DRN-2 qualifier; GEN-2 interface and later production integration |
| Primary | DRN-1A shared integration | Context source/SPI, configuration/owner/reply hooks, cross-review and composed release gates | OVL-QUAL-2 in an idle window; compiler handoff; PRF-1/2 and dual-remote publication |

These four assignments are parts of **one DRN-1A integration packet**.
Native validation is not a later DRN-1B, and result-credit review alone is not
implemented result ownership. All four parts and their focused failure tests
land together before capture is marked implemented. DRN-3B expands the composed
failure matrix afterward; it is not permission to defer basic capture safety.

### Capture File Ownership

Paths are relative to `crates/`; these modules now exist.

| Owner | Isolated implementation | Shared integration handoff |
| --- | --- | --- |
| `r66_native_coexistence` | New `fe2o3-runtime/src/kfd_backend/drain_capture.rs`, typed SDMA seam helper, mapped-read forwarding/tests in KFD `sdma.rs` and `shared_memory.rs` | Primary integrates `queue_live.rs` and `kfd_backend.rs` hooks; no simultaneous pool or capacity edits in these files |
| `r66_coexistence_model` | New `fe2o3-runtime/src/async_engine/drain_capture_storage.rs` and ownership/credit tests, reusing `fe2o3-resource-accounting` | Primary wires configuration and reply construction; no duplicate snapshot or reply-byte ledger |
| `r66_runtime_coexistence` | New `fe2o3-runtime/src/async_engine/drain_capture.rs` and focused capture/drain tests | Primary integrates `async_engine.rs`, `async_engine/drain.rs` and `async_engine/owned.rs` |
| Primary | New `fe2o3-runtime/src/context/drain_capture.rs`, sealed source descriptor and backend request/error contract | Owns `context.rs`, shared exports, model/proof registration, all integration builds and publication |

The integrated source descriptor, fixed errors, charged-result API and private
quiescence witness were frozen before the three lanes started editing. Registration binds
the logical Context/device/allocation incarnation before cutoff; native backing
is resolved after drain because already-accepted D2H may materialize it. Reserve
slice-byte and reply-cell capacity and preallocate result metadata before
cutoff. The capture account does not charge metadata bytes. Caller allocation
is not retroactively prevented by admission.

Acceptance must test foreign/stale/released/pending/noncoherent sources,
registration versus cutoff races, completed-D2H shadow dirtiness, partial-copy
rejection, owner panic and reply extraction followed by owner shutdown. Result
storage must remain charged until its known disposal, without a raw uncharged
`Box`/`Vec` escape. A public drain report cannot mint capture authority.

Capture performs no GPU publication, completion polling, flush, synchronization
or native allocation. Required observation-only operational-currentness checks
may use fixed-stack reset-fence readiness polling and a DRM loss-counter ioctl;
these are not completion progress. The existing operational path and queue
loan/retake appear allocation-free under source review, unlike full
topology/observable-currentness reconstruction. Require allocation-counting
tests on success and ordinary rejection, fixed errors and preallocated results
before claiming no new userspace allocations. Panic machinery and kernel-internal
allocations are not proved bounded by this test.

Only the primary runs Cargo or MI300X jobs, integrates shared hooks, signs and
pushes. Agents cross-review another lane before release. An idle-hardware
dependency does not block the remaining CPU implementation queue.

### Explicit Accounting Prerequisites

These subdivide the existing MEM backlog, not additional parity claims. Native
backing admission alone does not provide any of these mechanisms.

| Packet | Owner / dependency | Deliverable and completion gate |
| --- | --- | --- |
| MEM-2A-FWD | Implemented in R70; resources contract and native/primary hooks | Immutable session-local limits precede N2 allocation and queue certification in both startup orders. CPU tests cover defaults, late/replacement/foreign rejection and actual configuration transfer/loan/retake; live acceptance remains open |
| MEM-N1A | Implemented locally in R72 | Ordinary coherent non-userptr GTT backing, page-padded host bytes and one allocation record, charged before VA/allocation/map effects; same object has one charge across CPU/GPU views and queue loans. Linux hardware acceptance remains separate |
| MEM-N1B | Broader profiles after N1A layout/disposal review | Userptr, doubled-VA AQL, executable and control variants; distinguish physical backing from reserved VA. The current resource vector has no VA-byte dimension; do not substitute VA bytes for residency |
| MEM-2B-HOST | Resources policy and native hooks after R72 N1A | Bound only the existing ordinary coherent host cache; preserve the resident debit through checkout/recycle and uncertain disposal. This narrow profile need not wait for all N1B variants |
| MEM-QUAL-HARNESS | Native example/checker; primary runner integration | Exercise actual optional N1/N2/cache budgets in both startup orders before signed pressure/reuse/disposal acceptance. Existing overlap/drain examples configure logical requested-byte limits, not these native budgets |
| MEM-TXN-1 | Implemented in R70; prerequisite for compound MEM-3 creation | Atomic complete roster of vectors and owner slots returns independent move-only reservations. Ordinary late-member/record/generation failure leaves state unchanged; detected internal corruption poisons. Native composition remains open |
| MEM-DOM-1 | Resources domain contract; primary root/construction hooks; prerequisite for MEM-5 aggregate closure | Bind root/device/Context accounts and reserve bootstrap/terminal headroom before ownership. Repeated Context creation and simultaneous failures cannot reset the ceiling or require unreserved bookkeeping. Session-local accounting may land first only with its narrower scope explicit |

The implemented [batch API](runtime-resource-batch-v1.md) creates independent
member reservations; it does not split an already-issued retained debit.
MEM-N1A must test default/configuration closure, exact padded costs, ordinary
admission rejection without effects, pre-record failure/panic quarantine, foreign
record/domain substitutions, map/read/write/seal and queue loan/retake, and host
recycle/checkout retention. Only confirmed complete backing/VA disposal refunds;
a later queue-retake failure must not resurrect an already disposed debit.
Native compound creation must adopt exact pre-reserved member credits rather
than charging the same backing again inside N1/N2. N1A alone does not qualify
all GTT profiles, host-pool ceilings, complete bootstrap or aggregate quarantine.
It does cover ordinary coherent allocations made during bootstrap, such as
completion/control storage, when they use the exact admitted ordinary profile.
MEM-DOM-1 must not be postponed to a documentation-only final
audit. Host images, journals, captures and independent native owners may be
implemented incrementally, but the final aggregate gate includes all of them.

### Bounded Work Packets

Each packet is a reviewable deliverable with its own tests. Suffixes subdivide
the existing tickets; they do not weaken or replace the parent acceptance gates.

| Packet | Owner | Concrete output and completion gate |
| --- | --- | --- |
| SCALE-3-PROTO | Native | Protocol/checker in `benchmarks/runtime_gfx942`; reject mismatched artifacts, geometry, bytes, completion/reuse policy, missing samples and unsupported overlap claims |
| OVL-DIAG-1 | Native; primary integrates | Qualification-only typed stage errors with cell/order/direction/observation phase; actual-R26-shape regressions and one-coordinate negatives; repeated observations preserve state/digests; retain binary on post-build rejection; no addresses, polling or weakened checks |
| OVL-QUAL-2 | Primary; native reviews | Signed two-run MI300X capture of all eight cells; independent canary/custody/census audit and exact owned-process/stage cleanup |
| SCALE-1 | Native | Separately admitted short/long artifacts with fixed work bounds, ABI/effects and independent oracles; qualify correctness before scheduling claims |
| MEM-5 inventory contract | Resources; primary approves | Allocation-site inventory with no double charging within each measured quantity, actual layouts and custody/refund events; settle the cross-crate adapter before MEM-2/3/4 edits |
| MEM-BASE | Resources; primary integrates | Move the existing ledger into a dependency-safe shared crate; preserve runtime device branding, exact account isolation, token transitions and error/usage compatibility; include moved tests in release gates |
| MEM-2A/B | Resources; native handoff | A: backing layout preflight and retained native lease. B: bounded existing pool checkout/recycle/trim; recycling keeps resident charges, physical disposal returns them |
| MEM-2A-FWD, MEM-N1, MEM-TXN-1, MEM-DOM-1 | Resources; primary/native handoff | Explicit construction, host-backing, compound-admission and account-domain prerequisites described above; none is implied by MEM-2A |
| MEM-3A/B | Resources; native handoff | A: queue/ring, signal, kernarg/control arena residency. B: occupied slots and fixed acquisition order; every reservation precedes publication |
| MEM-4A/B | Resources | A: retained host-image ceiling. B: materialized code/control and cache leases tied to exact native identity; failed unload retains charges |
| MEM-5 closure | Resources; primary integrates | Closed command/result/capture footprints, registry/arena overhead, retained replies, terminal records and aggregate quarantine; repeated Context creation and simultaneous failures remain bounded |
| VER-1A/B | Resources; primary hooks | A: host-write/copy Context journal. B: conservatively invalidate every other mutation path; unknown outcomes never preserve usable stale versions |
| VER-2 | Resources | Exact private copy-graph input leases; reject intervening writes, foreign Contexts, mixed generations and replay before issue |
| DRN-3A/B | Admission | A: ready failure/cancellation delta cases. B: capture failure after DRN-1; assert one reply, exact retention and no duplicate publication |
| DRN-1 | Admission; native helper review | Bounded host-only capture after quiescence, before cleanup; reject stale/pending/released/noncoherent storage without issuing GPU work |
| GEN-2 | Admission; compiler handoff | Private permit binds artifact, arguments, decoder, geometry, effects, device and generations through quiescence; blocking joins the same async path |
| DRN-2 | Admission; primary hardware | Outstanding accepted/queued and native-retained drain capture, copy profile first; complete outputs and explicit cleanup, with fixture and production cells separate |
| SCALE-CAP, SCALE-2 | Native | Opt-in native capacity, then measured short/long completion and native depth; no constant-only capacity increase or queued-as-published count |
| SCALE-3 measurement | Native; primary hardware | Matched HIP/HSA/KFD captures after correctness qualification; separately reviewed device timeline for any physical-overlap claim |
| PRF-1/2 | Primary; rotating cross-review | Incremental executable composition, full integration gates, exact evidence/source identities and publication; pure guard proofs do not close whole-state refinement |

The native accounting adapter is an explicit architectural prerequisite.
`RuntimeResourceCreditAccountV1` remains a runtime-private device-branded wrapper;
the engine and opaque tokens now live in `fe2o3-resource-accounting`. KFD cannot
depend upward on the runtime crate; its MEM-2A adapter now consumes the shared
engine. Resource and native owners must agree which layer measures actual layout, holds
each charge through cached residency and recognizes physical disposal. Existing
requested-allocation credits remain a distinct quantity, and public counter
mechanics are not native-disposal evidence.

### Earlier Packet Contracts

The earlier scoping review fixed the following small implementation boundaries.
MEM-2A and DRN-1A are implemented locally as described above; SCALE-1A remains
queued. These historical contracts do not replace the current assignments
above. None is hardware-accepted by this scoping review.

- **SCALE-1A, native owner:** two separate qualification-only fixed-work artifacts
  and a sequential correctness qualifier. Freeze a small complete ReadWrite
  allocation, geometry and compile-time work after IR/ISA review; authenticate
  source/object/policy/toolchain and check every output byte with an independent
  oracle. Keep R26/R60 unchanged. The native owner owns the fixture, admission
  module, example and checker; primary owns backend/module hooks. No capacity,
  duration, scheduling or overlap conclusion follows from intended short/long
  work classes. New R66 rejection diagnosis takes priority.
- **MEM-2A, resource/native owners:** first settle an explicitly session-local
  N2 backing-cap contract, then debit internally derived
  `device_memory_layout(...).backing_bytes` before native effects. Keep the
  retained charge in private `DeviceMemoryRecord` through mapping, initialization,
  retagging and ambiguity; refund only complete successful disposal. Resources
  owns an isolated `shared_memory/resource_accounting.rs` adapter; native owns
  `shared_memory.rs` allocation/disposal and fake-backend failure tests. Primary
  owns account-domain/configuration approval, dependency and proof registration.
  Parent/root admission, session setup, GTT, pool qualification and global
  ceilings remain separate. MEM-BASE alone supplies none of those domains.
- **DRN-1A, admission owner:** one pre-registered bounded HostVisible range,
  captured after privately witnessed conclusive drain and before cleanup.
  Reserve bytes/metadata/reply capacity before cutoff and carry credit with
  the owned result. Native supplies currentness-checked direct coherent
  read-into; do not use Context readback, which can synchronize/download.
  Reject stale, foreign, pending, unknown, released or noncoherent storage and
  partial results. Admission owns drain-capture state/tests; primary owns
  Context/owner-loop/credit hooks; native owns mapped-read forwarding. This
  copy/host-write slice can precede GEN-2, but decoded bytes and public drain
  reports cannot authorize generated results. Signed active-work and whole
  executor proof gates remain separate.

The native DRN-1A review refined the capture boundary. Register the logical
allocation incarnation before cutoff, but resolve its exact live native
session/allocation/queue/pool generation only after conclusive drain: accepted
D2H work may legitimately create or promote backing in between. Native
HostVisible storage must be initialized, directly mapped and free of pending
read/write custody. `sdma_shadow_dirty` alone is not a rejection: completed D2H
can make the native host bytes current while the separate Arc shadow is stale.
Capture must read the native bytes without repairing that shadow. Add bounded
typed-error read-into forwarding through the backend, SDMA seam, queue and
memory owner; existing boxed readback and synchronization fallback cannot be
used. Pre-copy rejection leaves the private destination untouched; closing
currentness failure discards it and publishes no partial capture. These are
implemented DRN-1A boundaries; their hardware and whole-executor proof gates
remain open.

## Acceptance Matrix

`Partial` means only the named restricted slice exists. `Open` means this
ticket's acceptance is not established, even where earlier reusable primitives
or fixture results exist. None of the rows establishes runtime-wide parity.

| Work | Implementation | Authenticated proof | CPU integration | Fixture hardware | Production-admitted hardware | Performance |
| --- | --- | --- | --- | --- | --- | --- |
| OVL-1/2 | R66 restricted profile | Bounded scan only; native extraction/composition open | Both orders, H2D/D2H, one/three bindings | Open: OVL-QUAL-1/2 | Open: GEN-2 also required | Open |
| OVL-QUAL-1/2 | Qualifier and typed diagnostics; R71 corrects active-owner membership; live gate open | Descriptive observation, not authority | Eleven runtime observation tests, six native diagnostic tests and runner/checker negatives | Open: new signed capture required | Open: GEN-2 also required | Physical overlap unmeasured |
| MEM-1 | Local transactional credits and Context requested-allocation profile | R67: 14 vector/record obligations and eight mutations passed full authenticated gate; adapter composition open | Ten Context, nine shared-engine and three device-wrapper tests; two core ownership compile-fail examples | New qualifier includes requested-credit saturation/retention/disposal; not yet accepted | Open | Unmeasured |
| MEM-2A/FWD | Optional N2 admission and immutable runtime forwarding through both native startup orders | R68 cost projection; constructor/native correspondence open | R68 coverage plus R70 native round-trip, failure and runtime-history tests; public Linux constructor wiring remains source-tested | Physical saturation/reuse and constructor qualification open | Open | Unmeasured |
| MEM-N1A | Optional ordinary coherent GTT admission, lifetime/panic guards and both-order forwarding | R72: five cost-projection obligations and nine negatives passed; native disposal/refinement open | 18 native fixtures, nine adapter, four model and eight runtime tests; Linux queue/pool behavior remains source-reviewed | Open | Open | Unmeasured |
| MEM-TXN-1 | Atomic bounded roster on the shared account with independent member ownership | R70 executable arithmetic/roster projection; concrete mutex/arena/token refinement open | Fourteen core and six model tests, mixed scalar/batch stress and one compile-fail example | Not a native consumer | Open | Unmeasured |
| MEM-2B | Device-only cache ceiling and runtime forwarding | R71 bounded policy scans; native extraction/disposal refinement open | Six model, five policy, eight native and six runtime forwarding tests; some native/facade wiring remains source-only | Live pressure/reuse/disposal in both startup orders open | Open | Unmeasured |
| MEM-N1/DOM/3..5 | Shared engine and site inventory available; broader host profiles, complete control/code accounting and global closure open | End-to-end accounting open | Complete composed failure matrix open | Physical saturation/reuse open | Open | Unmeasured |
| VER-1/2 | Open; R65 lineage is graph-local | Persistent authority open | Cross-run mutation/lease matrix open | Open | Kernel extension also needs GEN-2 | Unmeasured |
| GEN-1 | Owned data boundary implemented locally | No execution authority or whole-async proof | Eleven focused host tests and generated fixtures pass | Data-only boundary | Not an execution cell | Unmeasured |
| GEN-2R | Charged storage/packing and nonblocking slot polling implemented locally | Seven cost/shape obligations and eight mutations passed; no mutex/native/whole-executor refinement | Twenty charged-path tests, three model tests, four new generated negative cases and four compile-fail doctests; host thread is not runtime-owner shutdown | Data-only boundary | Not an execution cell | No speedup measurement |
| GEN-2A/B | Exact typed invocation/async authority and Context/native integration open | Compiler evidence and async composition open | Permit/currentness/native integration matrix open | Fixtures cannot fill production cells | Open | Unmeasured |
| DRN-1/2/3 | R65 drain, DRN-1A capture and DRN-3A/B failure composition exist | R69 range guard only; whole drain/executor refinement open | R69 tests plus twelve DRN-3B operation/waiter, graph, exhaustion and Stop scenarios | R65 idle only; DRN-2 outstanding-work open | Open | Unmeasured |
| DRN-2A | Eight-cell copy qualifier/checker/runner | No whole-drain or native refinement proof | Eight observer tests, actual scripted async-copy/pending-poll regression and seventeen checker/runner tests | Signed outstanding-work campaign open | Generated cells require GEN-2 | Unmeasured |
| SCALE-1/CAP/2 | Open; default remains 64 epochs per compute lane | Capacity/acquisition composition open | Larger native-capacity profile open | Short/long and native-depth open | Open | Unmeasured |
| SCALE-3 | Protocol/checker implemented; signed/timestamp producers open | Optimization/timeline boundary open | Fourteen protocol tests and independent review pass | Measurements open | Measurements open | No R66 result |
| PRF-1/2 | Whole executor composition open | Isolated guards are not whole-state refinement | Current-source local gates retained; optional legacy-musl compiler unavailable | Per-profile qualification open | Open | Per-workload only |

R66's first local runs could not inspect sockets or use ptrace. After those
restrictions were lifted, GNU and musl each passed all 1,992 tests with five
existing ignores; the separate attempts are retained in the validation record.
SSH to `mi300x` is reachable again, but no R66 hardware cell is filled merely
by restoring access. The qualifier and its independently audited capture remain
required.

## Native Execution Lane

### OVL-1: Private Native Disjoint-Custody Checker

**Implemented in R66; native extraction refinement remains open.** The private
checker in `crates/fe2o3-kfd/src/queue_live/compute_sdma_coexistence.rs` derives decisions
from retained compute storage and complete directional SDMA slot/window ledgers,
including exact session, incarnation and generation. Initially reject a shared
allocation even when claimed byte ranges are disjoint. Reject incomplete,
foreign, terminal or unknown custody. Caller IDs or booleans are not witnesses.
OVL-1 alone did not relax publication; R66 composes it with OVL-2 below.

Acceptance: production-usable bounded predicates, authenticated noninterference
obligations and negative mutations for aliasing, stale generations, missing
window anchors and foreign owners. This is checker validation, not overlap.

### OVL-2: Reciprocal Directional Compute/SDMA Admission

**Implemented in R66; live qualification remains open.** Integrates the checker into
`fe2o3-kfd/src/queue_live/fixed_dispatch.rs`, `queue_live.rs`, and runtime
`kfd_backend/compute_dispatch.rs` plus the reciprocal copy-publication path.
Preserve primary-only persistent compute and auxiliary-compute, generic-copy,
striped and XGMI exclusions outside the new directional profile. Preserve
initialization, retirement, poison, quarantine and shutdown contracts.

Acceptance: both publication orders, disjoint success, alias rejection, delayed
completion, retry without duplicate issue, timeout, currentness failure and exact
release. First hardware evidence uses the exact R26 compute profile and disjoint
copies, with complete output/padding checks and cleanup. Simultaneously retained
native submissions establish concurrent custody, not physical GPU overlap.

### OVL-QUAL-1: Signed Coexistence Qualifier

**Implemented locally; a new current-source freeze and live acceptance remain.** The
`gfx942-runtime-r66-coexistence` example and R66 runner/checker use the unchanged
R26 in-place compute artifact and separate directional H2D/D2H storage. Eight
cells cover both publication orders, both directions and one/two copy packets.
They check exact retained native identities, copy retirement while compute
remains retained, independent full outputs/padding, retirement and explicit
cleanup. The hardware profile is R26 qualification-only, not the three-binding
or general generated-production profile.

The qualifier also checks exact requested-byte and seven-record allocation
admission, rejection of an eighth allocation without credit changes, retention
through completion and zero usage after successful Context cleanup. This is
requested-allocation accounting, not physical residency or pool-budget evidence.

R61/R65 source, topology, census and process guards are reused.
`hardware-qualification` is authenticated in both the build and Cargo-metadata
commands. Initial local coverage included example compilation, four observation
tests and runner mutations; the later diagnostic checkpoint expanded coverage
as recorded above. This delivers a locally tested harness, not successful native
admission or overlap.

### OVL-QUAL-2: Live Coexistence Acceptance

**Depends on OVL-QUAL-1 and reachable idle hardware.** Primary runs the exact
signed qualifier on one admitted MI300X with private staging and bounded
processes. Independently audit output, native identities, topology/census and
owned-process/stage cleanup. Prior R26/R65 captures do not qualify R66. Pending
host intervals do not establish physical overlap; that remains SCALE-3.

The first signed `bd8aa3de` campaign built successfully but rejected the first
qualifier's native-roster observation. It did not reach a complete eight-cell
pass or the second run. All recorded owned processes/groups and the private
stage were independently confirmed absent afterward, with GPU 1 idle. Preserve
that rejection and diagnose it before a new signed campaign; access and cleanup
alone do not establish native acceptance. The subsequent signed `d5ada879`
campaign built and retained its audited binary but did not launch the qualifier
because the selected GPU became busy. Its owned staging/process cleanup passed;
native-roster diagnosis still needs an idle window.

### SCALE-1: Admitted Short/Long Qualification Profiles

**Ready for profile design now.** Add separately reviewed qualification artifacts
and gates with bounded geometry, small disjoint storage footprints, exact ABI and
effects, deterministic independent oracles and authenticated build identities.
Do not widen existing R26/R60 fixed-profile gates or relabel fixtures as general
compiler authority. Reject changed work bounds, artifacts, ABI, effects and inputs.

Acceptance: qualify each profile's correctness independently before making an
out-of-order scheduling claim. General production acceptance additionally needs
GEN-2 and matching compiler-owned evidence for each executable class.

### SCALE-CAP: Opt-In Native Capacity Profile

**Depends on native MEM admission and SCALE-1's bounded workload profile.**
Own KFD `queue_dispatch_binding.rs`, `queue_completion.rs` and runtime
`kfd_backend/compute_state.rs`; primary integrates queue configuration. Target
1,024 retained epochs on each existing compute lane in a separately admitted
profile, preserving the current default. Validate feasibility against actual
ring headroom, signal capacity and aggregate memory limits before promotion.
Widen private `u8` slot identities and audit all consumers; raising the constant
alone is insufficient. No new queues are assumed necessary by the initial design.

Acceptance: checked generation arithmetic, exact reservations/rollback,
wraparound and stale-slot rejection, signal-reader retention and no reuse before
retirement. The hardware gate requires at least 2,048 simultaneously
native-published/retained epochs with exact identities and complete cleanup.
Report observed completion status separately: retained does not mean physically
running or even still incomplete. Queued commands do not count. SCALE-2 owns
this measurement; capacity implementation alone does not pass it.

### SCALE-2: Hardware Depth And Out-of-Order Campaign

**Depends on SCALE-1 and the required MEM tickets; the thousands-native cell
also needs SCALE-CAP, and mixed compute/copy needs OVL-2.** Add an async-owner
example and signed runner/checker. Precommit the
depths, memory ceiling, deadlines and resource-reuse count. Exercise later-short
completion before earlier-long completion, exact operation/native identities,
dropped and timed-out observers, backpressure and successful cleanup.

Report accepted/queued, native-published, unresolved and retired counts
separately, including peak occupancy per native resource. The current restricted
compute profile has two lanes and at most 64 fixed-dispatch epochs per lane;
directional SDMA queues have their own finite slot limits. Thousands of accepted
operations cycling through these slots are not thousands of simultaneously
published native operations. CPU tests with 2,048 operations qualify neither.

SCALE-CAP supplies the separately reviewed native capacity expansion; increasing
engine queue capacity alone cannot satisfy that cell. Keep the native-depth gate
open until the claimed occupancy is actually measured. Active-drain content
checks can now consume implemented DRN-1A capture; DRN-2 still owns signed
outstanding-work content qualification. Successful content and custody-only
drain evidence remain distinct.

### SCALE-3: Measured Overlap And Matched HIP/HSA Results

**Protocol/checker implemented; measurements depend on qualified workloads and
signed producers.** [SCALE-3-PROTO](../benchmarks/runtime_gfx942/scale3-protocol-v1.md)
provides a versioned measurement schema, bounded checker and fourteen passing
tests without production queue changes or GPU execution. Its pinned plan binds
exact argument templates, artifacts, GPU/NUMA/CPU, stream/memory policy and a
balanced backend rotation. Its acceptance establishes consistency, not hardware
authenticity, budget closure or physical overlap. Match exact
artifacts, geometry, bytes, selected GPU,
completion semantics and resource-reuse policy. Separate setup, submission,
transfer, wait and validation costs. Rotate backend order and publish sample
counts, latency tails, throughput, CPU cost and memory/native-resource peaks.
HIP/HSA remain isolated benchmark oracles, never production fallback paths.

Physical-overlap claims need checked device-timeline evidence with compatible
clocks. Current host publication/observation intervals and SDMA diagnostics do
not provide that proof. Review any new timestamp producer separately. Without
appropriate timeline evidence, publish only the supported end-to-end timing
result. Correctness captures and timing captures remain distinct. No speedup
factor or runtime-wide parity claim is presumed.

## Resources And Versions Lane

### MEM-1: Transactional Resource Credits

**Implemented locally for primitive credits and requested-allocation admission.**
`crates/fe2o3-runtime/src/resource_credits.rs` supplies nineteen-dimensional
per-device vectors, move-only reservations and failure-atomic admission.
`context/allocation_admission.rs` charges requested allocation bytes plus records
before backend entry. Definite rejected attempts and successfully disposed
logical allocations return those charges; uncertain attempts and failed releases
retain them. Cleanup cannot report complete with unidentified quarantined credits.

Each account has a preallocated bounded owner-record arena and conservative
process-lifetime quarantine retention. This is not aggregate quarantine across
arbitrarily many Contexts. Fields for physical bytes and native slots are not
measurements until the corresponding MEM-2/3/4 adapter supplies truthful charges.
The complete allocation-site inventory and metadata/global ceiling remain MEM-5.

Acceptance: checked arithmetic, complete-vector rejection, conservation, exact
ownership transfer and exactly-once return. Mutations must detect overflow,
partial debit, duplicate refund and refund before successful disposal. A small
logical counter is not a native memory budget.

### MEM-2: Native Allocation And Pool Residency

**Depends on MEM-1 and the approved native accounting interface.** Add a
pool-budget module beside existing KFD SDMA pools;
integrate existing allocate/recycle/trim paths rather than a second allocator.
Charge actual backing including padding. Checked-out and cached-free buffers
both consume residency; recycling does not refund resident bytes.
Successful logical Context release may move backing into the free pool rather
than physically release it. Keep MEM-1 requested bytes distinct from this
retained native charge. Split work into actual-layout admission (MEM-2A), then
checkout/recycle/trim ownership and cache ceilings (MEM-2B).

MEM-2A may qualify a single backing allocation first. Whole Context allocation
accounting also needs MEM-3: allocating can lazily initialize queues, control
storage and scratch. The [native accounting inventory](runtime-native-resource-accounting-v1.md)
describes the extracted lower-level ledger and implemented session-local N2
admission. Exact root/device/Context domains and bootstrap costs remain proposed;
MEM-DOM-1 and the other explicit prerequisites must implement them before
aggregate admission is claimed.

Acceptance: fail before over-budget allocation, bound cached capacity, transfer
checkout custody without double charging, and return credit only for successfully
released backing. Test alignment, oversized extents, generation exhaustion,
failed trim and terminal retention.

### MEM-3: Native Submission-Control Resources

**Depends on MEM-1 and the approved native accounting interface.** Budget
completion signals, aligned kernargs/control storage,
queue/ring residency and per-operation slots. Charge preallocated arenas once
and their occupied slots separately. Integrate preparation, publication and
recycling across admitted compute/SDMA lanes; do not duplicate low-level queues.
MEM-3A measures/reserves actual arenas; MEM-3B integrates occupied-slot credits.
Compound creation requires MEM-TXN-1 atomic fixed-roster reservation of both
costs and owner records. Prefer independently retained member debits created by
that admission transaction; do not assume an issued retained charge can be
partially refunded. N1-backed arenas also require MEM-N1. R70 supplies the
single-account batch primitive, but native member-credit adoption and parent
admission remain open. MEM-DOM-1 is required before an aggregate ceiling claim.

Acceptance: no publication without every required reservation, exact generation
on reuse, and no timeout/cancellation refund of possibly referenced storage.
Delayed completion, saturation and ambiguous publication remain bounded and
retain their charges. Prove a consistent progress-resource acquisition order.

### MEM-4: Executable Residency

**Depends on MEM-1 and the approved native accounting interface.** Add proposed
runtime `kfd_backend/residency.rs`, reusing
existing module records, compute retain counts and recycled-dispatch release.
Separate retained host images from materialized executable/control bytes. Bind
cache entries to exact device/Context, image and materialization identity.
MEM-4A bounds retained host images; MEM-4B integrates materialization and cache
leases. Classify native executable GTT backing truthfully rather than calling
every device-accessible executable byte VRAM. Coordinate control-storage
ownership with MEM-3 to avoid double charging.

Acceptance: repeated loads/dispatches remain bounded; leased executables cannot
be evicted; substitution rejects; failed unload retains its residency charge.

### MEM-5: Close The Bounded Production Payload Profile

**Inventory contract first; closure integrates MEM-1 through MEM-4.** Record each
concrete allocation site, actual extent, category, account owner, custody
transfer and successful refund event. Include credit-arena/registry overhead,
version journals and observer-retained results. Define a bounded production
command/result surface whose owned bytes can actually be measured. Account for
captures, terminal records and retained results, including DRN-1 capture bytes.
Unrestricted generic closures cannot be made byte-bounded by trusting a caller's
claimed size; keep them outside the qualified bounded profile or replace them
there with closed, accounted commands.

The source-reviewed [inventory and interface proposal](runtime-native-resource-accounting-v1.md)
is available. Enumerating site families does not establish complete measured
metadata, driver/OS overhead or global quarantine bounds.

Reserve worst-case quarantine bookkeeping/headroom before accepting native
custody, or retain it within the already-charged global ceiling. Simultaneous
failures must not need an unreserved capacity increase. Uncertain native resources
retain charges in a named quarantine domain that outlives engine teardown when
necessary. Saturation stops new admission; it never authorizes freeing live
storage. Timeout, observer drop, logical drain and engine teardown do not refund
ambiguous custody. Acceptance must cover retained completed replies, abandoned
observers and repeated/simultaneous failures. A1/A2 budget closure requires a
documented complete resource inventory, not just success-path counters or an
exclusion footnote.

### VER-1: Persistent Context Mutation Journal

**Ready independently.** Add proposed runtime `context/versions.rs`. MEM-5 must
include its bounded journal storage. Begin with
a bounded copy/host-write profile, allocation identity and nonwrapping mutation
generations; conservative whole-allocation invalidation is a sound first slice.
Invalidate before mutation, retain exact pending ownership, and distinguish
available, pending and unknown outcomes. Existing generic mutation paths must
invalidate affected authority conservatively; precise kernel effects require
compiler-owned admission.

Primary integrates every applicable write, launch, copy, peer/atomic/collective,
retirement, allocation-reuse and generation-loss boundary. Unsupported mutation
paths must invalidate or reject, never silently preserve freshness.
Acceptance: failed writes, partial overlaps, ordinary Context operations,
cancellation, generation exhaustion and unknown publication cannot leave a stale
version usable. Currentness does not prove initialization or content correctness.

### VER-2: Cross-Run Input Leases

**Depends on VER-1 and the existing exclusive graph reservation.** Add proposed
runtime `async_engine/graph/input_leases.rs`. Context issues private tokens;
graph admission atomically checks and reserves exact allocation/range/generation.
Historical `RuntimeGraphDataVersionV1` reports remain non-authoritative.

Acceptance: repeated copy graphs consume current versions; intervening writes,
foreign Contexts, stale allocation generations, mixed versions and replayed
leases reject before issue. Dropping a report cannot create or release authority.
Extend to compiler-admitted kernel graphs only with GEN-2 effects.

## Admission And Drain Lane

### GEN-1: Owned Generated Arguments

**Implemented locally as an owned data boundary.** Host
`generated_runtime_arguments.rs` and generated wrappers reuse authenticated
packing plans and ABI identities with genuinely owned boxed inputs. A read-only
whole-invocation footprint preflight precedes encoding. Decoder custody survives
observer drop; complete shape and exact returned-buffer capacities are checked
before output delivery. Compile-fail cases reject borrowed escape, kernel
substitution, output aliasing and safe implementations of the unsafe generated
trait. The existing borrowed blocking bridge is not widened to `'static`.

These are per-invocation logical storage bounds, not global credits or native
residency. A decoder accepts inert data; it does not authenticate the producing
invocation or prove completion. GEN-2 must privately bind the decoder to its
exact invocation and normalize native returned-buffer capacity. Context
allocation freshness and publication authority are not granted by packing.

Owned-data acceptance: ABI/type/binding mismatch, stale output custody, aliasing,
payload bounds, dropped observers and compile-fail borrowed-storage escape.
Context allocation freshness and reservation integration belong to GEN-2 and
the primary's shared hooks. Positive production execution awaits GEN-2.

### GEN-2: Authenticated Typed Async Admission

**Runtime contract work can proceed; production acceptance is dependency-gated.**
The current dispatch splits this into GEN-2A nonexecuting owner-local preparation,
GEN-2R charged typed storage and GEN-2B Context/native publication integration.
Resources and Admission review separate host module contracts; Primary owns
all edits, including existing argument-binding, export, dependency and macro
changes. GEN-2R's implemented data interface has passed its local gates. The checked
device remains owner-local without widened lifetimes or thread-safety claims.
GEN-2R uses a distinct production result state: the existing GEN-1 bare-`Box`
result path must never receive production bytes. One R70 member per output
retains its complete encoded-plus-typed peak reservation through charged-result
disposal; private preallocated decoding publishes only after every output
validates. This is conservative reserved capacity, not instantaneous residency.

Integrate host `generated_kfd_invocation.rs`, a private async-admission adapter and
runtime `authorized_execution.rs`. Consume a non-forgeable permit binding exact
artifact, ABI/effects, arguments, geometry, required initialized regions, device
and resource generations. Retain compiler-publication/proof custody through
native quiescence and revalidate at the publication boundary. The blocking entry
must join this same async path, not retain a second execution implementation.

Invocation/native credits survive the required retirement and physical disposal.
Decoded-output byte credits survive movement out of reply cells and remain with
observer-retained storage. Native quiescence alone cannot refund that storage.
The [owned arguments and credits contract](runtime-owned-arguments-and-credits-v1.md)
separates the implemented data boundary from these still-open authority/lifetime
obligations.

Acceptance: fixture substitution, stale publication, mismatched receipt/artifact,
changed arguments/device and duplicated permits reject. Loss of currentness
after publication retains custody and never grants replay permission.
[#134](https://github.com/harsh-nod/fe2o3/issues/134) owns admitted compiler plans;
matching semantic-to-machine evidence is also required.
[#214](https://github.com/harsh-nod/fe2o3/issues/214) covers scalar GEMM, not
authorization for every kernel. Its receipt alone cannot close broad generated
kernel acceptance. The current protected path ships no concrete production
refinement backend/artifact; fixtures must not fill that authority gap.

### DRN-1: Bounded Host-Only Drain Capture

**DRN-1A implemented; signed outstanding-work and whole-executor qualification
remain open.** Runtime `async_engine/drain_capture.rs`, its charged-storage
owner, Context validation and lower-KFD coherent read-into compose the
[implemented contract](runtime-host-drain-capture-v1.md).
Pre-admit GPU downloads/canary copies before
the drain admission cutoff. Capture already-coherent host bytes only after
conclusive quiescence and before cleanup, with exact retained buffer generation
and bounded result storage.
Final MEM-5 closure incorporates the implemented capture storage and future
VER-1 version-journal owners, avoiding a circular prerequisite. Persistent
Context mutation journals are not implemented by DRN-1A.

Do not use unrestricted `Context::read_allocation`: existing fallback paths can
synchronize or download using SDMA. Host capture and its currentness checks must
never publish, upload/download, synchronize by dispatch or allocate native
storage. Reject stale generations, pending writers, partial coherence,
DeviceLocal fallback, excess bytes and released storage.

The cutoff closes new admission, not publication of previously accepted work.
Accepted commands and graphs may still issue during drain. Trace gates must
reject work outside that accepted prefix and any capture-induced publication
after quiescence, not legitimate post-cutoff execution of pre-admitted work.

### DRN-2: Outstanding-Work Hardware Qualification

**DRN-2A copy qualifier/checker/signed runner implemented; hardware acceptance
open.** Run its existing eight copy-only cells against signed source on an
admitted idle GPU. Subsequent DRN-2B extends coverage to separately named compute
fixtures under their own explicit qualification authority. Generated production
cells require GEN-2B and exact per-kernel compiler evidence.
Cover queued and already published unresolved work, multiple streams and dropped
observers; do not rebuild the copy qualifier or drain lifecycle.

Acceptance: recorded admission/publication/observation order, independent full
output/padding comparison, no duplicate issue, quiescent drain and explicit
cleanup. Unresolved at cutoff does not establish physical GPU activity at that
instant. An idle drain or small fixture run cannot close active native-depth,
mixed-duration or general-production cells.

### DRN-3: Failure And Retention Matrix

**DRN-3A, basic DRN-1A failures and DRN-3B CPU composition implemented.** Three earlier composed
regressions in `async_engine/tests/owned_tests/drain_tests.rs` check exact
accepted-submit rejection, repeated observation rejection without reissue and
pre-issue cancellation preserving a successfully submitted sibling. They check
reply/snapshot lifetimes, credit usage and retained allocation/module/stream/
submission identities. No production transition changed for these tests.
R69 adds capture rejection, terminal partial-copy, panic, abandoned observer,
cutoff-race and exhaustion tests. R70 adds four tests covering twelve controlled
scenarios: six operation/waiter/cancel-or-reject/capture cells, three active
two-stream graph/capture cells, repeated observation rejection with exhausted
drain, and Stop before pickup or during a paused synchronous capture. Graphs
remain exclusive of preexisting operation/waiter registries, so those matrices
are separate rather than weakening graph admission for a test. Exact reply,
snapshot/result credit, graph reservation, backend identity and release rosters
are checked without duplicate issue. Stop during capture cannot expose partial
bytes or preempt the synchronous callback.
R65 already covers quiescent failure, budget exhaustion, abandoned observers,
interruption and panic. Preserve those cases and DRN-3A when composing capture;
do not rebuild the drain lifecycle.

Shared-host hardware initially covers only non-disruptive rejection/cancellation.
Device resets, device-loss injection or partition failures require a separately
agreed hardware window; CPU fault scripts do not count as hardware fault evidence.

## Proof And Integration Gates

### PRF-1: Executable Reference-Executor Refinement

**Begin with the first implementation wave; compose incrementally.** Each ticket
owns production-used transitions/checkers, positive obligations, deliberate
negative mutations and focused adapter tests. Primary composes them into the
actual reference executor: lifecycle/registry retention, transactional credits,
admission-prefix closure, exact retirement, DAG readiness/trace refinement,
version begin/commit, pool reuse and drain terminal classification.

Whole-state invariants and correspondence cannot be replaced by additional
isolated arithmetic predicates. Lost leases, false quiescence, invalid versions,
premature release and duplicate issue must fail named obligations. Keep concrete
thread/channel behavior, executor fairness, KFD/firmware observations and kernel
machine semantics separately identified as checked, validated, contracted or
unsupported where executable refinement remains absent. Safety never assumes
eventual completion; any progress theorem names its additional premises.

### PRF-2: Integration, Evidence And Release

Maintain an acceptance matrix with separate implementation, authenticated proof,
CPU integration, admitted-fixture hardware, production-admitted hardware and
performance columns. Require targeted tests plus the existing relevant GNU/musl,
doctest, runner, lint, production dependency/symbol and authenticated Verus gates.
Tests of a rejected authority path do not fill its positive production cell.

Primary schedules one bounded MI300X campaign at a time: exact idle selected GPU,
private staging/cache, signed source, bounded subprocesses, topology/queue census,
full canaries and independent owned-process/stage cleanup. Do not touch foreign
jobs, reset devices or run an all-GPU campaign merely because SSH is available.
Publish evidence with explicit claim limits, then push reviewed task commits to
both `harsh-nod/fe2o3` and `powderluv/fe2o3`. Topic-branch pushes do not imply
merging to either main branch.

## Dependency Waves

| Wave | Native lane | Resource/version lane | Admission/drain lane | Primary |
| --- | --- | --- | --- | --- |
| Checkpoint, complete locally | OVL checker/integration, qualifier/diagnostics and SCALE-3 protocol | MEM-BASE, MEM-1 requested bytes and MEM-2A session-local N2 | GEN-1 owned data and DRN-3A regressions | Signed `dd202891` and R68 local/proof evidence; hardware gates remain open |
| 1, integrated R69 packet | DRN-1A coherent read-into | DRN-1A charged storage | DRN-1A capture state/tests | Source/currentness/owner hooks and DRN-1A local release gates; hardware separate |
| 2, integrated R70 packet | MEM-2A-FWD constructors and ownership round trips | MEM-TXN-1 atomic single-account member reservations | DRN-3B composed CPU failures | Runtime forwarding, current-source local/proof gates and R70 release; hardware separate |
| 2 follow-on, R71 packet | Device-only MEM-2B pool hooks and corrected lifecycle observation | MEM-2B policy/model/property proofs | DRN-2A copy qualifier/checker | Runtime observer/history, signed runner, integrated release gates; hardware separate |
| Follow-on, R72 packet | MEM-N1A native lifetime/panic and loan hooks | MEM-N1A cost/account adapter and property proofs | Host-budget forwarding tests and independent failure-path review | Both startup constructors, full local/proof gates; hardware acceptance remains open |
| Follow-on, R73 packet | Independent ownership/polling failure-path review | Charged storage/peak guards and separate result state | Packing route, generated fixtures and decoder-boundary review | Shared host integration, nonblocking slot-polling fix and seventeen final gates; GEN-2A/B and native authority remain open |
| Next dependency-ready packet | SCALE-1A-FIXTURE/qualifier; then MEM-QUAL-HARNESS and ordinary host-cache hooks | Host-cache policy, N1B and MEM-DOM-1; VER-1 can move earlier | GEN-2A nonexecuting owned invocation/decoder using R73; signed-copy campaign support | Own edits/shared hooks; OVL-QUAL-2 and DRN-2A signed campaigns; native-budget hardware only after its new harness; compiler handoff |
| 3, bounded local integration | MEM-3/4 native hooks, then SCALE-CAP | MEM-3A/B, MEM-4A/B, VER-1/2 and MEM-5 closure | GEN-2B Context/native bridge; DRN-2 fixture campaign support | Incremental PRF-1; production GEN-2 requires matching compiler/machine evidence |
| 4, acceptance campaigns | SCALE-2 depth/out-of-order, then SCALE-3 producers/measurements | Budget/version stress and independent review | Repeated generated graphs and active/failure drain | PRF-2, full A1/A2 exit audit and signed dual-remote publication |

VER-1 can move earlier when a slot is free;
the wave table is an execution order, not an artificial technical dependency.
Each lane executes one packet at a time; a row is not a promise of additional
concurrent agents. OVL-QUAL-2 can run against a separately frozen signed source
without waiting for DRN-1A. SCALE-CAP needs real backing/control/slot admission
and SCALE-1; MEM-2A or a larger command queue alone is insufficient. SCALE-3's
protocol is already implemented, but its authenticated producers, measurements
and device-timeline qualification are not.
External compiler work must have an explicit handoff artifact and owner. Its
absence does not block native checker, resource, copy-version or fixture work,
but keeps the affected positive production cells open. A1/A2 close only after
their complete acceptance matrix passes; later #182 milestones remain separate.

## Later Milestones

Keep the same lane boundaries when decomposing later work; these are not
additional A1/A2 acceptance claims or simultaneous worker assignments.

| Later work | Lead lane | Boundary still open |
| --- | --- | --- |
| Local multi-GPU execution | Native, with resources/versions | Unified compute/XGMI ownership and topology-current placement; current native XGMI backend is separate, exact-two-device and copy-only |
| Production atomics/collectives and broader device Rust | Admission with compiler owners | Exact compiler plans, machine refinement and native semantic qualification; typed transport contracts alone are insufficient |
| Device profiling and overlap attribution | Native | Trusted per-dispatch device timestamps, copy-engine events and cross-collector attribution; typed semantic profiling already exists |
| Distributed milestones in #182 | Primary decomposes after local contracts stabilize | Membership/epochs, bounded transport, distributed versions/collectives, failure/drain and executable refinement require their own tickets and test environments |

The [runtime parity profile](runtime-hip-hsa-parity-profile-v1.md) defines a
bounded behavioral surface, not every HIP/HSA API. Measured improvements must
name workloads and matched baselines; no speedup target replaces correctness,
proof or hardware acceptance.
