# Native Resource Accounting Contract V1

Status: MEM-BASE and the optional session-local MEM-2A N2 backing adapter are
implemented locally. The remaining MEM-5 inventory/interface contract is
proposed, reviewed on 2026-09-10. This does not close MEM-2 through MEM-5.
Pool qualification, compound native ownership, parent/batch/split operations and
global physical accounting still require the reviewed handoffs below.
The [A1/A2 swarm plan](runtime-a1-a2-swarm-plan.md) owns scheduling and acceptance.

## Current Boundary

[R67 resource credits](../crates/fe2o3-resource-accounting/src/lib.rs) provide
checked nineteen-dimensional admission, bounded owner records, move-only
reservations, retained charges and conservative quarantine. The
[Context adapter](../crates/fe2o3-runtime/src/context/allocation_admission.rs)
currently charges only `RequestedAllocationBytes` and `AllocationRecords` on an
opt-in, exact Context/device account. It retains before backend entry, refunds
definite rejection or successful logical allocation disposal, and preserves
ambiguous attempts even when no allocation handle was returned.

Successful logical disposal is not necessarily physical deallocation. The
[KFD backend](../crates/fe2o3-runtime/src/kfd_backend.rs) can recycle a released
allocation into the native SDMA pool. Requested credits can return then; native
backing credits must remain. Completion, pool recycling, observer drop, timeout,
drain and Context teardown are not physical-disposal witnesses.

R67's per-account retained `Arc` anchor prevents lost ambiguous credit custody.
It does not bound the aggregate of new accounts or Contexts, and does not itself
retain or authenticate a native allocation. Native ownership and accounting
ownership must be structurally composed before claiming native budget closure.

Existing staging-byte, snapshot-byte, reply-count, queue-slot and memory-session
limits are useful local guards. They are neither one transactional resource
vector nor a total runtime-owned memory ceiling. R67's unused dimensions are
not measured zeros.

## Implemented MEM-2A Slice

`SharedGttMemorySessionV1::configure_device_backing_budget_v1` installs one
immutable `Gfx942DeviceBackingBudgetV1`. Its two positive limits cannot exceed
the existing 192-GiB backing and 128-allocation native profile. Configuration
requires an active, session-owned instance before any N2 native attempt or
queue-foundation certification. A released allocation does not reopen
configuration; restoring queue ownership does not remove the closure.

The [private adapter](../crates/fe2o3-kfd/src/shared_memory/resource_accounting.rs)
binds its account to the actual session ID, device generation and VM. It derives
cost from a canonical validated `Gfx942DeviceMemoryLayoutV1`, using padded
`backing_bytes` and one `AllocationRecords` unit. Requested bytes, GTT, mapped
views and other accounts are not substituted or added to this backing cost.
Both ordinary and PUBLIC device-local allocation paths use the same adapter.
Usage snapshots are inert and do not expose charge or disposal authority.

Reservation precedes currentness and the first `reserve_va` call. An unissued
reservation can cancel; immediately before native entry it becomes retained.
The exact allocation ID, generation and layout accompany its move-only charge
in `DeviceMemoryRecord`. Mapping, initialization, lease drop and uncertain
results do not refund it. An ambiguous attempt before record insertion retains
the debit through the shared ledger's quarantine anchor. Configured native
transition panics quarantine the session and resume the original payload. A
paired XGMI operation quarantines both participants if either is configured.
Neither guard performs cleanup, retries native operations or replaces the panic.

Refund requires the exact account and allocation identity, successful backing
free and VA release, closing currentness and checked existing byte accounting.
A charged record cannot be recycled. Failed or interrupted disposal retains
the debit even if part of the native cleanup has already succeeded. Accounts
without this opt-in configuration keep the existing allocation behavior and
profile limits.

R68's production-used projection has a corresponding property-specific Verus
source and five named mutations. Its four obligations cover the bounded input,
exact backing/record vector, zero other dimensions and algebraic conservation.
They do not prove that native layout extraction, syscall outcomes, mutex/arena
ownership or whole-executor disposal correspond to the model. The full
authenticated roster and CPU fault matrix are separate release gates.

