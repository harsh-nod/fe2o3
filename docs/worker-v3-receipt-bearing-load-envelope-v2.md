# Worker V3 Receipt-Bearing Load Envelope V2

## Status

The canonical codec, live publication owner, schema-neutral durable persistence,
strict restart recovery, and backend-to-host production route are implemented.
Construction consumes the existing complete Worker V3 replay plus one
`CompilerExecutionReceiptCarriageV1` and rejects any compiler subject mismatch.

The Rustc backend derives the exact subject only after durable V3 handoff
publication, obtains the carriage from the protected issuer, and atomically
publishes the exact bytes in the same attempt slot. Cargo reads the handoff and
sidecar under one currentness token, validates the carriage against the sealed
client profile, and retains it across fresh execution and finalized-HSACO
restart recovery. Cargo durable readiness, application descriptor transfer,
and `fe2o3-host` admission accept only top-level V2. Top-level V1 is therefore
not a production route; its codec remains the complete nested replay format.

## Wire

The V2 wire preserves the exact canonical V1 replay bytes without projection:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F3LDENV2` |
| 8 | 2 | Version `2` |
| 10 | 2 | Zero flags |
| 12 | 8 | Complete V2 byte length |
| 20 | 4 | Exact nested V1 replay byte length |
| 24 | variable | Exact canonical V1 replay |
| next | 2,058 | Complete compiler-execution receipt carriage |
| final | 32 | Domain-separated V2 checksum over every preceding byte |

V2 adds exactly 2,114 bytes. Its maximum is therefore the complete 256 MiB V1
limit plus 2,114 bytes; no valid V1 replay is truncated or excluded. The opaque
durable-readiness ceiling is widened by the same exact amount and a compile-time
assertion prevents the two crate limits from drifting.

## Validation

Strict construction and decoding perform all of these checks:

1. validate bounded length, magic, version, flags, declared total, and nested
   length;
2. verify the top-level checksum before nested decoding;
3. strictly decode the nested V1 replay and complete receipt carriage;
4. re-encode the nested replay and require byte-for-byte equality;
5. decode the retained outer compiler handoff and compact finalizer transcript;
6. reconstruct the complete compiler-execution subject from the durable attempt,
   production slot, transaction identity, and outer handoff; and
7. require exact equality with the signed request subject in the carriage.

The codec has explicit wire and transient-allocation budgets. Decoding reserves
separate bounded halves for the retained nested owner and canonical comparison,
so hostile input cannot turn strict canonicality checking into unbounded memory
growth.

## Authority

V2 proves lossless association, not protected compiler origin. The carriage's
policy must still be compared with protected verifier configuration, its Ed25519
receipt must be joined to current external rollback state, and its Worker-ledger
ACK must be independently reacquired. Only that protected verifier decision may
grant compiler authority; load and launch remain separate move-only transitions.

## Remaining Authority Promotion

- bind the carriage identity and protected-policy decision into the Worker
  verification request and decision;
- compare the carried policy with independently retained verifier configuration;
- atomically enforce the external monotonic rollback anchor and Worker-ledger
  record before compiler authority exists; and
- qualify that authority through the final KFD/HSA load and gfx942 execution
  route under the deployed root-owned supervisor.
