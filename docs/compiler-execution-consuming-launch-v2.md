# Native Consuming Issuer Lifecycle

`ProtectedIssuerSupervisorV2::launch` consumes `PreparedProtectedIssuerLaunchV2`
through the same clone3, descriptor-installation, pidfd and cleanup engine as V1.
Native admission never constructs an admitted V1 owner or falls back to V1 on
refusal. Shared V1-named static wire and termination records are inert data.

This is a library lifecycle, not activation of the native production issuer or
service. A full isolated native child-to-readiness run remains required.
Protected proof execution, GPU qualification, M0-M7 acceptance and 47/47 completion
are not established by this implementation or its deterministic unit tests.

## State and Custody

1. **Launch:** revalidate supervisor and prepared inputs; capture the exact
   native process profile, owned SIGCHLD disposition and namespaces; prepay
   parent/child protocol work and cleanup capacity before clone. Stage descriptors
   once and adopt the atomic pidfd, slot and original spawn lease immediately.
2. **Gated child:** observe profile acknowledgement, revalidate child/parent
   profiles, namespaces and prepared authority, then release the exec gate.
   Exec-status EOF is not issuer readiness and does not release the spawn lease.
3. **Ready:** receive exactly one 120-byte native readiness frame followed by EOF,
   revalidate capability and supervisor, and match the exact PID, manifest and
   policy. Reserve readiness retention and observe liveness before releasing the
   spawn obligation. Close all unused parent writers before expecting EOF.
4. **Serving:** publish one atomic readiness packet over the retained Cargo
   control socket, then close that endpoint. No compiler/signing authority is
   exposed by the resulting process owner.
5. **Exited:** consume exactly one terminal pidfd wait and disarm custody before
   fallible result handling. The returned PID, readiness and termination are
   inert, and retain their original funding until dropped.

Consuming failure or cancellation performs at most one prepaid emergency cleanup
step. Pending or uncertain custody transfers into the existing persistently
funded pool. Later Drop cannot repeat the step. ECHILD and missing pidfd remain
quarantine, not proof of terminal exit. Explicit funded service turns must drain
deferred custody; no native background worker is silently started.

## Resource Lifetime

The session exclusively borrows the original request `Budget` and borrows its
native supervisor. Callers cannot retire, replace or reuse that budget while any
launched, ready, serving or exited owner remains live. Every nested check uses
that same ledger; cleanup service work uses its separate persistent account.

Let `B0` include prepaid supervisor, prepared and unrelated input reservations,
`Dp` be the full consumed prepared charge, `F = size_of(Session)` and `r` the
native readiness retention. Conservatively:

| Boundary | Request storage after success |
| --- | --- |
| Launched | `B0 + F` |
| Ready, serving, exited | `B0 + F + r` |
| Final Drop or funded consuming refusal | `B0 - Dp` |
| Invalid incoming floor | `B0`, unchanged |

Temporary scopes reserve scratch in addition to the full retained floor. Staging
reserves another `Dp` before duplicating executable descriptors, covering their
complete logical image charges while original inputs coexist. It is released
after staged descriptors close. Foreground metadata and persistent pool capacity
are both charged during overlap. Readiness growth is reserved after its temporary
scope restores the entry floor. Owners close or transfer before funding retires;
no consumed reservation is released inside a protected temporary scope.

Work is cumulative and never refunded. `ProtectedIssuerWaitV2` accepts 1-4096
attempts and a positive timeout up to 120 seconds. EINTR, EAGAIN, partial reads
and pending observations consume finite attempts. Four weighted syscall units
per parent attempt cover I/O, liveness, failure probes and optional polling.
Child prepayment includes all 64 gate attempts and bounded capability checks.
Native observation and authority revalidation costs are additional nested costs,
not hidden inside the outer launch constant.

Deadline checks precede observations. A successful atomic publication or
consuming terminal wait remains successful even if the subsequent clock read
would be late: externally committed outcomes cannot be undone by a timeout.
Other observations remain subject to post-observation deadlines.

These are logical resource bounds, not instruction, allocator, generated-stack,
RSS, kernel-memory, scheduler, mutex latency or wall-time guarantees.

## Remaining Production Work

**Known launch prerequisite:** source review identified a conflict in the gated
cross-process namespace observation. The exact profile requires a nondumpable
child and an unprivileged supervisor, but opening `/proc/<child>/ns/*` requires
ptrace-style access. Linux applies that check in its
[namespace-link implementation](https://github.com/torvalds/linux/blob/master/fs/proc/namespaces.c).
The expected permission refusal still needs a dedicated isolated reproduction
and a reviewed observation mechanism that preserves the locked profile. Do not
make the child dumpable, retain ptrace privileges or bypass the check to claim
success. Existing self-process profile fixtures do not exercise this boundary.

The deployed issuer and service still use the previous family. Native inherited
admission, readiness production, service/durable handlers, independent broker
occurrence observation, ACK/anchor ordering, Cargo recovery, runtime/finalizer
artifact joins and provisioning must be integrated before producer activation.
See [prepared custody](compiler-execution-prepared-launch-v2.md),
[publication](compiler-execution-publication-v2.md) and
[cleanup](compiler-execution-cleanup-custody.md) for those boundaries.

Deterministic coverage checks fixed framing, finite attempt schedules, committed
outcomes, budget refusal, nested restoration, owner moves/unwind and single-step
mechanics. It is not a substitute for executing the complete native lifecycle
under distinct credentials and the enforced production process profile.
