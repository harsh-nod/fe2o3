# Issuer Cleanup Custody

This contract covers the shared cleanup mechanics, the existing V1 supervisor,
and native V2 persistent pool funding. It does not enable native V2 process
launch, finish #271/#272, or qualify a kernel for protected proof or safe GPU
execution.

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

## Native Pool Account

`ProtectedIssuerCleanupServiceV2::admit` consumes an owned resource ledger and
reserves the full existing 64-slot pool. The ledger shares the canonical work
and storage implementation; it preserves a supplied work prefix and every
reached peak or denial. Temporary budget views borrow those same counters.
Neither the service nor a cleanup turn creates a replacement work meter.

Admission selects native mode permanently. The existing V1 path selects legacy
mode before starting its periodic worker. Neither can upgrade, replace or use
the other's mode. Both call the same `pump_cell` and `ChildCleanupV1::step`; native
mode does not start a second thread or pool.

Each explicit native `pump(visits)` prepays setup plus the complete allowance
for 1 through 64 cells before inspecting any cell or performing cleanup I/O.
Its cursor rotates across turns, including empty and quarantined cells. A
refused work charge leaves the cursor and children untouched and stops new
admissions. Admission also stops proactively when the remaining service work
cannot fund even one cell, including at initial admission or after recovery.
A smaller affordable turn may still drain custody, without clearing
the original denial or renewing the work limit. Quarantine never returns slot
capacity or becomes successful reaping evidence.

The move-only controller is a lease over a process-global account. Its Drop
releases only that lease; storage and deferred records remain charged and
reachable. `recover` reacquires the same account, at an explicit cumulative
cost. It accepts no new limits. Exhaustion can prevent recovery; this is a
retention guarantee, not an eventual-cleanup guarantee. There is no unmetered
fallback. After controller Drop, even empty teardown requires affordable
recovery; an unused handle-local shutdown allowance is not reusable by a later
handle. An outer supervisor remains responsible for service death or an account
that cannot make further progress.

`reserve_launch` charges the request ledger before reserving capacity and
prepays one emergency signal/wait/transfer plus finalization. It creates no
child and grants no execution authority. The capacity reservation may outlive
that request ledger: it lives in the independently funded service pool.
Discarding an unused reservation returns its slot without new work charges.
Having enough service work for one turn does not promise eventual reaping of
every retained child; later progress still depends on funding and observations.
Native consuming launch now connects this reservation to the guarded child and
finite parent/child protocol envelopes; protected deployment validation remains
a separate gate.

The pool charge includes the fixed table, controller/account metadata, embedded
records and spawn obligations, plus a logical descriptor charge per slot. It
persists for free, reserved, deferred and quarantined slots. Only an empty,
orderly shutdown releases it and returns the original account; the process
pool then remains closed. Each controller prepays its first shutdown attempt,
so an empty pool can close at its exact work limit. A busy attempt consumes that
allowance; further attempts require additional work. No handle may reset or
replace the account while retained custody exists. An admitted shutdown attempt
stops new reservations even when it finds the pool busy; pumping remains allowed.

## Retained Dependencies

The shared slot can also own complete service dependencies before clone.
`reserve_launch_retaining` consumes the fully charged input and returns a typed
view plus request GROWTH. Keep the original input reservation and reserve that
growth before retaining the view. The original service account independently
pays full payload storage and lifetime work before allocation. No replacement
ledger or second cleanup table is created.

The accounting invariant becomes `Cleanup::STORAGE + retained_payload_bytes`.
Fixed slot/controller metadata remains in `STORAGE`; dynamic payloads include
the declared transitive storage and mutex/Arc allocation overhead. The request
also pays the full payload while holding its view, since that view can outlive
slot retirement. Quota methods use checked arithmetic. Storage or work refusal
keeps original counters and denial history, stops service admission as appropriate,
and never publishes a partially funded payload.

`spawn_retaining` follows the existing root checks and clone/adoption path. Its
reservation already holds the dependencies before clone, so even a failed parent
pidfd check cannot release them while the new child is unresolved. The typed
child wrapper returns growth over the consumed input, includes the complete owner
floor in operations, and grants no admitted service authority.

An unused pre-clone reservation rolls back. After clone, only exact consuming
terminal reaping permits retirement; exec confirmation, signal success, NOWAIT,
ECHILD and Pending do not. A `RETIRING` slot stays unavailable while dependencies
drop outside both cell and account mutexes. Their destructors may therefore
defer another child into this same pool. Only afterward does accounting retire
and the slot become empty. A destructor contract violation or accounting
mismatch quarantines the slot instead of publishing reuse.

