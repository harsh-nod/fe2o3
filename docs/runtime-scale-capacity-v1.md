# Accounted Dispatch Capacity Development

This is partial SCALE-CAP implementation, not hardware qualification, A1/A2
acceptance, or HIP/HSA parity. The default remains 64 epochs per native lane.

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
Launch snapshots, kernargs, maps and provisional roster-index vectors are not
covered by this payload accounting.

These limits do not make every credit rejection retryable. Initial/auxiliary
DATA initializers can precede the epoch-table reservation. Exhaustion after
initial-binding admission or pristine-continuation consumption follows the
existing terminal-custody policy; it does not restore the prior live queue.
Budgeting must include replacement peaks. Moving this reservation ahead of all
native preparation, with proved rollback, remains further admission work.
That requires an owned preallocated table, not a transient budget check, before
initial binding enters mutable work, auxiliary construction takes rooted custody,
or pristine rebind consumes its continuation. Runtime classification after logical
acceptance also needs review: a lower pre-entry rejection does not by itself prove
a rejected/retryable Context launch.

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
typed consistency checker; the native cell has not executed. Reuse/rebind,
async-owner high-depth integration and signed independent replay remain open.

## Remaining Work

1. Finish aggregate backing/control/slot admission and scaled formal refinement;
   preserve the default capacity-65 negative and existing 64-only proofs.
2. Execute the new opt-in retained-depth canary; qualify scaled joins/rebinds,
   repeated reuse, async-owner integration and exact cleanup with signed native
   evidence. Keep the default qualification profile intact.
3. Measure at least 2048 simultaneously native-published/retained epochs across
   two lanes with exact identities, negative saturation, complete cleanup and
   matched HIP/HSA baselines. Retained, incomplete and physically concurrent
   counts must remain separate.

The public same-buffer 1025th-launch rejection occurs at runtime custody
admission, before native epoch reservation. It is not a native epoch-table
capacity-negative result. Keep those two negative qualification cells separate.
