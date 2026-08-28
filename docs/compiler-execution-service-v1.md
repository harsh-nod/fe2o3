# Compiler Execution Service V1

## Status

The canonical authority-free packet codec and the consumed Linux service loop
are implemented. The loop operates only on an already admitted
`ProtectedCompilerExecutionIssuerV1`; it does not create a second compiler or
runtime path. Direct challenge, issue, publication, and recovery methods are
crate-private, so an external caller must use this packet boundary.

This checkpoint is not the complete production deployment. The repository now
has a descriptor-only `x86_64-unknown-linux-musl` issuer executable, a strict
static-ELF build gate, and a sealed launch manifest binding the exact expected
client PID/UID/GID to the exact caller-pinned policy. The supervisor's
authority-free program stage now authenticates and independently seals the
launcher and issuer before root or key binding; no per-launch input can select
those images. The supervisor still needs to bind the credential profile, root,
and key, establish the real distinct-UID launch and inspection policy, map the
ten source descriptors through the existing static pre-exec launcher, and
consume readiness under pidfd lifecycle ownership.
The complete receipt carriage, subject-bound current-record recovery operation,
lossless Worker V3 V2 load-envelope codec, and bounded restart-safe client state
machine exist, but protected-supervisor integration, backend acquisition,
V2-only Cargo/host routing, external monotonic rollback anchoring, verifier
authority, and the exact Cargo-to-KFD run remain open.

The caller-pinned policy and service launch manifest have reusable immutable
memfd capabilities in `fe2o3-compiler-closure-capability`. Each preserves exact
canonical bytes under mode `0400`, complete seals, retained object identity,
and private close-on-exec custody. Policy FD 202 is reserved for rustc; launch
manifest FD 8 is reserved for the protected issuer. Cargo/backend and
protected-supervisor wiring still need to consume them.

## Transport And Ownership

The service consumes one admitted issuer and its retained unnamed Unix
`SOCK_SEQPACKET` peer. It polls that peer together with the admitted client's
pidfd. The peer, pidfd, signing key, issuer journal, Worker journal, occurrence,
and publication-currentness custody are never serialized into a request or
response.

Every request and response is one canonical packet. Receive and send use
nonblocking syscalls. The service rejects:

- stream or unconnected sockets during admission;
- zero-length, malformed, noncanonical, short, or extended packets;
- `MSG_TRUNC` and `MSG_CTRUNC`;
- all ancillary descriptors, including `SCM_RIGHTS`;
- partial `SOCK_SEQPACKET` sends;
- a changed executable, key, process, capability, journal, or Worker join;
- a dead client, closed/failed peer, invalid descriptor, or expired deadline;
  and
- more than eight packets in one session.

The static issuer accepts no arguments or environment configuration. Its
supervisor-to-issuer descriptor contract is fixed:

| FD | Exact role |
| ---: | --- |
| 3 | Service-owned durable issuer and Worker-ledger root |
| 4 | Connected service endpoint whose peer is the exact rustc client |
| 5 | Live process pidfd for that rustc client |
| 6 | Sealed caller-pinned issuer policy |
| 7 | Service-owned sealed Ed25519 signing-key image |
| 8 | Sealed expected-client and policy launch manifest |
| 9 | Nonblocking atomic readiness-pipe writer |

The production ELF enters through a syscall-only x86-64 shim that restores
nondumpability after `exec`, reasserts `no_new_privs` and the zero core limit,
verifies the process controls, and only then enters musl and Rust. The entrypoint requires policy,
manifest, peer credentials, and pidfd identity to agree, remeasures its running
static image, admits key material only after those checks, recovers both durable
ledgers, emits one canonical readiness record binding its PID, launch manifest,
and policy, and then consumes the bounded service loop. The writer must be a
close-on-exec `O_WRONLY | O_NONBLOCK` pipe with an atomic-write bound covering
the complete record; packet-mode, async, append, blocking, readable, ordinary
file, socket, and undersized substitutions reject. Its launch inputs and
readiness bytes are not compiler or publication authority.

Continuity is checked before receive, after receive, after the durable
operation, and after response delivery. The same absolute monotonic deadline,
300 seconds in the public entry point, governs all socket waits. Synchronous
filesystem durability calls cannot be interrupted by this library function.
If such an operation completes after the deadline or the client exits during
it, the service sends no response and restart plus exact replay determines
whether the prior or successor record committed. A hard process wall-clock
limit is a responsibility of the pending production supervisor.

## Operations

The session accepts exactly six operation kinds:

| Request | Required durable state | Result |
| --- | --- | --- |
| `Inspect` | Any valid state | Exact `Ready`, `Prepared`, or `Issued` recovery record |
| `Prepare` | Named `Ready`, or the same `Prepared` position | Fresh durable challenge, or exact challenge replay |
| `Issue` | Matching `Prepared`, or the same `Issued` request | Durable signed sidecar, or exact sidecar replay |
| `Publish` | Matching `Issued`, or the immediately acknowledged request | Worker commit followed by issuer ACK; terminal exact ACK |
| `Cancel` | Any valid state | Current rollback position; terminal without mutation |
| `Recover` | Requested compiler subject | Strict post-fsync reacquisition and complete carriage when the exact current record exists; terminal without mutation. If no Worker record exists yet, return nonterminal `ReceiptAbsent`. A different current subject fails closed. |

