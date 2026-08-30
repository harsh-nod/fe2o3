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
those images. Program, credential profile, root, and key now have one move-only
prepared-supervisor owner. A canonical outer handoff binds the child-reported
direct Cargo parent PID/UID/GID to the complete launch manifest. The client
transfers that record plus exactly the rustc service peer and pidfd over an
authenticated control connection; the supervisor independently admits those
process identities and descriptors. It separately owns an admitted external-
anchor endpoint and exact service pidfd, requires their service identity to
equal the launch manifest, and transfers both to the issuer. It now consumes
that handoff into a sealed 704-byte static pre-exec manifest and an exact
twelve-source table for destinations `0..=11`, retaining distinct stdout,
stderr, and readiness readers. The supervisor now installs the manifest and
issuer at FDs 198 and 199 and sources at FDs `200..211` through a gated
`clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)` child. The child self-checks its
inherited process profile and parent-death signal; the parent independently
checks procfs credentials, all capability sets, tracing, umask, and unchanged
user/mount/PID/network/IPC/UTS/cgroup/time namespaces before release. Exact
readiness transitions move-only launch custody to live ready custody, while
one exact descriptor-free readiness packet and EOF transition ready custody to
serving custody without surrendering the pidfd. Pidfd cancellation and
fixed-capacity deferred cleanup provide exactly-once reaping. The issuer
re-admits the anchor endpoint and pidfd against both the manifest service
identity and policy-pinned anchor key. Receipt publication now drives that
transport and requires a durably recorded exact proposed-position observation
before the Worker record or ACK can advance. The external-anchor entrypoint now
admits that exact profile and its fixed descriptor set before serving, and the
measured helper now performs the service-owned key, state, socket, and daemon
exec transition. The root-controlled coordinator now establishes the locked
child profile, retains pidfd/reaping custody, authenticates ready plus exec EOF,
and admits the live endpoint. The sole root coordinator transfers that admitted
endpoint into the protected supervisor, retains both child lifecycles, and is
exposed through one fixed systemd activation contract. Running the authoritative
root-only qualification remains open. Lifecycle custody is three independently
opened shared-lock descriptions: coordinator ownership, supervisor FD 12, and
anchor-helper FD 6 transferred to daemon FD 5. The protected children retain
their leases through their service loops and release only by last descriptor
close, so abrupt coordinator death cannot overlap exclusive provisioning with a
surviving child. Cargo now
admits the fixed root-owned client profile and connects only to the fixed
authenticated listener path.
The complete receipt carriage, subject-bound current-record recovery operation,
exact-carriage protected verification operation, bounded restart-safe client
state machine, backend acquisition, attempt-scoped sidecar transport, and
receipt-bearing Cargo/host route are implemented. `VerifyCurrent` rereads the
service-owned canonical Worker record, reconstructs the complete carriage,
compares every byte with the caller's expected carriage, queries the admitted
anchor with a client-bound recovery challenge, reacquires the Worker record,
and signs a canonical policy, Worker-ledger, commit, and currentness record with
the caller's challenge. The sole V3 record embeds both the retained signed
advance receipt and fresh signed recovery receipt. The client requires both
keys to equal its pinned policy, every nested coordinate to equal its original
expected carriage, and both receipts to report the exact proposed transition.
That evidence remains authority-free: it authenticates the signed external
commit and fresh current-head observation but does not prove protected key
custody or independently administered monotonic service deployment.
The external service's real persistence path is fault-injected before and after
cleanup, next-file creation, write, file sync, rename, and directory sync. Its
packet loop is interrupted before and after receive, durable exchange, and send.
At all eighteen boundaries, restart admits only the exact prior or proposed
state; replaying the persisted challenge converges to one proposed head without
a duplicate advance. A real closed response direction also proves that commit
precedes delivery and that a fresh daemon can replay after delivery failure.
The Worker V3 verifier request and decision now losslessly bind the exact
subject, carriage, policy, occurrence, Worker-ledger record, sequence, and
rollback anchors, and fail closed without independent protected-policy, ledger,
and external rollback verification identities. The concrete protected verifier,
independently deployed monotonic rollback process, hardened key use,
privileged root-coordinator qualification, and exact Cargo-to-KFD run remain
open.

