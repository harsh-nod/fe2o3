# Native Compiler Handoff

`ProtectedIssuerSupervisorV2::accept_handoff` consumes one control connection
and returns move-only `AcceptedCompilerExecutionHandoffV2`. It uses the native
policy/program/key/anchor/root binding, not an admitted V1 owner conversion.
This is session custody, not native prepared launch, process creation,
confinement, readiness, serving, protected proof or GPU qualification.

## Exact Joins

Admission checks, in order:

1. Nonzero timeout and representable absolute receive deadline, then complete
   native supervisor revalidation.
2. CLOEXEC connected Unix seqpacket control and its kernel-reported submitter
   PID/UID/GID. Client/submitter UID must differ from the supervisor UID.
3. Exactly one canonical frame and two ordered rights: service socket, pidfd.
4. Native policy identity, provisioned external-anchor service identity, exact
   submitter and distinct client/anchor process roles.
5. Distinct descriptor objects, unnamed connected CLOEXEC seqpacket service
   socket, and its exact client PID/UID/GID.
6. Fresh bounded native client-pidfd admission, supervisor revalidation, and
   complete accepted-owner revalidation.

The accepted owner retains one pidfd, not a raw pidfd plus a validation copy.
`LiveClientPidfdIdentityV2` uses the shared descriptor/target/probe-source/
start-time/liveness schedule with bounded native I/O. Cached object facts are
inert; revalidation checks the actual descriptor. UID/GID supplied to that
owner are not independently proved by a pidfd: acceptance joins them to the
service socket's kernel-reported connection-time credentials.

These observations do not prove ancestry, perpetual liveness or exclusive
endpoint ownership. Accepted revalidation repeats the exact policy/anchor,
descriptor, peer and live-process joins, including control CLOEXEC/shape.
Getters expose immutable frame/identity facts, never a raw descriptor or signing
operation. A native refusal never selects the legacy path.

## Shared Wire And Receive

`CompilerExecutionSupervisorHandoffV2` wraps the same 184-byte identity-only
frame and 112-byte launch manifest as the legacy protocol. One shared codec
retains the existing wire version, domains, canonical encoding and diagnostic
order. Structural acceptance of an opaque legacy policy identity does not admit
that policy as native. A consumer must independently match a pinned PolicyV2.

Native receive performs one poll and one nonblocking recvmsg. EINTR and readiness
races fail closed instead of retrying; received payload and ancillary storage
are fixed-size. All rights acquire RAII ownership before packet validation.
A private, aligned zeroed-buffer guard also closes unexpected Linux ancillary
pidfds skipped by the pinned socket library, including refusal and unwind.
It does not take ownership of SCM_RIGHTS a second time. Unsupported ancillary
messages are rejected. Both legacy and native handoff receivers use this guard;
legacy waiting/retry behavior otherwise remains unchanged. The guard assumes
kernel-generated Linux ancillary headers and is not a public raw-byte decoder.

## Resource Contract

All operations charge the caller's ledger, never a replacement budget. The
supervisor and consumed control FD must remain prepaid. Received sockets/pidfd,
native frame and owner growth are reserved as they become retained. The returned
delta excludes only the already prepaid control FD; eventual release uses the
accepted owner's complete `retained_storage()` after drop or transfer.

Let `S` be complete supervisor revalidation, `H` the handoff codec's 9,480 work
units, `L` native launch policy comparison, and `A`/`P` native client admission/
revalidation (26,482,792 / 17,829,960 units). The outer allowance `O` is 131,080:

```text
accept     = O + 3*S + H + 2*L + A + P
revalidate = O + S + L + P
```

Outer scratch is four accepted-owner/receipt layouts, four fixed payloads,
twice the ancillary backing size and a 4096-byte control/error envelope. Nested
checks add their scratch on the same ledger. Composite peak storage includes
already retained growth when the final chain runs. The frame codec prepays its
nested launch codec in one fixed quota; it does not decode through a V1 owner.

Scopes restore entry storage on success, refusal and unwind without refunding
work or clearing prior denials. Consuming refusal closes control and acquired
rights before the caller retires input reservations. These logical quotas do
not bound generated stack, RSS, kernel socket memory or syscall scheduling.

## Tests And Remaining Work

Coverage includes old/new wire agreement and mutations, relationship/error
precedence, exact/short budgets, retained floors, sticky denials, one-pidfd
custody, descriptor mutation, malformed/truncated transfers and cleanup.
An opted-in isolated process fixture uses real non-root anchor, supervisor,
submitter and client roles, with socketpairs created after credential drop.
No exported same-UID shortcut is used. Synthetic executable fixture admission
is not execution of a production issuer or launcher.

Accepted custody must next feed native prepared static launch and consuming
process lifecycle. Native service/durable-state, broker, Worker, runtime,
finalizer, host and provisioning joins still precede producer activation.
M0-M7 and 47/47 remain incomplete; these tests grant no kernel qualification.
