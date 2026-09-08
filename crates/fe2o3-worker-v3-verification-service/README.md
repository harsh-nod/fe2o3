# fe2o3 Worker V3 verification service boundary

This crate owns one bounded Linux Unix `SOCK_SEQPACKET` exchange for the authority-free Worker V3
verification protocol. It receives one canonical request with exactly two ordered `SCM_RIGHTS`
descriptors, resolves the caller's policy and expected verifier measurement through injected
fail-closed interfaces, consumes the fresh challenge through an injected atomic replay guard, and
captures each exact immutable payload into a receiver-owned sealed memfd.

The receiver enables Linux `SO_PASSCRED` before receiving. The sole request packet must carry
exactly one kernel-stamped `SCM_CREDENTIALS` record matching the connection's `SO_PEERCRED`, plus
exactly one `SCM_RIGHTS` record containing the two ordered descriptors.

`prepare_worker_v3_verification_receiver_v1` must run before the peer can queue a packet: on a
listener before `listen`/`accept`, or on a private socket-pair receiver before exposing the sender.
The service rejects an endpoint without `SO_PASSCRED`, and it rejects any already-queued packet
that lacks the required kernel-stamped credentials.

The caller must shut down the connected socket's write half immediately after the sole packet. The
service requires exact EOF before it resolves policy or consumes replay state; a second packet or
ancillary object terminates the session without a response.

The boundary emits only `RequestFramed` or `RequestRejected`. Framing, descriptor custody, byte
identity, policy selection, measurement selection, and replay exclusion are not verification
theorems. This crate cannot construct protected roster evidence and grants no load or launch
authority. A later protected verifier must independently establish every compiler, executable,
layout, effect, and universal-safety property before a reviewed host promotion can grant authority.

Production resolver implementations must authenticate their policy and measurement stores. A
production replay guard must atomically persist used challenges across every service instance and
restart covered by the policy. The crate deliberately provides no permissive default implementation
for any of these injected decisions.

## V2 state machine

`begin_worker_v3_verification_session_v2` reuses V1's exact Begin request admission, policy and
measurement resolution, caller replay guard, credential checks, and receiver-owned immutable
payload copies. It deliberately does not require write EOF after Begin. It rejects data queued out
of phase, calls a required `WorkerV3VerificationChallengeReservationProviderV2`, sends the
provider's nonzero challenge and opaque reservation identity, and returns a move-only pending
current-record session. No provider default exists. Provider implementations remain responsible for
entropy, uniqueness, atomic reservation, durable replay exclusion across covered service restarts,
and expiry; the generic service cannot prove those properties.

`begin_worker_v3_verification_session_until_v2` accepts one exact caller-created monotonic
`Instant` and retains it unchanged through Begin, current-record receipt, and the terminal send. The
compatible timeout entrypoint computes the deadline once and delegates to the absolute-deadline
entrypoint. Pre-expired deadlines fail before the service receives the queued request or its
descriptors.

The pending state receives one exact fixed-size current-record frame with matching kernel-stamped
credentials and no other ancillary data, then requires client write-half EOF. It strictly decodes
the separate verification and attestation, checks nested byte equality, and correlates the Begin,
challenge, and reservation identity before a terminal application capability can exist. Malformed
or mismatched records produce only a custody-retaining rejection capability.

Only the ready terminal capability can send one bounded opaque application response. Both ready and
rejected capabilities can instead send a generic rejection, and send/construction failures retain
receiver-owned payload custody where the socket result is recoverable. The response remains opaque:
this crate does not treat application bytes, canonical compiler records, or successful transport as
theorem, currentness, load, launch, or protected-key authority. V1 remains available and unchanged
for its one-shot framing-only exchange.

## Exact production capability carriage V5

The V5 entrypoint decodes the additive V5 request, applies the existing policy, measurement,
challenge-replay, credential, and immutable-payload checks, then requires an injected custody
resolver to remove the named prepared completion and live W4 witness as one pre-admission candidate
from local storage. Every coordinate is recomputed from those typed owners; transport bytes are
never admitted as authority.

The service additionally decodes the retained bundle with
`VerifiedSimulationBundleV8::from_canonical_bytes`, revalidates it, compares its native identity,
and checks its KIR, epoch, target, and kernel count against the same V5 handoff. Only after those
checks and exact W4 final-graph, policy, target, aggregate launch-roster, target-closure, and clean
stage-roster checks does it bind the prepared transaction to the live W4 owner.

