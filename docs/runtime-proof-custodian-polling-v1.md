# Pollable Proof Custodian Lifecycle

`ProductionProofCustodianDeploymentV1::begin_launch` returns a move-only,
thread-bound `PendingRootProofControllerLaunchV1`. Its borrowed `poll` advances
one startup phase: profile acknowledgment, cgroup attachment, gate release,
exec-status EOF, then authenticated resource readiness. Each transport attempt
is nonblocking, with no readiness waits, sleeps or EINTR retry loops. Setup and
bounded filesystem admission/revalidation are still synchronous; this is not a
wait-free or hard realtime API.

`take_ready` returns `None` before readiness and moves the original controller
out once after validation. Errors keep the child, scope and installed deployment
owned. After transfer, polling, taking or cancelling the emptied pending owner
rejects: it cannot report containment of the still-live transferred child.

`RootManagedProofControllerV1::begin_proof` similarly returns a
`PendingRootConditionalFillProofV1`. One successful Start send advances to one
authenticated Proved receive. Backpressure/interruption preserves the phase;
malformed packets, rejected proof, wrong phase and validation failures poison
the owner. A borrowed take moves the same controller and matching subject into
retained custody. Copied subject bytes are not proof or application authority.

One absolute 300-second deadline covers setup, startup and proof execution.
An observed completed proof remains retained beyond that deadline. Root-only
retained probing and release still use blocking operations with fresh 30-second
deadlines. Timeout after a request poisons the channel, preventing a late response
from crossing into a new transaction. Existing synchronous launch/prove methods
drive the same pending states and retain blocking cleanup compatibility.

## Cancellation

Borrowed `poll_cancel` poisons the protocol, kills the original cgroup once,
signals/reaps the exact child through its original pidfd using `waitid(NOHANG)`,
then checks aggregate emptiness and removes only the authenticated original
scope. `false` retains remaining custody; `true` requires completed cleanup and
is idempotent only for that same, untransferred owner. Errors are sticky, including
loss of exclusive reaping ownership. Neither a later polling call nor blocking
cancel can turn an earlier cancellation failure into a success witness.

The ten-second aggregate-empty bound begins after direct-child reap. A stuck
direct child can remain pending indefinitely. A manager must retain failed or
cancelling sessions, apply its outer failure deadline, and fail-stop its own
whole-cgroup unit if containment cannot be completed. Drop remains a blocking
containment backstop and aborts if safe containment fails; discarding an active
or failed owner inside a reactor is not nonblocking cancellation.

## Scope

The owners confer root-side lifecycle custody only, not application registration,
compiler currentness, remote proof leases, GPU launch or GPU settlement. This
increment does not install a manager service or hand off Cargo's application
proof peer. The existing filtered compiler coordinator cannot launch this role.
The next integration must stage approved resources before Ready and activate
application reads only after root relinquishes its peer alias without shutdown.
