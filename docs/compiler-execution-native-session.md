# Native Issuer Session Custody

The nominal V2 and V3 issuers share one native dispatcher. Its local session
retains the actual `NativeOccurrence`, including both its publication lease and
currentness token, from Prepare until durable publication. A signed journal is
recoverable data, not a replacement for that live owner.

## Request Invariants

| State | Required live custody | Allowed transition |
| --- | --- | --- |
| Ready | No pending local occurrence | Observe once and prepare |
| Prepared | Same occurrence and exact subject | Compare request and issue |
| Issued | Same occurrence, subject, and request | Replay or durably publish |
| Published | Both local lock owners dropped | Acknowledge or recover carriage |

Issue never re-observes a replacement occurrence. Inspect, Issue replay, Recover,
currentness verification, and subsequent Prepare all pass the session/journal
join. Recovered Prepared or Issued state without the original live owner fails
closed. Cancellation and terminal failure drop the local session.

A private borrowed `PublicationGuard` validates actual service and occurrence
custody around the external-anchor exchange and each durable publication phase:
prepared anchor, anchor receipt, Worker, published anchor, and issuer advance.
Before returning Published, the dispatcher checks the exact durable carriage,
revalidates custody, and drops the entire occurrence. Dropping only the lease
would leave its token retaining the writer lock.

Per-packet scopes restore entry storage. The outer loop retains the full live
occurrence charge between packets and releases it only after destruction. Work,
resource-denial history, deadline, and I/O limits are never reset.

## Remaining Integration

This is the **local issuer-owned lifecycle**, not completed root/issuer control.
The original-root observation API exists, but its connection to this dispatcher
still requires an authenticated private channel and retained root-owned
occurrence. In that design, an empty issuer session cannot prove root retirement.
The root must acknowledge exact, idempotent retirement before Published,
Recovered, currentness, or a new Prepare can pass. Losing the issuer or its reply
must not discard independently retained root custody.

The channel must be created after compiler clone/exec and never inherited by the
compiler. Each message needs actual sender credentials, manifest/policy/launch
binding, sequence checks, and cumulative resource bounds. Readiness must precede
synchronous observation. Existing public handoff and UID rules remain unchanged.

Journal fixture tests do not construct an `Admission` or `NativeOccurrence`.
They test refusal predicates and durable recovery, not actual-owner lock lifetime,
protected proofs, GPU execution, or the 47-kernel release gate. Those require
the integrated production attempt and target-matched qualification described in
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
