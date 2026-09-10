# Remaining A1/A2 Work: Swarm Plan

Planning baseline: `9e822ad737a21bd7cb563623ba3266066a6179d9` plus the R66
implementation described below, reviewed 2026-09-10. This decomposes the remaining local A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), not the later multi-GPU
and distributed milestones. Three agents independently reconciled their lanes
against the current code. Their read-only breakdown is complete; the queued
implementation assignments below are not claims of completed work.

The [R65 contract](runtime-async-drain-versions-v1.md) is the current baseline:
cooperative drain, reply-count admission and graph-local version lineage are
implemented. Complete native budgets, persistent cross-run authority, general
generated async launches, active-work drain qualification and measured
compute/copy overlap remain open. Existing pure-guard proofs do not prove the
whole executable reference executor.

R66 implementation follow-up: the
[native compute/SDMA coexistence contract](runtime-compute-sdma-coexistence-v1.md)
describes OVL-1/2's implemented checker and runtime
integration with bounded-scan proofs and CPU tests. Their live native admission
and hardware acceptance remain open. No other ticket is completed by that work.

## Ownership

| Lane | Assigned agent | Implementation backlog |
| --- | --- | --- |
| Native execution and qualification | `r66_native_coexistence` | OVL-QUAL-1/2, SCALE-1/2/3 and SCALE-CAP: qualify disjoint compute/SDMA custody, admitted mixed-duration profiles, native capacity and matched performance |
| Resources and versions | `r66_coexistence_model` | MEM-1 through MEM-5, VER-1/2: complete resource accounting, pools/residency, persistent mutation journal and cross-run leases |
| Admission and drain | `r66_runtime_coexistence` | GEN-1/2, DRN-1/2/3: owned generated launches, authenticated async admission, host-only capture and active/failure drain qualification |
| Integration and proof composition | Primary | PRF-1/2: shared Context hooks, executable refinement, authenticated proof roster, cross-review, hardware scheduling and publication |

These are three concurrent worker slots, not one simultaneous worker per ticket.
Each lane takes its tickets sequentially. Other lanes review changes before
integration. Start with isolated modules; the primary owns edits to shared
`context.rs`, Context reservation/completion hooks, model exports, proof rosters
and thin integration points in `kfd_backend.rs`/`queue_live.rs`. Coordinate
generated macro changes with the generated-host owner. Only the primary schedules
MI300X jobs or pushes integrated changes to both remotes.

## Next Assignments

| Lane | First bounded ticket | Deliverable | Then |
| --- | --- | --- | --- |
| Native | OVL-QUAL-1 | New R66 example, signed runner, independent checker and negative runner tests; unchanged admitted compute artifact and disjoint directional copies | OVL-QUAL-2 when hardware is reachable; SCALE-1 and SCALE-3 protocol work can proceed locally |
| Resources | MEM-1 | Complete resource-vector contract and move-only transactional credits, tests, production-used model transitions and named proof mutations | MEM-2/3/4 integration; MEM-5 closure; VER-1/2 when a slot is free |
| Admission | GEN-1 | Owned typed argument/result packing boundary with compile-fail borrowed-escape tests; no new launch authority | DRN-3 delta tests, DRN-1 capture, then GEN-2 adapter and DRN-2 qualification |
| Primary | PRF-1/2 | Freeze shared hooks and MEM-5 inventory, review integration, authenticate proof roster, maintain evidence matrix | Serialize hardware campaigns and publish reviewed commits |

These assignments name the next implementation work, not background jobs left
running by the planning audit. Each starts in its isolated module. Shared-file
edits require an explicit handoff to the primary; no lane independently raises
capacity, changes admission authority or schedules hardware.

## Acceptance Matrix

`Partial` means only the named restricted slice exists. `Open` means this
ticket's acceptance is not established, even where earlier reusable primitives
or fixture results exist. None of the rows establishes runtime-wide parity.

