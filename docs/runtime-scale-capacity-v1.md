# Accounted Dispatch Capacity Development

This is partial SCALE-CAP implementation with a bounded native retained-depth
witness, not complete SCALE-CAP, A1/A2 acceptance or HIP/HSA parity. The default
remains 64 epochs per native lane.

## Implemented

`fe2o3-resource-accounting::HostMetadataTableV1` provides fallible fixed-capacity
host tables. It reserves checked `len * size_of::<T>()` ControlResidentBytes
before allocation or initialization, checks the resulting Rust capacity, and
exposes only slices. Each independently allocated copy needs its own debit.
Allocation/initializer failure refunds unissued credit; normal destruction
destroys the table before refunding retained credit. A panicking element
destructor quarantines the debit. Leaking the table does not refund it.

The closed native profiles are Default64 and Qualification1024. Construction of
the latter requires `fe2o3-kfd/scale-qualification` and a shared resource account.
The configuration is immutable on a queue family. Fresh and bootstrap primary
queues, initial binding, auxiliary lanes, detached rebind, recycled replacement,
and pristine abort/resume preserve it. A pristine continuation must match both
profile and ledger identity before consumption. Multi-packet recipes and
persistent-compute binding are rejected in the qualification profile; persistent
inputs remain retryable under the existing nonterminal rejection contract.

Native epoch and runtime pipeline slot identities use u16. The default native
observation digest retains its old encoding. Qualification1024 uses a distinct
domain, the exact capacity, and a two-byte slot identity. Existing native ring,
signal, ownership, and currentness checks are not relaxed.

With `fe2o3-runtime/scale-qualification`, the separate
`KfdRuntimeBackendV1::open_gfx942_vecadd_scale_qualification_v1(device_unique_id,
host_table_budget_bytes, max_host_table_reservations)` constructor admits only
the existing exact HostVisible vecadd fixture. It fallibly allocates both
1024-entry runtime pipeline tables against one private account before opening
KFD, then propagates that account/profile through both native startup routes.
The table debit moves with lane swaps. Existing constructors remain default-64;
there is no arbitrary-authority scaled setter. The feature also enables
`hardware-qualification` and `fe2o3-kfd/scale-qualification`.

Scaled per-allocation custody preallocates exactly 1024 owners, reserves the
entire requested payload before allocation, never grows during admission, and
retains its debit until the final owner is removed and storage is disposed.
Partial multi-allocation preflight drops/refunds only provisional new rosters.
This is a per-allocation total across compute and SDMA, not 1024 per lane for
shared allocations. Default custody remains incrementally allocated with its
256-owner bound; explicit dependency and XGMI limits are unchanged.

Scaled generated preparation, shell admission and lane readiness independently
reject before callbacks or mutable admission work. DeviceLocal allocation and
persistent-compute preparation also reject early; HostVisible allocation still
uses the native SDMA bootstrap. The original exact fixture authority continues
to reject other modules/invocations and atomic/collective launches.

## Shared Host Account

The opt-in
`open_gfx942_vecadd_repeat_scale_qualification_with_shared_host_account_v1(device_unique_id,
account)` constructor accepts an existing `ResourceCreditAccountV1`. Runtime
pipeline tables, native epoch host tables, per-allocation custody tables and
retained kernarg/binding slices compete for that same account's byte and record
limits. A domain child preserves its exact leaf and ancestor limits, including
charges from other participants. No new account, domain or execution authority
is created. The constructor still admits only the exact repeat-vecadd profile.

Both runtime pipeline tables are reserved before opening KFD. Later table
replacement peaks and every independently owned payload require their own
credits. Payload Arc aliases retain one charge through final storage disposal;
capacity refusal does not reclaim quarantined credit. Existing constructors
keep their previous table-only or separately attached payload accounts.

With this constructor, `scale_qualification_host_table_usage_v1()` and
`scale_qualification_launch_payload_account_usage_v1()` observe the same account.
Do not sum those two readings: they overlap, include other account participants,
and are sampled independently. Zero usage is not a native quiescence witness.

Signed component `838f1e8a1`, integrated at `43d97c8fa`, binds the exact tested
source bytes. Its development checks pass 33 scale CPU tests and 11 payload CPU
tests, with three unchanged native cases ignored, warning-free no-default
compilation, strict Clippy and scoped formatting. Execution preceded signing;
this is not a merged full-runtime, native, formal or performance qualification.
The shared-account adapter's formal refinement and a total-memory bound remain
open.

## Accounting Boundary

The new debit covers requested slot-table and custody-roster payloads, not
allocator overhead, allocator-internal rounding, the account's own arena, fixed owner metadata,
other maps, GPU backing, or aggregate process memory. Default tables remain
unaccounted, matching the previous default admission surface. Native backing
continues to have separate owners and disposal rules.