This is one session's N2 accounting, not a runtime-wide switch or closed memory
ceiling. Session bootstrap, GTT, queue/control backing, host metadata, parent
budgets, aggregate quarantine and runtime configuration forwarding remain
open. Runtime forwarding must configure during native session construction,
before queue-foundation transfer, rather than weakening freshness to permit
late Context configuration. Pool checkout/recycle/trim qualification is MEM-2B;
retaining charges in the underlying allocation record does not by itself
qualify every pool path.
No hardware or performance result is implied by this implementation.

## Units And Counting Rules

The qualified ceiling must name its domain: resources owned or retained by the
bounded runtime profile, including abandoned/terminal work. It is not a claim
about arbitrary application allocations, observed RSS, or unused physical GPU
capacity. Admission charges allocated backing capacity conservatively; it does
not infer physical page residency from a virtual mapping or a payload length.

Each inventory item has one physical allocation identity, placement, extent,
purpose, owning account and disposal transition. Identity includes exact native
session/device/VM and allocation incarnation where applicable. A changed pool or
submission generation changes use authority, not the identity of its still-live
backing allocation.

Proposed projection rules, subject to the shared-interface review:

| Quantity | Meaning | Counting rule |
| --- | --- | --- |
| Requested/logical bytes | Accepted logical allocation extent or retained command payload | Independent admission dimension; never substituted for backing bytes |
| Host backing bytes | Owned heap/slab/anonymous/GTT backing capacity | Count each backing allocation once, including additional staging copies; CPU and GPU views of the same GTT object are not two allocations |
| Device backing bytes | Actual device-local backing extent from the admitted layout | Page padding is included; mapping the same object into another view does not create a second payload allocation |
| Purpose bytes | Executable, queue, signal, kernarg, control, reply or quarantine sub-ceiling | Tagged projections of unique backing items, not additional bytes to add to physical totals |
| Occupied slots | Queue entries, signals, kernarg slices, operation/reply/allocation records | Charge occupancy separately from the preallocated arena's backing; recycling a slot does not free the arena |
| CPU/GPU VA | Reservations/mappings, including MMIO views and guards | Bound separately from DRAM/VRAM; overlapping views must not inflate physical totals |
| Driver/OS objects | BOs, mappings, queues, events, descriptors and threads | Bound exact admitted counts; opaque implementation bytes need an explicit cost contract before a broader total-memory claim |

In particular, materialized executable code currently uses executable GTT in
`shared_memory.rs`, not necessarily VRAM. The existing
`ExecutableDeviceBytes` name cannot be interpreted as device-local placement.
The shared interface must document or rename this purpose dimension before
native use. Likewise, `AllocationRecords` must not be relabeled `OperationSlots`.

R67's schema is an initial contract, not an exhaustive physical inventory. Host
metadata capacity, CPU/GPU VA and driver-object counts need explicit reviewed
dimensions or separately bounded components of the same root ledger. Do not
silently put arbitrary registry bytes into native `ControlResidentBytes`.
Any schema change needs corresponding executable/model/proof updates and pins;
this document changes none of those artifacts.

## Allocation-Site Inventory

References identify existing code, not an assertion that credits are integrated.
For Rust containers, logical `len * size_of::<T>()` is not allocated capacity or
allocator overhead. A closed byte-bounded implementation needs retained capacity
and a supported allocation-layout bound, or a precharged fixed-layout arena.

### Native Backing And Control

