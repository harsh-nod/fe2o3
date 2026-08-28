# Compiler Execution Service V1

## Status

The canonical authority-free packet codec and the consumed Linux service loop
are implemented. The loop operates only on an already admitted
`ProtectedCompilerExecutionIssuerV1`; it does not create a second compiler or
runtime path. Direct challenge, issue, publication, and recovery methods are
crate-private, so an external caller must use this packet boundary.

This checkpoint is not the complete production deployment. A static issuer
executable and supervisor still need to establish the real distinct-UID launch
and ptrace inspection policy before admission. The complete receipt carriage,
subject-bound current-record recovery operation, and lossless Worker V3 V2
load-envelope codec now exist, but the production service client, V2-only
Cargo/host routing, external monotonic rollback anchoring, verifier authority,
and the exact Cargo-to-KFD run remain open.

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
| `Recover` | Exact current Worker record with the requested compiler subject | Strict post-fsync reacquisition and complete carriage; terminal without mutation |

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
| Ready or Cancelled response | 160 |
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
subject equality before returning the complete carriage. It recovers only the
current record; immutable history and custody confirmation before successor
issuance remain part of the production service deployment.

## Authority Limit

The service authenticates and durably publishes one protected compiler
execution receipt. Its response is inert evidence. It grants no compiler,
HSACO publication, load, or GPU launch authority. Only the pending Worker V3
verifier can join this receipt to the retained proof, exact final HSACO, and
machine-effect evidence and close `CompilerExecutionProvenance`.

## Qualification

The protocol suite mutates every byte of all six request and six response
forms and checks exact lengths, canonical re-encoding, nested pairing, policy,
position, and terminal identities. The service suite covers exact packet
boundaries, oversize and ancillary rejection, deadline and pidfd cancellation,
blocked response delivery, all operation transitions and exact replay, complete
carriage recovery, subject and policy substitution without mutation, four
continuity checks per packet, cancellation, and packet exhaustion. Compile-fail
tests reject direct external access to all issuer transition methods.