Each request commits to the caller-pinned policy identity and a
domain-separated identity over its complete canonical bytes. Prepare names the
expected sequence and prior rollback anchor. Issue and Publish derive that
position from their complete nested attestation request. Recover carries the
complete canonical compiler subject and names no caller-asserted rollback
position. Every response binds
the exact request identity, policy identity, resulting position, payload, and
its own terminal identity.

No error packet exists. A malformed request, failed continuity check,
ambiguous durable failure, or transport error closes the session. The client
must reconnect to a newly admitted service, issue `Inspect`, and replay only
the exact legal request. This avoids granting an unauthenticated peer a second
diagnostic protocol and keeps crash recovery authoritative.

## Exact Packet Sizes

| Packet | Bytes |
| --- | ---: |
| Inspect, Prepare, or Cancel request | 128 |
| Issue request | 1,074 |
| Publish request | 1,658 |
| Recover request | 818 |
| Ready, Cancelled, or ReceiptAbsent response | 160 |
| Prepared response | 360 |
| Issued response | 744 |
| Published response | 456 |
| Recovered response | 2,218 |

The implementation uses fixed stack storage sized to the maximum request and
response. The complete publication ACK or recovered 2,058-byte carriage remains
inline in the terminal result; the authority boundary does not add a fallible
heap allocation path.

## Publication Ordering

Publish performs one ordered composition:

1. validate the issuer and Worker journal position;
2. verify the exact request and signed publication sidecar;
3. durably commit and reacquire the canonical Worker record;
4. derive the move-only committed-publication witness from that reacquisition;
5. durably acknowledge the same publication in the issuer journal; and
6. return the authority-free Worker ACK with `Advanced` or
   `AlreadyAcknowledged` disposition.

The issuer never acknowledges before the Worker record is durable. Repeating
the exact Publish after a lost response returns the same ACK with
`AlreadyAcknowledged`; changing the request, sidecar, Worker identity, policy,
sequence, or anchor fails closed.

Recover rereads the canonical Worker record, repeats strict policy, signature,
request, publication, identity, and ACK validation, and requires exact compiler
subject equality before returning the complete carriage. A genuinely absent
canonical Worker record produces a canonical nonterminal `ReceiptAbsent`
response so the same bounded client can continue with issuance. A corrupt record
or a current record for another subject is never reported as absent. Recovery
still covers only the current record; immutable history and custody confirmation
before successor issuance remain part of the production service deployment.

`fe2o3-compiler-execution-client` implements the corresponding single-session
machine. It attempts Recover first, correlates every response to the exact
request and caller-pinned policy, and resumes the minimum legal suffix from
Ready, Prepared, or Issued. Issued restart reconstructs the exact challenge and
request from authenticated receipt fields before Publish. Recovery-only clients
send Cancel after ReceiptAbsent so the service terminates without mutation.

The same crate now implements the direct-parent channel handoff. The socketpair
is created after fork inside the rustc child, its client endpoint is installed
at fixed FD 195, and only its service endpoint crosses `SCM_RIGHTS` to the
parent. The parent requires the transferred PID, `SO_PEERCRED`, child PID, and
live pidfd to agree, and rejects descriptor collision, endpoint replacement,
truncation, ancillary ambiguity, timeout, or child exit. The binding wrapper
does not yet transfer these launch inputs to the distinct-UID protected issuer.

## Authority Limit

The service authenticates and durably publishes one protected compiler
execution receipt. Its response is inert evidence. It grants no compiler,
HSACO publication, load, or GPU launch authority. Only the pending Worker V3
verifier can join this receipt to the retained proof, exact final HSACO, and
machine-effect evidence and close `CompilerExecutionProvenance`.

## Qualification

The protocol suite mutates every byte of all six request and seven response
forms and checks exact lengths, canonical re-encoding, nested pairing, policy,
position, and terminal identities. The service suite covers exact packet
boundaries, oversize and ancillary rejection, deadline and pidfd cancellation,
blocked response delivery, all operation transitions and exact replay, complete
carriage recovery, nonterminal absence, subject and policy substitution without
mutation, four continuity checks per packet, cancellation, and packet
exhaustion. Compile-fail tests reject direct external access to all issuer
transition methods.

`scripts/build-static-compiler-execution-issuer.sh` builds the pinned musl
target and rejects an interpreter, dynamic section, runtime dependency, RPATH,
RUNPATH, executable stack, undefined symbol, or an ELF entry address other than
the syscall-only secure-start symbol. It also starts the issuer with
FDs 3 through 9 closed and requires silent fail-closed exit status 1. This
qualifies the executable image shape and pre-runtime hardening edge. The
supervisor image-admission suite independently qualifies exact source
measurements, static-profile validation, anonymous read-only executable memfd
custody, complete content/exec seals, caller-policy agreement, mutation and
substitution rejection, and move-only descriptor hiding on Linux and MI300X.
It does not qualify the still-pending authority binding or distinct-UID launch.