| ID / Sites | Actual extent and owner | Required charge lifetime / current gap |
| --- | --- | --- |
| N1: [shared GTT allocation](../crates/fe2o3-kfd/src/shared_memory.rs), `profile_layout`, `SharedMemoryEngine::allocate`, `release` | `SharedGttAllocationLayoutV1::{cpu_mapping_bytes,gpu_va_bytes}`, derived from the exact profile; native memory-session record and linear token own it | Reserve backing, VA and record capacity before `reserve_va`, userptr preparation or allocation ioctl. Retain through mapping/sealing/queue loans. Refund only the successfully released low-level object and its mappings; current session VA/count guards are not the proposed global budget |
| N2: same file, `device_memory_layout`, `allocate_device_memory_with_flags`, `release_device_memory` | `Gfx942DeviceMemoryLayoutV1::backing_bytes`, not `requested_bytes`; exact device/VM allocation record | Retain across initialization, mapping, dispatch promotion/demotion and pool custody. GPU unmap alone is not backing disposal. Existing 128-record/192-GiB admission bounds are local profile limits, not availability or global-account evidence |
| N3: [SDMA buffers](../crates/fe2o3-kfd/src/sdma.rs), `allocate_host_buffer`, `allocate_device_buffer`; [pool](../crates/fe2o3-kfd/src/queue_live.rs), `checkout_sdma_pool`, `recycle_sdma_buffer`, `trim_sdma_memory_pool`, `release_sdma_buffer` | Buffer `physical_bytes`/alignment and exact underlying N1/N2 identity; checked-out owner or `sdma_pool_free` retains the same backing | No second backing charge on checkout; no refund on recycle or generation advance. Cache bytes/count remain charged. Trim refunds only successful low-level release; failed trim retains ambiguous charges. Current pool observation reports cached bytes and checked-out counts, not a complete ceiling |
| N4: [compute queue creation](../crates/fe2o3-kfd/src/queue_live.rs), `create_compute_aql_queue_with_runtime`, queue preparation; [queue plan](../crates/fe2o3-kfd/src/queue_resources.rs) | Actual planned ring span; one-page userptr control; EOP extent; context-save mapping extent from `Gfx942AqlQueueResourcePlanV1`; queue owner retains N1 objects | Charge the complete creation plan before the first native effect, including later N5/N6 objects. Ring packets are occupied slots inside this backing, not fresh allocations. The admitted CWSR mapping constant is `0xb167000` bytes; derive from the exact plan, not a caller estimate |
| N5: [Linux queue ownership](../crates/fe2o3-kfd/src/queue_linux.rs), `LinuxCwsrShadowPagesV1`, `map_cwsr_payload_page`, `doorbell_mmap_plan` | Admitted 24 anonymous control-stack shadow pages plus separate payload page; complete doorbell slice mapping; KFD event and queue objects | Anonymous shadows are additional backing even though they replace CPU views inside an existing CWSR VA span; do not charge that VA span twice. MMIO doorbell mapping is not ordinary DRAM. Retain exact event/payload/shadow teardown order; pre-create cleanup and published terminal retention have different contracts |
| N6: [completion owner](../crates/fe2o3-kfd/src/queue_completion.rs), `COMPLETION_SIGNAL_ARENA_BYTES_V1`, `allocate_completion_slot_records_v1`, `CompletionSignalArenaOwnerV1` | 8,192 signal slots of 64 bytes in the current arena, plus boxed CPU slot records and dependency-reader/event metadata | Charge the coherent arena once and live signal/readership slots separately. Completion observation does not authorize reuse while readers/events still retain a slot. CPU record backing is additional host metadata, not included in signal payload bytes |
| N7: [SDMA queue creation](../crates/fe2o3-kfd/src/sdma.rs), queue-owner creation | Per native SDMA queue: 4,096-byte ring, one-page control, 4,096-byte completion storage, doorbell mapping and fixed record/window ledgers | Current directional owner uses its exact queue roster; striped/mux/XGMI profiles need their own complete roster. Charge backing once, packet/window occupancy separately; a 64-slot ring has at most 63 occupied entries. No generic queue-count assumption may omit additional owners |
| N8: [dispatch resources](../crates/fe2o3-kfd/src/queue_dispatch_binding.rs), `plan_public_fixed_dispatch_resources`, `prepare_public_fixed_dispatch_resources_with_generation` | Checked materialized image spans, aligned combined kernarg arena, retained data roster, owned metadata/premises and epoch tables | Images/kernargs are new backing; retained data may be existing N1/N2 and must not be charged again. Reserve all new plan members before preparation. Cached control and unused inspected programs remain charged until their actual release, not merely packet retirement |
| N9: same file, `release_non_data_after_recycle`, `release_persistent_data_after_recycle`; [runtime lane cache](../crates/fe2o3-runtime/src/kfd_backend/compute_state.rs) | Recycled dispatch, resident-data roster and persistent control own exact native objects beyond the last operation | Move their charge with the owner on detach/restore/cache replacement. Module unload cannot refund code/control still retained by another owner; partial release retains unreleased members |
| N10: [memory-session construction](../crates/fe2o3-kfd/src/shared_memory.rs), session model/indices and native backend setup | Pre-reserved 256 shared and 128 device record slots, ID-to-slot indices, journal storage, selected-device topology/descriptor resources and VA reservations | Count limits and source-contract manifests identify bounds but not all actual host bytes. Session metadata, descriptors and acquisition-time temporary storage require a cost manifest before claiming the complete native setup budget |

