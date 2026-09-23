# Issuer Cleanup Custody

This contract covers the shared cleanup mechanics and their integration into
the existing V1 supervisor. It does not enable native V2 process launch, finish
#271/#272, or qualify a kernel for protected proof or safe GPU execution.

## Ownership Before Clone

The supervisor reserves one of 64 cleanup slots before creating a child. It
also acquires a move-only `ArtifactProcessSpawnLeaseV1` from the existing artifact
transaction coordinator. This is the same coordinator used by artifact-lock
descriptor release, not another lock protocol.

Immediately after successful clone, the parent adopts the child identity,
atomic pidfd, slot and spawn lease before any fallible validation. The foreground
owner is the only consuming waiter until it transfers the complete cleanup
record to its reserved slot. The worker cannot consume a foreground owner's
wait. No new slot or thread is allocated during that transfer.

Artifact custody is retained through launch, error and unwind. The supervisor
releases the spawn lease only after validated issuer readiness or its own exact
terminal wait. The former is a post-exec witness: the pre-exec child routine
cannot emit that readiness record. An exec-status pipe EOF alone is insufficient
because descriptor closure is sequential; Linux may reschedule during the
[CLOEXEC descriptor sweep](https://github.com/torvalds/linux/blob/master/fs/file.c).

The checked public lease acquisition returns a fixed count-overflow error.
Moving a lease across threads keeps its original coordinator obligation. A
forked copy checks process identity before touching an inherited mutex and
cannot decrement the parent's count. Lease ownership is not execution authority.

## Finite Cleanup Steps

`ChildCleanupV1::step` performs at most one pidfd `SIGKILL` and one consuming
`waitid(EXITED | NOHANG)`. It never loops, allocates or falls back to signaling a
scalar PID. A failed signal remains eligible for a later retry. An accepted
signal, including `ESRCH`, is not proof of terminal reaping.

| Observation | Disposition |
| --- | --- |
| Exact consuming terminal wait | Release spawn obligation and retire custody once |
| Pending, interrupted or uncertain wait | Retain descriptor, slot and unresolved spawn obligation |
| `ECHILD` or missing atomic pidfd | Quarantine; retain capacity without further signals or waits |

Drop performs one cleanup step and transfers any unresolved record. Explicit
V1 cancellation allows at most 1,024 steps, checking a two-second deadline
between them. Only confirmed terminal reaping returns success; otherwise cleanup
is retained and cancellation returns an error. Launch failures preserve their
original error after cleanup is attempted.

Each legacy worker pass visits each deferred record once. Quarantined records
are not recycled. Capacity exhaustion refuses new launches before clone. The
fixed pool stores owned records directly, including the transferable lease;
there is no raw-descriptor reconstruction in its worker. Accidentally dropping
an unresolved private record preserves its resources rather than asserting
disposal, but such an unreachable leak is not a substitute for the pool's
reachable custody or an acceptable native accounting mechanism.

## Remaining Limits

These are finite syscall-attempt and capacity bounds, not hard elapsed-time
bounds. Syscalls, mutex acquisition and scheduling can take unbounded time.
Artifact-lock release still waits for outstanding spawn obligations: code must
not wait for its own retained lease. The legacy worker is still periodically
scheduled without explicit logical work grants and therefore is not native
prepaid cleanup. A quarantined record requires outer service recovery; no API
currently certifies its release.

The native consuming API still needs a persistent service storage reservation,
explicit cumulative cleanup-work grants, pre-clone emergency/finalization
permits, finite parent/child protocol envelopes and a service shutdown/recovery
owner. A request-local scratch scope or expired ledger identity cannot pay for
cleanup that outlives that request. Native V2 launch remains unavailable until
these obligations are integrated with the existing engine.

## Tests

Fake syscall schedules cover failed kills, interruptions, uncertain waits,
ownership loss, terminal classification, missing pidfd, transfer and exactly-once
lease retirement. Coordinator tests cover cross-thread transfer, multiple
leases, unwind, overflow refusal and origin-PID handling. Pool tests cover
reservation rollback, pending/quarantined capacity and terminal slot reuse.
These are cleanup tests, not protected-runtime or GPU qualification evidence.
