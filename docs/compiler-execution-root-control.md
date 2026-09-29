# Root-to-Issuer Control

Status: **integration pending**. The native dispatcher currently retains a local
issuer-owned occurrence, as described in [Native Issuer Session
Custody](compiler-execution-native-session.md). The original-root observation API
and bounded authenticated receive primitive exist. They do not yet form an
end-to-end root-owned issuance path or qualify a tutorial kernel.

## Transport Boundary

`fe2o3-protected-service-spawn::launch_io::receive_authenticated_packet` receives
one fixed-size record, between 2 and 4096 bytes. It reuses the readiness
receiver's ancillary parser and descriptor ownership. It requires exactly one
matching `SCM_CREDENTIALS`, an exact payload, and no descriptor transfers,
unknown control messages, or truncation. Every disclosed descriptor is owned
and closed even when the record is refused, including `SCM_PIDFD`.

The caller must provide a validated `SOCK_SEQPACKET` endpoint with `SO_PASSCRED`
enabled before any sender can enqueue data. Charge `packet_receive_work(N)`
before every attempt and retain `packet_receive_scratch(N)` throughout it.
`EAGAIN` and `EINTR` return no record, without an internal retry. Deadline,
cumulative attempt limits, live-child validation, and retained output charges
belong to the caller. No budget is created or reset by the primitive.

The matching `send_packet` operation reuses the nonblocking readiness sender.
The same conservative quotes cover one send attempt. Its success means the
record was sent, not that the peer acknowledged it. It neither retires an
occurrence nor discards the caller's pending response or replay state.

Matching a `MessageSender` is a transport check, not compiler admission. The
expected PID/UID/GID must come from actual retained-child and profile custody.
`SO_PEERCRED` alone identifies the socket creator, not necessarily the process
writing a later packet. The control protocol must also bind every message to
the admitted policy, launch manifest, fresh launch epoch, and sequence.

## Control Envelope

`CompilerExecutionRootControlRecordV3` is an inert, fixed 4096-byte envelope.
Its binding constructor requires a matching native policy and launch manifest,
plus nonzero root-epoch and connection-generation values. The retained attempt
must generate those values independently; a decoder cannot prove freshness.
Replacement issuers need a new admitted generation, not a reset sequence on the
old connection.

| Bytes | Content |
| --- | --- |
| 0..20 | Magic `F2O3CRC3`, LE version 3, operation, fixed total length |
| 20..24 | Request/reply flag followed by three zero bytes |
| 24..152 | Policy, manifest, root epoch, connection generation (32 bytes each) |
| 152..160 | Positive LE sequence |
| 160..192 | Zero for a request; exact request digest for a reply |
| 192..200 | LE payload length and four zero bytes |
| 200..4064 | Up to 3864 opaque payload bytes, followed by zero padding |
| 4064..4096 | SHA-256 of the domain and preceding fixed frame |

The domain is `FE2O3/ROOT-ISSUER-CONTROL/V3\0`. Reconcile, Observe, Validate and
Retire are closed operation labels, not permission to execute them. Payload
decoding and lifecycle transitions remain broker obligations. The digest is not
authentication: an attacker can rehash changed fields. `matches_binding` checks
all four retained association fields; `matches_reply` additionally checks the
direction, operation, sequence, and exact original request digest. Neither
comparison authenticates credentials, proves durable state, releases locks, or
establishes currentness.

Every codec operation prepays its fixed work and scratch on the original budget;
successful constructors/decoders return the full retained charge unreserved.
The framing API does not own sockets, retry, advance a sequence, maintain a
retirement tombstone, or activate the FD12 ABI. A root RPC consumer is still
required.

## Connection Replay

The private broker `RootControlReplayWindowV3` orders inert records for one
connection. It is not yet connected to an admitted root session. It retains a
binding and at most one request/reply pair, with the full maximum storage charge
prepaid even while empty. Every operation checks the original ledger, Budget
address, process and kernel thread. Its work and scratch quotes include nested
protocol comparisons; refusal restores entry storage without refunding work or
denial history.

