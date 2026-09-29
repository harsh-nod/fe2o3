# fe2o3 compiler-execution supervisor

This package owns the protected process boundary around the static
compiler-execution issuer. Program admission authenticates the provisioned
static launcher and issuer before either can enter authority-bearing custody.

## Native Custody Status

`run_inherited_protected_issuer_service_v2/v3` now compose the actual native
deployment through finite dispatch in dedicated V2/V3 binaries. The fixed eleven
descriptor roles are consumed with bounded cleanup. Native policy, contextual
deployment, process/namespaces, running image, root-bound lifecycle, signing key,
program and anchor admission precede listener activation and exact readiness.
No V1 admitted authority is upgraded. Full incoming image ownership and duplicate
overlap are charged on the original account.

Before any child launch, a separately charged lifecycle-lock alias moves into
the existing persistent cleanup pool. Late failure, unwind, controller Drop,
quarantine and work exhaustion cannot release it; only confirmed empty shutdown
closes it. This is process-lifetime custody, not protection against process death
or a substitute for the root coordinator's independent lease and recovery.

Build the selected static binary with
`scripts/build-static-compiler-execution-supervisor.sh v2` or `v3`; omitting the
argument keeps V1. This does not provision or activate a deployment. Native
root inherited composition and consuming supervisor launch remain unfinished,
and successful protected native startup has not been validated. See the
[startup checkpoint](../../docs/evidence/conditional-native-supervisor-startup-20260926.md).

`ProvisionedProtectedIssuerServiceInputsV2` admits the fixed production listener
and service-owned root on the original resource ledger. It shares the legacy
filesystem/socket predicates, pins both objects, permits bound-to-listening
continuity, and refuses further transfer after activation. It can validate the
exact final staged listener/root Files without duplicating them. Returned owner
growth or cloned-pair charges are unreserved; retain the original owner and its
charge through final validation. This policy-neutral custody is usable by both
native families but grants neither compiler authority nor provisioning provenance.
It is a prerequisite for native compiler-coordinator integration, not that
integration itself. See the
[transfer checkpoint](../../docs/evidence/conditional-native-transfer-20260926.md).

`AdmittedIssuerProgramV2` and `AdmittedIssuerProgramV3` each freshly consume their
own native policy capability and the
provisioned launcher/issuer sources. It uses bounded native executable admission,
checks runtime then launcher then issuer, and retains independently sealed
objects. Every nested operation uses the same caller ledger; exact clone checks
reject equal bytes in a different inode. Supplied sources may alias because the
retained images are separate objects. Trusted provisioning must independently
pin the policy and launcher measurement.

`ProtectedIssuerSupervisorV2::bind` and `ProtectedIssuerSupervisorV3::bind`
consume their respective native program, its policy-bound
native signing key, the strict native anchor transport and a service-owned root.
It checks current effective UID/GID, program, full-policy key binding, anchor,
root admission and full revalidation in that order. Root admission requires
CLOEXEC, read-only non-O_PATH directory custody, exact UID/GID, mode0700, nonzero
links and no capability or POSIX access/default ACL. V1 and V2 share this root
predicate. V2 and V3 reuse the same admission mechanics but retain distinct
authority types; neither converts an admitted owner from another family.

Binding alone is pre-session custody. `accept_handoff` now consumes an actual
control connection and authenticates its canonical native frame, submitter,
service socket, policy/anchor identities and one retained live client pidfd.
Its move-only accepted result exposes only immutable facts. Shared socket
predicates preserve the V1 checks; native receive uses fixed buffers and finite
attempts. Unsupported ancillary messages fail closed with descriptor cleanup.

`prepare_launch` now consumes the native accepted handoff into move-only
`PreparedProtectedIssuerLaunchV2` or `PreparedProtectedIssuerLaunchV3`, matching
the supervisor and accepted handoff family. It retains exact native descriptor transfers,
a fresh native service-launch capability, seven pipe ends and a sealed 704-byte
static manifest binding the current parent and twelve ordered source roles.
Revalidation repeats native owner/transfer continuity, parent, pipe, metadata,
canonical-byte and non-aliasing checks. The shared `StaticPreexecManifestV1`
codec has fixed-capacity inert storage: its V1 wire name is not a conversion
from admitted V1 authority. Native manifest I/O is finite and fixed-size.

