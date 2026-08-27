# Compiler Execution Receipt Publication V1

## Status

This document freezes the authority-free sidecar and acknowledgment records used
to carry one protected compiler-execution receipt into the Worker V3 rollback
ledger. The authoritative codecs are in `fe2o3-runtime-protocol`.

The records are bounded and canonical. They do not prove that a filesystem
write, rename, sync, or Worker-ledger transition happened. Production issuer
acknowledgment must consume a move-only result created only after independent
durable Worker-ledger reacquisition. Deserializing either record grants no
compiler, publication, load, or launch authority.

## Receipt Publication Sidecar

The fixed 584-byte sidecar carries the exact signed receipt plus the two
issuer-owned identities that are not fields of that receipt: the signed
`Issued` journal record and the supervised compiler occurrence.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 24 | Header: `F2O3CES1`, V1, zero flags, length `584`, zero reserved |
| 24 | 32 | Issuer policy identity from the signed receipt |
| 56 | 32 | Exact signed `Issued` journal record identity |
| 88 | 32 | Exact supervised compiler-occurrence identity |
| 120 | 32 | Exact signed receipt identity |
| 152 | 400 | Complete canonical signed receipt |
| 552 | 32 | Domain-separated sidecar identity |

The terminal identity covers bytes `0..552` under
`FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V1`. Decoding verifies the nested
receipt signature first, then requires exact policy and receipt-identity
agreement, nonzero journal and occurrence identities, a canonical header, and
byte-for-byte re-encoding.

The issuer constructs this sidecar from its private durable record after the
`Issued` transition. A caller-supplied journal or occurrence identity cannot be
substituted at that boundary. Recovery reconstructs the same sidecar from the
same signed journal bytes.

## Publication ACK Claim

The fixed 288-byte ACK claim binds the sidecar to the exact Worker ledger record
that claims to have consumed it and to the resulting current rollback anchor.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 24 | Header: `F2O3CEA1`, V1, zero flags, length `288`, zero reserved |
| 24 | 32 | Issuer policy identity |
| 56 | 32 | Exact signed `Issued` journal record identity |
| 88 | 32 | Exact supervised compiler-occurrence identity |
| 120 | 32 | Exact signed receipt identity |
| 152 | 32 | Exact receipt-sidecar identity |
| 184 | 32 | Nonzero protected Worker ledger record identity |
| 216 | 8 | Receipt and Worker rollback sequence |
| 224 | 32 | Worker ledger's resulting current rollback anchor |
| 256 | 32 | Domain-separated ACK-claim identity |

The terminal identity covers bytes `0..256` under
`FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V1`. Construction fixes the
sequence and current anchor to the nested receipt. Matching independently
compares every sidecar-derived field and the separately reacquired Worker ledger
record identity.

The ACK remains an inert claim because its terminal digest is not a signature
and does not attest storage durability. The protected service must not advance
the issuer journal from raw ACK bytes. It must first:

1. reacquire the exact protected Worker rollback ledger through retained
   descriptor custody;
2. decode and verify the immutable receipt sidecar;
3. verify the issuer policy, signed receipt, exact request and subject, current
   prior anchor, sequence, and derived next anchor;
4. durably commit the Worker record containing the complete sidecar;
5. recover or re-read that exact record and compare its terminal identity;
6. construct a move-only committed-publication token; and
7. let the issuer consume that token and persist the complete ACK before it may
   discard the receipt bytes.

## Crash And Replay Contract

- A crash before the Worker commit leaves the issuer in `Issued`; the exact
  sidecar is re-emitted.
- A crash after the Worker commit but before issuer ACK replays the same Worker
  record and ACK claim.
- A crash during issuer ACK recovers either the exact `Issued` record or its one
  legal `Ready` successor.
- The ready successor retains the complete ACK claim, not only the receipt
  digest, so a lost ACK response can be recognized only by exact publication,
  Worker record, sequence, and rollback-anchor equality.
- A stale, subject-equivalent, same-receipt, different-publication, or
  different-Worker-record ACK must fail closed.

## Qualification

The protocol tests fix both record sizes, exercise canonical round trips and
wrong lengths, reject mutation of every one of the 872 wire bytes, and test
independently valid policy, journal, occurrence, receipt, publication, and
Worker-ledger substitutions. Filesystem crash injection and protected-ledger
reacquisition belong to the durable Worker integration milestone, not this
authority-free codec.