N1/N2 are allocation mechanisms, not additive rows on top of N3 through N9.
For example, a queue's completion arena is one N1 allocation tagged as N6. The
inventory must resolve such references to one physical record. Shared physical
backing retained by several use authorities remains one charge until its last
owner can dispose it; independent native use-slot charges remain distinct.

### Host Runtime And Results

| ID / Sites | Actual extent and owner | Required charge lifetime / current gap |
| --- | --- | --- |
| H1: [Context](../crates/fe2o3-runtime/src/context.rs), `allocate`, `release_allocation`, cleanup; [admission](../crates/fe2o3-runtime/src/context/allocation_admission.rs) | Logical requested bytes and exact Context allocation-record count | Implemented R67 slice. Pre-reserved registry capacity precedes backend entry, but its allocated host bytes are not covered by requested-byte credit. Logical release can leave N3 cached backing live |
| H2: [KFD backend](../crates/fe2o3-runtime/src/kfd_backend.rs), `allocate_v1`, `try_zeroed_staging_v1`, writes/readback and `Arc::make_mut`; [staged data](../crates/fe2o3-runtime/src/kfd_backend/compute_state.rs), `DataSpecV1::try_owned_bytes` | Host staging for both memory kinds, retained `Arc<[u8]>` snapshots, COW copies, boxed native-input copies and upload/download/zeroing scratch | Charge each actual allocation before creating it; Arc cloning only extends its charge lifetime. COW creates a second charge. `staged_context_bytes` counts logical current allocation bytes, not all retained copies, temporary peaks or native backing |
| H3: [module load/unload](../crates/fe2o3-runtime/src/kfd_backend.rs), `load_module_v1`, `unload_module_v1`; `ModuleRecordV1` in compute state | Retained validated host image plus parser metadata, symbols, kernel records and materialization caches | MEM-4A must charge host image/capacity before copying/parsing. Native materialization is separately N8. Retain leases through pending/recycled dispatch and reject eviction; failed unload is not a refund witness |
| H4: [standalone snapshots](../crates/fe2o3-runtime/src/async_engine/snapshot.rs), `RuntimeAsyncLaunchRequestV1`, `SnapshotBudgetV1` | Boxed explicit kernarg, bindings and dependency arrays; `Charged<T>` owns a byte permit | Existing exact slice-payload charge ends when that snapshot is consumed/dropped. It does not charge native custody, Arc/allocator/record overhead, encoder temporary allocations or arbitrary closure captures |
| H5: [GEN-1](../crates/fe2o3-host/src/generated_runtime_arguments.rs), owned slices, budget, packed data, decoder/result | Owned typed seeds, encoded inputs, two packing-time kernarg copies, returned bytes and typed decoded output; output Arc/custody records | Existing per-invocation logical preflight counts those overlapping payload lifetimes. GEN-2 must attach global credits to actual data owners and carry them through result extraction; decoder shape/currentness does not confer execution authority |
| H6: [reply budget](../crates/fe2o3-runtime/src/async_engine/reply_budget.rs) and [reply cells](../crates/fe2o3-runtime/src/async_engine/owned.rs), `ReplyState<R>` | Arc/mutex cell, retained result and waker, including completed caller-retained cells | Existing permit bounds cell count until last owner releases it. Generic `R`, generic errors and waker-owned memory are not byte bounded. A returned owned result must carry its byte permit after leaving the cell; refunding at reply delivery is too early |
| H7: [async owner](../crates/fe2o3-runtime/src/async_engine/owned.rs), [engine](../crates/fe2o3-runtime/src/async_engine.rs), operation control | Bounded channels, boxed command/operation closures, waiter/progress registries, reply/startup channels, thread stack and synchronization objects | Channel occupancy does not measure closure captures or channel/registry allocation capacity. Closed commands need measured payloads; thread/stack and channel bootstrap storage need a precharged profile. Arbitrary callbacks/closures are not accepted as byte-bounded merely because a caller reports a size |
| H8: [Context registries](../crates/fe2o3-runtime/src/context.rs), [graph reservations](../crates/fe2o3-runtime/src/context/graph.rs), [graph execution](../crates/fe2o3-runtime/src/async_engine/graph.rs) | Device/stream/allocation/module/kernel/event/submission maps and reverse indices; graph nodes/edges/effects, prepared actions, readiness/trace/report storage | Current finite element-count limits do not measure hash-table capacity, graph-owned bytes or retained report storage. Include preflight temporaries, spare capacity and callback records; reserve publication/retirement metadata before native custody |
| H9: [R65 versions](../crates/fe2o3-runtime/src/async_engine/graph/versions.rs), planned VER-1 journal/VER-2 leases | Segment/end-point tables, pending/current versions, references and copied report records; future persistent mutation and lease entries | R65 is graph-local lineage. Account preparation peaks and terminal report lifetime. VER-1/2 must reserve journal/lease capacity before mutation/admission and join the final MEM-5 inventory before closure |
| H10: [drain](../crates/fe2o3-runtime/src/async_engine/drain.rs), planned DRN-1 capture | Pending-submission/stream collections and cleanup report; future owned coherent capture bytes and result metadata | Existing drain is not the planned host-only content capture. DRN-1 must reserve destination/capture metadata before cutoff and retain result credit after cleanup. GPU download staging is a separate pre-admitted N3/H2 operation |
| H11: [credit account](../crates/fe2o3-runtime/src/resource_credits.rs), owner/backend terminal retention and `mem::forget` paths | Credit record/free-slot arena, account Arc/mutex, terminal native bundles, retained Contexts, error strings, panic payloads and abandoned results | Current per-account arena is count bounded and quarantined without fresh allocation, but not globally byte charged. Unknown generic panic/error payload sizes and repeated leaked Contexts prevent a general global ceiling; the bounded profile needs explicit alternatives, reserved headroom and a root lifetime |

