# Pending GTT Allocation Custody V1

R87 implements the pre-record allocation prerequisite of NATIVE-1-CONTROL.
It does not complete dispatch preparation or native generated adoption.
The [local evidence](evidence/local-r87-pending-allocation-2026-09-11/README.md)
retains the corrected source gates, focused tests and expected-negative mutations.

## Problem And Boundary

The previous shared GTT allocator kept the returned VA reservation, ALLOC
output and CPU mapping in local variables until all checks passed. An error or
panic could discard those descriptors before a usable allocation record existed.
Linux reservation/mapping destructors deliberately perform no implicit unmap or
FREE, so this was lost typed custody, not evidence of an observed native free.

The existing production allocator now keeps one inline pending owner in its
`SharedMemoryEngine`. No second allocator, public authority or cleanup policy is
introduced. The host-visible, executable, kernarg, AQL and private userptr profiles
continue through the same allocation sequence and profile validators.

## Transition

1. Check the existing layout, record-slot, nonwrapping identity and VA bounds.
   Reserve any applicable ordinary-host backing debit and check opening
   currentness. Rejection before a native attempt does not create pending custody.
2. Install the planned ID/profile/layout/record slot before `reserve_va`. Consume
   the ID and reserve the requested VA bound at that point, including when the
   attempt later fails. This counts reserved attempt capacity, not confirmed
   physical residency.
3. Set the attempt stage before each native call. Store every returned reservation,
   raw ALLOC output and mapping in the pending owner before checking status,
   validating fields, preparing the CPU mapping or observing closing currentness.
   Raw output retained after errno or malformed fields remains untrusted data,
   never mapped authority or permission to free a handle.
4. On error or unwind after the attempt starts, quarantine the session and keep
   pending native custody. Consume only the optional Host charge into its existing
   account's quarantine record; do not refund it. Preserve the original panic
   payload without another currentness observation or native cleanup.
5. Only after every check succeeds, move the same returned owners and charge into
   the pre-reserved completed record slot and clear pending custody. This commit
   has no native call, callback, fallible validation or growing allocation.

At most one pending allocation can exist in the exclusively borrowed engine. It
reserves an available slot within the existing 256-record admission bound and
the existing VA ceiling. Failure closes the session; pending state cannot be
overwritten, retried or promoted to a usable token. Existing fully released-slot
reuse and exact record indexing remain in place.

## CPU Coverage

Fourteen new tests use the production sequencer and actual fake-native records.
They cover non-userptr native errors/panics, all ordinary and userptr allocation
currentness positions, full raw errno/malformed output, invalid and overlapping
VA, handle/offset collisions, wrong mapped address, configured/unconfigured
failure, exact existing Host/Device records and charges, capacity/ID/VA exhaustion,
successful disposal, and successful/failed released-slot reuse at maximum capacity.

Kernarg and executable backing remain outside optional N1 ordinary-host charging.
Existing N1/N2 data charges are checked for preservation during failed control
allocation; this is not implementation of the remaining control-memory budgets.

## Open Boundaries

- Only values returned by the backend can enter session custody. Userptr
  preparation can create a mapping internally and fail before returning it; the
  session retains the actual reservation and attempted stage, not a fabricated
  mapping. That intermediate backend ownership requires a later in-place hook.
- Linux CPU-mapping preparation may explicitly restore a guard and mark its
  descriptor inactive. Retained descriptor ownership does not imply an active VMA.
- Process-aborting allocation failure and native effects whose outputs never
  return are not recoverable Rust unwinds or certified native outcomes.
- After successful allocation, model projection and later seal/map/retain steps
  still need the complete preparation owner. Packet/plan/generation, successful
  code prefixes, kernarg typestates and completed dispatch output must survive
  outer construction, closing retake and validation failure/panic.
- Both persistent bind callers must root successful dispatch and attachment
  before post-construction validation. Three-binding cancellation must not return
  a retryable input after closing-retake has already terminalized the queue.
- Full NATIVE-1/NATIVE-2 custody, Linux qualification, executable refinement,
  native adoption/publication/completion and matched performance remain open.
  Historical accounting/model proofs do not prove this new adapter.