The bound owner stores the V5 request identity. The reply socket, request, immutable payloads,
bound transaction, W4 witness, and any machine proof remain inside one move-only session; there is
no API that returns a separate pending reply and owner. Quarantine receives only a borrow, returns
an error, and must persist and fsync its tombstone before the service drops custody or emits a
rejection. `DurableWorkerV3VerificationStateV5` provides the concrete atomic-link and directory-
fsync implementation. It also pins policy/measurement configuration and atomically persists fresh
challenge tuples across reopened instances. A repeated request identity with different record
bytes is a conflict, not an idempotent retry.

### Implemented machine proof class

The only locally implemented semantic-to-final-machine proof is a singleton, no-effect kernel:

- Bundle V8 must decode strictly and contain one verified canonical KIR V13 kernel.
- KIR must contain exactly one zero-argument, zero-result kernel-entry function, one block, no
  operations or capabilities, and `return` with no values.
- The compiler symbol manifest must contain exactly that kernel entry and its `.kd` descriptor.
- The final object must be a COV6 `EM_AMDGPU` ELF for the exact handoff target on `gfx942`.
- The object must contain one executable section and exactly four executable bytes,
  little-endian `s_endpgm`; its sole function and descriptor must bind that entry, with zero
  kernarg, group/private segment, dynamic-stack, and reserved descriptor state.

Every helper, parameter, result, KIR operation, memory effect, barrier, call, branch, additional
instruction, relocation, executable section, function/object symbol, target mismatch, malformed
ELF, multi-root transaction, scalar GEMM, and other operation is unsupported and gets the same
generic wire rejection. The service never accepts a machine receipt, proof digest, or evidence
bytes from the caller.

Issue #214 currently exposes the intended move-only
`CheckedGfx942ScalarF32MachineRefinementV1` shape, with canonical bytes and identity and no public
constructor, but its production checker still returns `Infallible` and names nine missing typed
inputs. Consequently scalar GEMM remains closed here. Once #214 supplies the checked owner, this
crate must invoke that checker in-process and consume the owner directly; it must not add a decoder
or callback that accepts arbitrary evidence bytes.

The artifact transaction crate likewise does not yet expose the forthcoming sealed completion
capability. This crate therefore retains a successful no-effect proof with its request-bound
session but deliberately cannot turn it into a successful V5 response through the existing public
inert-record terminal. When the sealed capability lands, the adapter must consume this whole state,
durably record the complete result and proof owner through
`WorkerV3VerificationCapabilityCompletionJournalV5`, fsync it, and only then send success. The
association coordinate must be `capability_associations().identity()` for the complete roster,
never the first association.

### Deployment contract

The production pathname is exactly `/run/fe2o3/worker-v3-verifier.sock`; it must be a root-owned
Unix `SOCK_SEQPACKET` socket with the deployment's reviewed `0660` group and `SO_PASSCRED` enabled
on the listener before `listen`. The runtime and durable state directories must be local,
non-symlink directories. Policy, measurement, challenge, quarantine, and completion records must
use the durable state implementation or an implementation with the same atomic-create, file-fsync,
commit, and directory-fsync guarantees.

FD 195 belongs to the compiler current-record application service and must already be inherited by
the host-side auditor. It is not a substitute V5 proof channel and this service must never read
machine evidence from it. The fixed pathname session and FD 195 current-record audit are both
required by the production host deployment; neither authenticates or replaces the other. No
standalone success daemon is shipped until the sealed artifact-completion capability and the
checked #214 producer exist, because a daemon that reconstructed either owner from bytes would
create the authority bypass this boundary is intended to prevent.

## Connected pathname admission

`WorkerV3VerificationAcceptedServiceEndpointV2::admit` is the explicit V2 admission boundary for
an already accepted pathname connection. It requires the local address to exactly match a
caller-supplied, lexically validated canonical absolute filesystem pathname, an unnamed client
address, exact nonblocking close-on-exec read/write custody, and `SO_PASSCRED` inherited from a
listener prepared before `listen` and `accept`. Admission snapshots the connecting process's
`SO_PEERCRED`; Begin and current-record packets must retain exact kernel-stamped `SCM_CREDENTIALS`
continuity with that snapshot.

`begin_worker_v3_verification_accepted_session_until_v2` consumes only that admitted endpoint. It
does not discover a path, create or connect a socket, listen, or accept. The process whose identity
must be observed must therefore perform `connect` itself, and the service process whose identity
must be observed by the client must create the listener itself. Passing a connected descriptor to
another process does not update Linux's connection-time `SO_PEERCRED` snapshot; a later packet from
that process fails the per-message credential check. The existing unnamed V1 and V2 entrypoints
remain strict and unchanged.