Process-profile and namespace observations now have native metered owners in
`fe2o3-protected-service-profile`. They use the same bounded, allocation-free
predicates as the existing service path. The supervisor no longer has its own
profile parser or namespace implementation. Descriptor staging is also one
shared fixed fourteen-entry table, with deterministic cleanup on partial failure.
See the [process-observation contract](../../docs/compiler-execution-process-observations-v2.md).

`ProtectedIssuerCleanupServiceV2` now funds the existing fixed cleanup pool from
one persistent owned ledger. Native turns prepay bounded work, retain cumulative
history, and use the same cleanup engine as V1. Controller Drop/recovery retains
the account and records; only empty orderly shutdown releases pool storage.
Native and legacy modes are mutually exclusive. Consuming native launch transfers
the prepaid reservation into the shared child owner. See the
[cleanup custody contract](../../docs/compiler-execution-cleanup-custody.md).

`ProtectedIssuerSupervisorV2::launch` consumes its prepared custody into separate
launched, ready, serving and exited states over the existing clone3/pidfd engine.
It requires the protected process profile, gates exec on native revalidation,
and prepays finite child and parent attempts. Exact native readiness and EOF
precede spawn-lease release; publication and terminal reaping are distinct steps.
The session exclusively borrows the original request ledger until its final Drop.
Both families share one lifecycle implementation and the policy-neutral profile,
cleanup, and static-launcher machinery. Each retains its own admitted owners and
readiness type. Shared wire framing is not authorization: readiness must match
the exact child PID, launch manifest, and policy of the same family.
See the [consuming-launch contract](../../docs/compiler-execution-consuming-launch-v2.md).

V3 indirect launch now refuses before payload extraction, cleanup-slot allocation
or clone, including the funded path reached through `run_session`. Its mandatory
issuer FD12 root-control channel is supplied only by the coordinator's private
direct-root launch primitive. The supervisor's own FD12 remains its lifecycle
lock. Restoring the indirect route requires actual original-root custody and
channel transfer; there is no fallback or substitute descriptor. See the
[root-control contract](../../docs/compiler-execution-root-control.md).

`ProtectedIssuerSupervisorV2::run_session` composes handoff, preparation, consuming launch, readiness, publication and
terminal wait without replacing the original request ledger. Each retained
growth is reserved while its owner is guarded; refusal closes owners before
retiring their adopted charges. Native session limits preserve distinct finite
waits, not a conversion of V1's 24-hour policy. The returned exited owner still
borrows the account. The service controller retains responsibility for pumping
the independently funded cleanup pool. This is a session API, not listener or
deployment activation; its complete protected execution is not yet validated.

```rust
use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as Supervisor,
    ProtectedIssuerCleanupServiceV2 as Cleanup, ProtectedIssuerSessionLimitsV2 as Limits,
    ProtectedIssuerSessionErrorV2 as Error, ExitedProtectedIssuerV2 as Exited};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
fn session<'a, 'w>(s: &'a Supervisor, control: std::os::fd::OwnedFd,
    cleanup: &mut Cleanup, limits: Limits, budget: &'a mut Budget<'w>)
    -> Result<Exited<'a, 'w>, Error>
{
    // Supervisor and consumed control storage must already be prepaid.
    s.run_session(control, cleanup, limits, budget)
}
```

Production native issuer/service/recovery integration and producer activation
remain open. Four isolated distinct-UID **V2** synthetic consuming cases passed on
MI350: ready/publication/natural exit, missing EOF, trailing bytes, and
Drop-before-readiness cleanup through the actual static launcher.
V3 consuming custody has local deterministic lifecycle and exact-readiness-join
tests, plus a historical opt-in distinct-UID consuming fixture using the V3 public
APIs and a separately built synthetic static issuer. The fixture covers
publication/exit, missing EOF, trailing data and drop-before-readiness cleanup;
negative cases require the public readiness peer to close without publication.
No isolated V3 child-launch or protected-runtime execution is credited yet. Those
positive indirect V3 fixtures cannot pass the current preclone refusal and are
not current acceptance evidence; their source is retained for route migration.
Separate opt-in V2/V3 fixtures now call the public `run_session` entry point for
success, missing EOF and trailing data. They do not inject production stage hooks:
the submitter verifies actual publication and asks the original client to stop.
These new session fixtures are compiled but not yet credited as isolated runs.
All four V3 static fixture modes now pass build-time ELF checks; none was executed
for that validation. See the [session/listener evidence checkpoint](../../docs/evidence/conditional-native-listener-20260926.md)
for exact results and remaining gates.