Module/parser, graph, topology, profiling and error-detail allocations must be
expanded into measured layout entries during integration. This site-family
inventory deliberately exposes those unresolved components; it is not an
audited enumeration of every allocator call or a completed MEM-5 claim. Optional
profiling hooks and arbitrary user code cannot be smuggled into the closed
profile with an exclusion note while still claiming its total byte bound.

## Proposed Dependency-Safe Adapter

### One Ledger, Native Ownership

The approved MEM-BASE extraction moves the existing account engine into the
lower-level `fe2o3-resource-accounting` crate depending on `fe2o3-runtime-model`
and `std`. Runtime depends on it and retains branded thin wrappers and
Context configuration. KFD now consumes it for the session-local N2 slice above.
The implementation is moved, not forked: the shared crate owns arithmetic,
record identity and transaction mechanics, not native allocation, device
currentness or disposal observations. Nine engine regressions and two compile-fail
ownership examples live with the engine; three runtime wrapper regressions
cover device labels, independent accounts and interface compatibility.

Further native adapters must compose moved credits with private native owner
bundles. Public core counter-release methods are accounting mechanics, not
physical-disposal evidence; only the reviewed native adapter may decide when
its represented native resources were actually disposed.

The simpler alternative is a native-owned ledger in KFD with runtime forwarding
native budget configuration and inert usage observations. That fits N1-N9 and
avoids an extraction, but does not cover H4-H11, non-KFD Context adapters or host
results that outlive native teardown. Adding another runtime ledger later would
leave cross-owner admission and aggregate quarantine nontransactional. Extracting
one engine is therefore preferred for the stated complete profile; the minimal
native-only alternative is valid only as a named partial scope, not MEM-5 closure.

Neither design may call runtime callbacks while holding native/account locks,
import runtime-private tokens into KFD, or accept a caller-provided `safe`,
`disposed` or byte-count assertion as native evidence.

### Exact Account Domains

