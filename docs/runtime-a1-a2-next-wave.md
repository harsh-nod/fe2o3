# A1/A2 Next-Wave Dispatch

Updated 2026-09-10 against signed R78
`042dcec1bc67f919e02ce4908a62ecc8d1e8b113`, pushed to both remotes.
R79 B3-OWNER is implemented in the local working tree, but is not committed or
release-qualified; the distinction is recorded below.
Three read-only agents reconciled the remaining work with source and the current
open [#182](https://github.com/harsh-nod/fe2o3/issues/182). This document assigns
bounded analysis/review queues, not unattended implementation jobs. Primary
owns edits, integration, tests, hardware scheduling and publication. A queued
ticket is not new implementation or an accepted proof/hardware result.

The user-requested swarm refresh completed on 2026-09-10. All three named
workers returned source-grounded assignments; their review turns are complete,
not continuing implementation jobs. The issue remains open (last issue update
observed: `2026-09-10T10:50:51Z`). The next independent packets are R79 validation
with Admission, Resources' MEM-2B-HOST and Native's SCALE-1A-FIXTURE. Admission
and Resources also freeze B4-RESERVE before native execution integration. The
[integration queue](#generated-integration-queue) below names the individual
handoffs; the [remaining tickets](#remaining-tickets) include qualification-only
work and the later A3-A7 milestones.

The [full roadmap](runtime-a1-a2-swarm-plan.md) retains historical packet scope
and the property-level acceptance matrix. The [R71 evidence](evidence/local-r71-device-pool-drain-2026-09-10/README.md)
covers the implemented device-cache limits and copy-first drain qualifier.
Those features need hardware acceptance, not duplicate implementation.

## R72 Follow-On

MEM-N1A-COST/NATIVE/FWD are now implemented and pass the
[R72 local/proof gates](evidence/local-r72-host-backing-2026-09-10/README.md):
2,219 runtime tests per GNU/musl target, 39 added tests, and an authenticated
61-source/1,367-obligation/678-negative proof run. The new proofs cover five
cost-projection properties, not native disposal or whole-executor refinement.
Hardware acceptance remains open; the selected MI300X was busy at the read-only
check, so no stage or workload was started.

## R73 Follow-On

GEN-2R charged storage and the generated-argument packing route are implemented
and pass the [R73 local gates](evidence/local-r73-charged-results-2026-09-10/README.md):
2,222 runtime tests per GNU/musl target, 227 GNU and 110 musl host tests, generated
fixtures and all seventeen source gates. The focused host suite has 31 tests:
20 charged-path and 11 legacy. Authenticated Verus passes 62 positive sources,
1,374 obligations and 686 rejected mutations, including R73's seven obligations
and eight mutations. These local results do not complete GEN-2 execution.

The implementation uses a separate charged result state, reuses the caller's typed seed,
reserves the full roster before encoding, and publishes outputs together only
after complete validation/decoding and returned-buffer disposal. Each output
retains its full conservative encoded-plus-typed debit through disposal;
read-only returned storage has its own debit. Its cost/shape proofs do not prove
the mutex/Box adapters, native completion or whole-executor refinement.
Polling no longer waits for the seed-slot mutex. A bounded regression failed
against the old blocking candidate and passes with `try_lock`; a future adapter
still needs an explicit wake/retry policy after contention. No SSH or hardware
work was started for R73.

GEN-2A must retain the private decoder with invocation custody. In particular,
read-result credits cannot be detached from possibly live returned storage.
The public charged arguments are data only: native publication, Context
generations, runtime-owner shutdown and authenticated compiler execution remain
GEN-2B/integration acceptance, not consequences of the storage tests.

### R74 Follow-On

GEN-2A nonexecuting invocation custody is now implemented in the
[owned invocation contract](runtime-owned-generated-invocation-v1.md), with
[local evidence](evidence/local-r74-owned-invocation-2026-09-10/README.md).
It consumes raw generated arguments and their authenticated executable, retains
the private production-only authority and uses a storage-before-decoder owner
through preparation failures and unwind. Explicit `!Send`/`!Sync` checks do not
rely on the checked-device documentation. The borrowed qualification path is
unchanged. Positive production construction, native execution, GEN-2B and
whole-executor refinement remain open.

R74's seventeen source gates pass 2,222 runtime tests per GNU/musl target,
233 GNU and 116 musl host tests, all generated fixtures and lint/policy gates.
Model/proof sources are unchanged; the 686-negative inventory was rechecked,
not the Verus solver. These results do not prove the new ownership adapter.

Resources takes ordinary host-cache limits without reimplementing GEN-2R.
Native first supplies its reusable retained-device preparation scope.
Native's bounded fixture work is independent of both. Primary owns all edits,
cross-lane integration, gates and releases.

### R75 Follow-On

GEN-2B-1 now implements the [owner-local operation boundary](runtime-owner-local-operations-v1.md).
Ordinary operations use Send factories materialized after capacity admission;
installed drivers need not be Send. The owned engine retains unresolved drivers
through cleanup/native shutdown and quarantines them with Context on failure.
Reply disposal is independent of retained driver custody, while still-unissued
ordinary host callbacks are discarded on stop. Context-returning APIs remain
Send-compatible and reject owner-local factories before materialization.

Sixteen new CPU tests and the 186-test focused async suite pass. The
[R75 local record](evidence/local-r75-owner-local-operations-2026-09-10/README.md)
separates final gates from the two intermediate payload/drop-order regressions.
No new proof, native generated authority, R73 byte-owner integration or GPU
acceptance follows from these driver tests. At the R75 checkpoint, Admission's
next packet was GEN-2B-2's Context/host adapter. R76 implements that local boundary;
the current queue advances to GEN-2B-3's exact persistent projection and publication.

### R76 Follow-On

GEN-2B-2's [retained-device and Context preparation](runtime-context-generated-preparation-v1.md)
is implemented locally. Immutable scopes bracket preparation with full currentness;
the Context-bound host owner retains exact identity and charged storage without a
second device admission or persistent Context borrow. Queue/session guards check
the actual model admission, VM and selected queue phase. Initial VM/queue absence
is not frozen. No native publication or result completion is added.

The [R76 local record](evidence/local-r76-context-preparation-2026-09-10/README.md)
retains fourteen new CPU tests, type-boundary checks and exact-source gates.
Genuine Linux scope success before/after lazy bootstrap and positive production
construction remain unqualified. GEN-2B-3 now takes exact persistent projection,
one-shot adoption and publication-time authority; -4 through -6 remain open.

### R77 Follow-On

GEN-2B-3's consuming [persistent projection](runtime-persistent-generated-projection-v1.md)
is implemented locally and wired into Context preparation. It retains the complete
original storage/digest/timeout, checks canonical image and hidden bytes, and
reuses actual host-layout and fixed-dispatch policy. The
[local record](evidence/local-r77-persistent-projection-2026-09-10/README.md)
separates tests from native/publication/proof acceptance. B3 adoption/publication
remains open, as do fixed-path empty/scalar, larger-roster, overlapping-alias and
runtime-service hidden-field support. No deadline behavior or native authority
follows from this projection.

## Current Swarm Dispatch

R78 adds [borrowed coherent initialization](runtime-borrowed-coherent-initialization-v1.md)
and removes the extra encoded host copy from both ordinary materializers. Its
[local record](evidence/local-r78-borrowed-coherent-2026-09-10/README.md) does not
complete generated adoption. R79's local working tree adds a finite preparation
future, parked owner-local custody, an opaque non-Clone ticket and explicit
owner-thread discard. Capacity precedes preparation; parking precedes the ready
reply. Parked entries consume operation capacity but do not flush, poll as active
work or block drain/graph admission. They remain retained through owned shutdown.
The real protected host constructor is wired through the immutable R76 scope.

The focused local preparation run passes 22 runtime and 10 host tests, including
16 new runtime cases and one host source-wiring case. This is not R79's full
release record: broader async regression, doctest, GNU/musl, lint/policy and
exact-source evidence gates remain pending. No positive compiler-backed
construction, native generated execution, new adapter proof or performance
acceptance follows from those tests. The source is intentionally not included
in this documentation-only dispatch commit.

Ticket readiness does not expose the projection or authorize native adoption.
Native must first design the closed complete-roster representation, then change
the same rooted owner to a non-discardable adopting state before native effects.
That state must participate in lane exclusion, drain, graph and cleanup tracking.
Neither today's parked state nor `MaterializedPrepared` is that state; the latter
can publish through deferred flush. Fixtures and host-cache policy remain
independent work; R78's borrowed hooks must not be reimplemented.

These are three read-only worker slots plus the primary. A lane supplies a
source-grounded design, test/proof obligations and an independent review for
one bounded packet at a time. Primary implements the agreed packet. Follow-ons
are sequential queues, not additional concurrent workers or delegated edits.

| Lead | Next bounded assignment | Module scope and primary handoff |
| --- | --- | --- |
| Native: `r66_native_coexistence` | SCALE-1A-FIXTURE now; B3-DATA-REP then B3-DATA-ADOPT | Freeze bounded fixture policy and complete-output oracle. Adoption follows accepted OWNER, implemented B4-RESERVE, the closed representation, retained native prefix and lane/drain contract. R76 still needs live bootstrap qualification. |
| Resources: `r66_coexistence_model` | MEM-2B-HOST and shared B4-RESERVE contract | Own isolated `sdma/host_pool_policy.rs` policy/model design. Preserve R72 debit through reuse; zero disables caching and defaults stay unchanged. B4 reserves a new completion reply and full readback storage in the original result account. |
| Admission: `r66_runtime_coexistence` | R79 review/validation handoff, then B4-RESERVE and B3-ISSUE | Finish finite preparation acceptance without claiming a launch API. Define complete readback/reply ownership with Resources, then a private submission-bound permit surviving actual flush/retry publication. |
| Primary | R79 release gates; reviewed implementation, proof composition and qualification | Own every edit, integration/build/proof run, evidence record and signed dual push. Serialize shared native hooks. Schedule existing OVL-QUAL-2 and DRN-2A against frozen signed source; native-budget pressure first needs MEM-QUAL-HARNESS. |

Admission and Resources agree the permit/decoder/result boundary before code
integration. Native's fixture work does not mint production generated-launch
authority. Primary serializes shared-file edits; each worker reviews another
lane's contract before the packet's local gates.

Native takes fixtures now, followed by the sequential qualifier and
MEM-QUAL-HARNESS. Full-roster adoption resumes when Admission's explicitly
nonpublishing owner is ready; ordinary host-cache and broader budget hooks remain
coordinated follow-ons.
Resources takes MEM-N1B, MEM-DOM-1, MEM-3/4, VER-1/2 and MEM-5. Admission follows
GEN-2B with generated graph/drain qualification. Waiting for hardware must not
block CPU harness work. Version-journal mechanics or host-image accounting may
move earlier when the Resources slot is free.

### Immediate Assignment Contracts

The refreshed three-agent review splits the next work into the following
bounded handoffs. Workers own analysis and independent review; Primary owns
implementation and shared-file changes. These assignments do not themselves
claim new implementation, proof or hardware acceptance.

| Worker | First deliverable | Source boundary | Dependency and exit gate |
| --- | --- | --- | --- |
| Native | SCALE-1A-FIXTURE; B3-DATA-REP then ADOPT | Isolated short/long artifact policy and output oracle; later proposed `kfd_backend/generated_materialization.rs` with R78's implemented borrowed hooks | Fixtures are independent. Representation must retain original vectors and all ordinals without the ordinary snapshot path. Adoption requires implemented B4-RESERVE, changes custody before effects, retains partial native prefixes and cannot publish. |
| Admission | Validate local R79 owner/driver, then reserve/issue contract | Working-tree `async_engine/generated_operation.rs`; narrow Context and host invocation adapters | Finish full release gates after focused CPU success. Test combined capacity, no-flush parking, cancellation, ticket retry, observer loss, graph coexistence, Stop races and destructor panic. Native adoption/publication remains separate. |
| Resources | MEM-2B-HOST ordinary coherent host-cache limits | Proposed `sdma/host_pool_policy.rs` and model checker; Native reviews exact records; Primary integrates shared-memory, queue and runtime budget hooks | Independent of generated execution and broader N1B. Preserve the same R72 debit; validate padded bytes, complete mixed rosters and exact identities. Zero disables caching, pressure disposes, defaults remain unchanged. |
| Primary | Freeze interfaces, integrate and qualify | Shared exports, Context/native hooks, proof rosters, tests and evidence | R78 is signed and published. Finish R79 validation/release; integrate adoption in bounded commits, run authenticated proofs for changed model properties and publish to both remotes. Serialize shared-machine campaigns. |

R77 reconciles a concrete format difference: the prepared request
contains initialized COV6 hidden arguments, whereas the fixed-dispatch packet
expects a zero hidden suffix and derives those bytes inside native custody.
It independently derives and compares canonical initialized bytes before creating
the equivalent zero template. The inert projection retains the timeout; operation
adoption must carry it into deferred/active custody and define its execution
deadline separately from an observer's wait deadline.
Use the existing persistent `Materialized` path, not specialized R26 admission
for arbitrary generated kernels. Reconcile the original authenticated HSACO and
selected kernel with the complete prepared image/descriptor/resources. Preserve
buffer order, aliases, interior offsets and unused storage; reject unsupported
representations before native effects rather than silently cropping them.

Before B3 issues work, Admission and Resources freeze B4's complete readback,
decoder and reply reservation contract. This permits separate implementation
packets without publishing work whose bounded completion path is unspecified.
Keep generated submission private until publication and completion compose.

The acceptance-only queue is separate: genuine R76 retained-device scopes before
and after bootstrap; existing OVL-QUAL-2 and DRN-2A signed campaigns; and N1/N2/pool
hardware checks after MEM-QUAL-HARNESS exists. Protected production construction
still requires the compiler owner's exact artifact/refinement handoff. None of
these dependencies blocks host-cache policy, fixture or version-journal work.

### Generated Integration Queue

These smaller tickets refine GEN-2B-3 through -6 without changing their acceptance
boundary. Worker leads own the design and review handoff; Primary implements and
integrates each packet. No worker may turn an inert preparation state into a
publishing state to bypass a dependency.

| Ticket | Worker handoff and source scope | Prerequisite and exit gate |
| --- | --- | --- |
| B3-OWNER / R79 | Admission review; Primary validation/release of working-tree owner and host adapters | Implemented locally, not committed or release-qualified. Finite preparation, bounded parked custody, exact ticket and discard have focused CPU coverage. Complete broader gates and evidence; do not present this as a launch/completion API. |
| B4-RESERVE | Admission with Resources: private generated readback owner and reply budgets | Contract work is independent of OWNER release. Reserve a new completion reply and every readback destination, including unused/read-only buffers, before native execution integration. Reuse encoded storage through a closed transition or precharge full overlap in the original account. Test late-member exhaustion, rollback and disposal-before-refund. |
| B3-DATA-REP | Native with Admission: proposed `kfd_backend/generated_materialization.rs` plus narrow host/Context views | Design now; integration requires accepted OWNER. Add a closed descriptive view and complete-roster backing representation, retaining original vectors, decoder/account, artifact, authority, timeout and ordinal identity. No generic payload extraction or ordinary allocate/write/snapshot substitution. |
| B3-DATA-ADOPT | Native with Admission/Resources: retained-prefix owner and logical/native registration | Requires REP, implemented B4-RESERVE and agreed lane contract; reuse R78 slices. Change custody to non-discardable Adopting before effects, preallocate prefix slots and register every ordinal. Test bootstrap states, exact generations, partial allocation/copy/map/retake failures and panic retention. Include lane/drain/graph/cleanup ownership; still no publication. |
| B3-ISSUE | Admission with Native: `compute_state.rs`, `compute_dispatch.rs`, Context registration | Requires DATA-ADOPT and implemented RESERVE. Retain a private linear permit bound to the exact submission and resource incarnations through actual deferred flush/retry. Test substitutions, replay, contention, ambiguous issue and timeout retention. |
| B4-COMPLETE | Admission with Resources/Native: driver, private decoder/storage and readback | Requires ISSUE. Validate the exact submission and complete returned roster before any output publication. Test malformed late output, read-only mutation, failed readback/currentness, decoder panic, one reply and retained results after shutdown. |
| B5-API | Admission API review; Primary host/async integration | Requires composed ISSUE and COMPLETE. Async and blocking entry points share the same engine. Test wakeup races, contention, reentrancy, equivalent outcomes and compile-fail ownership boundaries. |
| B6-GRAPH/DRAIN | Admission; Native/Resources review | Requires B5; cross-run input reuse also requires VER-1/2. Test repeated/concurrent graphs, dependencies, Stop/cutoff ordering, dropped observers, complete typed outputs and cleanup. Production acceptance needs the exact compiler handoff. |

Native's fixture and R78 rebound-harness work and Resources' host-cache policy
do not wait for this chain. Version-journal mechanics and host-image accounting
are additional independent follow-ons, not grounds to claim their full runtime
integration. The three worker slots rotate through the queues; the primary
serializes shared-file edits, proof execution and shared-machine campaigns.

### Reviewed B3 Handoff

The refreshed swarm reviewed these interfaces. R78 implements the borrowed
initializer subset; R79's local owner is awaiting release gates. Native
representation/adoption, publication and completion remain unimplemented:

- Keep the entire `RuntimeGfx942PreparedV1` carrier in the owner-local driver.
  A runtime-defined safe borrowed-view interface may expose descriptive projection,
  exact retained HSACO and the existing Worker authority. Runtime checks the
  coupling itself; no generic payload extraction or second unsafe authorizer.
- An inert Send factory transports unprepared inputs. Preparation occurs only
  after operation-capacity admission, against the Context's retained device.
  Prepared custody remains owner-local; Context handles and allocation credits
  still require normal registration and exact full-buffer index mapping.
- R78 supplies slice-based entry points sharing the existing synchronous coherent
  initializer's allocation/copy/map/currentness body. The next full-roster
  adapter consumes these hooks and keeps the invocation's original immutable
  vectors. Copying to charged native GTT is distinct from allocating another
  encoded host buffer. Preserve detached ordinals and native loan/retake custody
  through failure and panic; the local hook tests do not qualify Linux adoption.
- Ordinary `prepare_launch` snapshots bound windows and reconciles under the
  kernel signature. It cannot substitute for R77's complete roster and original
  dispatch identity. A private exact-projection branch must feed the existing
  materialized publication path.
- Deferred flush/retry may publish outside driver advancement. Retain exact
  authority/currentness there with a private linear submission-bound permit;
  adoption-only validation or an ephemeral borrowed authority is insufficient.
- Before publication, reserve the complete readback destinations and one bounded
  reply. Reuse R73 encoded buffers through closed ownership transitions or reserve
  additional overlap in that account before allocating it. Native N1/N2 credits
  do not cover this host-result overlap. Include unused and read-only storage.

The proposed composed acquisition order is retained operation slot, new
completion reply, same-account readback reservation, complete destination and
metadata allocation, native/control adoption, then issue. Preparation's already
completed reply cannot serve as the launch-completion reply. Fail fast rather
than waiting while holding a partial progress reservation. Before native effects,
destroy new destinations before cancelling their unissued credits and preserve
the original parked carrier/ticket for a retry. After native effects, retained
custody, not ticket disposal, governs cleanup. A whole-roster overlap debit avoids
assuming unsupported partial credit splitting; it remains until all additional
storage is destroyed. This is the next implementation contract, not an accepted
B4 adapter proof.

Resources independently reviewed the carrier/permit and complete-readback
contract with no blocking conflict. These decisions are design handoffs, not a
proof of their adapters. B3 implementation is followed by B4 complete results,
B5 the shared async/blocking API, then B6 generated graph/drain qualification.
The public entry point must wait for publication and completion to compose.

### GEN-2B Breakdown

The R74 owner is standalone: it consumes a checked device. A persistent Context
cannot repeat that ownership pattern for each launch. The
[device admission model](../crates/fe2o3-runtime-model/src/device_identity.rs)
rejects a second live admission of the same physical GPU. Keep the actual device
in the backend and prepare each invocation against that retained owner, without
creating a second queue or widening thread-safety/lifetimes.

R74's additional operation-lifetime prerequisites are implemented in R75: Send
factories now feed owner-local drivers, and the registry survives owned Context
cleanup. This was not an early-native-release defect in the existing handle-only
operations, whose native resources remain Context-owned. Native compute success
still only records dirty output extents; it does not itself provide generated
host readback or authorize the private decoder.

| Packet | Lead, source boundary and dependency | Required acceptance |
| --- | --- | --- |
| GEN-2B-1: owner-local drivers | Implemented in R75; Admission review, Primary integration. `async_engine/operation.rs`, `async_engine.rs`, `owned.rs`. | CPU tests cover Rc-holding drivers, queue/operation/reply exhaustion, cancellation, observer drop, factory/advance panic and Stop/cleanup failure; Send-Context APIs and reply lifetime are preserved. Exact generated/native custody and whole-executor refinement are not established by this packet. |
| GEN-2B-2: reusable preparation | Implemented in R76: immutable native scopes and runtime-defined Context/host wrapper. | CPU/type checks cover currentness envelope, owner/domain guards, exact Context/native identity, rejection, no mutable borrow escape and inert shutdown storage. Successful Linux scopes before/after lazy bootstrap and positive production construction remain separate acceptance cells. Native operation adoption follows in -3. |
| GEN-2B-3: persistent publication | R77 implements nonexecuting projection; Native/Admission still own consuming Context/native adoption and publication in `compute_dispatch.rs` and `compute_state.rs`. Requires -1 and -2. | Preserve artifact, ABI, complete buffers/fixups, hidden kernargs, geometry and timeout through deferred/active custody. Retain per-invocation authority before native effects and revalidate at actual publication. Reject substitutions/double consumption; pending polls and ambiguous publication never authorize replay. Readback reservation and unsupported fixed-profile expansion remain open. |
| GEN-2B-4: completion/results | Admission; Resources reviews private charged decoder/reply boundary. Completion tests can start early; integration requires -3. | Reserve readback resources before issue; bind exact invocation/submission/generations and validate every returned buffer, including read-only effects. Decode the complete roster, dispose encoded storage, commit all outputs, drop producer custody, then resolve one budgeted completion future. Test malformed late output, readback/currentness failure, decode panic, retained results after shutdown and credit conservation. |
| GEN-2B-5: public async/blocking API | Primary integration; Admission API review. Requires -1 through -4 composed. | Both entry points use the same nonblocking publication/retirement engine; no blocking one-shot executor inside a command callback. Test reentrancy, equivalent outcomes, completion-before/during-poll, waker replacement and no lost wakeups. Private decoder and owner-local authority remain inaccessible. |
| GEN-2B-6: generated graph/drain | Admission; Native/Resources review. Requires -5 and exact compiler evidence for production cells; cross-run input reuse also requires VER-1/2. | Repeated/concurrent launches and graphs, dependencies, active drain, dropped observers, complete typed outputs and owned cleanup. Keep fixture and production acceptance distinct and preserve existing graph/standalone exclusivity. |

GEN-2B-2 must allow the same retained device to move into a lazily created VM
and queue. Bind actual VM/lane/allocation incarnations at adoption/publication;
do not freeze their initial absence or accept replacement merely because a GPU
unique ID matches. The prepared-to-persistent projection is a checked contract,
not hash equality or a backend-wide `authorize = true` callback.

The completion future in -4 is per invocation. Keep R73 `try_take()` polling-only:
transient mutex contention cannot become `Pending` without a subsequent wake.
Failed, quiescent-without-result, cancelled and engine-stopped observations never
commit output. Unresolved native custody survives even when its reply is stopped.

### Independent Handoffs

Resources' MEM-2B-HOST needs only R72's ordinary coherent backing profile.
Use the exact native record's padded cost and the existing N1 debit; do not
charge a second account on cache insertion. With a 4 KiB page fixture, test
4097 requested bytes against 8192 padded bytes, zero/full byte or record ceilings, unchanged defaults,
foreign/stale/duplicate roster entries, charged reuse, immutable configuration,
partial trim and panic. Pressure disposes the incoming idle buffer; this packet
does not add resident eviction/retry. Confirmed disposal refunds once; later
queue-retake failure cannot resurrect the charge.

Native's SCALE-1A-FIXTURE defines bounded short/long work, frozen ABI/effects and
source/object/toolchain identity, mutation rejection and independent complete
output/padding oracles. Sequential signed correctness follows; labels alone
establish no duration. MEM-QUAL-HARNESS separately exercises actual optional
N1/N2/device-cache settings in both startup orders, including bootstrap use,
padded byte/record pressure, retained reuse and disposal. Existing R66 and drain
examples do not configure those native budgets. Host-cache cells follow its
implementation.

Primary freezes interfaces, implements reviewed packets and owns focused tests,
proof registration and shared-file integration. The dispatch itself changes no
runtime code or acceptance status; R75/R76's implemented boundaries are recorded
separately above. CPU work needs no hardware window; positive production
generated execution still needs the compiler handoff. Fixture correctness and
fixture drain do not require that production handoff.

## Execution Order

| Stage | Ready work | Gate before advancing |
| --- | --- | --- |
| 0: R73 locally complete | Charged result data and nonblocking extraction; reviewed next-packet contracts | Retained current-source gates and property proofs; native/whole-executor acceptance remains separate |
| 1: independent local packets | Host-cache policy/hooks, short/long fixtures and native-budget harness; GEN-2A custody is implemented | Freeze cross-lane interfaces; complete deterministic rejection, ownership, failure and oracle tests |
| 2: compose ownership | Broader native profiles, domains, control/code budgets, versions and GEN-2B | No duplicate backing debit, unbounded child accounts, stale input authority or detached decoder; preserve progress headroom |
| 3: qualify A1/A2 | Out-of-order native depth, repeated generated graphs, active drain, memory pressure and overlap | Admitted workloads, exact compiler evidence for production generated cells, signed full-output captures and cleanup; existing copy/overlap campaigns may run earlier |
| 4: measure and close | Matched KFD/HSA/HIP producers, executable proof composition and exit audit | Predeclared workloads/thresholds, resource/CPU/tail metrics, separate device-timeline evidence for physical overlap; every A1/A2 acceptance cell resolved |

These stages express prerequisites, not rigid barriers between independent
lanes. Missing compiler artifacts do not block host-cache, copy-version or
fixture implementation. A busy selected GPU does not block local proof/tests.

## Remaining Tickets

| Packet | Lead | Deliverable and acceptance gate |
| --- | --- | --- |
| MEM-QUAL-HARNESS | Native; Resources review, Primary runner integration | New example/checker exercises actual optional N1/N2 budgets and device-cache limits in both startup orders, with pressure, retained backing, reuse and disposal. Existing R66/drain examples only configure logical requested-byte limits; rerunning them cannot qualify native budgets. |
| R78 coherent initialization qualification | Native harness; Primary signed hardware | Existing R60 covers initial HostVisible materialization only. Add a separate two-launch exact-vecadd cell with distinct full HostVisible triplets, one Context/stream/native queue, three materializations per launch and complete outputs to force rebound insertion. Keep existing R60 reuse criteria unchanged. |
| MEM-N1A-QUAL and N2/pool qualification | Primary after MEM-QUAL-HARNESS | Signed Linux admission-pressure captures, exact native usage/identities, complete data and cleanup. Host-cache cells follow that policy's implementation. Fake-backend tests do not fill these cells. |
| GEN-2A production acceptance | Admission/Primary | Nonexecuting custody and local adapter/type tests are implemented in R74. Positive full construction still requires an actual checked device and exact production compiler evidence; local predicate/structural fixtures do not fill that cell. Context allocation generations/admission belong to GEN-2B. |
| GEN-2R runtime integration | Primary; Resources/Admission review with GEN-2B | R73 data storage is implemented and locally validated. Bind its private decoder and credits to exact runtime-owner lifetime; ordinary host-thread tests do not qualify shutdown. Preserve no raw escape, partial publication or capacity mismatch. |
| GEN-2B-2 native acceptance | Primary; Native/Admission review | Qualify repeated immutable scopes on a genuine retained device before and after lazy bootstrap. Positive generated construction additionally needs exact protected compiler evidence. |
| GEN-2B-3 through -6 | Admission/Native; Primary integration | Build on implemented local drivers and Context preparation with exact persistent publication/readback, one completion future and shared blocking/async execution, followed by graph/drain qualification. See the ordered breakdown above. |
| GEN-2B-PROFILES | Native; Admission/Resources review | Extend the fixed path for scalar-only/empty rosters, larger rosters, admitted overlapping aliases and runtime-service hidden fields. Each needs explicit native representation, complete bounded accounting and profile-specific authority/tests/qualification; weakening projection rejection does not implement support. |
| MEM-2B-HOST | Resources policy; Native hooks | Bound ordinary coherent host-cache reuse with cache-or-dispose admission using R72 N1A. Test padded byte/record limits, zero caching, generation reuse and failed trim; retain charges for incompletely disposed backing, but never resurrect confirmed-disposed charges. Broader N1B is not a prerequisite for this narrow profile. |
| MEM-N1B | Resources/Native | Implement canonical cost/admission/disposal for userptr, doubled-VA AQL, executable, kernarg and control profiles, then qualify each separately. Distinguish physical backing, aliases and reserved VA; test exact lifetime/disposal and avoid double charging. |
| MEM-DOM-1 | Resources; Primary construction hooks | Root/device/Context account ownership with bootstrap and terminal headroom reserved in advance. Repeated Context creation and simultaneous quarantine must not reset or exceed aggregate limits. |
| MEM-3A/B | Resources/Native | Account for queue/ring, signal, kernarg/control storage and occupied slots. Adopt exact MEM-TXN-1 members without duplicate backing debits; enforce a progress-safe acquisition order before publication. |
| MEM-4A/B | Resources/Native | Bound retained host executable images and materialized code/control caches. Exact identity and live-operation leases govern eviction; ambiguous unload retains charges. |
| VER-1A/B, then VER-2 | Resources; Primary mutation hooks | Bounded persistent Context mutation journal, conservative invalidation of every mutation path, and private cross-run input leases. Reject foreign, stale, unknown, mixed-generation and replayed versions before issue. |
| MEM-5 closure | Resources; Primary integration | Close aggregate command/result/capture, registry, arena, journal, reply, terminal and quarantine footprints. Include all native and host owners, not just requested allocation bytes. |
| OVL-QUAL-2 and DRN-2A hardware | Primary; Native/Admission review | Independently checked signed coexistence and copy-only outstanding-work drain captures, complete outputs/canaries, exact native identity and owned-process/stage cleanup. Retention alone does not prove physical overlap. |
| SCALE-1A-FIXTURE / QUAL | Native; Primary builds/hardware | Freeze bounded geometry, ABI/effects, source/object/toolchain and complete-output oracles. CPU policy tests precede signed sequential Linux correctness. Intended short/long work classes are not measured durations. |
| SCALE-CAP, then SCALE-2 | Native | Real backing/control/slot admission precedes any larger native profile; preserve existing defaults. Measure native publication/retention, unresolved work and retirement separately from host queue depth. Thousands of queued records do not establish thousands native in flight. |
| DRN-2B-FIXTURE | Admission/Native; Primary hardware | After SCALE-1 fixture admission and signed sequential correctness, qualify queued/native-retained compute work, streams, dropped observers and drain under explicit qualification authority. No production compiler handoff is required; complete outputs and cleanup remain mandatory. |
| Repeated generated graphs and production drain | Admission; Primary hardware | After GEN-2B and exact per-kernel compiler evidence, qualify repeated kernel/copy graphs, active drain, dropped observers, complete typed outputs and cleanup. Cross-run input reuse also needs VER-1/2. Fixture acceptance cannot fill production cells. |
| PRF-1/2 | Primary; rotating cross-review | Compose production executor transitions with lifecycle, credits, dependency readiness, versions, reuse and drain proofs. Run authenticated positives/negative mutations and integration gates; retain explicit external contracts. |
| SCALE-3 measurements | Native; Primary hardware | Signed matched HIP/HSA/KFD producers and correctness-first captures for latency, throughput, copy bandwidth, CPU use and memory bounds. Device timelines are separately required for physical-overlap claims. |

### First-Packet Acceptance

- Host cache: padded bytes and record ceilings, zero caching, cache-or-dispose pressure,
  stale/foreign token rejection and generation-safe reuse. Failed trim retains
  charges for incomplete/uncertain disposal; it does not resurrect charges for
  backing already confirmed disposed. Broader N1B is not required for this profile.
- GEN-2A: changed artifact/packing/geometry/effects/device/publication reject;
  compile-fail tests prohibit borrowed escape, duplicate permits and decoder
  extraction. Explicitly enforce and compile-check the new invocation's
  owner-local `!Send`/`!Sync` requirement; the existing device's documented
  auto-trait claim is not sufficient evidence.
- Fixtures: freeze bounded geometry, ABI/effects, complete ReadWrite footprint,
  source/object/toolchain identity and independent output/padding oracles;
  mutate coordinates to test rejection. Short/long labels are not measurements.
- Native-budget harness: exercise actual N1/N2/cache configuration in both
  startup orders, bootstrap consumption and padded byte/record exhaustion.
  Pre-effect budget rejection leaves usage unchanged; post-native failure or
  ambiguity retains/quarantines the debit. Test charged reuse and confirmed
  disposal. Existing R66 and DRN-2A captures cannot substitute for this harness.

### Later Issue Milestones

These remain separate from closing A1/A2. The same three slots rotate into
these queues after local contracts stabilize; no extra workers are implied.

| Milestone | Lead and deliverable | Exit evidence |
| --- | --- | --- |
| A3: local multi-GPU | Native, with Resources/Admission: unified compute/peer ownership, topology/currentness placement, shards/replicas and group drain | All admitted GPUs execute a sharded workload; exact versions, stale-generation rejection and partial-failure isolation. Existing exact-two-device copy-only XGMI is insufficient |
| A4: distributed control | Admission, with Primary protocol integration: authenticated sessions, membership epochs, artifact negotiation, receipts and drain | Two hosts reject substitutions and classify interrupted publication without duplicate issue or treating timeout as quiescence |
| A5: data and collectives | Native transport plus Resources versions/credits: bounded reference host-staged transfers and deterministic collective plans | Two-host complete canaries, exact membership/version receipts and bounded network/staging use; broadcast, reduce-scatter, all-gather and all-reduce qualified separately |
| A6: failure qualification | Admission; rotating cross-review and Primary fault campaigns | Boundary-by-boundary participant/network/device/collective faults; no unsafe replay, early disposal or false complete outputs. Disruptive tests need an agreed isolated window |
| A7: production performance | Native measurements; Primary evidence/release | Precommitted single-device, eight-GPU and two-host thresholds, tail latency, CPU/memory bounds, recovery costs and direct-KFD dependency/symbol audits |

Broad device Rust and production atomics/collectives also require their exact
compiler semantic-to-machine contracts and native qualification. Existing
host-observed profiling is not missing wholesale; trusted device timelines and
cross-collector attribution remain separate work.

## Interface Decisions

N1A provides public `Gfx942HostVisibleBackingBudgetV1::new(bytes, records)` and
an inert usage snapshot. Limits are positive, at most 8 GiB and 256 records.
The byte ceiling is a chosen numerical bound matching the existing envelope,
not a measurement of physical residency from VA usage.

Private account/reservation/charge types bind the exact session, device, VM,
allocation ID, generation and full canonical `HostVisibleCoherentGttV1` layout.
For that ordinary non-userptr profile, the page-padded CPU span equals the
native allocation size: charge it once plus one allocation record. CPU/GPU
views, loans and recycle do not create new backing or refund it.

Ordinary coherent completion/control allocations made during bootstrap are
included by profile. Userptr, executable AQL, kernarg and complete bootstrap
accounting are not implied. Failures and panics after the first native effect
but before record insertion must retain/quarantine the charge. Pre-effect
rejection cancels only the unissued reservation. Later queue-retake failure cannot resurrect a
debit already returned after confirmed disposal.

GEN-2B must bind the actual
[checked device](../crates/fe2o3-kfd/src/device.rs) retained by the persistent
backend through an explicitly owner-local invocation scope, without lifetime
or thread-safety widening. R74's standalone custody is not a per-launch
device-admission strategy. Source
review found that `OpenedKfd` uses `PhantomData<Cell<()>>`, which prevents
`Sync` but does not itself prevent `Send`; the concrete device auto traits have
not been compile-checked in this planning pass. Enforce the new invocation's
`!Send`/`!Sync` requirement directly and test it rather than relying on device
rustdoc or changing the existing device API implicitly. Existing GEN-1
[`try_take()`](../crates/fe2o3-host/src/generated_runtime_arguments.rs)
returns bare `Box<[T]>` and represents decoded data only. R73 adds
a distinct charged result owner whose credit follows the returned storage,
not merely the observer. The legacy decoded-result state must never receive
production bytes, even behind a wrapper. This is a new production contract, not
a defect in GEN-1's intentionally inert data API.

The agreed GEN-2R contract reserves one R70 batch member per output for its
encoded-plus-typed peak and retains that entire conservative reservation until
the charged typed result is disposed. It does not require partial splitting of
an issued debit or claim that reserved peak equals current physical residency.
Preallocate private destinations, validate the complete scalar/shape/count and
capacity roster, decode all outputs, then publish results together. Expose
borrowed slices, not `Clone` or raw `Box`/`Vec` extraction. Test late-output
failure, decoder mismatch/panic, exhaustion, dropped observers and shutdown.
Caller seed storage is charged on transfer, not retroactively bounded before
the caller allocated it. Storage ownership alone grants no completion authority.

GEN-2A consumes that interface in an owner-local prepared invocation. It is
nonexecuting; no checked-device lifetime widening, unsafe thread-safety claim or
caller-provided authorizer is permitted. Primary integrates Context allocation
generations and operation admission in GEN-2B with Native's existing publication
and retirement mechanisms, not a second queue implementation.

GEN-2A can live as a private child of `generated_kfd_invocation`, reusing its
private production-admission helpers without a second unsafe authority
implementation. Consume raw generated arguments with their authenticated
executable, not detached prepacked data. Host already depends on runtime, so
GEN-2B needs a runtime-defined admitted interface rather than a Context
dependency on a concrete host type. Runtime preparation's executable/hidden
kernarg allocations and read-only initialization copies are outside R73's result
budget and need their own accounting.

Guard storage-before-credit disposal during preparation errors and unwind, not
just the final invocation's field-drop order. The whole prepared dispatch,
including its read-only initialization copies, must be disposed before its
decoder. GEN-2B also owns contention retry/wakeup and lost-wakeup tests; R73's
nonblocking `try_take` alone does not provide Future progress.

MEM-2B-HOST must project the exact native record's page-padded
`cpu_mapping_bytes()`, not the SDMA token's requested `physical_bytes()`.
It limits cached-free backing without minting a second N1 charge. Reuse existing
irreversible SDMA activity history for configuration; failed trim retains only
unresolved backing and cannot resurrect already disposed charges.

## Dependencies And Stops

1. N1A cost/native/forwarding is locally implemented as one integration packet.
   Ordinary host-cache limits can follow immediately; broader N1 profiles,
   domain ownership and control/code budgets remain separate prerequisites;
   MEM-5 cannot close before all owners are accounted for.
2. GEN-2R's data interface and GEN-2A's nonexecuting custody are implemented.
   Their local predicate, ownership and type tests do not establish a positive
   production constructor or native completion. GEN-2B
   consumes these interfaces; positive production
   execution also needs the next handoff. VER-1 may move earlier when its slot
   is free.
3. Positive production GEN-2 admission requires compiler/generated-host owners
   to supply exact protected verification and semantic-to-machine refinement
   evidence: finalized artifact/ISA, semantic KIR/final LLVM, ABI/effects,
   authenticated proof inputs, currentness/ledger and publication occurrence.
   The concrete production protected-verifier/refinement backend and exact
   artifact handoff remain absent from the repository; test adapters do not
   supply them. The [existing fail-closed boundary](../crates/fe2o3-host/src/worker_v3_verification_admission.rs)
   must remain; caller digests or
   qualification fixtures cannot replace that handoff.
4. SCALE-CAP requires real backing/control/slot admission and correctness-qualified
   fixtures. Native depth and timing follow admission, not a constant increase.
5. Proof composition starts with each implementation packet. Final acceptance
   requires the complete A1/A2 matrix, not the sum of isolated cost proofs.
   Matched performance follows correctness and signed hardware acceptance.

Only Primary runs builds, proof gates, MI300X campaigns, signing and pushes.
Use one bounded campaign at a time on an admitted idle selected GPU, private
staging, exact binary/census evidence and independent cleanup. Foreign work is
untouched; disruptive reset/fault campaigns need a separately agreed window.
Every completed implementation packet is cross-reviewed, validated against its
actual source and pushed to both remotes. A topic-branch push is not a main merge.

A1/A2 remain open. The later A3-A7 table assigns milestone leads and exit gates;
those milestones still need packet-level designs and their own test environments.
Neither this plan nor R76 establishes full HIP/HSA
parity or an unmeasured speedup.