`ProtectedIssuerServiceV2` and `ProtectedIssuerServiceV3` now consume the actual
native supervisor plus the fixed-path bound listener. Shared descriptor/path
predicates and activation preserve V1 behavior without admitted V1 policy owners.
Native `serve_one` prepays finite accept turns, calls the same family's
`run_session`, retires session custody, then revalidates service continuity on the
original ledger. Only inert terminal observations escape. This is single-dispatch
support, not an installed native worker pool, provisioning or recovery path.
Named-socket validation remains required; local `bind` is refused with EPERM in
the current restricted environment. No listener-to-issuer execution is credited.
Native `run_turns` adds a finite sequential controller over that same service.
It uses the original request account and exclusive persistent cleanup controller,
requires funding for the selected cleanup pump before accepting another session,
and pumps cleanup after every outcome, including request-budget refusal. The first
non-idle dispatch error stops the call; its exact error and any simultaneous
cleanup failure are returned separately. The fixed result counts earlier
completions and retains only the last completion observation. Caller-owned cleanup
must remain available for further draining; batch completion does not prove an
empty pool. This is not deployed native provisioning or recovery.
Every nested check uses the caller's ledger. Logical
work/retained/scratch charges are not wall-time, RSS, kernel-memory or
generated-stack bounds. See the
[prepared-launch contract](../../docs/compiler-execution-prepared-launch-v2.md),
[handoff contract](../../docs/compiler-execution-handoff-v2.md),
[binding resource contract](../../docs/compiler-execution-capabilities-v2.md#native-supervisor-binding)
and [anchor custody contract](../../docs/compiler-execution-anchor-custody-v2.md).
No protected proof or GPU qualification is credited; M0-M7 and 47/47 remain incomplete.

The existing serving/deployment path described below remains **V1**.

## Existing Service Path

Both source images are read through stable file descriptions, checked against
exact SHA-256 and length measurements, validated as loader-independent x86-64
ELF images, copied into distinct anonymous mode-0555 memfds, sealed with
`WRITE`, `GROW`, `SHRINK`, `EXEC`, and `SEAL`, reopened read-only, and measured
again through the shared protected-static-executable custody contract. The
issuer must match the exact executable and runtime measurements in the sealed
caller policy. The launcher measurement belongs to trusted service provisioning
and is never accepted in a per-launch request.

The admitted program is move-only and exposes no descriptor. A second move-only
state now binds that exact program and policy to the canonical signing-key
capability, a dedicated non-root UID/GID profile, a retained service-owned
root, and one supervisor-provisioned external-anchor endpoint plus its live
service pidfd. Root admission requires a close-on-exec read-only directory descriptor,
exact owner UID and GID, mode `0700`, nonzero link count, and no file capability
or POSIX access/default ACL. Program, policy, key, service identity, and root
security metadata are all revalidated together. External-anchor admission
requires an unnamed connected nonblocking Unix `SOCK_SEQPACKET`, exact pinned
peer UID/GID, an exact live peer-process pidfd, and a service UID distinct from
the issuer UID. The key, root descriptor, source paths, and signing operation
remain inaccessible.

The credential profile fixes the eventual child state: equal
real/effective/saved/filesystem IDs, no supplementary groups, empty capability
sets including bounding and ambient sets, locked `NOROOT`, locked set-ID fixup
and ambient-capability prevention, `no_new_privs`, nondumpability, zero core
limit, umask `077`, and unchanged supervisor namespaces. This checkpoint binds
the configured effective UID/GID and now accepts one authenticated cross-process
rustc handoff. The handoff is one canonical direct-parent/launch-manifest packet
with exactly two `SCM_RIGHTS` descriptors. Admission requires the control
socket's exact submitter PID/UID/GID, nested policy, rustc service-peer
`SO_PEERCRED`, pidfd target/liveness, descriptor identities, and all role
non-aliasing checks to agree; all observations are repeated without exposing a
descriptor. The nested launch manifest now also carries the canonical
external-anchor service UID/GID selected by the root-owned client profile.
Handoff admission requires those credentials to equal the identity of the
supervisor-provisioned anchor endpoint before any launch material is created.

One accepted handoff can now be consumed into a move-only prepared launch. The
supervisor clones and revalidates the exact launcher, issuer, root, service
peer, rustc pidfd, policy, signing key, sealed service-launch capability,
external-anchor endpoint, and external-anchor pidfd;
creates distinct nonblocking close-on-exec pipes for stdin, stdout, stderr, and
readiness; and constructs the fixed twelve-entry source table for issuer FDs
`0..=11`. It binds that table and the issuer image to the current supervisor PID
and exact procfs start time in one canonical 704-byte manifest. The manifest is
stored in an anonymous read-only mode-`0400` memfd with exact `WRITE`, `GROW`,
`SHRINK`, and `SEAL` seals. Revalidation repeats all authority, client-liveness,
capability, access-mode, object-snapshot, byte, parent-continuity, and role
non-aliasing checks. The retained stdout, stderr, and readiness readers remain
private, and no prepared value exposes a descriptor.

Production launch consumes that prepared state through one `clone3` call with
exactly `CLONE_PIDFD | CLONE_CLEAR_SIGHAND` and `SIGCHLD`. Every launcher input
is first duplicated above FD 215. The direct-syscall child resets signals,
arms and verifies `PDEATHSIG=SIGKILL`, self-checks the inherited service
profile, freshly observes all ten namespaces, and emits one fixed private report.
Both report writers are closed before the gate wait. It cannot execute until the
parent requires report EOF, exact PID and calling-thread namespace matches, proc-visible profile
checks, and the complete prepared authority set. No tracing privilege or
dumpability relaxation is needed. It then isolates standard streams, installs the
manifest at FD 198, issuer at FD 199, sources at FDs `200..211`, and executes
the authenticated static launcher with one fixed argument and an empty
environment.

The move-only result has three states. `LaunchedProtectedIssuerV1` owns the
atomically returned close-on-exec pidfd but grants no issuer authority.
`await_readiness` accepts exactly one canonical record followed by EOF, binds
it to that PID, launch manifest, and policy, and returns
`ReadyProtectedIssuerV1` only while the same pidfd child is live. Wrong,
truncated, extended, stale, or timed-out readiness fails closed. A consuming
publication sends those exact bytes once over the authenticated Cargo control
connection, closes that endpoint, and returns `ServingProtectedIssuerV1`
while retaining the same pidfd. A closed or stalled Cargo peer fails closed
before serving custody exists. Serving custody can be consumed by one bounded
pidfd wait that returns an inert PID/readiness/termination record only after
`waitid(P_PIDFD)` has reaped the exact child once. A wait timeout fails closed
and cancels that child. Explicit cancellation uses finite nonblocking cleanup
attempts; success means a confirmed terminal reap. Timeout and inconclusive
errors retain custody in the fixed 64-slot reaper. Failed termination requests
remain retryable; ownership loss is quarantined rather than reported as reaped.
The pre-exec artifact-spawn lease follows that same custody until validated
issuer readiness or a terminal reap. See the [cleanup contract](../../docs/compiler-execution-cleanup-custody.md),
including the remaining native accounting requirements.
Abrupt supervisor death is covered both by the bootstrap gate
and the static launcher's parent identity check.

`ProtectedIssuerSupervisorV1::run_session` is the sole complete per-connection
operation. It consumes one accepted listener connection through handoff
authentication, launch preparation, gated static exec, issuer readiness, Cargo
readiness publication, bounded serving, and natural-exit reaping. Its trusted
timeout policy fixes a separate absolute bound for every stage, and its error
preserves the exact failed stage. No intermediate move-only state or descriptor
escapes this operation; failure at any stage closes or cancels all later
custody.

`ProtectedIssuerServiceV1` consumes that supervisor together with the sole
production listener at
`/run/fe2o3/compiler-execution-supervisor.sock`. Root provisioning admits an
exact bound, non-listening, nonblocking close-on-exec Unix `SOCK_SEQPACKET` with
no connected peer or pending socket error. Service binding performs the sole
`listen(2)` call after the supervisor has entered its protected identity, then
revalidates the stable descriptor and filesystem-socket identities. The
retained pathname policy additionally requires a root-owned
mode-`0755` `/run/fe2o3` without POSIX ACLs or file capabilities and a
root-owned, deployment-service-GID, mode-`0660` socket with the same metadata
exclusions. Each accept operation waits under one absolute bound and uses
`CLOEXEC | NONBLOCK`, repeats listener and supervisor validation around the
accept, and dispatches the control descriptor directly into `run_session`.
Alternate production paths and caller-visible accepted descriptors do not
exist.

The public service operation is a consuming fixed worker loop. Its validated
worker count is between one and the same 64-process custody limit used by the
pidfd reaper. Each worker accepts directly from the retained listener and can
enter only `run_session`; a bounded completion channel returns inert completed
or stage-typed rejected outcomes to the owner thread. Session rejection does
not stop unrelated clients. Listener, supervisor, worker, or channel failure
requests global stop and is returned after all workers join. An authority-free
cloneable stop handle provides graceful shutdown with a one-second maximum
idle accept observation; active sessions retain their trusted timeout bounds.

The launcher deliberately inherits an already established profile instead of
performing privileged credential transitions after `clone3`. Deployment must
therefore start the supervisor under the dedicated UID/GID with empty groups
and capabilities, exact locked securebits, `no_new_privs`, nondumpability,
zero core limits, umask `077`, default owned `SIGCHLD`, and stable namespaces.
Cargo-wrapper fixed-path service acquisition is implemented. Deployment must
also provision the supervisor with the already connected external-anchor peer
and matching live pidfd; neither Cargo nor rustc can select or replace them.
The descriptor-only deployed entrypoint is implemented and accepts no arguments
or environment. It consumes the canonical deployment manifest at FD 220 plus
fixed inherited bound socket, root, launcher, issuer, policy, root-owned signing-key
template, external-anchor descriptors, and an independent shared lifecycle
lease at FD 12; validates the complete locked service
profile; invokes `listen(2)` only after entering the protected supervisor UID,
reissues the exact policy-bound key template into a fresh anonymous
service-owned sealed image only after the deployment UID/GID and policy agree;
requires that manifest to pin the exact protected-supervisor executable as a
role distinct from the issuer pre-exec launcher; and enters only the existing
fixed-worker service loop. The systemd, sysusers, tmpfiles, and reference anchor
definitions exist; installed root/distinct-UID qualification remains pending.
The reviewed supervisor image is built
and checked as a loader-independent static executable by
`scripts/build-static-compiler-execution-supervisor.sh`.

The same private root bootstrap is inherited at FD 11 and must be an unnamed,
connected, nonblocking Unix `SOCK_SEQPACKET` whose exact direct parent has root
credentials. After all deployment inputs and service authority bind, the
supervisor publishes the canonical PID/deployment readiness record under a
fixed bound, closes the bootstrap, and only then enters the worker loop.

The lifecycle lease is bound descriptor-relatively to the canonical root-owned
sibling of the retained service root and is revalidated before readiness. It is
held through the fixed-worker loop and released only by process descriptor
close, preserving provisioning exclusion if the root coordinator is killed.

Root-side provisioning reuses the same listener and durable-root validators
through one move-only `ProvisionedProtectedIssuerServiceInputsV1`. It admits the
fixed listener pathname and exact target-service ownership without changing the
root coordinator's identity, retains descriptor and filesystem snapshots, and
permits only one consuming ordered listener/root transfer. The deployed process
independently repeats those checks after entering the locked service profile.