| Work | Implementation | Authenticated proof | CPU integration | Fixture hardware | Production-admitted hardware | Performance |
| --- | --- | --- | --- | --- | --- | --- |
| OVL-1/2 | R66 restricted profile | Bounded scan only; native extraction/composition open | Both orders, H2D/D2H, one/three bindings | Open: OVL-QUAL-1/2 | Open: GEN-2 also required | Open |
| MEM-1..5 | Open; existing counts/pools are partial inputs | End-to-end accounting open | Complete inventory/failure matrix open | Saturation/reuse open | Open | Unmeasured |
| VER-1/2 | Open; R65 lineage is graph-local | Persistent authority open | Cross-run mutation/lease matrix open | Open | Kernel extension also needs GEN-2 | Unmeasured |
| GEN-1/2 | Open; borrowed blocking bridge exists | Owned async composition and compiler evidence open | Positive owned path open | Fixtures cannot fill production cells | Open | Unmeasured |
| DRN-1/2/3 | R65 drain exists; capture and delta cases open | Whole drain/executor refinement open | Substantial R65 coverage; delta matrix open | R65 idle only; outstanding-work open | Open | Unmeasured |
| SCALE-1/CAP/2 | Open; default remains 64 epochs per compute lane | Capacity/acquisition composition open | Larger native-capacity profile open | Short/long and native-depth open | Open | Unmeasured |
| SCALE-3 | Protocol and timestamp producer work open | Optimization/timeline boundary open | Runner/checker open | Measurements open | Measurements open | No R66 result |
| PRF-1/2 | Whole executor composition open | Isolated guards are not whole-state refinement | Final unrestricted gate open | Per-profile qualification open | Open | Per-workload only |

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

**First ready native ticket; depends on final R66 integration.** Add a new
runtime example plus runner/checker and negative tests under
`benchmarks/runtime_gfx942`. Reuse the unchanged admitted R26 in-place compute
artifact and separate directional H2D/D2H storage. Cover both publication
orders, exact retained native identities, copy retirement while compute remains
retained, independent full outputs/padding, retirement and explicit cleanup.

Reuse the R61/R65 source, topology, census and process guards. Unlike the
copy-only runner, authenticate `hardware-qualification` in both the build and
Cargo-metadata commands; changing only the example is insufficient. This ticket
delivers a locally tested harness, not successful native admission or overlap.

### OVL-QUAL-2: Live Coexistence Acceptance

**Depends on OVL-QUAL-1 and reachable idle hardware.** Primary runs the exact
signed qualifier on one admitted MI300X with private staging and bounded
processes. Independently audit output, native identities, topology/census and
owned-process/stage cleanup. Prior R26/R65 captures do not qualify R66. Pending
host intervals do not establish physical overlap; that remains SCALE-3.

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
open until the claimed occupancy is actually measured. Active-drain content checks need
DRN-1; successful-completion and custody-only drain runs must remain distinct
until then.

### SCALE-3: Measured Overlap And Matched HIP/HSA Results

**Protocol/checker work can start now; measurements depend on OVL-2 and qualified
workloads from SCALE-1/2.** Match exact artifacts, geometry, bytes, selected GPU,
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

**First ready resource ticket.** Add proposed
`crates/fe2o3-runtime/src/resource_credits.rs`: typed per-device credit vectors,
move-only reservations and failure-atomic multi-resource admission. Distinguish
requested logical bytes, resident physical bytes and occupied native slots.
Reuse `async_engine/snapshot.rs` byte permits and `reply_budget.rs` lifetime
patterns where appropriate, but do not mistake their individual bounds or
Context handle-count limits for this complete vector. Freeze MEM-5's inventory
categories before choosing the vector fields.

Acceptance: checked arithmetic, complete-vector rejection, conservation, exact
ownership transfer and exactly-once return. Mutations must detect overflow,
partial debit, duplicate refund and refund before successful disposal. A small
logical counter is not a native memory budget.

### MEM-2: Native Allocation And Pool Residency

**Depends on MEM-1.** Add a pool-budget module beside existing KFD SDMA pools;
integrate existing allocate/recycle/trim paths rather than a second allocator.
Charge actual backing including padding. Checked-out and cached-free buffers
both consume residency; recycling does not refund resident bytes.

Acceptance: fail before over-budget allocation, bound cached capacity, transfer
checkout custody without double charging, and return credit only for successfully
released backing. Test alignment, oversized extents, generation exhaustion,
failed trim and terminal retention.

### MEM-3: Native Submission-Control Resources

**Depends on MEM-1.** Budget completion signals, aligned kernargs/control storage,
queue/ring residency and per-operation slots. Charge preallocated arenas once
and their occupied slots separately. Integrate preparation, publication and
recycling across admitted compute/SDMA lanes; do not duplicate low-level queues.

Acceptance: no publication without every required reservation, exact generation
on reuse, and no timeout/cancellation refund of possibly referenced storage.
Delayed completion, saturation and ambiguous publication remain bounded and
retain their charges. Prove a consistent progress-resource acquisition order.

### MEM-4: Executable Residency

**Depends on MEM-1.** Add proposed runtime `kfd_backend/residency.rs`, reusing
existing module records, compute retain counts and recycled-dispatch release.
Separate retained host images from materialized executable/control bytes. Bind
cache entries to exact device/Context, image and materialization identity.