The caller-pinned policy, service launch manifest, external-anchor deployment,
and service-owned Ed25519 keys have reusable immutable memfd capabilities in
`fe2o3-compiler-closure-capability`. Each preserves exact bytes under mode
`0400`, complete seals, retained object identity, and private close-on-exec
custody. Key capabilities additionally require anonymous service-owned
read-only custody and zeroize caller seed buffers. Issuer key custody binds to
the policy and exposes neither bytes nor a signing operation. External-anchor
key custody has a distinct role-tagged wire image, binds to the complete anchor
deployment identity and its public key, and can only be consumed into an
in-memory key after revalidation. Policy FD 202 is reserved for rustc;
launch-manifest FD 8 and signing-key FD 7 are reserved for the protected issuer;
anchor deployment FD 221 and anchor signing-key FD 222 are reserved for the
external-anchor daemon. The key image must be created under the dedicated
anchor UID: a root-owned or ordinary-file substitute is rejected. The prepared
supervisor now materializes both issuer descriptors. Cargo now installs the
policy and child-created
service channel for the selected rustc, performs the authenticated supervisor
handoff, and retains readiness through fresh publication. The backend consumes
both inherited descriptors, publishes its exact V3 handoff, acquires and
independently decodes the signed receipt carriage for that subject, and
durably publishes the exact carriage bytes beside the handoff. Cargo admits
the sidecar against the same sealed profile before consuming the handoff. The
carriage remains inert until the implemented verifier boundary is backed by a
real protected verifier and external rollback admission.

The external-anchor executable accepts no arguments or environment and consumes
exactly these inherited descriptors:

| FD | Content |
|---:|---|
| 3 | Connected unnamed nonblocking Unix `SOCK_SEQPACKET` peer |
| 4 | Existing service-owned mode-`0700` durable state root |
| 221 | Sealed external-anchor deployment manifest |
| 222 | Role-separated deployment-bound anchor signing key |

The 168-byte FD 221 manifest also pins a nonzero bounded SHA-256 measurement of
the exact daemon image. The daemon admits FD 221 and the complete locked service
profile before inspecting FD 222. It then opens `/proc/self/exe` and requires a
read-only close-on-exec descriptor for the same anonymous service-owned regular
mode-`0555` object, with no file capabilities and complete write, size,
execution-mode, and further-seal prevention. It hashes the sealed bytes once and
retains that exact object for constant-time revalidation. Only then does it
inspect and consume FD 222. It privately retains root and peer at FDs 256 and
257, closes every other descriptor including stdio, revalidates the profile,
and strictly opens existing state. Initialization is unavailable from the
daemon entrypoint. The measured unprivileged helper now admits the provisioning
manifest and its own exact static image, reissues the root-owned key template as
service-owned custody, atomically opens or initializes state under one retained
lock, creates the unnamed socketpair after the UID transition, transfers only
the supervisor endpoint, and executes the measured daemon with this exact
descriptor set and an empty environment. The remaining privileged coordinator
now prepares those exact inputs, launches the helper under the dedicated
UID/GID, retains pidfd and reaping custody, requires both the canonical ready
transfer and bootstrap close-on-exec EOF, and admits the endpoint against the
same live process. The sole root coordinator transfers that exact live endpoint
into the protected supervisor and retains anchor-after-supervisor teardown
custody. Its installed root-only qualification remains unfinished.

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
| 10 | Connected external-anchor service endpoint |
| 11 | Live process pidfd for that external-anchor service |

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

The session accepts exactly seven operation kinds:

