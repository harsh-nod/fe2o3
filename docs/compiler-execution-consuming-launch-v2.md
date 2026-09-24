# Native Consuming Issuer Lifecycle

`ProtectedIssuerSupervisorV2::launch` consumes `PreparedProtectedIssuerLaunchV2`
through the same clone3, descriptor-installation, pidfd and cleanup engine as V1.
Native admission never constructs an admitted V1 owner or falls back to V1 on
refusal. Shared V1-named static wire and termination records are inert data.

This is a library lifecycle, not activation of the native production issuer or
service. All four isolated distinct-UID synthetic consuming fixtures passed on
MI350 under Ubuntu 24.04/Linux 6.8.0-124-generic through the actual static launcher.
Protected proof execution, GPU qualification, M0-M7 acceptance and 47/47 completion
are not established by this implementation or its deterministic unit tests.

## State and Custody

1. **Launch:** revalidate supervisor and prepared inputs; capture the exact
   native process profile, owned SIGCHLD disposition and namespaces; prepay
   parent/child protocol work and cleanup capacity before clone. Stage descriptors
   once and adopt the atomic pidfd, slot and original spawn lease immediately.
2. **Gated child:** require its complete private namespace report and EOF,
   revalidate child/parent profiles, namespaces and prepared authority, then
   release the exec gate.
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
It also includes the fresh child's ten open/stat/close namespace observations,
PID observations and report staging. Each parent attempt reads at most once;
even a queued complete report requires a separate attempt to observe EOF.
Native observation and authority revalidation costs are additional nested costs,
not hidden inside the outer launch constant.

Deadline checks precede observations. A successful atomic publication or
consuming terminal wait remains successful even if the subsequent clock read
would be late: externally committed outcomes cannot be undone by a timeout.
Other observations remain subject to post-observation deadlines.

These are logical resource bounds, not instruction, allocator, generated-stack,
RSS, kernel-memory, scheduler, mutex latency or wall-time guarantees.

## Namespace Observation

The locked profile makes the child nondumpable and the supervisor unprivileged.
Direct parent access to `/proc/<child>/ns/*` can therefore fail the kernel's
ptrace-style permission check. An isolated Ubuntu 24.04/Linux 6.8 diagnostic
reproduced `EACCES` while parent and child self-observations succeeded and matched.

The shared direct-child stub now freshly opens all ten namespace links under
`/proc/thread-self` after its exact profile check. A fixed 192-byte little-endian
report binds child and parent PIDs and every namespace device/inode pair. Both
the original and staged pipe writers close before the gate wait. The parent
requires exact framing, EOF, unchanged PID/time child namespaces, exact PID
bindings and equality with its freshly revalidated calling-thread baseline.
Current profile status also uses `/proc/thread-self/status`, not the group leader.
The proc-visible child profile checks remain in place.

Report bytes alone grant no authority: trusted pre-exec code, a private pipe,
atomic pidfd custody, the closed gate and the coordinated spawn contract supply
their provenance. An extra writer or incomplete observation causes bounded
refusal. No tracing capability, dumpability relaxation, legacy fallback, new
process engine or replacement request ledger is introduced. Explicit remote
namespace APIs retain their existing permission checks and refusal behavior.

## Isolated Validation

The four opt-in `native_consuming_test_process::native_consuming_` tests exercise
the public native lifecycle with separately measured, sealed static test issuers:

- Exact child-produced readiness, publication to the real submitter, a client
  stop packet and natural exit with status zero.
- A complete readiness frame without EOF, refused at the readiness bound.
- Trailing readiness bytes, refused before a ready owner exists.
- Dropping launched custody before a silent child produces readiness.

Every case checks original-ledger continuity, funded cleanup, descriptor and
pidfd restoration, terminal reaping and all fixture-role completion packets.
The isolated run uses distinct UIDs 65532/65533/65534, one CPU, no network or GPU
devices, and unconfined container seccomp for clone3. The supervisor and child
still enforce the exact locked process profile. Container and private scratch
removal are checked afterward. This does not qualify a deployment seccomp policy.

Build the four fixtures with `bash scripts/build-native-ready-fixture.sh DIR`,
where `DIR` is an existing empty caller-owned mode-0700 directory. The script
prints the four `FE2O3_NATIVE_READY_FIXTURE_*` paths. Supply those paths and
`FE2O3_STATIC_PREEXEC_LAUNCHER` to the supervisor test executable in a disposable
root container with KILL/SETUID/SETGID/SETPCAP bootstrap capabilities, then run:

```sh
FE2O3_RUN_NATIVE_CONSUMING_SUPERVISOR_V2_TEST=1 "$SUPERVISOR_TEST_BIN" \
  native_consuming_test_process::native_consuming_ \
  --ignored --nocapture --test-threads=1
```

These test issuers never sign, compile requests, create durable state or recover
a service. Positive fixture stages allow 30 seconds because unoptimized custody
repeatedly measures full static images; the initial five-second fixture deadline
expired at the exec boundary. Successful gated launch took 5.8-5.9 seconds on the
one-CPU run. Production limits, exact profile checks and finite attempt budgets
were unchanged. The missing-EOF case retains its separate 200ms refusal bound.

## Remaining Production Work

The deployed issuer and service still use the previous family. Native inherited
admission, readiness production, service/durable handlers, independent broker
occurrence observation, ACK/anchor ordering, Cargo recovery, runtime/finalizer
artifact joins and provisioning must be integrated before producer activation.
See [prepared custody](compiler-execution-prepared-launch-v2.md),
[publication](compiler-execution-publication-v2.md) and
[cleanup](compiler-execution-cleanup-custody.md) for those boundaries.

Deterministic coverage checks fixed framing, finite attempt schedules, committed
outcomes, budget refusal, nested restoration, owner moves/unwind and single-step
mechanics. Separate locked-profile and calling-thread/leader namespace fixtures
also pass, but do not establish their combined divergent-thread consuming case.
Malformed gated-child report and post-clone budget-failure cleanup need additional
process integration coverage. Neither these fixtures nor their unit tests replace
protected source/proof, production service/recovery or GPU qualification.