Two scaled runtime lanes require two runtime tables and two native tables at
steady state, plus each live per-allocation custody roster and separately
reserved replacement tables during transitions. The configurable record ceiling
also bounds simultaneous debits. Sharing the account handle does not share or
duplicate any table's debit. `scale_qualification_host_table_usage_v1()` returns
an inert snapshot of this ledger; it is not total process or GPU memory usage.
Pipeline debits remain after native shutdown until backend/table destruction.
The table-only constructors do not charge retained launch payloads. The optional
payload account charges kernarg/binding slice extents, not other launch metadata,
maps, provisional roster-index vectors, Arc headers or allocator overhead.
Even the shared host account is not an aggregate process-memory ceiling.

Qualification1024 now owns its epoch table before native preparation entry and
transfers that same table into preparation. Initial and auxiliary binding reserve
before DATA callbacks; rebind reserves before consuming the pristine continuation
or moving DATA into preparation. Primary and replacement construction retain the
preallocated owner with their consumed inputs. Default64 preserves its prior
allocation timing. Pre-entry exhaustion avoids new native mutation but does not
return consumed inputs or establish retryable Context launch admission. In
particular, auxiliary initializer captures rejected by the epoch-table preflight
remain deliberately retained, not disposed or returned. Later failures retain
existing terminal custody. See the
[owned-preflight development packet](evidence/dev-epoch-preflight-2026-09-25/README.md).

The runtime now reserves `Gfx942FixedDispatchPreallocationV1` after the ordinary
publication path's attached-reuse decision, before recycled detach, admitted-device
consumption or resident DATA movement. Fresh primary, bootstrap initial binding,
auxiliary construction and live rebind consume that same allocation without a
second debit. Default64 and attached reuse require no new reservation. The
borrowed lane producer selects the exact primary/auxiliary queue without swapping
or consuming owners. An attached recycled binding requires simultaneous credit
for its old table and the reserved replacement.

This token conveys vacant storage and credit, not execution authority. Rebind
tokens bind queue, ledger, profile and next generation, not one unique pristine
continuation. Handoff revalidates current state; existing native currentness checks
remain intact. Only `HostAllocationCapacity` from this explicit runtime preflight
becomes nonterminal `Capacity`. An already accepted submission settles `Failed`,
not synchronous Context launch rejection or automatic retry. Earlier runtime
staging can already synchronize allocations or release other-lane caches; this is
not a side-effect-free public submission guarantee. Aggregate backing/control/slot
admission, retryable Context admission and scaled formal refinement remain open.
See the [runtime preallocation packet](evidence/dev-runtime-epoch-preflight-2026-09-26/README.md)
for exact CPU coverage and outstanding native gates.

CPU tests use production construction/preparation sequencers with simulated
native operations. Their epoch reservations are not GPU publication receipts.
The retained [development results](evidence/dev-scale-cap-storage-2026-09-25/README.md)
describe the exact checks and limitations. Existing ledger proofs do not establish
formal refinement of the new allocator adapter or scaled queue composition.
The [runtime integration results](evidence/dev-scale-runtime-2026-09-25/README.md)
are a separate development packet; simulated lane occupancy is not native depth.

The scale-only ignored [retained-depth canary](evidence/dev-scale-depth-2026-09-25/README.md)
now inspects every original native receipt at 1,024 epochs per lane, joins exact
runtime ownership and complete profiler metadata, and checks cleanup through
backend destruction. It separately probes runtime custody saturation and actual
native epoch-table saturation on both lanes. CPU mutation tests validate the
typed consistency checker. The corrected
[native campaign](evidence/dev-native-depth-budget-2026-09-28/README.md) now passes
this cell and signed-source/ELF-bound replay: 2048 distinct retained receipts,
6179 profile events, complete output checks, separate runtime/native saturation
refusals and backing/table cleanup. The requested host-table payload peaks at
4,489,216 bytes/10 records and ends at zero. Private custody comparisons remain
in-process assertions; unfinished-kernel count and physical overlap are unmeasured.
Repeated reuse/rebind, async-owner high-depth integration and formal native
refinement remain open.

## Remaining Work

1. Finish aggregate backing/control/slot admission and scaled formal refinement;
   preserve the default capacity-65 negative and existing 64-only proofs.
2. Extend the passing opt-in retained-depth witness to scaled joins/rebinds,
   repeated reuse and async-owner integration with signed native evidence.
   Keep the default qualification profile intact.
3. Add matched HIP/HSA baselines and separately measured incomplete/concurrent
   counts to the 2048-retained-epoch witness. Retained, incomplete and physically
   concurrent counts must remain separate.

The public same-buffer 1025th-launch rejection occurs at runtime custody
admission, before native epoch reservation. It is not a native epoch-table
capacity-negative result. Keep those two negative qualification cells separate.