| Request | Required durable state | Result |
| --- | --- | --- |
| `Inspect` | Any valid state | Exact `Ready`, `Prepared`, or `Issued` recovery record |
| `Prepare` | Named `Ready`, or the same `Prepared` position | Fresh durable challenge, or exact challenge replay |
| `Issue` | Matching `Prepared`, or the same `Issued` request | Durable signed sidecar, or exact sidecar replay |
| `Publish` | Matching `Issued`, or the immediately acknowledged request | Worker commit followed by issuer ACK; terminal exact ACK |
| `Cancel` | Any valid state | Current rollback position; terminal without mutation |
| `Recover` | Requested compiler subject | Strict post-fsync reacquisition and complete carriage when the exact current record exists; terminal without mutation. If no Worker record exists yet, return nonterminal `ReceiptAbsent`. A different current subject fails closed. |
| `VerifyCurrent` | Complete expected receipt carriage and fresh 32-byte challenge | Independently reread the canonical Worker V2 record, reconstruct and byte-compare the complete carriage, query the admitted external anchor with an exact client-bound recovery challenge, reread the same Worker record, and sign the exact policy/ledger/commit/currentness evidence; terminal without local mutation. |

Each request commits to the caller-pinned policy identity and a
domain-separated identity over its complete canonical bytes. Prepare names the
expected sequence and prior rollback anchor. Issue and Publish derive that
position from their complete nested attestation request. Recover carries the
complete canonical compiler subject and names no caller-asserted rollback
position. VerifyCurrent names the expected sequence and current rollback anchor,
carries the complete 2,090-byte carriage, and adds a nonzero 32-byte caller
challenge. Every response binds
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
| VerifyCurrent request | 2,250 |
| Ready, Cancelled, or ReceiptAbsent response | 160 |
| Prepared response | 360 |
| Issued response | 744 |
| Published response | 456 |
| Recovered response | 2,250 |
| VerifiedCurrent response | 1,784 |

The implementation uses fixed stack storage sized to the maximum request and
response. The complete publication ACK or recovered 2,090-byte carriage remains
inline in the terminal result; the authority boundary does not add a fallible
heap allocation path.

## Publication Ordering

Publish performs one ordered composition:

1. validate the issuer and Worker journal position;
2. verify the exact request and signed publication sidecar;
3. durably prepare or recover the exact external-anchor transaction and
   challenge;
4. unless an exact prior attempt already recorded anchor commit, exchange that
   challenge over the admitted endpoint and durably record the signed
   observation; an exact prior-position observation aborts the transaction;
5. durably commit and reacquire the canonical Worker V2 record with the complete
   exact proposed-position receipt embedded in the same atomic record;
6. durably mark the matching anchor journal `Published` and reacquire it;
7. derive the move-only committed-publication witness from the exact Worker
   reacquisition;
8. durably acknowledge the same publication in the issuer journal; and
9. return the authority-free Worker ACK with `Advanced` or
   `AlreadyAcknowledged` disposition.

The issuer never acknowledges before both the proposed anchor receipt and Worker
record are durable, the receipt in both records is byte-identical, and the anchor
journal is `Published`. Repeating the exact
Publish after a lost response reuses the durable challenge or completes from an
already committed stage, then returns the same ACK with `AlreadyAcknowledged`;
changing the request, sidecar, Worker identity, policy, sequence, challenge,
observation, or anchor fails closed.

Recover rereads the canonical Worker record, repeats strict policy, signature,
request, publication, identity, and ACK validation, and requires exact compiler
subject equality before returning the complete carriage. A genuinely absent
canonical Worker record produces a canonical nonterminal `ReceiptAbsent`
response so the same bounded client can continue with issuance. A corrupt record
or a current record for another subject is never reported as absent. Recovery
still covers only the current record; immutable history and custody confirmation
before successor issuance remain part of the production service deployment.

VerifyCurrent performs the same strict durable reread, including verification
of the Worker V2 record's embedded signed external-anchor commit receipt, then
reconstructs the complete carriage and requires both structural equality and
byte-for-byte equality with the request. It derives a recovery nonce from the
caller's fresh challenge, exact carriage identity, and retained commit-receipt
identity; reconstructs the same external transition as a `Recover` challenge;
and exchanges that challenge over the already admitted external-anchor endpoint.
Only an exact signed `Proposed` observation is current. A signed `Prior`
observation, stale nonce, wrong phase, changed transition, later/unrelated head,
transport failure, or duplicate response fails closed. After the exchange the
service reopens and byte-compares the Worker record again before forming any
result.

