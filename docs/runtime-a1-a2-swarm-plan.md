# Remaining A1/A2 Work: Swarm Plan

Baseline: `4b897b3a` (R66 implementation, unrestricted evidence and initial
swarm assignments), reviewed 2026-09-10. This decomposes the remaining local A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), not the later multi-GPU
and distributed milestones. The issue's A1/A2 exit criteria were rechecked on
2026-09-10. Three agents independently reconciled their lanes against the current
code after the first implementation wave. The source checkpoint below is
separate from the committed baseline; queued tickets are not completed work or
unattended background jobs.

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

The next swarm packet implements OVL-DIAG-1 and MEM-BASE. Native and runtime
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
`fe2o3-resource-accounting`; the runtime keeps its device-branded wrapper. Only
this extraction is approved/implemented. KFD physical costs, parent budgets,
batch/split transactions and aggregate quarantine are not implied. The core's
moved tests are included in CPU and release-test rosters, and dependency policy
prevents an upward runtime dependency.

| Slice | Implemented locally | Boundary still open |
| --- | --- | --- |
| OVL-QUAL-1 | Eight-cell R26 coexistence example, immutable native-custody observations, independent checker, signed-source runner and negative tests | Signed live capture, native extraction refinement, physical overlap and production generated-kernel authority |
| MEM-1 | Nineteen-dimensional credit primitive and opt-in Context requested-byte/allocation-record admission; retain uncertain charges and reject false cleanup completion | Actual native residency/slot cost extraction, global budget closure and whole-account/executor refinement |
| GEN-1 | Owned typed arguments/results, complete-invocation preflight, authenticated packing-plan reuse and compile-fail ownership checks | Invocation-bound async authority, Context freshness, global credit lifetime and compiler machine evidence |
| DRN-3A | Three composed accepted-submit rejection, repeated observation rejection and pre-issue cancellation regressions | Host capture failures, active-work hardware drain and whole-executor refinement |
| SCALE-3-PROTO | Bounded matched-plan/checker protocol; fourteen tests and independent cross-review pass | Signed producers, genuine correctness/timing captures, performance and physical overlap |
| MEM-BASE / MEM-5 inventory | Shared credit-engine extraction and device-branded wrapper; concrete allocation-site inventory | Native adapter approval, implemented physical admission and complete aggregate byte-budget closure |

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
| Native | OVL-DIAG-1, then SCALE-1 | Explain the rejected actual native roster without weakening checks; retain typed failure stage and actual binary; design bounded short/long profiles | New OVL-QUAL-2 campaign; SCALE-CAP after physical budgets; SCALE-3 producers/measurement |
| Resources | MEM-BASE integration, then MEM-2A contract | Validate the extracted lower-level ledger; agree exact native domain/cost/disposal ownership before individual backing admission | MEM-2B, MEM-3A/B, MEM-4A/B, MEM-5 closure; VER-1/2 can move earlier |
| Admission | DRN-1 capture and GEN-2 interface | Bounded host-only capture and exact private typed-async authority contracts | DRN-3B capture-failure cases and DRN-2; production GEN-2 after accounting/compiler evidence |
| Primary | PRF-1/2 and native accounting interface | Finish current-wave verification; approve cross-crate accounting ownership; compose shared hooks and proof roster | Signed OVL-QUAL-2 hardware campaign, retained evidence and dual-remote publication |

The three agents completed SCALE-3-PROTO, the MEM-5 inventory proposal and DRN-3A,
then cross-reviewed those packets. These rows name their next implementation
queue, not background jobs left running by the audit. Each starts in its isolated module. Shared-file
edits require an explicit handoff to the primary; no lane independently raises
capacity, changes admission authority or schedules hardware.

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
depend upward on the runtime crate and does not yet consume the shared engine.
Resource and native owners must agree which layer measures actual layout, holds
each charge through cached residency and recognizes physical disposal. Existing
requested-allocation credits remain a distinct quantity, and public counter
mechanics are not native-disposal evidence.

### Next Packet Contracts

The second scoping review fixes the following small implementation boundaries;
these are queued work, not implemented or hardware-accepted features.

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

## Acceptance Matrix

`Partial` means only the named restricted slice exists. `Open` means this
ticket's acceptance is not established, even where earlier reusable primitives
or fixture results exist. None of the rows establishes runtime-wide parity.

