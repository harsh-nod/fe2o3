# Bounded Scalar XGMI Progress

Development toward A1/A2 of #182, not milestone, native, formal-refinement or
performance acceptance. The separate two-device XGMI backend remains copy-only.

## Contract

`KfdNativeXgmiRuntimeBackendV1::progress_peer_copy_v1(submission)` performs at
most one scalar progress action per call:

- one FIFO publication attempt containing at most 63 ready copies;
- one observation of an already published ticket; or
- settlement of one copy whose retained dependency has failed.

The selected copy may be a transitive dependency or an unrelated published
copy occupying the same directional FIFO. The return value describes only the
requested submission. Completing an intermediate copy does not complete its
consumer. Recovered publication failures of unrelated prefix members do not
fail the requested submission. Native error classes pass through unchanged;
an error creates no additional release or quiescence authority.

Selection is iterative, with at most 256 levels and 256 dependencies per level.
Each visited scalar node revalidates dependency identity, unique edges, retained
status, nonzero retention, exact admitted depth and its successful cached prefix.
A failed dependency takes precedence over an earlier pending dependency.
Malformed state terminates the backend rather than repairing indexes or
granting publication authority. Dependency consumers remain in the active table
while selection and leaf callbacks run. The old unticketed consuming helper
only reinserts its owner and returns Pending; it cannot recursively poll,
publish or enqueue.

An active ordered-segment root is unsupported. An ordered dependency must have
its exact directional owner marker and leaves the scalar consumer Pending until
the existing ordered progress path advances it. Scalar and ordered submissions
cannot share a direction under the existing admission rules.

Ordinary scalar `poll_v1` and `wait_v1` do not publish. Scalar flush is still
nonwaiting complete-ready-set publication, rejecting overflow above 63 and a
busy nonempty publication window. Callers needing partial FIFO advancement use
the new inherent backend method, not repeated oversized flushes.

## Index And Cost Boundaries

An explicit FIFO-membership flag lives in each existing active submission;
there is no extra membership allocation. Admission, dependency wakeup, prefix extraction,
prefix restoration, aggregate execution, ordered execution and settlement keep
the flag and FIFO paired. Repeated readiness insertion is idempotent without
changing FIFO order. Shutdown and Drop already reject retained active owners
and nonempty FIFOs. A separate HashSet was rejected during development because
deletion can reduce its reported insertion capacity and invalidate a restoration
guard, despite earlier admission reservation.

Selection uses expected constant-time hash lookups, not a scan of the ready
backlog. Dependency uniqueness uses bounded stack storage and sorting. Waiting
and in-flight settlement skip the unrelated ready FIFO. Removing an arbitrary
ready item for cancellation still has the existing linear FIFO cost.

Before native effects, the selected in-flight window and publication prefix
receive bounded index/dependency checks. This is not a full scan for arbitrary
corruption elsewhere in the backlog; the complete FIFO/flag invariant depends
on private mutation discipline and is separately audited by aggregate admission.

One action is not a constant-time or hard real-time promise. Publication still
performs native admission and mapping work and allocates bounded batch scratch.
Completion can process its admitted waiter roster, dependency indexes and
allocation owners. Native calls, hash-table worst cases and driver scheduling
are not given hard wall-clock bounds by this API. There is no added sleep,
completion wait, recursive traversal, background thread or thread per copy.

## Validation Boundary

The [CPU checkpoint](evidence/dev-xgmi-scalar-progress-cpu-2026-09-23/README.md)
records both full runtime-library targets, doctests and static checks.

The CPU tests use the production selector and driver with scripted adapters and
the shared ready-index helpers. They cover both directions, FIFO windows and
tail submissions, blocking tickets, consumer-only chains, diamond joins,
failure precedence, one-step failure propagation, callback errors/unwinds,
retained identities, depth/roster bounds, hostile indexes and ordered isolation.
Fixtures carry no GPU authority. Callback-unwind tests stop at the scripted
adapter boundary, before any move-only native custody is consumed. Native
custody-consuming unwind retains the existing fail-stop policy, not resumable
panic recovery. The tests do not qualify Linux, mapping restoration,
ticket ownership under actual native faults, or machine execution.

No new Verus theorem or executable/native correspondence follows from these
tests. The constructor-origin lifecycle proof packet remains a separate,
source-bound result. This API does not add Context pending-producer read
authority, Worker transport or a protected generated-kernel path.

Next is exact Context producer/dependency retention and producer-first outcome
reconciliation, followed by native consumer-driven chains in both directions,
full data/canary checks, cleanup evidence and separate refinement qualification.
The Native R125, Admission R118B and Resources R116/V3 checkpoints remain the
broader accepted lane boundaries.