The sole 1,440-byte V3 result repeats every relevant coordinate, embeds the
policy-pinned external-anchor key, complete 528-byte retained advance receipt,
complete 528-byte fresh recovery receipt, and deterministic protected policy
and Worker-ledger evidence labels. The service signs that complete result with
the caller challenge in a 1,624-byte V3 attestation. Client verification starts
from the original expected carriage and challenge, reconstructs the compiler
anchor transaction and recovery challenge independently, and requires both
strict signatures and every sequence, prior head, proposed head, transaction,
key, kind, position, and nonce coordinate to match. This authenticates the
signed commit and fresh signed current-head observation. It grants no authority;
protected key custody and independently administered, monotonic, crash-durable
anchor deployment remain separate production joins.

`fe2o3-compiler-execution-client` implements the corresponding single-session
machine. It attempts Recover first, correlates every response to the exact
request and caller-pinned policy, and resumes the minimum legal suffix from
Ready, Prepared, or Issued. Issued restart reconstructs the exact challenge and
request from authenticated receipt fields before Publish. Recovery-only clients
send Cancel after ReceiptAbsent so the service terminates without mutation.
Verification-only clients generate a fresh challenge, use one terminal
VerifyCurrent exchange, verify the issuer signature under the pinned policy
key, compare every returned coordinate with the original expected carriage,
and independently re-verify both external receipts and the reconstructed
recovery challenge under the policy-pinned anchor key. No response-derived
verification clone is used as the expected value.

The same crate now implements the direct-parent channel handoff. The socketpair
is created after fork inside the selected child, its client endpoint is installed
at fixed FD 195, and only its service endpoint crosses `SCM_RIGHTS` to the
parent. The parent requires the transferred child and direct-parent PIDs,
`SO_PEERCRED`, spawned child PID, current parent identity, and live pidfd to
agree. It can now consume that move-only launch value into one canonical
direct-parent/manifest packet and transfer exactly the service peer and pidfd to
an authenticated distinct-UID supervisor control connection. The supervisor
requires its observed control `SO_PEERCRED` to equal the canonical direct
parent, then repeats policy, service-peer, pidfd target/liveness, socket shape,
descriptor identity, and alias checks. It can consume the accepted value into
the exact sealed static-launcher input set, repeating those checks together with
all source-object, capability, parent, pipe, and canonical-manifest checks. The
binding wrapper now invokes this path for the selected protected kernel root,
waits for exact issuer readiness, and kills/reaps rustc on any failed handoff.
Fresh publication fails closed unless the parent still retains both exact
rustc-invocation and compiler-execution-readiness custody. Privileged service
provisioning, backend receipt acquisition, deployed external monotonic rollback,
and final verifier/runtime authority joins remain separate requirements. The
production application runner uses the same child-created channel and fixed
supervisor path without exposing policy FD 202, establishes readiness before waiting for its
ACK, and retains that custody through exit. `fe2o3-host` exports a move-only
one-use auditor that consumes the inherited endpoint and verifies the signed
current-record transaction without constructing verifier, load, or launch
authority.

One absolute monotonic deadline now spans child admission, supervisor transfer,
and readiness; production does not restart a full timeout at each transition.

## Authority Limit

The service authenticates and durably publishes one protected compiler
execution receipt. Its response is inert evidence. It grants no compiler,
HSACO publication, load, or GPU launch authority. Only the pending Worker V3
verifier can join this receipt to the retained proof, exact final HSACO, and
machine-effect evidence and close `CompilerExecutionProvenance`.

## Qualification