| Work | Implementation | Authenticated proof | CPU integration | Fixture hardware | Production-admitted hardware | Performance |
| --- | --- | --- | --- | --- | --- | --- |
| OVL-1/2 | R66 restricted profile | Bounded scan only; native extraction/composition open | Both orders, H2D/D2H, one/three bindings | Open: OVL-QUAL-1/2 | Open: GEN-2 also required | Open |
| OVL-QUAL-1/2 | Qualifier and typed diagnostics locally validated; live gate open | Descriptive observation, not authority | Eight runtime observation tests, six native diagnostic tests and runner/checker negatives | Open: new signed capture required | Open: GEN-2 also required | Physical overlap unmeasured |
| MEM-1 | Local transactional credits and Context requested-allocation profile | R67: 14 vector/record obligations and eight mutations passed full authenticated gate; adapter composition open | Ten Context, nine shared-engine and three device-wrapper tests; two core ownership compile-fail examples | New qualifier includes requested-credit saturation/retention/disposal; not yet accepted | Open | Unmeasured |
| MEM-2..5 | MEM-BASE extraction and site inventory available; physical integration and global closure open | End-to-end accounting open | Measured costs and complete failure matrix open | Physical saturation/reuse open | Open | Unmeasured |
| VER-1/2 | Open; R65 lineage is graph-local | Persistent authority open | Cross-run mutation/lease matrix open | Open | Kernel extension also needs GEN-2 | Unmeasured |
| GEN-1 | Owned data boundary implemented locally | No execution authority or whole-async proof | Eleven focused host tests and generated fixtures pass | Data-only boundary | Not an execution cell | Unmeasured |
| GEN-2 | Exact typed async authority and integration open | Compiler evidence and async composition open | Permit/currentness/native integration matrix open | Fixtures cannot fill production cells | Open | Unmeasured |
| DRN-1/2/3 | R65 drain and DRN-3A exist; capture/failure integration open | Whole drain/executor refinement open | Three new composed failure/cancellation cases pass; capture failures open | R65 idle only; outstanding-work open | Open | Unmeasured |
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

**Implemented locally; signed source freeze and live acceptance remain.** The new
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
commands. Local example compilation, four observation tests and runner mutation
tests passed. This delivers a locally tested harness, not successful native
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
alone do not establish native acceptance.

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
proposes one extracted lower-level ledger, exact root/device/Context/session
domains and explicit bootstrap costs. Primary approval precedes integration;
the proposal itself implements no physical admission.

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
Compound creation requires atomic fixed-roster reservation. Separately disposed
members also need proved charge splitting; R67's current primitive supplies
neither parent-account transactions nor divisible native bundles.

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

**Contract work can start independently; bounded capture needs the approved
accounting contract and capture reservation, not already-complete MEM-5.**
Add proposed runtime `async_engine/drain_capture.rs` and an audited lower-KFD
coherent-storage read operation. Pre-admit GPU downloads/canary copies before
the drain admission cutoff. Capture already-coherent host bytes only after
conclusive quiescence and before cleanup, with exact retained buffer generation
and bounded result storage.
Final MEM-5 closure incorporates the implemented capture and version-journal
storage, avoiding a circular prerequisite.

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

**DRN-3A implemented; capture-failure work awaits DRN-1.** Three new composed
regressions in `async_engine/tests/owned_tests/drain_tests.rs` check exact
accepted-submit rejection, repeated observation rejection without reissue and
pre-issue cancellation preserving a successfully submitted sibling. They check
reply/snapshot lifetimes, credit usage and retained allocation/module/stream/
submission identities. No production transition changed for these tests.
Extend the scripted adapters for capture failure after DRN-1. Check exact reply
delivery, identity, retained resources and no retry/duplicate publication.
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
| Current checkpoint | OVL-QUAL-1 and SCALE-3-PROTO locally validated | MEM-1 requested-allocation slice and MEM-5 inventory proposal | GEN-1 owned data boundary and DRN-3A regressions | Current-source gate and signed freeze; hardware separate |
| 1 | Prepare SCALE-1; review OVL-QUAL-2 | Approved shared ledger, then MEM-2A/B | Design DRN-1 and GEN-2 authority interface | Approve native accounting boundary; execute OVL-QUAL-2 after freeze |
| 2 | Qualify SCALE-1; SCALE-CAP after MEM prerequisites | MEM-3A/B, MEM-4A/B, then provisional MEM-5 integration | GEN-2 adapter; implement DRN-1; DRN-3B | Integrate shared hooks; arrange compiler handoff |
| 3 | SCALE-2, including SCALE-CAP's measured native-depth gate | VER-1 then VER-2; final MEM-5 closure includes journal/lease and capture storage | DRN-2; production GEN-2 only when evidence exists | Compose proofs and repeated mixed-graph acceptance |
| 4 | SCALE-3 measurements | Budget/version stress and cross-review | Active/failure drain and cross-review | PRF-2 evidence, A1/A2 exit audit, dual-remote publication |

VER-1 and the DRN-1 contract can move earlier when a slot is free;
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
