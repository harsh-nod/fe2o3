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