The view uses `Arc<Mutex<T>>` and requires `T: Send`, not `Sync`. Metered scoped
access borrows only `&T`; it exposes no mutable guard, raw pointer, Arc clone or
owner extraction. Panic poisons subsequent access. Recursive access to the same
view is forbidden. The trusted construction boundary still requires all
transitive resources to survive shared access and all destruction to be bounded,
nonpanicking and funded, including existing nested cancellation prepayments.
These mechanical APIs do not validate a caller's declared charge or deployment.

The native compiler coordinator now transfers its actual prepared owner, live
anchor and both lifecycle leases through this path. It validates the final
staged Files and uses nominal readiness, EOF and continuity checks before exec
confirmation. Native inherited descriptor composition now uses the same guard
before anchor launch; installed native activation and protected deployment
validation remain outstanding. The generic wrapper alone establishes neither provenance
nor successful startup. See the
[compiler launch checkpoint](evidence/conditional-native-compiler-launch-20260926.md).

## Remaining Limits

Native pool storage also prepays one close-only deployment-guard descriptor.
`retain_deployment_guard` consumes the caller-authenticated lock File and debits
both original ledgers before transfer. Only an empty, admission-open pool with
no guard accepts it; there is no detach or replacement operation. Refusal closes
the incoming File, not an existing guard. The caller retires its full input
charge after return; successful transfer remains charged to the persistent pool.
The generic cleanup API grants no deployment authority and checks no pathname.

`try_clone_deployment_guard` charges both original ledgers before one CLOEXEC
duplication at FD 256 or above. The full new File and scratch overlap are charged
on the request; persistent custody remains on the cleanup account. A controlled
clone can outlive pool shutdown and must never be explicitly unlocked. Native
anchor preparation installs its actual root-bound alias before the first child;
consuming launch validates the installed guard against the actual retained lease.
The clone itself proves neither deployment identity nor uninterrupted past custody.

Native supervisor startup supplies a duplicate of its actual admitted lifecycle
lease before any child launch. Controller Drop, late request failure, unwind,
quarantine and work exhaustion preserve that alias. Verified exec releases only
the separate artifact-spawn obligation, never this deployment guard. Empty
shutdown closes the guard only after every slot is terminal and the pool's
storage invariant holds. This custody lasts within the process; outer root
custody and recovery remain necessary for service death.

These are finite syscall-attempt and capacity bounds, not hard elapsed-time
bounds. Syscalls, mutex acquisition and scheduling can take unbounded time.
Artifact-lock release still waits for outstanding spawn obligations: code must
not wait for its own retained lease. The legacy worker is still periodically
scheduled without explicit logical work grants and therefore is not native
prepaid cleanup. A quarantined record requires outer service recovery; no API
currently certifies its release.

Native [consuming launch](compiler-execution-consuming-launch-v2.md) transfers the
funded reservation into the shared foreground child owner and prepays bounded
parent/child protocol work. Explicit cancellation and Drop share one emergency
transition; after transfer Drop cannot signal or wait again. Production service
integration must still connect funded pumping to service lifetime. A request-local
scratch scope or expired ledger identity cannot pay for deferred custody. The
V1 periodic worker remains unmetered and is never a fallback for native refusal.

The private native runtime trace adds a separate foreground retirement phase.
Cancellation first marks the original trace and retains the compiler attempt;
funded steps on the original request account consume every acquired task's
terminal wait. Only then can the attempt drop and the original cleanup pool
finish deferred domain retirement. The trace's cached `Pending` disposition is
not a later empty-pool observation. Signal restoration still requires successful
original-pool shutdown. The fixed schedule adds one immediate foreground step
and one per existing cleanup turn, without renewing a deadline or account.
Exhaustion, unresolved trace custody, or unwind retains the dedicated-process
fail-stop and outside-custodian obligation; it does not promise eventual cleanup.
This plumbing does not enable compiler execution or select the native default.

## Tests

Fake syscall schedules cover failed kills, interruptions, uncertain waits,
ownership loss, terminal classification, missing pidfd, transfer and exactly-once
lease retirement. Coordinator tests cover cross-thread transfer, multiple
leases, unwind, overflow refusal and origin-PID handling. Pool tests cover
reservation rollback, pending/quarantined capacity and terminal slot reuse.
Native account tests cover exact/short quotas, cumulative recovery, exclusive
modes, rotating funded turns, retained quarantine and empty shutdown. Owned
ledger tests cover shared accounting across views, scratch rollback/unwind and
non-escaping borrows.
Five isolated real-flock schedules check guard transfer, exclusion, refusal,
recovery, unwind and exhaustion with synthetic slot events. The existing real
child subprocess cases also check lock retention after exec, request-budget
refusal, unwind and reaping, with release only at empty shutdown. Isolation avoids
temporary lock inheritance by unrelated concurrent test forks; strict immediate
lock assertions remain intact.
These are cleanup tests, not protected-runtime or GPU qualification evidence.
