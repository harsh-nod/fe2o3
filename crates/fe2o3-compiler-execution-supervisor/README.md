# fe2o3 compiler-execution supervisor

This package owns the protected process boundary around the static
compiler-execution issuer. Program admission authenticates the provisioned
static launcher and issuer before either can enter authority-bearing custody.

## Native Custody Status

`AdmittedIssuerProgramV2` freshly consumes a native policy capability and the
provisioned launcher/issuer sources. It uses bounded native executable admission,
checks runtime then launcher then issuer, and retains independently sealed
objects. Every nested operation uses the same caller ledger; exact clone checks
reject equal bytes in a different inode. Supplied sources may alias because the
retained images are separate objects. Trusted provisioning must independently
pin the policy and launcher measurement.

`ProtectedIssuerSupervisorV2::bind` consumes that native program, its policy-bound
native signing key, the strict native anchor transport and a service-owned root.
It checks current effective UID/GID, program, full-policy key binding, anchor,
root admission and full revalidation in that order. Root admission requires
CLOEXEC, read-only non-O_PATH directory custody, exact UID/GID, mode0700, nonzero
links and no capability or POSIX access/default ACL. V1 and V2 share this root
predicate; the native owner never converts an admitted V1 owner.

Binding alone is pre-session custody. `accept_handoff` now consumes an actual
control connection and authenticates its canonical native frame, submitter,
service socket, policy/anchor identities and one retained live client pidfd.
Its move-only accepted result exposes only immutable facts. Shared socket
predicates preserve the V1 checks; native receive uses fixed buffers and finite
attempts. Unsupported ancillary messages fail closed with descriptor cleanup.

`prepare_launch` now consumes the native accepted handoff into move-only
`PreparedProtectedIssuerLaunchV2`. It retains exact native descriptor transfers,
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
These are launch prerequisites, not a native consuming process-launch API. See
the [process-observation contract](../../docs/compiler-execution-process-observations-v2.md).

`ProtectedIssuerCleanupServiceV2` now funds the existing fixed cleanup pool from
one persistent owned ledger. Native turns prepay bounded work, retain cumulative
history, and use the same cleanup engine as V1. Controller Drop/recovery retains
the account and records; only empty orderly shutdown releases pool storage.
Native and legacy modes are mutually exclusive. A prepaid launch reservation
is capacity only, not an enabled native child launch. See the
[cleanup custody contract](../../docs/compiler-execution-cleanup-custody.md).

Full child confinement, consuming native process creation, readiness,
serving/recovery and producer activation remain open. Every nested check uses
the caller's ledger; reserve returned growth while preserving consumed input
reservations. Logical work/retained/scratch charges are not wall-time, RSS,
kernel-memory or generated-stack bounds. See the
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
profile, reports through a private gate, and cannot execute until the parent
independently rechecks the profile, all ten namespaces, and the complete
prepared authority set. It then isolates standard streams, installs the
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