Acceptance: repeated loads/dispatches remain bounded; leased executables cannot
be evicted; substitution rejects; failed unload retains its residency charge.

### MEM-5: Close The Bounded Production Payload Profile

**Design now; integrates MEM-1 through MEM-4.** Define a bounded production
command/result surface whose owned bytes can actually be measured. Account for
captures, terminal records and retained results, including DRN-1 capture bytes.
Unrestricted generic closures cannot be made byte-bounded by trusting a caller's
claimed size; keep them outside the qualified bounded profile or replace them
there with closed, accounted commands.

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

**Ready independently.** Add proposed runtime `context/versions.rs`. Begin with
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

**First ready admission ticket.** Add proposed host
`generated_runtime_arguments.rs` and narrowly scoped generated wrappers. Reuse
authenticated packing plans and ABI identities. The current
`GeneratedWorkerV3KfdInvocation<'allocation, K>` retains output borrows: it cannot
be sent to the owned engine by widening lifetimes. Provide runtime-owned storage
and explicit custody; inert argument snapshots grant no execution authority.

Acceptance: ABI/type/binding mismatch, stale allocation, aliasing, payload bounds,
dropped observers and compile-fail borrowed-storage escape. Main owns Context
reservation integration. Positive production execution awaits GEN-2.

### GEN-2: Authenticated Typed Async Admission

**Runtime contract work can proceed; production acceptance is dependency-gated.**
Integrate host `generated_kfd_invocation.rs`, a private async-admission adapter and
runtime `authorized_execution.rs`. Consume a non-forgeable permit binding exact
artifact, ABI/effects, arguments, geometry, required initialized regions, device
and resource generations. Retain compiler-publication/proof custody through
native quiescence and revalidate at the publication boundary. The blocking entry
must join this same async path, not retain a second execution implementation.

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

**Contract work can start independently; budget integration needs MEM-1/MEM-5.**
Add proposed runtime `async_engine/drain_capture.rs` and an audited lower-KFD
coherent-storage read operation. Pre-admit GPU downloads/canary copies before
the drain admission cutoff. Capture already-coherent host bytes only after
conclusive quiescence and before cleanup, with exact retained buffer generation
and bounded result storage.

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

**Depends on DRN-1.** Extend the signed R65 runner pattern with a distinct
qualifier. Start with copy work and separately named compute fixtures; repeat
the production cell when GEN-2 has exact evidence. Cover queued and already
published unresolved work, multiple streams and dropped observers.

Acceptance: recorded admission/publication/observation order, independent full
output/padding comparison, no duplicate issue, quiescent drain and explicit
cleanup. Unresolved at cutoff does not establish physical GPU activity at that
instant. An idle drain or small fixture run cannot close active native-depth,
mixed-duration or general-production cells.

### DRN-3: Failure And Retention Matrix

**CPU work ready now.** Extend `async_engine/tests/owned_tests/drain_tests.rs`
and scripted adapters for submit rejection, repeated observation rejection,
quiescent failure, pre-issue cancellation, terminal ambiguity, exhausted budget,
interruption and capture failure. Check exact reply delivery, identity, retained
resources and no retry/duplicate publication.
R65 already covers quiescent failure, budget exhaustion, abandoned observers,
interruption and panic. Preserve that coverage and add the missing combinations,
particularly accepted-submit rejection, repeated observation rejection,
pre-issue cancellation and capture failure after DRN-1, instead of rebuilding
the drain lifecycle.

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
| 1 | OVL-QUAL-1; design SCALE-1/3 | MEM-1; define MEM-5 inventory | GEN-1; design DRN-1; DRN-3 delta | Freeze shared contracts and start PRF-1 |
| 2 | OVL-QUAL-2; qualify SCALE-1; SCALE-CAP after MEM prerequisites | MEM-2/3/4, then MEM-5 | GEN-2 adapter; implement DRN-1 | Integrate shared hooks; arrange compiler handoff |
| 3 | SCALE-2, including SCALE-CAP's measured native-depth gate | VER-1 then VER-2 | DRN-2; production GEN-2 only when evidence exists | Compose proofs and repeated mixed-graph acceptance |
| 4 | SCALE-3 measurements | Budget/version stress and cross-review | Active/failure drain and cross-review | PRF-2 evidence, A1/A2 exit audit, dual-remote publication |

VER-1 and DRN-3 are independently ready and can move earlier when a slot is free;
the wave table is an execution order, not an artificial technical dependency.
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