The protocol suite mutates every byte of all seven request and eight response
forms and checks exact lengths, canonical re-encoding, nested pairing, policy,
position, terminal identities, the complete V2 receipt, wrong anchor keys,
wrong compiler transactions, and prior-position substitution. The service suite
covers exact packet
boundaries, oversize and ancillary rejection, deadline and pidfd cancellation,
blocked response delivery, all operation transitions and exact replay, complete
carriage recovery, exact-current verification, stale carriage rejection,
nonterminal absence, subject and policy substitution without mutation, four
continuity checks per packet, cancellation, and packet
exhaustion. The anchor service additionally exercises twelve persistence and six
packet-loop interruption points plus a real response-delivery failure, while the
Worker ledger exercises both cross-system commit orders and all retained-record
boundaries. Compile-fail tests reject direct external access to all issuer
transition methods.

`scripts/build-static-compiler-execution-issuer.sh` builds the pinned musl
target and rejects an interpreter, dynamic section, runtime dependency, RPATH,
RUNPATH, executable stack, undefined symbol, or an ELF entry address other than
the syscall-only secure-start symbol. It also starts the issuer with
FDs 3 through 11 closed and requires silent fail-closed exit status 1. This
qualifies the executable image shape and pre-runtime hardening edge. The
same script now submits the actual release bytes to the production
static-application parser. `scripts/build-static-external-anchor-service.sh`
applies the identical secure-entry, static-ELF, dynamic-loader, undefined-symbol,
parser, and silent fail-closed gates to the anchor daemon. Both executables use
the one shared secure-start assembly in `fe2o3-protected-service-profile`. The
supervisor image-admission suite independently qualifies exact source
measurements, static-profile validation, anonymous read-only executable memfd
custody, complete content/exec seals, caller-policy agreement, mutation and
substitution rejection, and move-only descriptor hiding on Linux and MI300X.
The suite also qualifies authority binding, same-process hostile handoff cases,
the exact twelve-entry prepared table, manifest seals and canonical bytes, parent
continuity, source and manifest substitution, and post-preparation rustc death.
The lifecycle suite additionally covers atomic clone3 pidfd launch, isolated
stdio, profile parsing, namespace continuity, readiness success, PID
substitution, trailing bytes, timeout cleanup, fresh launch after termination,
synchronous exactly-once reaping, and abrupt supervisor-parent death. A real
freestanding-launcher integration crosses both `execveat` boundaries and
admits exact readiness from the resulting pidfd occurrence. The client and
supervisor suites also qualify exact descriptor-free Cargo publication,
mandatory EOF, malformed and substituted packets, ancillary descriptors,
trailing packets, timeout, closed-peer cleanup, and serving typestate custody.
The root-owned distinct-UID launcher, profile gate, fixed descriptor transfer,
canonical deployment readiness, and supervisor/anchor lifecycle custody are
implemented. The fixed systemd socket/service, sysusers, tmpfiles, and 14 named
activation descriptors are implemented and structurally checked. A canonical
caller-pinned install manifest and static musl verifier now admit the complete
14-file source bundle through retained descriptors and preserve its 13 content
files in sealed anonymous custody. Atomic privileged installation from that
custody and execution under the provisioned root/distinct-UID accounts remain
pending.
The fixed static reference provisioner now resolves those accounts, validates
and measures all five installed service images, creates or verifies both key
seeds, constructs all four cross-bound records, and publishes them durably
without replacement. It owns an exclusive lease on a dedicated root-only file
through a retained root-owned parent. Service admission derives the same sibling
from existing state-root FD 4 and takes all three corresponding shared leases
before key material is read. Supervisor FD 12 and anchor helper FD 6 carry
independent child open file descriptions; the helper installs its lease at
daemon FD 5, where the daemon privately retains the lock and canonical parent
at FDs 258 and 259. Non-root subprocess tests kill the coordinator holder and
prove exclusive admission remains blocked until both service holders exit in
either order. The lock
therefore adds no activation descriptor and does not conflict with the issuer's
independent state-root singleton. Idempotence, partial-publication recovery,
generation substitution, listener, mutual exclusion, mode, static-image,
lock/parent replacement, and retained-path failures are covered by the
coordinator suite.
Cargo's fixed listener acquisition, child-channel transfer, and readiness gate
are covered by unit suites; they have not yet been qualified against that fixture.
