# A1/A2 Next-Wave Dispatch

Reviewed 2026-09-10 against signed R74
`3a1861282ac5bb89492f029b6850594a0545e95a`, pushed to both remotes.
Three read-only agents reconciled the remaining work with source and the current
open [#182](https://github.com/harsh-nod/fe2o3/issues/182). This document assigns
bounded analysis/review queues, not unattended implementation jobs. Primary
owns edits, integration, tests, hardware scheduling and publication. A queued
ticket is not new implementation or an accepted proof/hardware result.

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
acceptance follows from these driver tests. Admission now takes GEN-2B-2's
Context/host adapter and the exact persistent projection contract with Native;
GEN-2B-3 publication still depends on that retained-device scope.

## Current Swarm Dispatch

These are three read-only worker slots plus the primary. A lane supplies a
source-grounded design, test/proof obligations and an independent review for
one bounded packet at a time. Primary implements the agreed packet. Follow-ons
are sequential queues, not additional concurrent workers or delegated edits.

| Lead | Next bounded assignment | Module scope and primary handoff |
| --- | --- | --- |
| Native: `r66_native_coexistence` | GEN-2B-2 retained-device scope; then independent SCALE-1A-FIXTURE | Specify checked-device access through session/queue ownership and exact Context binding. Next, bounded short/long artifacts, isolated `qualification_gfx942_scale_v1.rs` and complete-output oracles. Fixtures have no GEN-2 prerequisite. |
| Resources: `r66_coexistence_model` | MEM-2B-HOST: ordinary coherent host-cache bounds | Specify isolated `sdma/host_pool_policy.rs`, exact padded-cost/identity projection and policy/model tests. Preserve R72 backing debit across reuse; zero cache limit disables caching, default stays unchanged. Coordinate native hooks before integration. |
| Admission: `r66_runtime_coexistence` | GEN-2B-2 Context/host adapter and GEN-2B-3 projection contract | Build on R75's owner-local operation/retention path and Native's retained-device scope. Bind exact Context/device generations and lossless persistent recipe coordinates before per-invocation publication. No public generated submit yet. |
| Primary | Shared GEN-2/cache integration, proof composition and qualification | Own every edit, integration/build/proof run, evidence record and signed dual push. Schedule existing OVL-QUAL-2 and DRN-2A against frozen signed source; native-budget pressure first needs MEM-QUAL-HARNESS. |

Admission and Resources agree the permit/decoder/result boundary before code
integration. Native's fixture work does not mint production generated-launch
authority. Primary serializes shared-file edits; each worker reviews another
lane's contract before the packet's local gates.

After its retained-device handoff, Native takes fixtures, the sequential qualifier and
MEM-QUAL-HARNESS, then ordinary host-cache and broader native budget hooks.
Resources takes MEM-N1B, MEM-DOM-1, MEM-3/4, VER-1/2 and MEM-5. Admission follows
GEN-2B with generated graph/drain qualification. Waiting for hardware must not
block CPU harness work. Version-journal mechanics or host-image accounting may
move earlier when the Resources slot is free.

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
| GEN-2B-2: reusable preparation | Native scope in `memory_linux.rs`, `shared_memory.rs`, `queue_live.rs`; Admission's runtime-defined Context/host adapter. Independent of -1. | Full pre/post currentness, exact private Context/backend/device generation and no device extraction. Multiple preparations before/after lazy VM/queue creation must work without a new admission; foreign/stale/terminal scopes and borrowed escape reject. Adopt prepared custody without retaining a mutable Context borrow. |
| GEN-2B-3: persistent publication | Native/Admission; runtime `kfd_backend/compute_dispatch.rs`, `compute_state.rs` and narrow prepared-request projection. Requires -1 and -2. | Preserve artifact, ABI, buffers/fixups, hidden kernargs, geometry and timeout policy in the existing persistent path. Retain per-invocation authority before native effects and revalidate at actual publication, including deferred paths. Reject substitutions/double consumption; pending polls and ambiguous publication never authorize replay. |
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
runtime code or acceptance status; R75's implemented boundary is recorded
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
| MEM-N1A-QUAL and N2/pool qualification | Primary after MEM-QUAL-HARNESS | Signed Linux admission-pressure captures, exact native usage/identities, complete data and cleanup. Host-cache cells follow that policy's implementation. Fake-backend tests do not fill these cells. |
| GEN-2A production acceptance | Admission/Primary | Nonexecuting custody and local adapter/type tests are implemented in R74. Positive full construction still requires an actual checked device and exact production compiler evidence; local predicate/structural fixtures do not fill that cell. Context allocation generations/admission belong to GEN-2B. |
| GEN-2R runtime integration | Primary; Resources/Admission review with GEN-2B | R73 data storage is implemented and locally validated. Bind its private decoder and credits to exact runtime-owner lifetime; ordinary host-thread tests do not qualify shutdown. Preserve no raw escape, partial publication or capacity mismatch. |
| GEN-2B-2 through -6 | Admission/Native; Primary integration | Build on implemented owner-local retention with reusable Context preparation, exact persistent publication/readback, one completion future and shared blocking/async execution, followed by graph/drain qualification. See the ordered breakdown above. |
| MEM-2B-HOST | Resources policy; Native hooks | Bound ordinary coherent host-cache reuse with cache-or-dispose admission using R72 N1A. Test padded byte/record limits, zero caching, generation reuse and failed trim; retain charges for incompletely disposed backing, but never resurrect confirmed-disposed charges. Broader N1B is not a prerequisite for this narrow profile. |
| MEM-N1B | Resources/Native | Qualify userptr, doubled-VA AQL, executable, kernarg and control profiles separately. Distinguish physical backing, aliases and reserved VA; test exact lifetime/disposal and avoid double charging. |
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
Neither this plan nor R75 establishes full HIP/HSA
parity or an unmeasured speedup.
