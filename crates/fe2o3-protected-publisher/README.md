# fe2o3-protected-publisher

Opt-in, loopback-only reference service for GitHub OIDC authorization and
client-compatible protected publisher receipts. Issuance uses a bounded
single worker and an fd-bound, checksummed, hash-chained append-only ledger.
A CSPRNG client request key is durably bound to the canonical request digest
and stable authorization projection, so a fresh valid token can recover an
already committed receipt after the original JWT expires.

Enrollment accepts a token only through an inherited FIFO or an `AF_UNIX`
socket, never a token pathname, argv value, environment value, TCP socket, or
other socket family. Before any key or token read, the process verifies that it
is nondumpable and both core limits are zero. Service-owned bearer, enrollment
scratch, token, and signing-key PEM buffers use zeroizing ownership; dependency
and kernel copies are not claimed absent. The inherited descriptor must already
be nonblocking; the reader verifies that contract and never mutates shared
open-file status flags. Unknown JWKS keys share singleflight refresh, a bounded
negative cache, and issuer-wide refresh backoff. Configuration, enrollment,
key, and ledger authority is descriptor-checked through retained owner-only
directories. Every candidate ledger frame passes the same canonical decoder
and semantic validator used during restart before it can be appended. A new
ledger header is fully written and synced in an anonymous same-directory inode,
published atomically no-replace, parent-synced, reopened, locked, and replayed;
no empty or partial final ledger is initialized in place.
Idempotent recovery uses bounded positional frame reads and cannot disturb the
append cursor or live sequence/hash state. Any indexed-read or validation
failure poisons the live store and blocks subsequent acknowledgement while
leaving the durable ledger unchanged for independent restart replay.

This crate is inert by default. It is not deployed, production-ready, or an
acceptance/parity claim. See `docs/protected-publisher-service-v1.md` for the
wire contract, bounds, ledger format, recovery behavior, limitations, tests,
and external controls still required.
