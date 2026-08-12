# Protected Publisher Reference Service V1

This document describes the opt-in reference implementation in
`fe2o3-protected-publisher`. It is not deployed, is not production authority,
does not enroll anything by itself, and does not change a parity row or
dashboard.

## Scope

The binary serves only with `--serve --config PATH` and only on a configured
loopback address. Before service startup, an operator must validate a real
GitHub merge-group token through an inherited FIFO or `AF_UNIX` socket
descriptor that the trusted launcher opened with `O_NONBLOCK` before `exec`:

```text
trusted-nonblocking-launcher --producer TOKEN_PRODUCER -- \
  fe2o3-protected-publisher --enroll --config CONFIG \
    --token-fd INHERITED_NONBLOCKING_FD --artifact ARTIFACT
```

That launcher name is illustrative; no production launcher is shipped. An
ordinary shell pipeline normally supplies a blocking descriptor and does not
satisfy this contract.

The token must never appear in a regular file, argv, environment, log, or
artifact. Enrollment rejects regular-file, directory, terminal, and other
unexpected descriptors using `fstat`; only FIFOs and sockets whose
`getsockname` family is exactly `AF_UNIX` are accepted. Merely having
`S_IFSOCK` mode is insufficient: IPv4, IPv6, and other socket families are
rejected before token input. Before argument handling, configuration loading,
signing-key loading, enrollment input, or HTTP bearer extraction, the process
sets and verifies `PR_SET_DUMPABLE=0` and soft and hard `RLIMIT_CORE=0`; failure
stops startup or the request path. The artifact stores a token SHA-256, not
token bytes.

Enrollment preallocates one bounded owned token allocation and wraps it and each
read scratch array in zeroizing ownership. The HTTP adapter removes the bearer
header, copies its payload once into a zeroizing secret wrapper, and does not
materialize a service-owned ordinary bearer `String`. Signing-key PEM ownership
is also zeroizing, including invalid UTF-8 handling. These are narrow process
and owned-buffer properties: they do not prove that kernel transport buffers,
Axum/HTTP header storage, `jsonwebtoken`, cryptographic libraries, allocators,
or other dependencies never make or retain internal copies. Root,
`CAP_SYS_PTRACE`, kernel compromise, swap/hibernation, and physical memory are
outside this boundary.

The reader requires and rechecks `O_NONBLOCK`, duplicates the inherited
descriptor with close-on-exec only for bounded local ownership, and never calls
`F_SETFL`. A duplicate shares the original open-file description, so it would
not isolate status flag changes. The trusted launcher must keep that description
nonblocking and must not mutate its flags after handoff. One absolute read
deadline covers polling and all chunks. `poll`, `read`, `EINTR`, spurious
readiness, and `EAGAIN` are retried only while time remains; readiness never
permits a blocking read or a read after the deadline. EOF terminates input,
empty input fails, and byte `16,385` fails rather than being truncated.

The service accepts only `POST /v1/receipts` with canonical JSON, one
`Authorization: Bearer ...` header, and one `Idempotency-Key` header. A
separately reviewed local TLS frontend is still required. It must preserve the
body and both headers exactly and must never log them.

## Stable Recovery

`Idempotency-Key` is exactly 64 lowercase hexadecimal characters representing
a client-generated 256-bit random value. The production client generates it
with the operating-system CSPRNG and stores it as a single-link 0600 file under
descriptor-checked `RUNNER_TEMP`. The filename is derived from the canonical
request SHA-256, so a process retry for the same request reuses the key while a
different request gets a different key. The service persists only:

```text
request_key_sha256 = SHA256(
  "fe2o3-protected-publisher-idempotency-key-v1\0" || Idempotency-Key)
request_sha256 = SHA256(canonical_request_bytes)
stable_authorization_sha256 = SHA256(canonical_stable_authorization_bytes)
```

The ledger also retains the exact canonical request and exact committed
response. Reusing a key with another body, request identity, request digest, or
stable authorization projection is a conflict. Reusing another key for an
already recorded request identity or digest is also a conflict.

JWT `jti` is deliberately not the durable recovery key. A fresh valid JWT with
a different JTI may retrieve only the exact committed record whose stable
authorization projection and canonical body match. An authorization or body
substitution fails closed. A new issuance token must have remaining lifetime
of at least the configured request deadline rounded up to seconds plus a
30-second recovery margin. This reduces avoidable expiry races but is not the
recovery mechanism.

## Authentication Profile

