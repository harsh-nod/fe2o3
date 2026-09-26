# fe2o3 protected-service spawn

This package owns the one root-to-protected-service process transition used by
fe2o3 deployment coordinators. It stages one caller-admitted executable and a
bounded, destination-unique descriptor table above every admitted target,
requires the root parent to own `SIGCHLD`, creates the direct child with
`clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)`, and performs only direct syscalls
in the post-clone child. Executable measurement, ownership, sealing, and static
ELF admission remain mandatory policy checks in the calling coordinator.

Before reporting profile readiness, the child resets signals, binds
`PDEATHSIG=SIGKILL` to the exact parent, installs the dedicated UID/GID with no
supplementary groups, empties and locks all capability paths, sets
`no_new_privs`, nondumpability, a zero core limit, and umask `077`, and reads
every property back. Credential transitions clear the parent-death setting, so
the child rearms it and rechecks the exact parent after installing the profile.
The parent must independently validate the child and its
namespaces before sending the one-byte release token. Only then does the child
install the fixed descriptor table and execute the staged image with one fixed
argument and an empty environment.

The V1 returned move-only child retains the atomic pidfd and exact reaping
ownership. Dropping it kills and synchronously reaps the child. The package
does not interpret service protocols, manifests, keys, paths, compiler data,
publication evidence, or GPU authority.

## Native Cleanup

`ProtectedServiceCleanupServiceV2` owns funding for the single process-global
64-slot cleanup pool. It was moved here from the issuer supervisor; the old
`ProtectedIssuerCleanup*V2` names are aliases, not a second implementation.
Native mode excludes the legacy background worker. Each explicit funded turn
makes at most one pidfd signal and one nonblocking consuming wait per visited
slot. Pending and quarantined records retain capacity, descriptors and any
unverified inherited artifact-spawn lease. `ECHILD` never counts as success.

Controller loss preserves the original account and records. Recovery does not
reset limits or denial history. Native issuer launch and the trusted native root
spawn primitive use this pool. The root coordinator does not yet call that
primitive. The V1 root child above retains its synchronous Drop semantics.

The shared pre-exec gate reader permits at most 64 attempts, retrying only EINTR.
Capability-ceiling observation uses a fixed buffer and finite reads. These bound
logical attempts, not blocking syscall duration. Their inert results grant no
deployment or execution authority.

## Native Spawn Mechanics

`native_spawn` exposes unsafe mechanical staging and spawning, not an admitted
deployment API. The trusted caller must derive full source charges from the
actual owners, retain all original inputs, and validate the final staged Files
against native image, context, key and lifecycle owners before spawning. Inert
descriptor bindings and caller-supplied storage numbers establish no authority.

Staging duplicates the bounded table at or above FD 400 using fallible allocation.
The original ledger prepays parent and bounded child setup, then reserves cleanup
capacity and an artifact-spawn lease before clone. The atomic pidfd, lease and
reservation enter one move-only child guard before any fallible parent check.
Cancellation and Drop take one prepaid cleanup step and defer unresolved custody
to the shared pool, with no blocking wait, retry loop or fresh ledger.
Only verified exec or exact consuming terminal disposal releases the spawn lease.
Deferred or quarantined leases can delay artifact-lock descriptor release
indefinitely; finite cleanup funding does not guarantee eventual reaping.

Final staged-file validation, gated readiness/exec/endpoint admission and managed
native lifetime still need integration in the root coordinator. Rootless clone
and cleanup tests do not establish successful protected startup. See the
[checkpoint](../../docs/evidence/conditional-native-root-spawn-20260926.md).
