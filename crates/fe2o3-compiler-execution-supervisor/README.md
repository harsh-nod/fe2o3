# fe2o3 compiler-execution supervisor

This package owns the protected process boundary around the static
compiler-execution issuer. Program admission authenticates the provisioned
static launcher and issuer before either can enter authority-bearing custody.

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
and cancels that child. Explicit cancellation uses `pidfd_send_signal`; every
synchronous path reaps once, and dropped live custody transfers to a fixed
64-slot reaper. Abrupt supervisor death is covered both by the bootstrap gate
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
`/run/fe2o3/compiler-execution-supervisor.sock`. Admission requires an exact
nonblocking close-on-exec listening Unix `SOCK_SEQPACKET`, no connected peer,
no pending socket error, and stable descriptor plus filesystem-socket
identities. Each accept operation waits under one absolute bound and uses
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
fixed inherited listener, root, launcher, issuer, policy, root-owned signing-key
template, and external-anchor descriptors; validates the complete locked service
profile; reissues the exact policy-bound key template into a fresh anonymous
service-owned sealed image only after the deployment UID/GID and policy agree;
requires that manifest to pin the exact protected-supervisor executable as a
role distinct from the issuer pre-exec launcher; and enters only the existing
fixed-worker service loop. System provisioning of
the distinct service accounts, independently administered external anchor,
and root/socket policy remains pending. The reviewed supervisor image is built
and checked as a loader-independent static executable by
`scripts/build-static-compiler-execution-supervisor.sh`.