Production configuration fixes GitHub's issuer and JWKS URL, audience,
repository name and numeric IDs, owner ID, environment, default branch,
workflow paths, and actor-ID allowlist. Runtime compares all authority-bearing
claims using typed, exact structured equality. Workflow references must equal:

```text
REPOSITORY/CALLER_WORKFLOW_PATH@MERGE_QUEUE_REF
REPOSITORY/PROTECTED_WORKFLOW_PATH@MERGE_QUEUE_REF
```

Substring workflow matching is forbidden. The merge-queue ref itself must
start with the configured `refs/heads/gh-readonly-queue/main/` namespace.

The canonical stable projection contains actor ID, issuer, audience,
repository/owner names and IDs, event, environment, base/head refs, merge
queue ref, job, caller and protected workflow refs/SHAs, run attempt/ID/number,
runner environment, candidate SHA, subject, workflow name, policy ID, and
projection schema. These values are identical across a permissible fresh-token
retry.

The ephemeral projection contains `jti`, `iat`, `nbf`, `exp`, and
`check_run_id`. They are signed and validated on every authentication but are
not embedded in the canonical request or used as durable idempotency. JWT
header `kid` and optional `x5t` select and bind the verification key but grant
no request authority. Additional bounded provider metadata is nonauthoritative.

Only RS256 JWTs are accepted. Header, claims, and JWKS parsing rejects duplicate
members. The selected JWK must be the unique exact KID, RSA signing use, and
RS256 algorithm. Signed times are JSON integers, token lifetime is at most ten
minutes, and configured skew and minimum-remaining-lifetime rules apply.

## Enrollment

Enrollment verifies the token signature using the configured live HTTPS GitHub
JWKS provider. It builds typed `stable` and `ephemeral` projections, constructs
the exact workflow/request context that runtime would see, and invokes the same
runtime claim validator. Therefore a generated artifact corresponds to a claim
profile runtime accepts at enrollment time.

The canonical enrollment V2 artifact contains the exact typed projection, its
SHA-256, complete configuration SHA-256, enrollment and 30-day artifact expiry
times, and token SHA-256. Startup revalidates the projection against the same
runtime policy at the recorded enrollment time and verifies all digests. The
artifact is owner-only, single-link, created without replacement, and required
before signing key or ledger access. Configuration changes invalidate it.

No real GitHub merge-group token was available during this implementation.
Tests use a crate-private synthetic provider and do not establish that the
shipped enrollment path works with a real token.

## JWKS Egress

JWKS uses rustls validation, HTTPS-only mode, no redirects, no proxy
environment, exact final URL/content type, bounded content length/body, and one
absolute request deadline for connect, headers, semaphore, and chunks. One
outbound permit and one refresh leader provide singleflight behavior.

An unknown KID can request one refreshed document during one authentication.
Across authentications, each provider/issuer has a 128-entry FIFO negative KID
cache and a global forced-refresh floor. Forced egress backoff starts at one
second, doubles after waves, and is capped at 30 seconds. Concurrent and
sequential attacker KIDs therefore cannot force one request per token.
Suppressed authentication retries only the current cached document and fails
closed. Legitimate rotation becomes eligible after the bounded floor/backoff or
normal cache expiry. Negative entries store at most 256 KID bytes each.

## Durable Ledger

The issuance store is not SQLite and has no pathname reopen, journal, WAL, or
sidecar. Startup traverses every parent component with
`openat(O_DIRECTORY|O_NOFOLLOW)`, retains the owner-only directory descriptor,
and opens an existing 0600 single-link ledger with `openat(O_NOFOLLOW)` plus
full pre/open/post metadata equality. If the name is absent, startup creates an
anonymous `O_TMPFILE` inode in that retained directory, writes the complete
bounded header with short-write and `EINTR` handling, verifies its size and
identity, calls `fdatasync`, and atomically publishes it no-replace with
`linkat`. It then `fsync`s the parent, reopens and revalidates the final name,
and only then acquires the exclusive nonblocking advisory lock and replays.

Direct `linkat(AT_EMPTY_PATH)` is attempted first. On the Linux errors that
indicate an unprivileged or unsupported direct form, publication uses the Linux
`O_TMPFILE` recipe with `/proc/self/fd/<retained-fd>` as the `linkat` source and
`AT_SYMLINK_FOLLOW`, targeting the retained destination dirfd. The source still
names the already-open anonymous inode; the final destination is never resolved
through `/proc`. Missing or incompatible procfs behavior fails startup closed.
No named temporary ledger exists to leak before publication. Concurrent
initializers either publish the one complete inode or observe `EEXIST`, sync the
parent, and open that same complete final object. A preexisting empty file or
any exact partial header is rejected unchanged; it is never repaired in place.