The first request must have sequence 1. An exact duplicate of a pending request
is reported as pending, never as a new operation. Completion requires a reply
bound to the exact original request, then retains both records before any send.
A duplicate completed request selects the cached reply. Only the next sequence
can replace that pair; changed content, direction or association, skipped/stale
sequences, and sequence overflow refuse without altering state. Send failure or
caller unwind does not clear this separately retained window.

This is ordering, not permission to execute an operation. Authentication,
operation-specific validation and bounded cumulative attempts remain session
obligations. In particular the window is **not a retirement tombstone**: the
root-owned occurrence and exact retirement record must survive independently
when a new sequence replaces the cached reply or a new issuer replaces the
connection. There is no reset, authority conversion, or durable recovery API.

## Startup Migration

The broker now owns `RootLaunchChannelV3`, a root-created pair with no `from_fd`
constructor or root-end extraction. It enables passcred on both nonblocking,
close-on-exec endpoints before exposing an issuer-side staging borrow. It checks
the original account, Budget address, process and kernel thread on each operation.
Closing its parent issuer alias is idempotent, but does not certify closure of
external stage aliases or retire a compiler occurrence. The owner is not yet
connected to `launch_issuer` or a broker root session. Positive privileged socket
validation remains required.

Its private packet methods reuse the shared transport after checking that its
own parent issuer alias is closed and the root endpoint still has the exact
required shape. Each call prepays one complete nonblocking attempt on the
original account. Receive output is unreserved inert data; send success is not
an acknowledgment. Neither method exports the root descriptor, authenticates a
deployment, or changes occurrence/replay state. Caller-supplied sender values
must still be derived from the actual admitted issuer, with liveness checks and
a fresh post-readiness challenge. External staging aliases remain the launcher's
responsibility.

The direct launcher now independently checks the actual retained issuer's running
image after exact readiness and EOF, before returning the child. The same check
runs during `ManagedIssuer::validate_ready`, inside its original account and
retained-policy view. Broker image mechanics are shared with V2/V3 self-admission:
the policy length is enforced before payload reads, the static image/runtime are
measured, and the current executable is reopened for an exact snapshot join.
Original-child liveness brackets the observation. Launch and continuity quotas
include the image-sized work and scratch; inspection denial remains terminal.

This observation is point-in-time image equality, not issuer authentication or
policy provenance. The launcher still checks its retained deployment, process
profile, namespaces and manifest. The image helper cannot replace those checks,
the post-readiness control challenge, or original compiler trace custody. It does
not activate the root-control descriptor or complete the production owning attempt.

Both native issuer entrypoints now preflight the complete current FD3..11 table
after nonallocating process hardening and before descriptor-allocating input
admission. Missing or close-on-exec slots refuse before a policy/manifest
duplicate can reuse them. Object-role validation still follows, and V1 is
unchanged. This ordering fix does not activate FD12.

The pair must be created inside the direct launch's confirmed, held-exec input
callback, after the compiler clone. Neither end may enter the compiler. The
compiler's FD195 is its public service channel, not a root-control bootstrap.

The proposed issuer-only slot is FD12. This is **not yet the implemented ABI**:

| Path | Current contract | Required change |
| --- | --- | --- |
| Direct V3 root launch | Nine bindings, FD3 through FD11 | Stage only the new issuer endpoint at FD12; retain the root endpoint separately |
| Conditional issuer startup | Shared V2/V3 intake; private duplicates start at FD12 | V3-only mandatory intake before allocation can reuse missing slots; V3 duplicate floor 13 |
| Indirect V3 launch | Stdio plus FD3 through FD11 | Transfer a real per-attempt root endpoint and update its exact tables and accounting |
| V2 startup | Existing FD3 through FD11 ABI | Preserve its descriptor contract |