The proposed root creates private, nonwrapping identities for its ledger,
physical-device account, Context account and native-session binding. The runtime
maps its existing `RuntimeDeviceIdV1` to an exact account handle; the numeric
`get()` value alone is insufficient. Each native session stores its expected
handle/domain and derives the selected device/VM association from its actual
retained authorities, not supplied addresses or an observation digest.

A domain consists of the exact root identity, device parent, Context child and
native-session generation where applicable. These are opaque root-issued
capabilities, not public scalar constructors. Reconfiguration cannot replace a
nonempty/ambiguous domain. A later Context cannot reset the physical-device or
root parent to evade retained usage. A same-root cache transfer across children,
if later supported, must atomically transfer child custody without refunding
the device/root total; this is not part of the first adapter.

Every reservation binds its account handle, record generation, complete cost
vector and immutable native plan. KFD's private bundle owns the retained token
alongside the actual native allocation. No public lease method returns the
accounting token separately while native storage remains live. Applications may
observe costs or configure a new permitted domain, but cannot substitute its
account into an existing session, clone a retained debit, mint a native cost
certificate, or authorize disposal. Shared-ledger release mechanics remain a
reviewed adapter contract: a counter transition alone does not prove that the
kernel or OS freed anything.

### Before-Effect Transaction

1. Derive a bounded immutable cost plan inside the owning layer from actual
   profile/layout arithmetic and the complete retained owner roster. For cache
   hits identify the existing debit instead of planning another allocation.
   Include lazy first-use queue/control creation and initialization scratch.
   An ordinary Context allocation currently can call `ensure_sdma_queue_v1`;
   MEM-2 alone cannot claim this whole path's budget without MEM-3's setup plan.
2. Pre-reserve account/native/Context metadata and result-token storage. For
   multiple backing objects, reserve all cost items, owner generations and
   record slots atomically under one ledger transaction, including device/root
   limits. No earlier debit survives a rejected whole-vector preflight.
3. Before `reserve_va`, anonymous mapping, BO ioctl or another potentially
   effectful construction call, consume the cancelable reservation into a
   retained attempt guard. The guard is already registered and needs no
   allocation to preserve its charge on failure or unwind.
4. On success, move the guard's debit into the exact native owner without
   post-success metadata growth. On definite no-effect rejection, refund the
   exact unissued attempt. After entry into a possibly effectful boundary,
   missing handles, malformed results, panic and unknown currentness retain the
   attempt in quarantine. Partial construction may conservatively retain the
   whole planned bundle until explicit successful cleanup; it must not discard
   unrepresented backing merely because no public handle exists.
5. Checkout, mapping, initialization, publication, demotion and cache recycling
   transfer the existing backing debit. Occupied use slots have separate exact
   generation-bearing credits. Native release returns only the charges whose
   complete low-level disposal protocol succeeded. Failed teardown retains the
   unreleased subset, or conservatively the whole bundle, without duplicate
   refund or retry permission for indeterminate native transactions.

R67 currently has one-vector/one-record reservation, not the proposed
parent-account transaction or divisible native cost bundle. MEM-3 batch plans
need a proved fixed-roster batch reservation/splitting operation before partial
successful disposal can refund separate objects. Do not simulate that feature
by reserving independent objects sequentially and claiming whole-plan atomicity.
The first MEM-2A adapter can remain one-backing-object-at-a-time and explicitly
leave compound creation open.

### Bootstrap And Quarantine

The root itself consumes memory before it can issue accounts. Define one fixed,
precharged bootstrap arena/layout and include it as baseline usage before child
or native admission. Child-account arenas and free lists must reserve their
full actual capacity from that parent before construction; account creation
cannot recursively mint uncharged ledgers. Bound root/device/Context record
counts, threads and metadata growth as well as payload bytes. A fixed-layout
slab/mapping is preferable where portable container APIs cannot expose a
defensible allocation bound.

The root must outlive all native, reply, journal and quarantine owners. The
bounded profile's repeated-Context path must reuse this root, not create fresh
unlimited roots. Reserve worst-case terminal bookkeeping before native effects,
or retain it within already charged ownership records. Saturation rejects new
admission; it never licenses freeing uncertain storage. Process teardown is a
terminal disposition of the whole profile, not a refund permitting continued
admission in the same process.