The ledger descriptor remains open for the process lifetime. Shutdown
explicitly applies `LOCK_UN` before close so a transient fork-inherited
open-file description cannot extend the lock until its close-on-exec boundary.

The file header binds the format and configuration-derived service identity.
Each append-only frame contains:

```text
magic | version | payload_length | sequence | previous_frame_hash
canonical_record_json
SHA256(frame_domain || frame_prefix || canonical_record_json)
```

Records contain separate request-key, request, stable-authorization, and
evidence identities plus exact request/response bytes in canonical base64.
Startup replays frames sequentially with bounded record size, row count, and
file size. It builds stable-offset indexes and rejects wrong versions,
sequence gaps, hash-chain breaks, noncanonical records, bad base64, digest or
identity mismatches, duplicate keys/requests/evidence, complete checksum
corruption, and an incompatible header. No automatic migration from SQLite is
attempted.

An idempotent lookup reads its indexed frame with bounded positional `read_at`
operations. It validates against sequence, previous hash, frame hash, frame
length, request key, request identity, request digest, and stable authorization
stored in the index. Only after every index field matches that validated
durable record are the caller's request key, request identity, request digest,
stable authorization, and canonical request body compared with the decoded
record. A genuine caller conflict then returns `ReplayConflict` without
poisoning. Lookup never seeks the shared file cursor and never changes the live
append sequence, chain head, tail offset, or index. Any positional-read, EOF,
bounded-`EINTR`, identity, frame, canonical/base64, digest, semantic, or
index-validation failure poisons the process-lifetime store before returning.
A poisoned store cannot append, commit, acknowledge, or serve a later retry.
The original durable bytes are not modified by lookup; restart independently
replays those bytes.

Before write admission, issuance constructs the exact canonical
`LedgerRecord` and complete frame, including its hash, then invokes the same
frame decoder, canonical parser, base64 decoder, digest checks, and semantic
record validator used by restart replay. Decoder failure therefore occurs
before append and `fdatasync`; such a record is never written or acknowledged.
The generic canonical JSON string limit remains 4,096 bytes. Only the ledger
record and generated response parsers use their separately bounded base64
field limits.

EOF inside a prefix or a structurally valid frame is classified as an
unacknowledged torn tail, truncated to the previous complete frame, and
`fdatasync`ed before service. A complete frame with a bad hash is corruption
and fails startup rather than being truncated. Short writes are retried. A
write error, injected ENOSPC, identity loss, or sync failure poisons the live
store and prevents acknowledgement; restart can remove only the incomplete
tail.

Before append and after `fdatasync`, the retained descriptor and directory
entry must still agree on device, inode, mode, UID, GID, and link count. A
rename, unlink, hardlink, or replacement fails stop. If substitution occurs
after sync, the committed bytes are not acknowledged and the process remains
poisoned. These checks assume the owner-only parent excludes an untrusted
renamer; root, the service UID, kernel compromise, and a hostile filesystem are
outside this mechanism.

One bounded blocking worker owns the ledger. Its nonblocking queue capacity is
the configured admitted-request count, 1 through 256. The absolute request
deadline is checked before worker admission and critical pre-append stages.
Once append is admitted, synchronous write and `fdatasync` run to a definitive
result and are not described as cancellable. If the HTTP waiter times out after
commit, a fresh valid token and the same stable request key recover the exact
stored response.

A whole-ledger rollback to an earlier internally valid complete prefix cannot
be distinguished from the original historical file using only that same file.
The hash chain rejects corruption, reordering, insertion, and duplicate frames,
but is not an external monotonic rollback anchor. Production rollback rejection
requires a separately protected monotonic checkpoint, HSM/TPM seal, or
equivalent reviewed facility. None is implemented or claimed here.

## Bounds

