# fe2o3-runtime-protocol

Production, authority-aware runtime protocols shared by `cargo-fe2o3` and the
host runtime. The crate owns the Worker V3 load-envelope custody transition,
the application handoff wire, and sealed static-application identity.

Version suffixes identify frozen wire records. They are not selectable
compiler pipelines. Legacy Worker V2 codecs remain outside this crate.

## Compiler-execution attestation V1

The crate defines four fixed, allocation-free records for the protected
compiler-attestation boundary:

- a 184-byte caller-pinned issuer policy;
- a 200-byte issuer challenge bound to one canonical compiler-execution
  subject and rollback position;
- a 946-byte request carrying that challenge and the complete 690-byte
  subject; and
- a 400-byte Ed25519 receipt binding the exact request and rollback
  transition.

Strict decoding rederives all nested identities, rejects noncanonical headers
and rollback positions, and verifies receipt signatures with the key pinned by
the policy. These records do not supervise a process, protect a signing key,
persist or advance a replay ledger, or grant compiler, publication, load, or
launch authority. The protected issuer service and Worker V3 verifier must add
those properties while consuming the same exact bytes.

## Compiler-execution receipt publication V1

The crate also defines a 584-byte immutable receipt sidecar and a 288-byte
publication ACK claim. The sidecar binds the complete signed receipt to the
exact issuer journal and supervised compiler occurrence. The ACK additionally
binds one protected Worker ledger record, sequence, and advanced rollback
anchor. Both records strictly rederive nested identities and reject every
noncanonical byte.

These records are inert transport data. In particular, the ACK digest does not
prove that its named Worker record is durable. Only independent protected-ledger
reacquisition may construct the move-only result that allows the issuer to
discard an issued receipt.

The 2,058-byte carriage record preserves the complete policy, request, sidecar,
and ACK without projection. Construction and decoding verify every nested
record, the signature against the carried policy and request, and the ACK
relationship. The carried policy is still inert input: a production verifier
must compare it with protected configuration and enforce rollback currentness
before granting compiler authority.

## Compiler-execution service V1

The packet codec carries the attestation lifecycle over one connected Unix
`SOCK_SEQPACKET` boundary. Requests select inspect, prepare, issue, publish, or
cancel and bind the caller-pinned policy plus the exact expected sequence and
rollback anchor. Responses carry ready state, the complete challenge, the
complete receipt publication, or the complete Worker-ledger ACK. Every packet
has an exact operation-specific length and a terminal domain-separated
identity. The maximum request is 1,658 bytes and the maximum response is 744
bytes.

The codec is allocation-free and authority-free. It does not inspect peer
credentials, retain a pidfd, impose a deadline, mutate either journal, or turn
wire bytes into a capability. Those operations belong only to the protected
service implementation.

## Receipt-bearing Worker V3 load envelope V2

The receipt-bearing envelope nests the exact canonical V1 replay and the complete
2,058-byte compiler-execution carriage under a distinct `F3LDENV2` header and
checksum. It reconstructs the complete compiler subject from the replay's attempt,
slot, transaction identity, and outer handoff, then requires exact equality with
the signed request subject. Strict decode re-encodes and compares the nested V1
bytes. The full 256 MiB V1 maximum remains representable; V2 adds exactly 2,114
bytes and has explicit wire and transient-allocation budgets.

Live construction, opaque durable persistence, and strict V2 restart recovery are
implemented. They remain authority-free: protected policy comparison, rollback
currentness, and Worker verifier authority are still required. Cargo and host have
not yet switched their production decoder from top-level V1 to V2.