Current generic errors, closures, wakers and panic payloads can own unmeasured
storage, including deliberately forgotten panic payloads. A bounded profile
must use closed bounded error/result/control representations and specify how
user callbacks are isolated, rejected, or terminally contained. It cannot promise
unlimited in-process recovery from arbitrary unmeasured panics under a fixed
ceiling. Driver-internal and OS-object overhead also needs a bounded admitted
contract, or the claim must remain expressly narrower than a total-memory bound.

## Implementation Handoff And Gates

| Packet | Ownership boundary | Required gate |
| --- | --- | --- |
| Shared ledger/interface decision | Primary approves crate/dependency changes, domain API, projection units, bootstrap strategy and schema/proof changes; resource owner drafts isolated implementation | Preserve existing Context behavior/tests; no second ledger, upward dependency or forgeable native authority. Parent/root admission and failures must be executable transitions, not caller claims |
| MEM-2A/B | Resources owns isolated cost/lease module; native owner hands off `shared_memory.rs`, SDMA and pool paths; primary integrates Context/native session configuration | Actual padding, before-effect rejection, exact same-debit cache reuse, generation exhaustion, failed trim and unwind; native source/receipt tests must show no omitted allocation |
| MEM-3A/B | Native owns queue/completion/dispatch publication boundaries; resources supplies batch/slot accounting; primary integrates shared hooks | Full lazy setup roster, backing-versus-slot conservation, exact reader retention, acquisition order, saturation and ambiguous publication. SCALE-CAP waits for this gate |
| MEM-4A/B | Resources owns proposed runtime residency module; native owner integrates materialized code/control leases | Repeated loads and cache replacement bounded; truthful GTT/VRAM placement; no live-code eviction, identity substitution or failed-unload refund |
| MEM-5 integration | Primary composes host/native root; resource, admission and native owners integrate their own measured data owners | Closed command/result surface, actual metadata capacity, retained outputs, simultaneous failures and repeated Context creation stay within root/device ceilings |
| VER/DRN follow-through | Resources owns journal/lease storage; admission owns DRN-1 capture/GEN-2 result custody; primary owns shared Context hooks | Final MEM-5 closure occurs after these concrete storage/lifetime costs are integrated, not before future journal/capture allocations exist |

Each packet supplies focused adapter tests, production-used transitions,
positive obligations and named mutations for omitted cost items, duplicate
backing charges, missing cached residency, wrong parent/domain, partial debit,
premature refund and quarantine escape. Review allocation/drop/unwind code as
well as successful ownership transfers. Full reference-executor conservation,
native extraction and observed disposal correspondence remain PRF-1 composition
work; proofs of vector arithmetic alone do not establish them.

## Earlier R67 Evidence

The current all-feature library gate passed 527 runtime tests and 693 model
tests, with two existing model ignores, including all ten Context admission
tests and nine credit-adapter tests. R67's positive source verifies fourteen
obligations. The full authenticated proof gate passed 56 positive sources,
1,330 obligations and 640 expected negatives, including R67's eight mutations;
source/inventory, pinned release closure and exact transcript checks passed.
This proves neither the proposed hierarchy nor native extraction, whole-account
ownership or the reference executor. The R66 qualifier now
contains requested-credit saturation/retention/logical-disposal checks, but
there is no new signed hardware result for native physical budgets.

## MEM-2A Evidence

The [R68 local record](evidence/local-r68-native-backing-2026-09-10/README.md)
passes 2,065 runtime tests on each of GNU and musl, with five existing ignores,
plus host, fixture, doctest, lint, dependency and lockfile gates. The full
authenticated proof run passes 57 positive sources, 1,334 obligations and 645
expected negatives. R68 adds only four projection obligations and five named
mutations. Public XGMI wiring remains source-tested rather than live-qualified;
the configuration transfer/loan/retake round trip lacks an integrated regression.
The shared GPU remained busy, so no new hardware stage or qualifier was started.

Existing native layout, pool, slot and teardown tests are reusable evidence
inputs. They are not tests of the proposed ledger extraction, parent limits,
native debit composition or global quarantine. This packet itself is a
source-reviewed design and inventory; it runs no Cargo, Verus or hardware test.