| Input/state | Bound |
| --- | ---: |
| HTTP request | 65,536 bytes |
| JWT / encoded segment | 16,384 / 12,288 bytes |
| JWKS / response / receipt | 262,144 / 524,288 / 262,144 bytes |
| headers | 32 entries / 32,768 aggregate bytes |
| JSON depth / members / lexical tokens | 32 / 512 / 4,096 |
| JWKS keys / negative KIDs | 16 / 128 |
| negative KID bytes | 256 each |
| admitted requests / durable workers | 1-256 / exactly 1 |
| durable queue | admitted-request count, 1-256 |
| request deadline | 1-30,000 milliseconds |
| JWKS network deadline | at most request deadline and 10,000 milliseconds |
| JWKS normal cache | 1-3,600 seconds |
| forced JWKS refresh backoff | 1-30 seconds |
| ledger records | configured, 1-1,000,000 |
| ledger bytes | configured, 1 MiB-64 GiB |
| initial ledger header | 4,096 bytes |
| ledger decoded request / request base64 | 65,536 / 87,384 bytes |
| ledger decoded response / response base64 | 524,288 / 699,052 bytes |
| decoded receipt / receipt base64 | 262,144 / 349,528 bytes |
| ledger record payload / complete frame | 851,968 / 852,056 bytes |
| enrollment/config artifacts | 65,536 bytes each |
| private key | 16,384 bytes |

The append-only ledger has no online deletion or compaction. New issuance stops
before configured row or byte capacity. Exact committed duplicates remain
recoverable. Rotation to a new ledger is an offline, separately reviewed
operation.

## Configuration

Configuration is canonical JSON schema V2, owner-only, single-link, and at
most 65,536 bytes. This placeholder example is intentionally unusable:

```json
{"allowed_actor_ids":["ACTOR_ID"],"audience":"https://publisher.example/github-actions","caller_workflow_path":".github/workflows/parity-promotion.yml","default_branch":"main","enrollment_artifact_path":"/var/lib/fe2o3-publisher/github-enrollment.json","environment":"protected-publisher","issuer":"https://token.actions.githubusercontent.com","jwks_cache_seconds":300,"jwks_url":"https://token.actions.githubusercontent.com/.well-known/jwks","ledger_path":"/var/lib/fe2o3-publisher/publisher.ledger","listen":"127.0.0.1:9443","max_inflight_requests":32,"max_ledger_bytes":1073741824,"max_receipts":100000,"network_deadline_milliseconds":5000,"protected_workflow_path":".github/workflows/parity-publisher-gate.yml","repository":"powderluv/fe2o3","repository_id":"1233498266","repository_owner_id":"74956","request_deadline_milliseconds":10000,"schema_version":2,"signature_domain":"production","signing_key_id":"operator-publisher-v1","signing_key_path":"/run/secrets/fe2o3-publisher/operator-publisher-v1.pem"}
```

Configuration, enrollment, key, and ledger paths must be absolute. Authority
files are checked before open, on the opened descriptor, after open/read, and
against retained directories. The implementation requires Linux `openat`,
`fstatat`, `O_TMPFILE`, `linkat` with `AT_EMPTY_PATH` or the exact
`/proc/self/fd` fallback above, `renameat2`, `flock`, and local crash-consistent
`fdatasync`/directory `fsync` semantics. It assumes procfs faithfully exposes
the calling process's retained descriptor links and the local filesystem gives
the documented atomic no-replace link and sync ordering. Deployments without
those semantics fail closed and are unsupported.

## Validation

Run the opt-in harness with the reviewed Rust toolchain first in `PATH`:

```text
scripts/test-protected-publisher-service.sh
```

It runs debug and release Rust tests, hostile body and client transport tests,
fresh-token recovery, key/body/authorization collisions, descriptor races,
torn/corrupt/duplicate frames, maximum-ref restart, append/replay decoder
closure, short-write and ENOSPC injection, blocking-FD rejection, immutable
status-flag observation, nonblocking FIFO/socket enrollment races, and
socket-family rejection, JWKS concurrency/waves/rotation/deadlines,
client-service conformance, the synthetic AF_UNIX nondumpable/core-limit secret
process probe, every partial initial-header write, six initialization crash
boundaries, 24-process concurrent first publication, strict Clippy, formatting,
and diff checks.
`cargo audit`, repository fsck, repeated stress, and a clean exact-commit
MI300X replay are separate validation steps.

## Nonclaims

This remains single-node, single-ledger, single-worker, Linux-specific, and
loopback-only. It has no inbound TLS frontend, HSM-backed key, HA replication,
production supervisor, production monitoring/rate limiting, external rollback
anchor, online ledger compaction, tested backup/restore, or tested physical
power-loss/full-disk recovery. Injected write failures are not physical storage
qualification. Advisory locking is not a defense against root or the service
UID. No real GitHub merge-group enrollment token was available.

Candidate status remains rejected until a fresh hostile independent review
accepts the exact code, deployment, GitHub controls, enrolled real claims,
hardware evidence, and operational controls. Passing these tests is not an
acceptance, deployment, production, or parity claim.
