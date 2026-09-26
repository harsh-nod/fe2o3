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

Runtime pipeline storage can also allocate 1024 entries against the shared
account and moves its debit with the table during lane swaps. The public runtime
constructors still allocate the default 64-entry tables. There is no runtime
1024-launch opt-in yet.

## Accounting Boundary

The new debit covers the requested slot-table payload, not allocator overhead,
allocator-internal rounding, the account's own arena, fixed owner metadata,
other maps, GPU backing, or aggregate process memory. Default tables remain
unaccounted, matching the previous default admission surface. Native backing
continues to have separate owners and disposal rules.

Two scaled runtime lanes would require two runtime tables and two native tables
at steady state, with separately reserved replacement tables during transitions.
Sharing the account handle does not share or duplicate any table's debit.

These limits do not make every credit rejection retryable. Initial/auxiliary
DATA initializers can precede the epoch-table reservation. Exhaustion after
initial-binding admission or pristine-continuation consumption follows the
existing terminal-custody policy; it does not restore the prior live queue.
Budgeting must include replacement peaks. Moving this reservation ahead of all
native preparation, with proved rollback, remains further admission work.

CPU tests use production construction/preparation sequencers with simulated
native operations. Their epoch reservations are not GPU publication receipts.
The retained [development results](evidence/dev-scale-cap-storage-2026-09-25/README.md)
describe the exact checks and limitations. Existing ledger proofs do not establish
formal refinement of the new allocator adapter or scaled queue composition.

## Remaining Work

1. Wire a fallible vecadd-only runtime opt-in before native activity. Both runtime
   tables and native construction must share one account without transient
   unaccounted default-table allocation.
2. Remove the qualification profile's runtime bottleneck at the current 256
   retained owners per allocation, with bounded/accounted custody storage.
3. Reject unsupported generated, device-local and persistent paths before their
   callbacks or side effects. Generated preparation/adoption bypass the ordinary
   launch gate and require independent guards.
4. Finish aggregate backing/control/slot admission and scaled formal refinement;
   preserve the default capacity-65 negative and existing 64-only proofs.
5. Measure at least 2048 simultaneously native-published/retained epochs across
   two lanes with exact identities, negative saturation, complete cleanup and
   matched HIP/HSA baselines. Retained, incomplete and physically concurrent
   counts must remain separate.
