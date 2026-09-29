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
The unsafe native caller must also retain the actual cloning thread until the
child terminates: Linux ties `PDEATHSIG` to that thread, and process-ID readback
alone does not enforce its lifetime.
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
slot. A fresh-domain record additionally performs one bounded aggregate
kill/empty/removal step. Pending and quarantined records retain capacity, descriptors and any
unverified inherited artifact-spawn lease. `ECHILD` never counts as success.

Controller loss preserves the original account and records. Recovery does not
reset limits or denial history. Native issuer and external-anchor root coordinator
launches use this pool. The V1 root child above retains its synchronous Drop semantics.

The native pool also retains a close-only deployment guard until confirmed empty
shutdown. Its metered `try_clone_deployment_guard` transfers a CLOEXEC alias for
validation against the caller's actual native lifecycle lease. Both original
ledgers pay before duplication; refusal, unwind and controller recovery retain
the pool's guard. A clone is not deployment authority and must never be unlocked
or passed to uncontrolled code.

`reserve_launch_retaining` and `StagedProtectedServiceExecV2::spawn_retaining`
put complete dependencies in that same slot before process creation. The pool
funds their full retained storage independently of the request's typed view.
Deferred or quarantined cancellation keeps them alive; exec confirmation does
not release them. Terminal retirement drops dependencies outside both pool locks,
so one dependency can defer another child without a second reaper or deadlock.

`RootOwnedRetainedServiceChildV2<T>` preserves typed, metered read-only access
through an exclusive mutex. Inputs need `Send`, not `Sync`; existing `Cell`-based
owners keep their programming model. A panicking access poisons later access.
Caller-declared storage and generic values are not admitted authority. The unsafe
boundary requires complete charges and bounded, funded, nonpanicking destruction.
The native compiler coordinator now transfers its actual complete preparation
through this primitive. Contextual admission remains that coordinator's duty,
not a property established by the generic wrapper.

## Shared Readiness

`launch_io` supplies the one bounded profile/gate/readiness/status scheduler to
root coordinators. It returns inert bytes and either one or zero descriptor
rights; family-specific decoding remains in the owning coordinator. Attempt
charges, finite retries, deadlines and descriptor disposal share one implementation.
The anchor uses 16 bytes plus one right; native supervisor transport supports
88 bytes without rights. Coordinators supply nominal decoding and admission;
transport alone does not establish a successful protected launch.

Terminal status enables Linux `SO_PASSCRED` so even a queued empty record has a
kernel credential marker, while EOF has none. The marker is framing, not identity
authority. Socket-option refusal fails closed without a hangup-only fallback.
Local live tests cover readiness transfer and descriptor cleanup; the terminal
credential tests still encounter sandbox EPERM. Protected startup remains an
independent validation gate.

The shared pre-exec gate reader permits at most 64 attempts, retrying only EINTR.
Capability-ceiling observation uses a fixed buffer and finite reads. These bound
logical attempts, not blocking syscall duration. Their inert results grant no
deployment or execution authority.

## Native Spawn Mechanics

`native_spawn` exposes unsafe mechanical staging and spawning, not an admitted
deployment API. The trusted caller must derive full source charges from the
actual owners, retain the admitted source owners, and validate the final staged Files
against native image, context, key and lifecycle owners before spawning. Inert
descriptor bindings and caller-supplied storage numbers establish no authority.
Redundant temporary transfer Files may close after final validation; retire their
charges only after closure. Keep native owners and contexts through spawn.

Staging duplicates the bounded table at or above FD 400 using fallible allocation.
The original ledger prepays parent and bounded child setup, then reserves cleanup
capacity and an artifact-spawn lease before clone. The atomic pidfd, lease and
reservation enter one move-only child guard before any fallible parent check.
Cancellation and Drop take one prepaid cleanup step and defer unresolved custody
to the shared pool, with no blocking wait, retry loop or fresh ledger.
Only verified exec or complete terminal disposal releases the spawn lease.
Deferred or quarantined leases can delay artifact-lock descriptor release
indefinitely; finite cleanup funding does not guarantee eventual reaping.

The optional unsafe `spawn_retaining_in_fresh_domain` extends the same path with
atomic `CLONE_INTO_CGROUP` placement into a newly created, root-controlled cgroup
v2 domain. The original reserved slot owns rollback before mkdir. Clone failure
or unwind can defer a domain-only record; once a child exists, retirement requires
both its actual consuming terminal wait and aggregate-empty domain removal.
Neither a killed root child nor a successful `cgroup.kill` write suffices.
All retained inputs and original service charges survive unresolved cleanup.

This is mechanical custody, not proof isolation. The administrator caller must
exclude competing privileged mutations and expose no cgroup control or migration
path to the child. Existing
service spawns retain their no-domain behavior; nothing silently enables a proof
child dumpability exception or changes a compiler authority gate.

`spawn_retaining_in_fresh_user_namespace` additionally uses `CLONE_NEWUSER` on
that same gated cgroup path. Before the child drops privileges, a separate mapping
gate holds it while the parent validates the actual nsfs handles, root ownership,
parent namespace, exact child pidfd and unchanged PID/time namespaces. Single-use
identity maps contain only root, helper and peer IDs. `setgroups` stays `allow`
so the existing profile can empty supplementary groups. Exact map readback must
succeed before release. The namespace owner enters complete child custody before
any fallible map work and remains retained through aggregate cleanup, including
partial configuration and quarantine. All work and storage are prepaid on the
existing request and cleanup accounts.

These namespace mechanics do not admit a protected proof helper. Administrator
provenance, exclusion of external memory and backing-file writers, authenticated
helper/compiler bootstrap, and production activation remain unfinished. No public
safe FD/PID constructor can manufacture the concrete namespace owner.

The native external-anchor coordinator now calls these primitives after deriving
full charges and validating staged Files, and owns gated readiness/exec/endpoint
admission and managed lifetime. The native compiler coordinator now also composes
retained spawn, nominal readiness and continuity. Native inherited root descriptor
composition also exists; installed native activation and protected startup remain
unfinished/unvalidated. Rootless clone and
cleanup tests do not establish successful protected startup. See the
[compiler launch checkpoint](../../docs/evidence/conditional-native-compiler-launch-20260926.md).