The indirect supervisor's own FD12 is already its lifecycle lock. It is not the
new channel. Changing only the direct table and conditional issuer would break
the indirect path. Migrate both V3 routes coherently, or explicitly make the
unsupported route refuse before clone; never silently fall back to local
observation, V2, or another binary.

The direct launch must charge ten channel descriptors instead of eight, ten
bindings instead of nine, the extra staged source descriptor, both passcred
operations, endpoint validation, owner overlap, and bounded RPC attempts.
All stage and parent aliases of the issuer end must close before completion.
Retain the full root-side state independently of the issuer connection.

Endpoint admission must precede readiness. Synchronous root RPC must follow
the exact readiness frame and writer EOF, otherwise startup can deadlock while
the root waits for readiness and the issuer waits for the root.

## Ownership and Retirement

The production attempt must retain the actual original compiler trace and the
actual root-observed `NativeOccurrence`, including both lease and token. A
subject, hash, PID, signed journal, or decoded carriage cannot recreate either
owner. A failed send, issuer exit, or replacement connection must not release
that occurrence or establish retirement.

Original-trace binding must remain stable while the trace moves and is polled.
An opaque allocation identity can establish sameness without exporting a PID,
descriptor, or wait authority. It cannot establish liveness: every operation
still requires the original scoped view, account, thread, and continuity checks.
The coordinator's per-attempt owner must keep that trace separate from the
replaceable issuer connection and must not expose trace substitution.

Retirement needs an exact durable publication join, fresh original custody
validation, and destruction of both lock owners before acknowledgment. A
decoded carriage's ACK is inert; decoding it alone is not evidence of durable
Worker state. Keep an exact retirement tombstone for idempotent replay. Unknown
root epochs and lost root state must refuse, not become an empty session.

The issuer-side gate must require Ready plus the exact completed Worker,
Published anchor, advanced issuer position, and last ACK. A previous completed
Worker can remain recoverable while the next transaction is pending; its
carriage must not retire the new transaction. The authenticated retirement
command is an assertion by the admitted, measured issuer executing this gate,
not transferable cryptographic proof of durability. That trust boundary must
remain explicit when integrating the root control protocol.

Exact retirement replay must not reacquire the retired artifact: the compiler
may already have changed it or exited. Replay confirms retirement only, never
fresh currentness. Replacing a failed issuer also requires a new admitted
connection generation; the current one-use issuer input transfer is not itself
a restart mechanism.

Published, Recover, VerifyCurrent, and the next Prepare must all pass this
retirement gate. Integrate that gate in the existing native dispatcher and its
private publication guard, not an alternative compiler or public authority
provider. Original-account checks and cumulative resource bounds must survive
connection changes and refusal paths.

## Acceptance Gates

1. Actual-socket credentials, framing, truncation, and complete descriptor cleanup.
2. Exact startup tables, absent/substituted FD refusal, compiler non-inheritance,
   alias closure, unchanged V2 behavior, and explicit indirect V3 behavior.
3. Original trace/account binding, issuer death and failed sends with custody
   retained, and independent cancellation/cleanup ownership.
4. Lost retirement replies, issuer restart, old-carriage replay, unknown epoch,
   and exact durable-state joins across every receipt-exposing operation.
5. Integrated production attempt, protected proof execution, and target-matched
   positive and negative GPU qualification for all 47 tutorial kernels.

Parser, journal, startup, and compile-only tests establish their individual
properties. They do not substitute for the later gates.

The [checkpoint evidence](evidence/root-control-foundations-20260929.md) records
the tested source, observed failures, and remaining validation limits.
The subsequent [envelope/intake evidence](evidence/root-control-envelope-20260929.md)
records the framing, root-channel owner and complete-table preflight checkpoint.
The [running-image evidence](evidence/root-issuer-image-20260929.md) records the
direct-launch image check, lifecycle test coverage and remaining privileged gates.
The [packet/replay evidence](evidence/root-control-replay-20260929.md) records the
private packet operations, inert replay window and still-unexecuted socket cases.
