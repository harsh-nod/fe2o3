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

### Opt-In Native Compiler Child Channel

`native_spawn::StagedProtectedServiceExecV2::stage_compiler_with_child_channel`
takes the same inputs as `stage_compiler`, with an additional borrowed
`child_channel_transfer` immediately after `exec_status`. It pins that endpoint
above FD 400 with CLOEXEC and exposes `compiler_child_channel_transfer()` for
final contextual validation. Ordinary `stage_compiler` and service callers keep
their existing behavior, including ordinary binding 195 when no channel is opted
in. Opt-in staging rejects destination 195 collisions before duplicating or
cloning. Its generated destination counts toward the total 32-descriptor ceiling,
including present standard streams; a channel-only table is nonempty.

Supply a fresh, private, unnamed UNIX seqpacket control pair created by the actual
root parent. Enable SO_PASSCRED on the receiving end before clone. The caller
must validate the final staged transfer against its original control custody and
must not install either control endpoint as a compiler binding. Staging checks
type and connected unnamed addresses; these structural checks do not establish
root ownership, caller authority or deployment admission. The raw child also
requires SO_PEERCRED on the control sender to name its expected parent with root
UID/GID, after the existing complete credential drop, readback and PDEATHSIG rearm.

Only then does the child create the actual CLOEXEC seqpacket pair, so its
SO_PEERCRED records the child's final PID/UID/GID. It duplicates the client above
FD 400, closes the original client, and sends exactly one service right and the
24-byte `FE2CEC2\0` v2 record with child PID, FD195 and parent PID. One direct
`sendmsg(MSG_DONTWAIT | MSG_NOSIGNAL)` attempt is allowed, with no retry even for
EINTR. The child closes the service and staged control sender before READY. The
client stays high and CLOEXEC while gated. After release, the common close_range
and ordinary descriptor bindings run first; only then does dup3 install the
client at 195 without CLOEXEC and close the high duplicate, before cwd and exec.
Socketpair may initially return 195 or a standard stream slot without colliding
with any high staged source. No parent reservation of numeric FD195 is needed:
clone does not share the descriptor table, and the raw child creates no threads.
Failures use the existing status/exit and cleanup owner; setup uses stage byte
`0xcb`, and final installation uses `0xcc`. A failed operation can follow a
delivered transfer, so receipt or READY alone must never confer authority.

The root-level `compiler_service_channel` module exports `TRANSFER_BYTES`,
`COMPILER_SERVICE_FD`, `encode_transfer(child_pid, parent_pid)` and
`transfer_matches(&wire, child_pid, parent_pid)`. Encoding returns `Option` and
refuses zero, out-of-range or equal child/parent Linux PIDs. These are inert,
bounded wire helpers, also available as `native_spawn::compiler_child_channel`;
decoding a `Transfer` does not create process or endpoint authority. The private
coordinator receiver separately
receives with `launch_io::receive_ready_from::<24, true, _>`, binds actual
SCM_CREDENTIALS and service SO_PEERCRED to admitted credentials and its retained
original clone pidfd, checks exact child/parent wire claims and liveness, and
is exercised with gated launch and terminal cleanup by an isolated diagnostic.
The installed coordinator does not yet accept compiler attempts or compose this
receiver with approved compiler/helper custody and an issuer. Do not reopen a numeric PID or use
the public client's same-UID handoff for this distinct-UID root receipt. That
public client path is unchanged.

Accounting uses the original request ledger, with no new production account:

- `COMPILER_CHILD_CHANNEL_STAGING_WORK` adds five operation allowances and 256
  scalar work units: three socket shape observations, duplication and eventual
  or partial close. The ordinary compiler stage quota is unchanged.
- `compiler_staging_scratch_for_sources` covers either stage. `source_storage`
  must include all original control/source owners and backing allocations. The
  returned FULL retained charge includes the optional File in the staged owner;
  original owners and reservations remain independent.
- `spawn_work` and `spawn_retaining_work` add ten operation allowances, 256
  scalar units and `(24 + 24) * 64` wire/ancillary units. These cover peer readback,
  getpid/getppid, socketpair, high duplication, sendmsg, three pre-gate closes and
  the final high close. The generated descriptor's ordinary count funds its dup3
  exactly once. The common child failure suffix remains prepaid.
- `SPAWN_SCRATCH` explicitly includes four copies of the fixed payload, rights,
  msghdr, iovec, ucred, FD pair and socklen ABI storage plus 256 control bytes.
  Receipt, returned right/pidfd storage and contextual admission have separate
  coordinator charges. Quotas bound logical work/storage, not kernel socket
  memory, syscall latency, generated stack or child RSS.

Raw-child additions use fixed scalar/ABI records and direct syscalls, without
allocation, panicking conversions, destructors or retry loops. Error returns go
immediately to the existing child failure exit; partial descriptor cleanup is
performed by kernel process teardown. The ignored
`child_created_channel_survives_gate_and_exec_and_refuses_failed_send_or_gate`
test requires isolated root, CAP_SYS_PTRACE, clone3/pidfd_getfd and a statically
built `tests/fixtures/native_compiler_exec.c`, selected with
`FE2O3_NATIVE_COMPILER_EXEC_FIXTURE`. It checks final credentials, original-pidfd
custody, high CLOEXEC gating, post-exec FD195 exchange, transfer backpressure and
wrong gate refusal. These are mechanical diagnostics, not compiler/deployment
admission evidence.
